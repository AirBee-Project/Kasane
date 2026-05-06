use utoipa::OpenApi;

use crate::models::table::response::GetValueResponse;
use crate::models::table::{
    CreateTableRequest, Geometry, InfoTableResponse, InsertValueRequest, PointCoordinate, Query,
    RemoveValueRequest, SpatialId, TableDataType, TableFilter, TableFilterBoolean,
    TableFilterFloat, TableFilterInt, TableFilterText, TableFilterType,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::handlers::table::table_create::create,
        crate::handlers::table::table_info::info,
        crate::handlers::table::table_remove::remove,
    ),
    components(schemas(
        CreateTableRequest,
        Geometry,
        GetValueResponse,
        InfoTableResponse,
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
