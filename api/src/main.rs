use log::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod api {
    include!(concat!(env!("OUT_DIR"), "/api.rs"));
}
use api::*;

mod representation {
    include!(concat!(env!("OUT_DIR"), "/representation.rs"));
}

mod walls {
    include!(concat!(env!("OUT_DIR"), "/walls.rs"));
}

mod object_state {
    include!(concat!(env!("OUT_DIR"), "/object_state.rs"));
}

mod obj_defs {
    include!(concat!(env!("OUT_DIR"), "/obj_defs.rs"));
}

mod undo {
    include!(concat!(env!("OUT_DIR"), "/undo.rs"));
}

mod submit {
    include!(concat!(env!("OUT_DIR"), "/submit.rs"));
}

fn unavailable<T: std::fmt::Debug>(err: T) -> Status {
    Status::unavailable(format!("Couldn't connect to child service: {:?}", err))
}

#[derive(Debug, Clone)]
struct Prefix {
    file: String,
    user: String,
    offset: i64,
}

impl Prefix {
    pub fn new(prefix_opt: Option<OpPrefixMsg>) -> Result<Prefix, Status> {
        if let Some(prefix) = prefix_opt {
            Ok(Prefix {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
            })
        } else {
            Err(Status::invalid_argument("Operation prefix is required"))
        }
    }
}

fn to_point3msg(pt_opt: Option<Point3ApiMsg>) -> Option<object_state::Point3Msg> {
    match pt_opt {
        Some(pt) => Some(object_state::Point3Msg {
            x: pt.x,
            y: pt.y,
            z: pt.z,
        }),
        None => None,
    }
}

struct ApiService {
    undo_url: String,
    wall_url: String,
    submit_url: String,
}

#[tonic::async_trait]
impl api_server::Api for ApiService {
    async fn begin_undo_event(
        &self,
        request: Request<BeginUndoEventInput>,
    ) -> Result<Response<BeginUndoEventOutput>, Status> {
        let msg = request.into_inner();
        info!("Begin Undo Event: {:?}", msg);
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .await
            .map_err(unavailable)?;
        undo_client
            .begin_undo_event(Request::new(undo::BeginUndoEventInput {
                file: msg.file,
                user: msg.user,
            }))
            .await?;
        Ok(Response::new(BeginUndoEventOutput {}))
    }

    async fn undo_latest(
        &self,
        request: Request<UndoLatestInput>,
    ) -> Result<Response<UndoLatestOutput>, Status> {
        let msg = request.into_inner();
        info!("Undo Latest: {:?}", msg);
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .await
            .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let changes = undo_client
            .undo_latest(Request::new(undo::UndoLatestInput {
                file: prefix.file.clone(),
                user: prefix.user.clone(),
            }))
            .await?
            .into_inner();
        let mut output = submit_client
            .submit_changes(Request::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes: changes.changes,
            }))
            .await?
            .into_inner();
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(UndoLatestOutput { offset })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }

    async fn redo_latest(
        &self,
        request: Request<RedoLatestInput>,
    ) -> Result<Response<RedoLatestOutput>, Status> {
        let msg = request.into_inner();
        info!("Redo Latest: {:?}", msg);
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .await
            .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let changes = undo_client
            .redo_latest(Request::new(undo::RedoLatestInput {
                file: prefix.file.clone(),
                user: prefix.user.clone(),
            }))
            .await?
            .into_inner();
        let mut output = submit_client
            .submit_changes(Request::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes: changes.changes,
            }))
            .await?
            .into_inner();
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(RedoLatestOutput { offset })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }

    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        let msg = request.into_inner();
        info!("Create Walls: {:?}", msg);
        let mut wall_client = walls::walls_client::WallsClient::connect(self.wall_url.clone())
            .await
            .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let mut walls = Vec::new();
        let mut ids = Vec::new();
        for wall in msg.walls {
            let id = id_gen::gen_id();
            ids.push(id.clone());
            walls.push(walls::WallMsg {
                id,
                first_pt: to_point3msg(wall.first_pt),
                second_pt: to_point3msg(wall.second_pt),
                width: wall.width,
                height: wall.height,
            });
        }
        let objects = wall_client
            .create_walls(Request::new(walls::CreateWallsInput { walls }))
            .await?
            .into_inner();
        let mut changes = Vec::new();
        for (obj, id) in objects.walls.into_iter().zip(ids.iter()) {
            changes.push(object_state::ChangeMsg {
                id: id.clone(),
                user: prefix.user.clone(),
                change_type: Some(object_state::change_msg::ChangeType::Add(obj)),
            });
        }

        let mut output = submit_client
            .submit_changes(Request::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes,
            }))
            .await?
            .into_inner();
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(CreateWallsOutput {
                obj_ids: ids,
                offset,
            })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let undo_url = std::env::var("UNDO_URL").unwrap().parse().unwrap();
    let wall_url = std::env::var("WALL_URL").unwrap().parse().unwrap();
    let submit_url = std::env::var("SUBMIT_URL").unwrap().parse().unwrap();
    let svc = api_server::ApiServer::new(ApiService {
        undo_url,
        wall_url,
        submit_url,
    });

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
