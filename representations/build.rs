fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile(
            &[
                "../proto/obj_defs.proto",
                "../proto/representation.proto",
                "../proto/object_state.proto",
            ],
            &["../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
