use utoipa::OpenApi;

use crate::models::auth::{LoginRequest, LoginResponse};
use crate::models::database::table::data::{
    GetDataRequest, GetDataResponse, InsertDataRequest, RemoveDataRequest,
};
use crate::models::database::table::{
    CreateTableRequest, TableDataType, TableInfoResponse, TableListResponse,
};
use crate::models::database::{CreateDatabaseRequest, DatabaseInfoResponse};
use crate::models::query::{
    Geometry, PointCoordinate, Query, TableFilter, TableFilterBoolean, TableFilterFloat,
    TableFilterInt, TableFilterText, TableFilterType,
};
use crate::models::spatial_id::SpatialId;
use crate::models::users::{
    CreateUserRequest, PrivilegeInfoResponse, UpdatePasswordRequest, UpdatePrivilegeRequest,
    UserInfoResponse, UserRole,
};

#[derive(OpenApi)]
#[openapi(
    servers(),
    paths(
        // Auth
        crate::handlers::auth::login,
        // Users
        crate::handlers::users::list_users,
        crate::handlers::users::create_user,
        crate::handlers::users::delete_user,
        crate::handlers::users::update_password,
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
        // Data
        GetDataRequest,
        GetDataResponse,
        InsertDataRequest,
        RemoveDataRequest,
        // Query
        Query,
        SpatialId,
        Geometry,
        PointCoordinate,
        TableFilter,
        TableFilterType,
        TableFilterText,
        TableFilterInt,
        TableFilterFloat,
        TableFilterBoolean,
    )),
    tags(
        (name = "Auth", description = "Authentication endpoints"),
        (name = "Users", description = "User management operations"),
        (name = "databases", description = "Database operations"),
        (name = "tables", description = "Table operations")
    )
)]
pub struct ApiDoc;
