//! `proto/*.proto` からRustの型を生成する

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 生成物を使うのは `grpc` feature の gRPC 層だけ。
    if std::env::var_os("CARGO_FEATURE_GRPC").is_none() {
        return Ok(());
    }
    // `Channel` で接続するクライアントの生成コードは、tonic の通信路（`server` feature）が要る。
    let transport = std::env::var_os("CARGO_FEATURE_SERVER").is_some();

    let proto_files = [
        "proto/common.proto",
        "proto/system.proto",
        "proto/auth.proto",
        "proto/database.proto",
        "proto/table.proto",
        "proto/data.proto",
        "proto/query.proto",
        "proto/users.proto",
    ];

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);

    tonic_prost_build::configure()
        .build_transport(transport)
        .file_descriptor_set_path(out_dir.join("kasane_descriptor.bin"))
        .compile_protos(&proto_files, &["proto"])?;

    Ok(())
}
