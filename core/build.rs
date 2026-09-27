fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("PROTOC").is_err() {
        if let Ok(protoc_path) = protoc_bin_vendored::protoc_bin_path() {
            std::env::set_var("PROTOC", protoc_path);
        }
    }
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["proto/events.proto"], &["proto"])?;
    Ok(())
}
