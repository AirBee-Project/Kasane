use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{self, table_service_server::TableService};
use crate::models::table::CreateTableRequest;

#[tonic::async_trait]
impl TableService for super::Service {
    async fn create(
        &self,
        request: Request<pb::CreateTableRequest>,
    ) -> Result<Response<pb::TableInfo>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let domain_req = CreateTableRequest {
            name: req.name.clone(),
            data_type: req.data_type.try_into()?,
            max_zoom_level: super::convert::u8_from_u32(req.max_zoom_level, "max_zoom_level")?,
            constraints: req.constraints.map(TryInto::try_into).transpose()?,
            description: req.description,
            is_temporal: req.is_temporal,
        };

        let table = self.0.create_table(&user, &req.db_name, domain_req).await?;
        Ok(Response::new(table.into()))
    }

    async fn get(
        &self,
        request: Request<pb::GetTableRequest>,
    ) -> Result<Response<pb::TableInfo>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let info = self.0.table(&user, &req.db_name, &req.table_name).await?;
        Ok(Response::new(info.into()))
    }

    async fn list(
        &self,
        request: Request<pb::ListTablesRequest>,
    ) -> Result<Response<pb::ListTablesResponse>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let tables = self.0.tables(&user, &req.db_name).await?;
        Ok(Response::new(pb::ListTablesResponse {
            tables: tables.into_iter().map(Into::into).collect(),
        }))
    }

    async fn update(
        &self,
        request: Request<pb::UpdateTableRequest>,
    ) -> Result<Response<pb::TableInfo>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        let constraints = req.constraints_update.map(TryInto::try_into).transpose()?;
        let description = req.description_update.map(Into::into);

        let table = self
            .0
            .update_table(
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

    async fn delete(
        &self,
        request: Request<pb::DeleteTableRequest>,
    ) -> Result<Response<()>, Status> {
        let user = authenticate(&self.0, &request).await?;
        let req = request.into_inner();

        self.0
            .remove_table(&user, &req.db_name, &req.table_name)
            .await?;
        Ok(Response::new(()))
    }
}
