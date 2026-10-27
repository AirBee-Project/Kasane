//! プロセス内から gRPC のリクエストを処理する入口（組み込み・インメモリ向け）。
//!
//! gRPC のメソッド名と protobuf のバイト列を受け取り、サーバーと同じサービス実装を呼ぶ。
//! 利用者は root だけなので、AuthService と UserService は持たない。

use std::future::Future;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};

use prost::Message;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::Service;
use super::pb::data_service_server::DataService;
use super::pb::database_service_server::DatabaseService;
use super::pb::query_service_server::QueryService;
use super::pb::system_service_server::SystemService;
use super::pb::table_service_server::TableService;
use crate::Kasane;

/// `method` は `/kasane.TableService/Create` の形。応答はメッセージの列で、unary なら 1 件、
/// server streaming ならチャンクの数だけ並ぶ。
///
/// `&mut Kasane` を受けるのは、処理が 1 回のポーリングで終わる前提（`run`）を守るため。
/// 共有参照から並行に呼べると、書き込み同士がロックで待ち合わせて前提が崩れる。
pub fn handle(kasane: &mut Kasane, method: &str, request: &[u8]) -> Result<Vec<Vec<u8>>, Status> {
    // サービス間でメソッド名が重なるので、トレイトを明示して呼ぶ。
    let this = Service::from_ref(kasane);
    match method {
        "/kasane.SystemService/GetSystemInfo" => {
            unary(request, |r| SystemService::get_system_info(this, r))
        }
        "/kasane.DatabaseService/Create" => unary(request, |r| DatabaseService::create(this, r)),
        "/kasane.DatabaseService/Get" => unary(request, |r| DatabaseService::get(this, r)),
        "/kasane.DatabaseService/List" => unary(request, |r| DatabaseService::list(this, r)),
        "/kasane.DatabaseService/Update" => unary(request, |r| DatabaseService::update(this, r)),
        "/kasane.DatabaseService/Delete" => unary(request, |r| DatabaseService::delete(this, r)),
        "/kasane.TableService/Create" => unary(request, |r| TableService::create(this, r)),
        "/kasane.TableService/Get" => unary(request, |r| TableService::get(this, r)),
        "/kasane.TableService/List" => unary(request, |r| TableService::list(this, r)),
        "/kasane.TableService/Update" => unary(request, |r| TableService::update(this, r)),
        "/kasane.TableService/Delete" => unary(request, |r| TableService::delete(this, r)),
        "/kasane.DataService/Search" => streaming(request, |r| DataService::search(this, r)),
        "/kasane.DataService/Insert" => unary(request, |r| DataService::insert(this, r)),
        "/kasane.DataService/Upsert" => unary(request, |r| DataService::upsert(this, r)),
        "/kasane.DataService/Remove" => unary(request, |r| DataService::remove(this, r)),
        "/kasane.QueryService/Execute" => streaming(request, |r| QueryService::execute(this, r)),
        _ => Err(Status::unimplemented(format!(
            "{method} is not available in the in-memory Kasane"
        ))),
    }
}

fn decode<T: Message + Default>(request: &[u8]) -> Result<Request<T>, Status> {
    T::decode(request)
        .map(Request::new)
        .map_err(|e| Status::invalid_argument(format!("malformed request: {e}")))
}

fn unary<Req, Res, Fut>(
    request: &[u8],
    call: impl FnOnce(Request<Req>) -> Fut,
) -> Result<Vec<Vec<u8>>, Status>
where
    Req: Message + Default,
    Res: Message,
    Fut: Future<Output = Result<Response<Res>, Status>>,
{
    let response = run(call(decode(request)?))??;
    Ok(vec![response.into_inner().encode_to_vec()])
}

