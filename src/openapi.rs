use utoipa::OpenApi;

use crate::models::query::{
    Geometry, PointCoordinate, Query, SpatialId, TableFilter, TableFilterBoolean, TableFilterFloat,
    TableFilterInt, TableFilterText, TableFilterType,
};
use crate::models::table::{
    CreateTableRequest, TableDataType, TableInfoResponse, TableListResponse,
};
use crate::models::table::value::{
    GetValueRequest, GetValueResponse, InsertValueRequest, RemoveValueRequest,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::table::table_create::table_create,
        crate::handlers::table::table_list::table_list,
        crate::handlers::table::table_info::table_info,
        crate::handlers::table::table_remove::table_remove,
        crate::handlers::table::value::insert::value_insert,
        crate::handlers::table::value::get::value_get,
        crate::handlers::table::value::remove::value_remove

    ),
    components(schemas(
        CreateTableRequest,
        Geometry,
        GetValueRequest,
        GetValueResponse,
        TableInfoResponse,
        TableListResponse,
        InsertValueRequest,
        PointCoordinate,
        Query,
        RemoveValueRequest,
        SpatialId,
        TableDataType,
        TableFilter,
        TableFilterBoolean,
        TableFilterFloat,
        TableFilterInt,
        TableFilterText,
        TableFilterType,
        RemoveValueRequest
    )),
    tags(
        (name = "tables", description = "Table operations")
    )
)]
pub struct ApiDoc;
