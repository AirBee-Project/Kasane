use utoipa::OpenApi;

use crate::models::query::{
    Geometry, PointCoordinate, Query, SpatialId, TableFilter, TableFilterBoolean, TableFilterFloat,
    TableFilterInt, TableFilterText, TableFilterType,
};
use crate::models::table::value::{
    GetValueRequest, GetValueResponse, InsertValueRequest, RemoveValueRequest,
};
use crate::models::table::{
    CreateTableRequest, TableDataType, TableInfoResponse, TableListResponse,
};

#[derive(OpenApi)]
#[openapi(
    paths(
        // GET  /layers
        crate::handlers::table::table_list::table_list,
        // POST /layers
        crate::handlers::table::table_create::table_create,
        // GET  /layers/{name}
        crate::handlers::table::table_info::table_info,
        // DELETE /layers/{name}
        crate::handlers::table::table_remove::table_remove,
        // PUT    /layers/{name}/data
        crate::handlers::table::value::insert::value_insert,
        // PATCH  /layers/{name}/data
        crate::handlers::table::value::upsert::value_upsert,
        // DELETE /layers/{name}/data
        crate::handlers::table::value::remove::value_remove,
        // POST   /layers/{name}/data/search
        crate::handlers::table::value::get::value_get,
    ),
    components(schemas(
        // Layer
        CreateTableRequest,
        TableDataType,
        TableInfoResponse,
        TableListResponse,
        // Data
        GetValueRequest,
        GetValueResponse,
        InsertValueRequest,
        RemoveValueRequest,
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
        (name = "layers", description = "Layer operations")
    )
)]
pub struct ApiDoc;
