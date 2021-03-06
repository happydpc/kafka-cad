fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile(
            &[
                "../../proto/api.proto",
                "../../proto/geom.proto",
                "../../proto/representation.proto",
            ],
            &["../../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
