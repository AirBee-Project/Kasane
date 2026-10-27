use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::convert::required;
use super::convert_data::{DEFAULT_CHUNK_SIZE, data_response_to_chunks};
use super::pb::{self, data_service_server::DataService};

#[tonic::async_trait]
impl DataService for super::Service {
    type SearchStream =
        Pin<Box<dyn Stream<Item = Result<pb::SearchDataResponse, Status>> + Send + 'static>>;

    async fn search(
        &self,
        request: Request<pb::SearchDataRequest>,
    ) -> Result<Response<Self::SearchStream>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();

        let result = self
            .0
            .search(
                &user,
                &req.db_name,
                &req.table_name,
                &spatial_ids,
                zoom_level_policy,
                req.format.into(),
            )
            .await?;
        let chunks = data_response_to_chunks(result, DEFAULT_CHUNK_SIZE);
        let stream = tokio_stream::iter(chunks.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn insert(&self, request: Request<pb::WriteDataRequest>) -> Result<Response<()>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();
        let value = required(req.value, "value")?.into();

        self.0
            .insert(
                &user,
                &req.db_name,
                &req.table_name,
                &spatial_ids,
                value,
                zoom_level_policy,
            )
            .await?;
        Ok(Response::new(()))
    }

    async fn upsert(&self, request: Request<pb::WriteDataRequest>) -> Result<Response<()>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();
        let value = required(req.value, "value")?.into();

        self.0
            .upsert(
                &user,
                &req.db_name,
                &req.table_name,
                &spatial_ids,
                value,
                zoom_level_policy,
            )
            .await?;
        Ok(Response::new(()))
    }

    async fn remove(
        &self,
        request: Request<pb::RemoveDataRequest>,
    ) -> Result<Response<()>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let spatial_ids: Vec<_> = req
            .spatial_ids
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let zoom_level_policy = req.zoom_level_policy.into();

        self.0
            .remove_data(
                &user,
                &req.db_name,
                &req.table_name,
                &spatial_ids,
                zoom_level_policy,
            )
            .await?;
        Ok(Response::new(()))
    }
}
