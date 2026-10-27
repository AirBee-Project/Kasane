use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::convert_data::{DEFAULT_CHUNK_SIZE, data_response_to_chunks};
use super::convert_query::ExecuteQuery;
use super::pb::{self, query_service_server::QueryService};

#[tonic::async_trait]
impl QueryService for super::Service {
    type ExecuteStream =
        Pin<Box<dyn Stream<Item = Result<pb::SearchDataResponse, Status>> + Send + 'static>>;

    async fn execute(
        &self,
        request: Request<pb::ExecuteQueryRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let parsed: ExecuteQuery = req.try_into()?;

        let result = self
            .0
            .execute_query(&user, parsed.request, parsed.format)
            .await?;
        let chunks = data_response_to_chunks(result, DEFAULT_CHUNK_SIZE);
        let stream = tokio_stream::iter(chunks.into_iter().map(Ok));
        Ok(Response::new(Box::pin(stream)))
    }
}
