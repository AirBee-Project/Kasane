use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::convert::{
    table_constraints_to_domain, table_data_type_to_domain, table_domain_to_summary_pb,
    table_info_to_pb, table_summary_to_pb, tri_string, update_table_constraints_update_to_domain,
};
use super::pb::{
    CopyTableRequest, CreateTableRequest, DeleteTableRequest, GetTableRequest, ListTablesRequest,
    ListTablesResponse, TableInfo, TableSummary, UpdateTableRequest,
    table_service_server::TableService,
};
use crate::AppState;
use crate::models::database::table::CreateTableRequest as DomainCreateTableRequest;

pub struct TableServiceImpl {
    pub app_state: AppState,
}

#[tonic::async_trait]
impl TableService for TableServiceImpl {
    async fn create(
        &self,
        request: Request<CreateTableRequest>,
    ) -> Result<Response<TableSummary>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let domain_req = DomainCreateTableRequest {
            name: req.name.clone(),
            data_type: table_data_type_to_domain(req.data_type)?,
            max_zoom_level: req.max_zoom_level as u8,
            constraints: table_constraints_to_domain(req.constraints)?,
            description: req.description,
            value_index: req.value_index,
            is_temporal: req.is_temporal,
        };

        let table = crate::services::database::table::create::create(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.name,
            domain_req,
        )
        .await?;
        Ok(Response::new(table_domain_to_summary_pb(table)))
    }

    async fn get(&self, request: Request<GetTableRequest>) -> Result<Response<TableInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let info = crate::services::database::table::info::info(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.table_name,
        )
        .await?;
        Ok(Response::new(table_info_to_pb(info)))
    }

    async fn list(
        &self,
        request: Request<ListTablesRequest>,
    ) -> Result<Response<ListTablesResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let tables =
            crate::services::database::table::list::list(&self.app_state, &req.db_name, &auth_user)
                .await?;
        Ok(Response::new(ListTablesResponse {
            tables: tables.0.into_iter().map(table_summary_to_pb).collect(),
        }))
    }

    async fn update(&self, request: Request<UpdateTableRequest>) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        let constraints = update_table_constraints_update_to_domain(req.constraints)?;
        let description = tri_string(req.description);

        crate::services::database::table::update::table_update(
            self.app_state.clone(),
            &auth_user,
            &req.db_name,
            &req.table_name,
            req.new_name.as_deref(),
            constraints,
            description,
            req.is_temporal,
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn delete(&self, request: Request<DeleteTableRequest>) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        crate::services::database::table::remove::remove(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.table_name,
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn copy(
        &self,
        request: Request<CopyTableRequest>,
    ) -> Result<Response<TableSummary>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        // 未指定ならコピー元と同じデータベース内へコピーする。
        let dest_db_name = req.copy_db_name.as_deref().unwrap_or(&req.db_name);

        let table = crate::services::database::table::copy::copy(
            &self.app_state,
            &auth_user,
            &req.db_name,
            &req.table_name,
            dest_db_name,
            &req.copy_table_name,
        )
        .await?;
        Ok(Response::new(table_domain_to_summary_pb(table)))
    }
}
