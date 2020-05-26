use ::api_client::*;
use anyhow::Result;
use log::*;

async fn create_floor(
    client: &mut ApiClient,
    file: String,
    user: String,
    level: u64,
) -> Result<(i64, Vec<String>)> {
    begin_undo_event(client, &file, &user).await?;
    let mut prefix = OpPrefixMsg {
        file: file.clone(),
        user: user.clone(),
        offset: 0,
    };

    let width: f64 = 1.0;
    let height: f64 = 10.0;
    let length: f64 = 100.0;

    let z = height * level as f64;
    let pt_1 = Point3Msg { x: 0.0, y: 0.0, z };
    let pt_2 = Point3Msg {
        x: length,
        y: 0.0,
        z,
    };
    let pt_3 = Point3Msg {
        x: length,
        y: length,
        z,
    };
    let pt_4 = Point3Msg {
        x: 0.0,
        y: length,
        z,
    };
    let walls = vec![
        WallApiMsg {
            first_pt: Some(pt_1.clone()),
            second_pt: Some(pt_2.clone()),
            width,
            height,
        },
        WallApiMsg {
            first_pt: Some(pt_2.clone()),
            second_pt: Some(pt_3.clone()),
            width,
            height,
        },
        WallApiMsg {
            first_pt: Some(pt_3.clone()),
            second_pt: Some(pt_4.clone()),
            width,
            height,
        },
        WallApiMsg {
            first_pt: Some(pt_4.clone()),
            second_pt: Some(pt_1.clone()),
            width,
            height,
        },
    ];
    let (offset, ids) = create_walls(client, &prefix, walls).await?;
    prefix.offset = offset;
    /*prefix.offset = join_objs_at_pt(client, &prefix, &id_1, &id_2, &pt_2).await?;
    prefix.offset = join_objs_at_pt(client, &prefix, &id_2, &id_3, &pt_3).await?;
    prefix.offset = join_objs_at_pt(client, &prefix, &id_3, &id_4, &pt_4).await?;
    prefix.offset = join_objs_at_pt(client, &prefix, &id_4, &id_1, &pt_1).await?;*/

    begin_undo_event(client, &file, &user).await?;
    let delta = Vector3Msg {
        x: length / 2.0,
        y: 0.0,
        z: 0.0,
    };
    prefix.offset = move_objects(client, &prefix, vec![ids[1].clone()], &delta).await?;
    //prefix.offset = delete_objects(client, &prefix, vec![id_2.clone()]).await?;
    prefix.offset = undo_latest(client, &prefix.file, &prefix.user, prefix.offset).await?;
    info!("Undone");
    prefix.offset = redo_latest(client, &prefix.file, &prefix.user, prefix.offset).await?;
    info!("Redone");
    Ok((prefix.offset, ids))
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();
    let mut args: Vec<String> = std::env::args().collect();
    let level: u64 = args.pop().unwrap().parse().unwrap();
    let mut client = ApiClient::connect("http://127.0.0.1:8080").await?;
    let now = std::time::SystemTime::now();
    let file = String::from("00000003-0003-0003-0003-000000000003");
    let user = uuid::Uuid::new_v4().to_string();
    let _ = create_floor(&mut client, file.clone(), user.clone(), level).await?;
    let elapsed = now.elapsed().unwrap();
    info!("Test took {:?} seconds", elapsed.as_secs_f32());

    Ok(())
}