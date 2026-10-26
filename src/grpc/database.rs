use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{
    CreateDatabaseRequest, DatabaseInfo, DeleteDatabaseRequest, GetDatabaseRequest,
    ListDatabasesRequest, ListDatabasesResponse, UpdateDatabaseRequest,
    database_service_server::DatabaseService,
};
use crate::AppState;
use crate::models::database::DatabaseInfoResponse;
use crate::models::users::UserRole;
use crate::services::auth::check_global_role;

pub struct DatabaseServiceImpl {
    pub app_state: AppState,
}

impl From<DatabaseInfoResponse> for DatabaseInfo {
    fn from(info: DatabaseInfoResponse) -> Self {
        Self {
            name: info.name,
            description: info.description,
        }
    }
}

#[tonic::async_trait]
impl DatabaseService for DatabaseServiceImpl {
    async fn create(
        &self,
        request: Request<CreateDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_role(&auth_user, UserRole::Manage)?;
        let res =
            crate::services::database::create(&self.app_state, &req.name, req.description).await?;
        Ok(Response::new(res.into()))
    }

    async fn get(
        &self,
        request: Request<GetDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let res =
            crate::services::database::info(&self.app_state, &auth_user, &req.db_name).await?;
        Ok(Response::new(res.into()))
    }

    async fn list(
        &self,
        request: Request<ListDatabasesRequest>,
    ) -> Result<Response<ListDatabasesResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;

        let res = crate::services::database::list(&self.app_state, &auth_user).await?;
        Ok(Response::new(ListDatabasesResponse {
            databases: res.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update(
        &self,
        request: Request<UpdateDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let description = req.description_update.map(Into::into);

        let res = crate::services::database::update(
            &self.app_state,
            &auth_user,
            &req.db_name,
            req.new_name,
            description,
        )
        .await?;
        Ok(Response::new(res.into()))
    }

    async fn delete(
        &self,
        request: Request<DeleteDatabaseRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_role(&auth_user, UserRole::Manage)?;
        crate::services::database::remove(&self.app_state, &req.db_name).await?;
        Ok(Response::new(()))
    }
}
