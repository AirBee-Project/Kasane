use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{
    CreateTableRequest, DeleteTableRequest, GetTableRequest, ListTablesRequest, ListTablesResponse,
    TableInfo, UpdateTableRequest, table_service_server::TableService,
};
use crate::Kasane;
use crate::models::database::table::CreateTableRequest as DomainCreateTableRequest;

#[tonic::async_trait]
impl TableService for Kasane {
    async fn create(
        &self,
        request: Request<CreateTableRequest>,
    ) -> Result<Response<TableInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let domain_req = DomainCreateTableRequest {
            name: req.name.clone(),
            data_type: req.data_type.try_into()?,
            max_zoom_level: super::convert::u8_from_u32(req.max_zoom_level, "max_zoom_level")?,
            constraints: req.constraints.map(TryInto::try_into).transpose()?,
            description: req.description,
            is_temporal: req.is_temporal,
        };

        let table = crate::services::database::table::create(
            self,
            &user,
            &req.db_name,
            &req.name,
            domain_req,
        )
        .await?;
        Ok(Response::new(table.into()))
    }

    async fn get(&self, request: Request<GetTableRequest>) -> Result<Response<TableInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let info =
            crate::services::database::table::info(self, &user, &req.db_name, &req.table_name)
                .await?;
        Ok(Response::new(info.into()))
    }

    async fn list(
        &self,
        request: Request<ListTablesRequest>,
    ) -> Result<Response<ListTablesResponse>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let tables = crate::services::database::table::list(self, &user, &req.db_name).await?;
        Ok(Response::new(ListTablesResponse {
            tables: tables.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update(
        &self,
        request: Request<UpdateTableRequest>,
    ) -> Result<Response<TableInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let constraints = req.constraints_update.map(TryInto::try_into).transpose()?;
        let description = req.description_update.map(Into::into);

        let table = crate::services::database::table::update(
            self,
            &user,
            &req.db_name,
            &req.table_name,
            req.new_name.as_deref(),
            constraints,
            description,
            req.is_temporal,
        )
        .await?;
        Ok(Response::new(table.into()))
    }

    async fn delete(&self, request: Request<DeleteTableRequest>) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        crate::services::database::table::remove(self, &user, &req.db_name, &req.table_name)
            .await?;
        Ok(Response::new(()))
    }
}
