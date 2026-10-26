//! `proto/*.proto` からRustの型を生成する

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    // transport（`connect` などの生成）は tonic の transport を有効にする server でだけ使える。
    let server = std::env::var_os("CARGO_FEATURE_SERVER").is_some();

    tonic_prost_build::configure()
        .build_transport(server)
        .file_descriptor_set_path(out_dir.join("kasane_descriptor.bin"))
        .compile_protos(&proto_files, &["proto"])?;

    Ok(())
}
