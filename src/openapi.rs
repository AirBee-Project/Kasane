use utoipa::OpenApi;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

use crate::models::auth::{LoginRequest, LoginResponse};
use crate::models::database::table::data::{
    GetDataQuery, GetDataRequest, GetDataResponse, InsertDataRequest, OutputFormat,
    RemoveDataRequest,
};
use crate::models::database::table::{
    CreateTableRequest, TableDataType, TableInfoResponse, TableListResponse,
};
use crate::models::database::{CreateDatabaseRequest, DatabaseInfoResponse};
use crate::models::spatial_id::SpatialId;
use crate::models::users::{
    CreateUserRequest, PrivilegeInfoResponse, UpdateAdminRequest, UpdatePasswordRequest,
    UpdatePrivilegeRequest, UserInfoResponse, UserRole,
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

#[derive(OpenApi)]
#[openapi(
    modifiers(&SecurityAddon),
    servers(),
    paths(
        // Auth
        crate::handlers::auth::login,
        // Users
        crate::handlers::users::list_users,
        crate::handlers::users::create_user,
        crate::handlers::users::delete_user,
        crate::handlers::users::update_password,
        crate::handlers::users::set_admin,
        crate::handlers::users::get_privileges,
        crate::handlers::users::set_privilege,
        crate::handlers::users::delete_privilege,
        // GET  /databases
        crate::handlers::database::database_list,
        // POST /databases
        crate::handlers::database::database_create,
        // GET  /databases/{name}
        crate::handlers::database::database_info,
        // DELETE /databases/{name}
        crate::handlers::database::remove_database,
        crate::handlers::database::table::create::table_create,
        crate::handlers::database::table::list::table_list,
        // GET  /databases/{db_name}/tables/{table_name}
        crate::handlers::database::table::info::table_info,
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
    ),
    components(schemas(
        // Auth
        LoginRequest,
        LoginResponse,
        // Users
        CreateUserRequest,
        UserInfoResponse,
        UpdatePasswordRequest,
        UpdatePrivilegeRequest,
        UpdateAdminRequest,
        PrivilegeInfoResponse,
        UserRole,
        // Database
        CreateDatabaseRequest,
        DatabaseInfoResponse,
        // Table
        CreateTableRequest,
        TableDataType,
        TableInfoResponse,
        TableListResponse,
        GetDataRequest,
        GetDataResponse,
        GetDataQuery,
        OutputFormat,
        InsertDataRequest,
        RemoveDataRequest,
        SpatialId,
    )),
    tags(
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Users", description = "User management operations"),
        (name = "databases", description = "Database operations"),
        (name = "tables", description = "Table operations"),
        (name = "data", description = "Data manipulation operations")
    )
)]
pub struct ApiDoc;
