//! gRPC と同じ proto メッセージで、Wasm 上の Kasane を RPC ごとに呼べることの検証。
//!
//! `wasm-pack test --node --release wasm` で実行する。

#[path = "../../tests/common/builders.rs"]
#[allow(dead_code)]
mod builders;

use builders::{num, shift_x, single_id, source, value_as_f64};
use kasane::grpc::pb;
use kasane_wasm::Kasane;
use prost::Message;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_test::wasm_bindgen_test;

/// RPC を呼び、応答メッセージ（ストリームならチャンクごと）を返す。
async fn call<Res: Message + Default>(
    db: &Kasane,
    path: &str,
    request: impl Message,
) -> Result<Vec<Res>, JsValue> {
    let chunks = db.call(path.into(), request.encode_to_vec()).await?;
    Ok(chunks
        .iter()
        .map(|chunk| Res::decode(chunk.to_vec().as_slice()).unwrap())
        .collect())
}

async fn open_with_table() -> Kasane {
    let db = Kasane::open().await.unwrap();
    call::<pb::DatabaseInfo>(
        &db,
        "/kasane.DatabaseService/Create",
        pb::CreateDatabaseRequest {
            name: "demo".into(),
            description: None,
        },
    )
    .await
    .unwrap();
    call::<pb::TableSummary>(
        &db,
        "/kasane.TableService/Create",
        pb::CreateTableRequest {
            db_name: "demo".into(),
            name: "risk".into(),
            data_type: pb::TableDataType::Int as i32,
            max_zoom_level: 25,
            constraints: None,
            description: None,
            value_index: false,
            is_temporal: false,
        },
    )
    .await
    .unwrap();
    db
}

async fn insert(db: &Kasane, value: f64, ids: Vec<pb::SpatialId>) {
    call::<pb::InsertDataResponse>(
        db,
        "/kasane.DataService/Insert",
        pb::InsertDataRequest {
            db_name: "demo".into(),
            table_name: "risk".into(),
            value: Some(num(value)),
            spatial_ids: ids,
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
        },
    )
    .await
    .unwrap();
}

/// ストリームの全チャンクから、値ごとの空間 ID 数を集める。
fn values(chunks: &[pb::SearchDataResponse]) -> Vec<(i64, usize)> {
    let dictionary: Vec<_> = chunks.iter().flat_map(|c| c.dictionary.clone()).collect();
    let mut out: Vec<(i64, usize)> = chunks
        .iter()
        .flat_map(|c| &c.data)
        .map(|group| {
            let value = match &group.value {
                Some(pb::data_group::Value::DictRef(r)) => &dictionary[*r as usize],
                Some(pb::data_group::Value::InlineValue(v)) => v,
                None => panic!("DataGroup.value is not set"),
            };
            (value_as_f64(value).unwrap() as i64, group.spatial_ids.len())
        })
        .collect();
    out.sort_unstable();
    out
}

fn status_code(error: JsValue) -> i32 {
    let error: js_sys::Error = error.unchecked_into();
    js_sys::Reflect::get(&error, &"code".into())
        .unwrap()
        .as_f64()
        .unwrap() as i32
}

#[wasm_bindgen_test]
async fn insert_then_search_streams_the_value() {
    let db = open_with_table().await;
    insert(&db, 42.0, vec![single_id(20, 0, 931277, 412899)]).await;

    let chunks: Vec<pb::SearchDataResponse> = call(
        &db,
        "/kasane.DataService/Search",
        pb::SearchDataRequest {
            db_name: "demo".into(),
            table_name: "risk".into(),
            spatial_ids: vec![single_id(20, 0, 931277, 412899)],
            zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            format: pb::OutputFormat::SingleId as i32,
        },
    )
    .await
    .unwrap();

    assert_eq!(values(&chunks), vec![(42, 1)]);
}

#[wasm_bindgen_test]
async fn query_shifts_values() {
    let db = open_with_table().await;
    insert(&db, 7.0, vec![single_id(20, 0, 931277, 412899)]).await;

    let chunks: Vec<pb::SearchDataResponse> = call(
        &db,
        "/kasane.QueryService/Execute",
        pb::ExecuteQueryRequest {
            value_type: None,
            spatial_ids: vec![single_id(20, 0, 931280, 412899)],
            query: Some(shift_x(source("demo", "risk"), 20, 3)),
            format: pb::OutputFormat::SingleId as i32,
        },
    )
    .await
    .unwrap();

    assert_eq!(values(&chunks), vec![(7, 1)]);
}

/// エラーはサーバーと同じ gRPC ステータスコード（`NotFound` = 5）で返る。
#[wasm_bindgen_test]
async fn errors_carry_the_grpc_status_code() {
    let db = open_with_table().await;
    let error = call::<pb::TableInfo>(
        &db,
        "/kasane.TableService/Get",
        pb::GetTableRequest {
            db_name: "demo".into(),
            table_name: "missing".into(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(status_code(error), tonic::Code::NotFound as i32);
}

/// ログインは Wasm では扱わない（`Unimplemented` = 12）。
#[wasm_bindgen_test]
async fn login_is_unimplemented() {
    let db = Kasane::open().await.unwrap();
    let error = call::<pb::LoginResponse>(
        &db,
        "/kasane.AuthService/Login",
        pb::LoginRequest {
            username: "root".into(),
            password: "password".into(),
        },
    )
    .await
    .unwrap_err();

    assert_eq!(status_code(error), tonic::Code::Unimplemented as i32);
}
