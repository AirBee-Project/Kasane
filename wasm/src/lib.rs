//! ブラウザで動く Kasane。データはメモリ上にだけ置き、インスタンスを捨てると消える。
//!
//! サーバーと同じ gRPC のハンドラを、通信路を通さずに直接呼ぶ。入出力は proto の
//! メッセージのバイト列なので、クライアントは gRPC と同じメッセージ型をそのまま使える。

use js_sys::{Error, Reflect, Uint8Array};
use kasane::AppState;
use kasane::grpc::pb::data_service_server::DataService;
use kasane::grpc::pb::database_service_server::DatabaseService;
use kasane::grpc::pb::query_service_server::QueryService;
use kasane::grpc::pb::system_service_server::SystemService;
use kasane::grpc::pb::table_service_server::TableService;
use kasane::grpc::pb::user_service_server::UserService;
use kasane::grpc::{data, database, query, system, table, users};
use kasane::models::auth::Claims;
use kasane::repositories::{ROOT_USERNAME, ReadRepository, Storage};
use prost::Message;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};
use wasm_bindgen::prelude::*;

/// メモリ上の Kasane 1 つ。すべての RPC を root として実行する。
#[wasm_bindgen]
pub struct Kasane {
    app_state: AppState,
    claims: Claims,
}

#[wasm_bindgen]
impl Kasane {
    /// root だけを持つ空のインスタンスを作る。
    pub async fn open() -> Result<Kasane, JsValue> {
        let db = kasane::backend::open("")
            .await
            .map_err(|e| js_error(e.into()))?;
        let app_state = AppState::new(db);
        let root = app_state
            .db
            .read(async |r| r.require_user(ROOT_USERNAME).await)
            .await
            .map_err(|e| js_error(e.into()))?;
        // JWT の検証を経ない代わりに、検証済みの `Claims` を直接持つ。有効期限は見られない。
        let claims = Claims {
            sub: root.username,
            uid: root.id.to_string(),
            ver: root.token_version,
            exp: 0,
        };
        Ok(Self { app_state, claims })
    }

    /// `path`（`/kasane.DataService/Search` の形）の RPC を、エンコード済みの `request` で呼ぶ。
    ///
    /// 戻り値は応答メッセージのエンコード済みバイト列の配列で、単項 RPC なら 1 要素、
    /// サーバーストリーミング RPC ならチャンクの数だけ並ぶ。失敗すると `code`（gRPC の
    /// ステータスコード）・`message`・`details`（`grpc-status-details-bin` と同じバイト列）を
    /// 持つ `Error` で reject する。`AuthService` は扱わない（`Unimplemented`）。
    pub async fn call(&self, path: String, request: Vec<u8>) -> Result<Vec<Uint8Array>, JsValue> {
        self.dispatch(&path, &request)
            .await
            .map(|messages| {
                messages
                    .iter()
                    .map(|bytes| Uint8Array::from(bytes.as_slice()))
                    .collect()
            })
            .map_err(js_error)
    }
}

