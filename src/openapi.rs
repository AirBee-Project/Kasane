use utoipa::OpenApi;

use crate::models::layer::data::{
    GetDataRequest, GetDataResponse, InsertDataRequest, RemoveDataRequest,
};
use crate::models::layer::{
    CreateLayerRequest, LayerDataType, LayerInfoResponse, LayerListResponse,
};
use crate::models::query::{
    Geometry, LayerFilter, LayerFilterBoolean, LayerFilterFloat, LayerFilterInt, LayerFilterText,
    LayerFilterType, PointCoordinate, Query, SpatialId,
};

#[derive(OpenApi)]
#[openapi(
    servers(),
    paths(
        // GET  /layers
        crate::handlers::layer::list::layer_list,
        // POST /layers
        crate::handlers::layer::create::layer_create,
        // GET  /layers/{name}
        crate::handlers::layer::info::layer_info,
        // DELETE /layers/{name}
        crate::handlers::layer::remove::layer_remove,
        // PUT    /layers/{name}/data
        crate::handlers::layer::data::insert::data_insert,
        // PATCH  /layers/{name}/data
        crate::handlers::layer::data::upsert::data_upsert,
        // DELETE /layers/{name}/data
        crate::handlers::layer::data::remove::data_remove,
        // POST   /layers/{name}/data/search
        crate::handlers::layer::data::get::data_get,
    ),
    components(schemas(
        // Layer
        CreateLayerRequest,
        LayerDataType,
        LayerInfoResponse,
        LayerListResponse,
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
        LayerFilter,
        LayerFilterType,
        LayerFilterText,
        LayerFilterInt,
        LayerFilterFloat,
        LayerFilterBoolean,
    )),
    tags(
        (name = "layers", description = "Layer operations")
    )
)]
pub struct ApiDoc;
