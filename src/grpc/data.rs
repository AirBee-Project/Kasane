use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::convert_data::{
    output_format_to_domain, spatial_ids_to_domain, typed_value_to_json,
    zoom_level_policy_to_domain,
};
use super::pb::{
    InsertDataRequest, InsertDataResponse, RemoveDataRequest, RemoveDataResponse,
    SearchDataRequest, SearchDataResponse, UpsertDataRequest, UpsertDataResponse,
    data_service_server::DataService,
};
use crate::AppState;
use crate::models::database::table::data::GetDataQuery;

pub struct DataServiceImpl {
    pub app_state: AppState,
}

#[tonic::async_trait]
impl DataService for DataServiceImpl {
    async fn search(
        &self,
        request: Request<SearchDataRequest>,
    ) -> Result<Response<SearchDataResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let spatial_ids = spatial_ids_to_domain(req.spatial_ids)?;
        let zoom_level_policy = zoom_level_policy_to_domain(req.zoom_level_policy);
        let query = GetDataQuery {
            format: output_format_to_domain(req.format),
            limit: req.limit.map(|v| v as usize),
        };

        let result = crate::services::database::table::data::get::get(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            &zoom_level_policy,
            &query,
        )
        .await?;
        Ok(Response::new(result.into()))
    }

    async fn insert(
        &self,
        request: Request<InsertDataRequest>,
    ) -> Result<Response<InsertDataResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let spatial_ids = spatial_ids_to_domain(req.spatial_ids)?;
        let zoom_level_policy = zoom_level_policy_to_domain(req.zoom_level_policy);
        let value = typed_value_to_json(req.value);

        crate::services::database::table::data::insert::insert(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            value,
            &zoom_level_policy,
        )
        .await?;
        Ok(Response::new(InsertDataResponse {}))
    }

    async fn upsert(
        &self,
        request: Request<UpsertDataRequest>,
    ) -> Result<Response<UpsertDataResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let spatial_ids = spatial_ids_to_domain(req.spatial_ids)?;
        let zoom_level_policy = zoom_level_policy_to_domain(req.zoom_level_policy);
        let value = typed_value_to_json(req.value);

        crate::services::database::table::data::upsert::upsert(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            value,
            &zoom_level_policy,
        )
        .await?;
        Ok(Response::new(UpsertDataResponse {}))
    }

    async fn remove(
        &self,
        request: Request<RemoveDataRequest>,
    ) -> Result<Response<RemoveDataResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let spatial_ids = spatial_ids_to_domain(req.spatial_ids)?;
        let zoom_level_policy = zoom_level_policy_to_domain(req.zoom_level_policy);

        crate::services::database::table::data::remove::remove(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            &zoom_level_policy,
        )
        .await?;
        Ok(Response::new(RemoveDataResponse {}))
    }
}
