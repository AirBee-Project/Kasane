use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::convert_query::ExecuteQuery;
use super::pb::{ExecuteQueryRequest, SearchDataResponse, query_service_server::QueryService};
use crate::AppState;
use crate::models::database::table::data::GetDataQuery;

pub struct QueryServiceImpl {
    pub app_state: AppState,
}

#[tonic::async_trait]
impl QueryService for QueryServiceImpl {
    async fn execute(
        &self,
        request: Request<ExecuteQueryRequest>,
    ) -> Result<Response<SearchDataResponse>, Status> {
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
        Ok(Response::new(result.into()))
    }
}
