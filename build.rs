//! `proto/*.proto` から gRPC のクライアント/サーバーコードを生成する。
//!
//! システムの `protoc` を使う（`PATH` に無ければ `PROTOC` 環境変数で指す）。

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

    // `QueryNode` を直接・間接に参照するフィールドだけを boxed 指定する。
    // `.boxed(".kasane.QueryNode")` のようにメッセージ全体を渡すと、
    // パスがプレフィックス一致で効いてしまい `QueryNode.Source` の
    // 無関係な `string` フィールドまで boxed 化されるため、フィールド単位で指定する。
    let boxed_query_node_fields = [
        "FilterValues.input",
        "Shift.input",
        "ZoomOut.input",
        "ExtrudeXy.input",
        "ExtrudeF.input",
        "Falloff.input",
        "Merge.left",
        "Merge.right",
        "SetOp.left",
        "SetOp.right",
        "MapValues.input",
        "MathValues.input",
    ];

    let mut builder = tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("kasane_descriptor.bin"));
    for field in boxed_query_node_fields {
        builder = builder.boxed(format!(".kasane.QueryNode.{field}"));
    }
    builder.compile_protos(&proto_files, &["proto"])?;

    Ok(())
}
