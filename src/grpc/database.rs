use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{
    CreateDatabaseRequest, DatabaseInfo, DeleteDatabaseRequest, GetDatabaseRequest,
    ListDatabasesRequest, ListDatabasesResponse, UpdateDatabaseRequest,
    database_service_server::DatabaseService,
};
use crate::Kasane;
use crate::models::database::DatabaseDetail;

impl From<DatabaseDetail> for DatabaseInfo {
    fn from(info: DatabaseDetail) -> Self {
        Self {
            name: info.name,
            description: info.description,
        }
    }
}

#[tonic::async_trait]
impl DatabaseService for Kasane {
    async fn create(
        &self,
        request: Request<CreateDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let res =
            crate::services::database::create(self, &user, &req.name, req.description).await?;
        Ok(Response::new(res.into()))
    }

    async fn get(
        &self,
        request: Request<GetDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let res = crate::services::database::info(self, &user, &req.db_name).await?;
        Ok(Response::new(res.into()))
    }

    async fn list(
        &self,
        request: Request<ListDatabasesRequest>,
    ) -> Result<Response<ListDatabasesResponse>, Status> {
        let user = authenticate(self, &request).await?;

        let res = crate::services::database::list(self, &user).await?;
        Ok(Response::new(ListDatabasesResponse {
            databases: res.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update(
        &self,
        request: Request<UpdateDatabaseRequest>,
    ) -> Result<Response<DatabaseInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let description = req.description_update.map(Into::into);

        let res =
            crate::services::database::update(self, &user, &req.db_name, req.new_name, description)
                .await?;
        Ok(Response::new(res.into()))
    }

    async fn delete(
        &self,
        request: Request<DeleteDatabaseRequest>,
    ) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        crate::services::database::remove(self, &user, &req.db_name).await?;
        Ok(Response::new(()))
    }
}
