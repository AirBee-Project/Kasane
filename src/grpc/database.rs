use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::convert::tri_string;
use super::pb::{
    CopyDatabaseRequest, CreateDatabaseRequest, DatabaseInfo, DeleteDatabaseRequest,
    GetDatabaseRequest, ListDatabasesRequest, ListDatabasesResponse, UpdateDatabaseRequest,
    database_service_server::DatabaseService,
};
use crate::AppState;
use crate::error::AppError;
use crate::middleware::auth::check_global_role;
use crate::models::database::DatabaseInfoResponse;
use crate::models::users::UserRole;

pub struct DatabaseServiceImpl {
    pub app_state: AppState,
}

fn to_pb(info: DatabaseInfoResponse) -> DatabaseInfo {
    DatabaseInfo {
        name: info.name,
        description: info.description,
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
        Ok(Response::new(to_pb(res)))
    }

    async fn get(
        &self,
        request: Request<GetDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let res =
            crate::services::database::info(&self.app_state, &auth_user, &req.db_name).await?;
        Ok(Response::new(to_pb(res)))
    }

    async fn list(
        &self,
        request: Request<ListDatabasesRequest>,
    ) -> Result<Response<ListDatabasesResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;

        let res = crate::services::database::list(&self.app_state, &auth_user).await?;
        Ok(Response::new(ListDatabasesResponse {
            databases: res.into_iter().map(to_pb).collect(),
        }))
    }

    async fn update(
        &self,
        request: Request<UpdateDatabaseRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let description = tri_string(req.description);
        crate::services::database::update(
            &self.app_state,
            &auth_user,
            &req.db_name,
            req.new_name,
            description,
        )
        .await?;
        Ok(Response::new(()))
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
        Ok(Response::new(to_pb(res)))
    }
}
