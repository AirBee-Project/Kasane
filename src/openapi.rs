use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

use crate::models::auth::{LoginRequest, LoginResponse};
use crate::models::database::table::data::{
    GetDataRequest, GetDataResponse, InsertDataRequest, OutputFormat, RemoveDataRequest,
    ZoomLevelPolicy,
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
    SetGlobalPrivilegeRequest, UpdatePasswordRequest, UserInfoResponse, UserListResponse, UserRole,
    UserSummary,
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

/// 権限モデルの説明。
const PRIVILEGE_LEGEND: &str = r#"
## 権限について
`スコープ` / `ロール` の形式で表します。

- `read` — データの読み込み
- `write` — データの書き込み
- `manage` — テーブルやデータベース自体の管理
- `admin` — ユーザーと権限の管理
"#;

#[derive(OpenApi)]
#[openapi(
    modifiers(&SecurityAddon),
    info(
        description = PRIVILEGE_LEGEND,
        license(name = "MIT", url = "https://opensource.org/licenses/MIT")
    ),
    servers(
        (url = "/", description = "Local server")
    ),
    paths(
        // Auth
        crate::handlers::auth::login,
        // System
        crate::handlers::system::get_system_info,
        // Databases
        crate::handlers::database::database_list,
        crate::handlers::database::database_create,
        crate::handlers::database::database_info,
        crate::handlers::database::remove_database,
        crate::handlers::database::database_update,
        crate::handlers::database::database_copy,
        // Tables
        crate::handlers::database::table::list::table_list,
        crate::handlers::database::table::create::table_create,
        crate::handlers::database::table::info::table_info,
        crate::handlers::database::table::remove::remove_table,
        crate::handlers::database::table::update::table_update_handler,
        crate::handlers::database::table::copy::table_copy,
        // Data: …/data → …/data/search
        crate::handlers::database::table::data::insert::data_insert,
        crate::handlers::database::table::data::remove::data_remove,
        crate::handlers::database::table::data::upsert::data_upsert,
        crate::handlers::database::table::data::get::data_get,
        // Query
        crate::handlers::query::execute_query,
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
        UserListResponse,
        UserSummary,
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
        crate::models::query::MathOperator,
    )),
    tags(
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Databases", description = "Database operations"),
        (name = "Tables", description = "Table operations"),
        (name = "Data", description = "Data manipulation operations"),
        (name = "Query", description = "Cross-table query execution"),
        (name = "Users", description = "User management operations"),
        (name = "System", description = "System operations"),
    )
)]
pub struct ApiDoc;
