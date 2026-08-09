use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

use crate::models::auth::{LoginRequest, LoginResponse};
use crate::models::database::table::data::{
    GetDataQuery, GetDataRequest, GetDataResponse, InsertDataRequest, OutputFormat,
    RemoveDataRequest, ZoomLevelPolicy,
};
use crate::models::database::table::{
    CopyTableRequest, CreateTableRequest, TableDataType, TableInfoResponse, TableListResponse,
    TableSummary, UpdateTableRequest,
};
use crate::models::database::{
    CopyDatabaseRequest, CreateDatabaseRequest, DatabaseInfoResponse, UpdateDatabaseRequest,
};
use crate::models::spatial_id::SpatialId;
use crate::models::users::{
    CreateUserRequest, DataRole, PrivilegeRule, PrivilegesResponse, SetDataPrivilegeRequest,
    SetGlobalPrivilegeRequest, UpdatePasswordRequest, UserInfoResponse, UserRole,
};

/// `bearer_auth` セキュリティスキーム（JWT Bearer）を OpenAPI コンポーネントに登録する。
///
/// 各エンドポイントの `security(("bearer_auth" = []))` 宣言はこのスキーム定義を
/// 参照するため、これが無いと仕様が不完全になり Swagger UI の Authorize も
/// 機能しない。
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::default);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
    }
}

/// 権限モデルの凡例。
///
/// 各エンドポイントの説明にある「必要な権限」は最小要件だけを示す。
/// 「上位スコープ／上位ロールが下位を含む」という規則はここに一度だけ書き、
/// 個々のエンドポイントでは繰り返さない。
const PRIVILEGE_LEGEND: &str = r#"
## 権限について

各エンドポイントの説明にある **必要な権限** は、その操作に必要な最小の権限です。
`スコープ` / `ロール` の形式で表します。

**スコープ**は `global` ⊃ `database` ⊃ `table` の階層で、上位スコープの権限は下位をすべて含みます。
たとえば `global` / `read` を持つユーザーは、すべてのデータベース・テーブルを読めます。
`database` / `table` と書かれている場合、対象はパスで指定したデータベース・テーブルです。

**ロール**は `read` < `write` < `manage` < `admin` の順に強く、上位ロールは下位をすべて含みます。

- `read` — 参照
- `write` — データの書き込み
- `manage` — テーブルやデータベースそのものの管理
- `admin` — ユーザーと権限の管理。制御面の権限であり、`global` スコープにのみ指定できます

`global` / `manage` は全データベースのデータを自由に扱えますが、ユーザーや権限は操作できません。
ユーザー管理を行えるのは `global` / `admin` だけです。
"#;

#[derive(OpenApi)]
#[openapi(
    modifiers(&SecurityAddon),
    info(description = PRIVILEGE_LEGEND),
    servers(),
    paths(
        // Auth
        crate::handlers::auth::login,
        // System
        crate::handlers::system::get_system_info,
        // Users
        crate::handlers::users::list_users,
        crate::handlers::users::create_user,
        crate::handlers::users::get_user,
        crate::handlers::users::delete_user,
        crate::handlers::users::update_password,
        crate::handlers::users::get_privileges,
        crate::handlers::users::set_global_privilege,
        crate::handlers::users::delete_global_privilege,
        crate::handlers::users::set_database_privilege,
        crate::handlers::users::delete_database_privilege,
        crate::handlers::users::set_table_privilege,
        crate::handlers::users::delete_table_privilege,
        // GET  /databases
        crate::handlers::database::database_list,
        // POST /databases
        crate::handlers::database::database_create,
        // GET  /databases/{db_name}
        crate::handlers::database::database_info,
        // DELETE /databases/{db_name}
        crate::handlers::database::remove_database,
        // PATCH /databases/{db_name}
        crate::handlers::database::database_update,
        // POST /databases/{db_name}/copy
        crate::handlers::database::database_copy,
        crate::handlers::database::table::create::table_create,
        crate::handlers::database::table::list::table_list,
        // GET  /databases/{db_name}/tables/{table_name}
        crate::handlers::database::table::info::table_info,
        // PATCH /databases/{db_name}/tables/{table_name}
        crate::handlers::database::table::update::table_update_handler,
        // POST /databases/{db_name}/tables/{table_name}/copy
        crate::handlers::database::table::copy::table_copy,
        // DELETE /databases/{db_name}/tables/{table_name}
        crate::handlers::database::table::remove::remove_table,
        // PUT    /databases/{db_name}/tables/{table_name}/data
        crate::handlers::database::table::data::insert::data_insert,
        // PATCH  /databases/{db_name}/tables/{table_name}/data
        crate::handlers::database::table::data::upsert::data_upsert,
        // DELETE /databases/{db_name}/tables/{table_name}/data
        crate::handlers::database::table::data::remove::data_remove,
        // POST   /databases/{db_name}/tables/{table_name}/data/search
        crate::handlers::database::table::data::get::data_get,
        // POST   /query
        crate::handlers::query::execute_query,
    ),
    components(schemas(
        // Auth
        LoginRequest,
        LoginResponse,
        // System
        crate::models::system::SystemInfoResponse,
        // Users
        CreateUserRequest,
        UserInfoResponse,
        UpdatePasswordRequest,
        SetGlobalPrivilegeRequest,
        SetDataPrivilegeRequest,
        PrivilegesResponse,
        PrivilegeRule,
        UserRole,
        DataRole,
        // Database
        CreateDatabaseRequest,
        UpdateDatabaseRequest,
        CopyDatabaseRequest,
        DatabaseInfoResponse,
        // Table
        CreateTableRequest,
        UpdateTableRequest,
        CopyTableRequest,
        crate::models::database::table::TableConstraints,
        TableDataType,
        TableInfoResponse,
        TableSummary,
        TableListResponse,
        GetDataRequest,
        GetDataResponse,
        GetDataQuery,
        OutputFormat,
        InsertDataRequest,
        RemoveDataRequest,
        ZoomLevelPolicy,
        SpatialId,
        // Query
        crate::models::query::ExecuteQueryRequest,
        crate::models::query::QueryNode,
        crate::models::query::MergePolicyKind,
        crate::models::query::FilterCondition,
    )),
    tags(
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Users", description = "User management operations"),
        (name = "Databases", description = "Database operations"),
        (name = "Tables", description = "Table operations"),
        (name = "Data", description = "Data manipulation operations"),
        (name = "Query", description = "Cross-table query execution"),
        (name = "System", description = "System operations"),
    )
)]
pub struct ApiDoc;
