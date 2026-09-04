use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::convert_data::{DEFAULT_CHUNK_SIZE, data_response_to_chunks};
use super::convert_query::ExecuteQuery;
use super::pb::{ExecuteQueryRequest, SearchDataResponse, query_service_server::QueryService};
use crate::AppState;
use crate::models::database::table::data::GetDataQuery;

pub struct QueryServiceImpl {
    pub app_state: AppState,
}

#[tonic::async_trait]
impl QueryService for QueryServiceImpl {
    type ExecuteStream =
        Pin<Box<dyn Stream<Item = Result<SearchDataResponse, Status>> + Send + 'static>>;

    async fn execute(
        &self,
        request: Request<ExecuteQueryRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let parsed: ExecuteQuery = req.try_into()?;
        let query_params = GetDataQuery {
            format: parsed.format,
            limit: parsed.limit,
        };

        let result = crate::services::query::execute(
            &self.app_state,
            &auth_user,
            parsed.request,
            &query_params,
        )
        .await?;
        let chunks = data_response_to_chunks(result, DEFAULT_CHUNK_SIZE);
        let stream = tokio_stream::iter(chunks.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }
}
