fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("PROTOC").is_err() {
        if let Ok(path) = protoc_bin_vendored::protoc_bin_path() {
            std::env::set_var("PROTOC", path);
        } else {
            let candidates = [
                "/home/edohwares/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/protoc-bin-vendored-linux-x86_64-3.2.0/bin/protoc",
                "/usr/bin/protoc",
                "/usr/local/bin/protoc",
            ];
            for candidate in candidates {
                if std::path::Path::new(candidate).exists() {
                    std::env::set_var("PROTOC", candidate);
                    break;
                }
            }
        }
    }
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(&["proto/events.proto"], &["proto"])?;
    Ok(())
}