impl Kasane {
    async fn dispatch(&self, path: &str, bytes: &[u8]) -> Result<Vec<Vec<u8>>, Status> {
        let app_state = self.app_state.clone();
        let system = system::SystemServiceImpl {
            app_state: app_state.clone(),
        };
        let database = database::DatabaseServiceImpl {
            app_state: app_state.clone(),
        };
        let table = table::TableServiceImpl {
            app_state: app_state.clone(),
        };
        let data = data::DataServiceImpl {
            app_state: app_state.clone(),
        };
        let query = query::QueryServiceImpl {
            app_state: app_state.clone(),
        };
        let users = users::UserServiceImpl { app_state };

        match path {
            "/kasane.SystemService/GetSystemInfo" => {
                unary(system.get_system_info(self.request(bytes)?).await)
            }

            "/kasane.DatabaseService/Create" => unary(database.create(self.request(bytes)?).await),
            "/kasane.DatabaseService/Get" => unary(database.get(self.request(bytes)?).await),
            "/kasane.DatabaseService/List" => unary(database.list(self.request(bytes)?).await),
            "/kasane.DatabaseService/Update" => unary(database.update(self.request(bytes)?).await),
            "/kasane.DatabaseService/Delete" => unary(database.delete(self.request(bytes)?).await),
            "/kasane.DatabaseService/Copy" => unary(database.copy(self.request(bytes)?).await),

            "/kasane.TableService/Create" => unary(table.create(self.request(bytes)?).await),
            "/kasane.TableService/Get" => unary(table.get(self.request(bytes)?).await),
            "/kasane.TableService/List" => unary(table.list(self.request(bytes)?).await),
            "/kasane.TableService/Update" => unary(table.update(self.request(bytes)?).await),
            "/kasane.TableService/Delete" => unary(table.delete(self.request(bytes)?).await),
            "/kasane.TableService/Copy" => unary(table.copy(self.request(bytes)?).await),

            "/kasane.DataService/Search" => {
                server_stream(data.search(self.request(bytes)?).await).await
            }
            "/kasane.DataService/Insert" => unary(data.insert(self.request(bytes)?).await),
            "/kasane.DataService/Upsert" => unary(data.upsert(self.request(bytes)?).await),
            "/kasane.DataService/Remove" => unary(data.remove(self.request(bytes)?).await),

            "/kasane.QueryService/Execute" => {
                server_stream(query.execute(self.request(bytes)?).await).await
            }

            "/kasane.UserService/List" => unary(users.list(self.request(bytes)?).await),
            "/kasane.UserService/Create" => unary(users.create(self.request(bytes)?).await),
            "/kasane.UserService/Get" => unary(users.get(self.request(bytes)?).await),
            "/kasane.UserService/Delete" => unary(users.delete(self.request(bytes)?).await),
            "/kasane.UserService/UpdatePassword" => {
                unary(users.update_password(self.request(bytes)?).await)
            }
            "/kasane.UserService/GetPrivileges" => {
                unary(users.get_privileges(self.request(bytes)?).await)
            }
            "/kasane.UserService/GrantPrivilege" => {
                unary(users.grant_privilege(self.request(bytes)?).await)
            }
            "/kasane.UserService/RevokePrivilege" => {
                unary(users.revoke_privilege(self.request(bytes)?).await)
            }

            _ => Err(Status::unimplemented(format!(
                "{path} is not available in the Wasm build"
            ))),
        }
    }

    /// サーバーの認証 interceptor を通った後と同じ形の要求にする。
    fn request<T: Message + Default>(&self, bytes: &[u8]) -> Result<Request<T>, Status> {
        let message = T::decode(bytes).map_err(|e| Status::invalid_argument(e.to_string()))?;
        let mut request = Request::new(message);
        request.extensions_mut().insert(self.claims.clone());
        Ok(request)
    }
}

fn unary<T: Message>(response: Result<Response<T>, Status>) -> Result<Vec<Vec<u8>>, Status> {
    Ok(vec![response?.into_inner().encode_to_vec()])
}

async fn server_stream<T, S>(response: Result<Response<S>, Status>) -> Result<Vec<Vec<u8>>, Status>
where
    T: Message,
    S: Stream<Item = Result<T, Status>> + Unpin,
{
    response?
        .into_inner()
        .map(|chunk| chunk.map(|message| message.encode_to_vec()))
        .collect()
        .await
}

fn js_error(status: Status) -> JsValue {
    let error = Error::new(status.message());
    // `Reflect::set` が失敗するのは凍結されたオブジェクトに対してだけで、作ったばかりの `Error` では起きない。
    let _ = Reflect::set(&error, &"code".into(), &(status.code() as i32).into());
    let _ = Reflect::set(
        &error,
        &"details".into(),
        &Uint8Array::from(status.details()).into(),
    );
    error.into()
}