fn streaming<Req, Res, S, Fut>(
    request: &[u8],
    call: impl FnOnce(Request<Req>) -> Fut,
) -> Result<Vec<Vec<u8>>, Status>
where
    Req: Message + Default,
    Res: Message,
    S: Stream<Item = Result<Res, Status>> + Unpin,
    Fut: Future<Output = Result<Response<S>, Status>>,
{
    let mut stream = run(call(decode(request)?))??.into_inner();
    let mut chunks = Vec::new();
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        match Pin::new(&mut stream).poll_next(&mut cx) {
            Poll::Ready(Some(chunk)) => chunks.push(chunk?.encode_to_vec()),
            Poll::Ready(None) => return Ok(chunks),
            Poll::Pending => return Err(pending()),
        }
    }
}

/// インメモリの処理は I/O を待たないので、1 回のポーリングで完了する。
fn run<F: Future>(future: F) -> Result<F::Output, Status> {
    match pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(output) => Ok(output),
        Poll::Pending => Err(pending()),
    }
}

fn pending() -> Status {
    Status::internal("an in-memory request unexpectedly waited")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::pb::{self, query_node::Node, spatial_id::Kind as IdKind, typed_value::Kind};
    use tonic::Code;

    fn call<Res: Message + Default>(
        db: &mut Kasane,
        method: &str,
        request: &impl Message,
    ) -> Result<Vec<Res>, Status> {
        handle(db, method, &request.encode_to_vec())?
            .iter()
            .map(|bytes| Res::decode(bytes.as_slice()).map_err(|e| Status::internal(e.to_string())))
            .collect()
    }

    fn int(value: i64) -> pb::TypedValue {
        pb::TypedValue {
            kind: Some(Kind::IntVal(value)),
        }
    }

    fn single(x: u32) -> pb::SpatialId {
        pb::SpatialId {
            kind: Some(IdKind::SingleId(pb::SingleId {
                z: 20,
                f: 0,
                x,
                y: 0,
                i: None,
                t: None,
            })),
        }
    }

    /// 辞書はチャンクをまたいで累積するので、受信順に連結してから引く。
    fn values(chunks: &[pb::SearchDataResponse]) -> Vec<(i64, usize)> {
        let mut dictionary = Vec::new();
        let mut out = Vec::new();
        for chunk in chunks {
            dictionary.extend(chunk.dictionary.iter().cloned());
            for group in &chunk.data {
                let value = match group.value.as_ref() {
                    Some(pb::data_group::Value::DictRef(i)) => dictionary[*i as usize].clone(),
                    Some(pb::data_group::Value::InlineValue(v)) => v.clone(),
                    None => panic!("a group without a value"),
                };
                let Some(Kind::IntVal(value)) = value.kind else {
                    panic!("not an int: {value:?}");
                };
                out.push((value, group.spatial_ids.len()));
            }
        }
        out.sort_unstable();
        out
    }

    fn with_table() -> Kasane {
        let mut db = Kasane::new();
        call::<pb::DatabaseInfo>(
            &mut db,
            "/kasane.DatabaseService/Create",
            &pb::CreateDatabaseRequest {
                name: "db".into(),
                description: None,
            },
        )
        .unwrap();
        call::<pb::TableInfo>(
            &mut db,
            "/kasane.TableService/Create",
            &pb::CreateTableRequest {
                db_name: "db".into(),
                name: "t".into(),
                data_type: pb::TableDataType::Int as i32,
                max_zoom_level: 20,
                ..Default::default()
            },
        )
        .unwrap();
        db
    }

    fn write(db: &mut Kasane, method: &str, value: i64, xs: &[u32]) {
        call::<()>(
            db,
            method,
            &pb::WriteDataRequest {
                db_name: "db".into(),
                table_name: "t".into(),
                value: Some(int(value)),
                spatial_ids: xs.iter().copied().map(single).collect(),
                zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            },
        )
        .unwrap();
    }

    fn search(db: &mut Kasane, xs: &[u32]) -> Vec<(i64, usize)> {
        let chunks = call::<pb::SearchDataResponse>(
            db,
            "/kasane.DataService/Search",
            &pb::SearchDataRequest {
                db_name: "db".into(),
                table_name: "t".into(),
                spatial_ids: xs.iter().copied().map(single).collect(),
                zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
                format: pb::OutputFormat::SingleId as i32,
            },
        )
        .unwrap();
        values(&chunks)
    }

    #[test]
    fn data_round_trips_through_handle() {
        let mut db = with_table();

        write(&mut db, "/kasane.DataService/Insert", 5, &[0, 1]);
        write(&mut db, "/kasane.DataService/Upsert", 7, &[1, 2]);
        assert_eq!(search(&mut db, &[0, 1, 2]), vec![(5, 2), (7, 1)]);

        call::<()>(
            &mut db,
            "/kasane.DataService/Remove",
            &pb::RemoveDataRequest {
                db_name: "db".into(),
                table_name: "t".into(),
                spatial_ids: vec![single(0)],
                zoom_level_policy: pb::ZoomLevelPolicy::Error as i32,
            },
        )
        .unwrap();
        assert_eq!(search(&mut db, &[0, 1, 2]), vec![(5, 1), (7, 1)]);
    }

    #[test]
    fn query_reads_the_stored_values() {
        let mut db = with_table();
        write(&mut db, "/kasane.DataService/Insert", 5, &[0]);
        write(&mut db, "/kasane.DataService/Insert", 7, &[1]);

        let source = pb::QueryNode {
            node: Some(Node::Source(pb::query_node::Source {
                database: "db".into(),
                table: "t".into(),
            })),
        };
        let query = pb::QueryNode {
            node: Some(Node::FilterValues(Box::new(pb::query_node::FilterValues {
                input: Some(Box::new(source)),
                condition: Some(pb::FilterCondition {
                    mode: Some(pb::filter_condition::Mode::Equals(
                        pb::filter_condition::Equals {
                            value: Some(int(7)),
                        },
                    )),
                }),
            }))),
        };
        let chunks = call::<pb::SearchDataResponse>(
            &mut db,
            "/kasane.QueryService/Execute",
            &pb::ExecuteQueryRequest {
                value_type: None,
                spatial_ids: vec![single(0), single(1)],
                query: Some(query),
                format: pb::OutputFormat::SingleId as i32,
            },
        )
        .unwrap();
        assert_eq!(values(&chunks), vec![(7, 1)]);
    }

    #[test]
    fn failed_write_leaves_state_unchanged() {
        let mut db = with_table();
        write(&mut db, "/kasane.DataService/Insert", 5, &[0]);

        let tightened = call::<pb::TableInfo>(
            &mut db,
            "/kasane.TableService/Update",
            &pb::UpdateTableRequest {
                db_name: "db".into(),
                table_name: "t".into(),
                new_name: Some("renamed".into()),
                constraints_update: Some(
                    pb::update_table_request::ConstraintsUpdate::SetConstraints(
                        pb::TableConstraints {
                            kind: Some(pb::table_constraints::Kind::Int(
                                pb::table_constraints::Int {
                                    min: None,
                                    max: Some(3),
                                },
                            )),
                        },
                    ),
                ),
                ..Default::default()
            },
        );
        assert!(tightened.is_err());

        let info = call::<pb::TableInfo>(
            &mut db,
            "/kasane.TableService/Get",
            &pb::GetTableRequest {
                db_name: "db".into(),
                table_name: "t".into(),
            },
        )
        .unwrap();
        assert_eq!(info[0].constraints, None);
        assert_eq!(search(&mut db, &[0]), vec![(5, 1)]);
    }

    #[test]
    fn instances_do_not_share_data() {
        let mut a = with_table();
        let mut b = with_table();
        write(&mut a, "/kasane.DataService/Insert", 5, &[0]);
        assert_eq!(search(&mut b, &[0]), vec![]);
    }

    #[test]
    fn rejects_unknown_methods_and_malformed_requests() {
        let mut db = Kasane::new();
        let err = handle(&mut db, "/kasane.AuthService/Login", &[]).unwrap_err();
        assert_eq!(err.code(), Code::Unimplemented);

        let err = handle(&mut db, "/kasane.DatabaseService/Create", &[0xff, 0xff]).unwrap_err();
        assert_eq!(err.code(), Code::InvalidArgument);
    }
}
