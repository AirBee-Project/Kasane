//! gRPC Server Streaming の分割送出・チャンキングの検証。
//!
//! 大量（例: DEFAULT_CHUNK_SIZE=2000 を超える件数）のデータを検索またはクエリした際に、
//! サーバーから複数の SearchDataResponse チャンクが順次ストリーム送信されることを確認する。

use kasane::grpc::convert_data::DEFAULT_CHUNK_SIZE;
use kasane::grpc::pb;

use crate::common::TestApp;
use crate::common::builders::{self, num};
use crate::common::data::put_data;

#[tokio::test]
/// 検索結果が DEFAULT_CHUNK_SIZE を超える場合、ストリームが複数チャンクに分割されて送出されることを検証する。
async fn test_search_streaming_chunks() {
    let test_app = TestApp::new().await;
    let table_name = "stream_test_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    // DEFAULT_CHUNK_SIZE (2000) を超える 4,500 個の空間IDを投入
    let total_count = 4500;
    let ids: Vec<pb::SpatialId> = (0..total_count)
        .map(|x| builders::single_id(20, 0, x, 100))
        .collect();

    put_data(&test_app, table_name, num(42.0), ids.clone()).await;

    let mut stream = test_app
        .data()
        .search(pb::SearchDataRequest {
            db_name: "test_db".to_string(),
            table_name: table_name.to_string(),
            spatial_ids: ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::SingleId as i32,
        })
        .await
        .expect("search stream request failed")
        .into_inner();

    let mut chunk_count = 0;
    let mut total_ids_received = 0;
    let mut dictionary_entries_received = 0;

    while let Some(chunk) = stream.message().await.expect("failed to receive chunk") {
        chunk_count += 1;
        // 辞書はストリーム全体で共有されるので、初出チャンクにしか載らない。
        dictionary_entries_received += chunk.dictionary.len();
        let chunk_ids: usize = chunk.data.iter().map(|g| g.spatial_ids.len()).sum();
        assert!(
            chunk_ids <= DEFAULT_CHUNK_SIZE,
            "chunk size {chunk_ids} should not exceed DEFAULT_CHUNK_SIZE {DEFAULT_CHUNK_SIZE}"
        );
        total_ids_received += chunk_ids;
    }

    // 4,500 件を 2,000 件上限で分割するため、3 チャンク（2000, 2000, 500）に分割されるはず
    assert!(
        chunk_count >= 3,
        "expected at least 3 chunks, got {chunk_count}"
    );
    assert_eq!(total_ids_received, total_count as usize);
    assert_eq!(
        dictionary_entries_received, 1,
        "single distinct value should register in the shared dictionary exactly once"
    );
}

#[tokio::test]
/// QueryService.Execute においても複数チャンクに分割されてストリーム送信されることを検証する。
async fn test_query_execute_streaming_chunks() {
    let test_app = TestApp::new().await;
    let table_name = "query_stream_test_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    let total_count = 3500;
    let ids: Vec<pb::SpatialId> = (0..total_count)
        .map(|x| builders::single_id(20, 0, x, 200))
        .collect();

    put_data(&test_app, table_name, num(100.0), ids.clone()).await;

    let query_node = builders::source("test_db", table_name);
    let mut stream = test_app
        .query()
        .execute(pb::ExecuteQueryRequest {
            value_type: None,
            spatial_ids: ids,
            query: Some(query_node),
            format: pb::OutputFormat::SingleId as i32,
        })
        .await
        .expect("query execute stream request failed")
        .into_inner();

    let mut chunk_count = 0;
    let mut total_ids_received = 0;
    let mut dictionary_entries_received = 0;

    while let Some(chunk) = stream
        .message()
        .await
        .expect("failed to receive query chunk")
    {
        chunk_count += 1;
        dictionary_entries_received += chunk.dictionary.len();
        let chunk_ids: usize = chunk.data.iter().map(|g| g.spatial_ids.len()).sum();
        assert!(
            chunk_ids <= DEFAULT_CHUNK_SIZE,
            "chunk size {chunk_ids} should not exceed DEFAULT_CHUNK_SIZE {DEFAULT_CHUNK_SIZE}"
        );
        total_ids_received += chunk_ids;
    }

    // 3,500 件を 2,000 件上限で分割するため、2 チャンク（2000, 1500）に分割されるはず
    assert!(
        chunk_count >= 2,
        "expected at least 2 chunks, got {chunk_count}"
    );
    assert_eq!(total_ids_received, total_count as usize);
    assert_eq!(
        dictionary_entries_received, 1,
        "single distinct value should register in the shared dictionary exactly once"
    );
}
