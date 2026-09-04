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

    tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("kasane_descriptor.bin"))
        .compile_protos(&proto_files, &["proto"])?;

    Ok(())
}
