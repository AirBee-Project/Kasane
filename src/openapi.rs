use utoipa::OpenApi;

use crate::models::table::{
    CreateTableRequest, TableDataType, TableInfoResponse, TableListResponse,
};
use crate::models::query::{
    Geometry, PointCoordinate, Query, SpatialId, TableFilter, TableFilterBoolean, 
    TableFilterFloat, TableFilterInt, TableFilterText, TableFilterType,
};
use crate::models::value::{
    GetValueResponse, InsertValueRequest, RemoveValueRequest,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::table::table_create::create,
        crate::handlers::table::table_list::list,
        crate::handlers::table::table_info::info,
        crate::handlers::table::table_remove::remove,
    ),
    components(schemas(
        CreateTableRequest,
        Geometry,
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
    )),
    tags(
        (name = "tables", description = "Table operations")
    )
)]
pub struct ApiDoc;
