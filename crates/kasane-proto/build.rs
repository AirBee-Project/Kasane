use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(protoc_path) = protoc_bin_vendored::protoc_bin_path() {
        unsafe {
            std::env::set_var("PROTOC", protoc_path);
        }
    }

    let proto_dir = PathBuf::from("../../proto");
    let proto_files = [
        proto_dir.join("common.proto"),
        proto_dir.join("system.proto"),
        proto_dir.join("auth.proto"),
        proto_dir.join("database.proto"),
        proto_dir.join("table.proto"),
        proto_dir.join("data.proto"),
        proto_dir.join("query.proto"),
        proto_dir.join("users.proto"),
    ];

    println!("cargo:rerun-if-changed=../../proto");

    prost_build::compile_protos(&proto_files, &[proto_dir])?;

    Ok(())
}
