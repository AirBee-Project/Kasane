use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::convert::required;
use super::convert_data::{DEFAULT_CHUNK_SIZE, data_response_to_chunks};
use super::pb::{
    RemoveDataRequest, SearchDataRequest, SearchDataResponse, WriteDataRequest,
    data_service_server::DataService,
};
use crate::Kasane;
use crate::models::database::table::data::GetDataQuery;

#[tonic::async_trait]
impl DataService for Kasane {
    type SearchStream =
        Pin<Box<dyn Stream<Item = Result<SearchDataResponse, Status>> + Send + 'static>>;

    async fn search(
        &self,
        request: Request<SearchDataRequest>,
    ) -> Result<Response<Self::SearchStream>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();
        let query = GetDataQuery {
            format: req.format.into(),
        };

        let result = crate::services::database::data::get(
            self,
            &user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            &zoom_level_policy,
            &query,
        )
        .await?;
        let chunks = data_response_to_chunks(result, DEFAULT_CHUNK_SIZE);
        let stream = tokio_stream::iter(chunks.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn insert(&self, request: Request<WriteDataRequest>) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();
        let value = required(req.value, "value")?.into();

        crate::services::database::data::insert(
            self,
            &user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            value,
            &zoom_level_policy,
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn upsert(&self, request: Request<WriteDataRequest>) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();
        let value = required(req.value, "value")?.into();

        crate::services::database::data::upsert(
            self,
            &user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            value,
            &zoom_level_policy,
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn remove(&self, request: Request<RemoveDataRequest>) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();

        crate::services::database::data::remove(
            self,
            &user,
            &req.db_name,
            &req.table_name,
            &spatial_ids,
            &zoom_level_policy,
        )
        .await?;
        Ok(Response::new(()))
    }
}
