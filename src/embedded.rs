//! プロセス内で Kasane を動かす入口（組み込み・インメモリ向け）。
//!
//! gRPC のメソッド名と protobuf のバイト列を受け取り、サーバーと同じサービス実装を呼ぶ。
//! ストレージはインメモリ、利用者は root だけなので、AuthService と UserService は持たない。

use std::future::Future;
use std::pin::{Pin, pin};
use std::task::{Context, Poll, Waker};

use prost::Message;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use crate::AppState;
use crate::grpc::pb::data_service_server::DataService;
use crate::grpc::pb::database_service_server::DatabaseService;
use crate::grpc::pb::query_service_server::QueryService;
use crate::grpc::pb::system_service_server::SystemService;
use crate::grpc::pb::table_service_server::TableService;
use crate::grpc::{data, database, query, system, table};
use crate::repositories::memory::MemDb;

/// インスタンスごとに独立したデータを持つ。
pub struct Embedded {
    system: system::SystemServiceImpl,
    database: database::DatabaseServiceImpl,
    table: table::TableServiceImpl,
    data: data::DataServiceImpl,
    query: query::QueryServiceImpl,
}

impl Default for Embedded {
    fn default() -> Self {
        let app_state = AppState::new(MemDb::new());
        Self {
            system: system::SystemServiceImpl {
                app_state: app_state.clone(),
            },
            database: database::DatabaseServiceImpl {
                app_state: app_state.clone(),
            },
            table: table::TableServiceImpl {
                app_state: app_state.clone(),
            },
            data: data::DataServiceImpl {
                app_state: app_state.clone(),
            },
            query: query::QueryServiceImpl { app_state },
        }
    }
}

impl Embedded {
    pub fn new() -> Self {
        Self::default()
    }

    /// `method` は `/kasane.TableService/Create` の形。応答はメッセージの列で、unary なら 1 件、
    /// server streaming ならチャンクの数だけ並ぶ。
    pub fn handle(&self, method: &str, request: &[u8]) -> Result<Vec<Vec<u8>>, Status> {
        match method {
            "/kasane.SystemService/GetSystemInfo" => {
                unary(request, |r| self.system.get_system_info(r))
            }
            "/kasane.DatabaseService/Create" => unary(request, |r| self.database.create(r)),
            "/kasane.DatabaseService/Get" => unary(request, |r| self.database.get(r)),
            "/kasane.DatabaseService/List" => unary(request, |r| self.database.list(r)),
            "/kasane.DatabaseService/Update" => unary(request, |r| self.database.update(r)),
            "/kasane.DatabaseService/Delete" => unary(request, |r| self.database.delete(r)),
            "/kasane.TableService/Create" => unary(request, |r| self.table.create(r)),
            "/kasane.TableService/Get" => unary(request, |r| self.table.get(r)),
            "/kasane.TableService/List" => unary(request, |r| self.table.list(r)),
            "/kasane.TableService/Update" => unary(request, |r| self.table.update(r)),
            "/kasane.TableService/Delete" => unary(request, |r| self.table.delete(r)),
            "/kasane.DataService/Search" => streaming(request, |r| self.data.search(r)),
            "/kasane.DataService/Insert" => unary(request, |r| self.data.insert(r)),
            "/kasane.DataService/Upsert" => unary(request, |r| self.data.upsert(r)),
            "/kasane.DataService/Remove" => unary(request, |r| self.data.remove(r)),
            "/kasane.QueryService/Execute" => streaming(request, |r| self.query.execute(r)),
            _ => Err(Status::unimplemented(format!(
                "{method} is not available in the in-memory Kasane"
            ))),
        }
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
