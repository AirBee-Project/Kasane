use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::convert::parse_description_update;
use super::pb::{
    CopyDatabaseRequest, CreateDatabaseRequest, DatabaseInfo, DeleteDatabaseRequest,
    DeleteDatabaseResponse, GetDatabaseRequest, ListDatabasesRequest, ListDatabasesResponse,
    UpdateDatabaseRequest, UpdateDatabaseResponse, database_service_server::DatabaseService,
};
use crate::AppState;
use crate::error::AppError;
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
    ) -> Result<Response<UpdateDatabaseResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let description = parse_description_update(req.description_update);

        crate::services::database::update(
            &self.app_state,
            &auth_user,
            &req.db_name,
            req.new_name,
            description,
        )
        .await?;
        Ok(Response::new(UpdateDatabaseResponse {}))
    }

    async fn delete(
        &self,
        request: Request<DeleteDatabaseRequest>,
    ) -> Result<Response<DeleteDatabaseResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_role(&auth_user, UserRole::Manage)?;
        crate::services::database::remove(&self.app_state, &req.db_name).await?;
        Ok(Response::new(DeleteDatabaseResponse {}))
    }

    async fn copy(
        &self,
        request: Request<CopyDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_role(&auth_user, UserRole::Manage)?;
        if req.db_name == req.copy_name {
            return Err(AppError::Conflict(
                "Source and destination database names must be different".to_string(),
            )
            .into());
        }

        let res =
            crate::services::database::copy(&self.app_state, &req.db_name, &req.copy_name).await?;
        Ok(Response::new(res.into()))
    }
}
