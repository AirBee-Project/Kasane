use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::convert_users::{
    data_role_to_domain, privilege_rule_to_pb, privilege_rules_to_domain, user_role_to_domain,
    user_role_to_pb,
};
use super::pb::{
    CreateUserRequest, DeleteDatabasePrivilegeRequest, DeleteGlobalPrivilegeRequest,
    DeleteTablePrivilegeRequest, DeleteUserRequest, GetPrivilegesRequest, GetUserRequest,
    ListUsersRequest, ListUsersResponse, PrivilegesResponse, SetDatabasePrivilegeRequest,
    SetGlobalPrivilegeRequest, SetTablePrivilegeRequest, UpdatePasswordRequest, UserInfo,
    UserSummary, user_service_server::UserService,
};
use crate::AppState;
use crate::middleware::auth::{check_global_admin, check_self_or_admin};
use crate::models::users::{
    CreateUserRequest as DomainCreateUserRequest, ListUsersQuery,
    PrivilegeRule as DomainPrivilegeRule, PrivilegeTarget,
    UpdatePasswordRequest as DomainUpdatePasswordRequest,
};

pub struct UserServiceImpl {
    pub app_state: AppState,
}

fn summary_to_pb(summary: crate::models::users::UserSummary) -> UserSummary {
    UserSummary {
        username: summary.username,
        global_role: summary.global_role.map(user_role_to_pb),
    }
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn list(
        &self,
        request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let query = ListUsersQuery {
            after: req.after,
            limit: req.limit.map(|v| v as usize),
        };
        let result = crate::services::users::list_users(&self.app_state, &query).await?;
        Ok(Response::new(ListUsersResponse {
            users: result.users.into_iter().map(summary_to_pb).collect(),
            next: result.next,
        }))
    }

    async fn create(&self, request: Request<CreateUserRequest>) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let domain_req = DomainCreateUserRequest {
            username: req.username,
            password: req.password,
            privileges: privilege_rules_to_domain(req.privileges)?,
        };
        crate::services::users::create_user(&self.app_state, domain_req).await?;
        Ok(Response::new(()))
    }

    async fn get(&self, request: Request<GetUserRequest>) -> Result<Response<UserInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_self_or_admin(&auth_user, &req.username)?;
        let user = crate::services::users::get_user(&self.app_state, &req.username).await?;
        Ok(Response::new(UserInfo {
            username: user.username,
            privileges: user
                .privileges
                .into_iter()
                .map(privilege_rule_to_pb)
                .collect(),
        }))
    }

    async fn delete(&self, request: Request<DeleteUserRequest>) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        crate::services::users::delete_user(&self.app_state, &req.username).await?;
        Ok(Response::new(()))
    }

    async fn update_password(
        &self,
        request: Request<UpdatePasswordRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_self_or_admin(&auth_user, &req.username)?;
        crate::services::users::update_password(
            &self.app_state,
            &req.username,
            DomainUpdatePasswordRequest {
                password: req.password,
            },
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn get_privileges(
        &self,
        request: Request<GetPrivilegesRequest>,
    ) -> Result<Response<PrivilegesResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_self_or_admin(&auth_user, &req.username)?;
        let privileges =
            crate::services::users::get_privileges(&self.app_state, &req.username).await?;
        Ok(Response::new(PrivilegesResponse {
            privileges: privileges.into_iter().map(privilege_rule_to_pb).collect(),
        }))
    }

    async fn set_global_privilege(
        &self,
        request: Request<SetGlobalPrivilegeRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let role = user_role_to_domain(req.role)?;
        crate::services::users::grant_privilege(
            &self.app_state,
            &req.username,
            DomainPrivilegeRule::Global { role },
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn delete_global_privilege(
        &self,
        request: Request<DeleteGlobalPrivilegeRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        crate::services::users::revoke_privilege(
            &self.app_state,
            &req.username,
            PrivilegeTarget::Global,
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn set_database_privilege(
        &self,
        request: Request<SetDatabasePrivilegeRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let role = data_role_to_domain(req.role)?;
        crate::services::users::grant_privilege(
            &self.app_state,
            &req.username,
            DomainPrivilegeRule::Database {
                db_name: req.db_name,
                role,
            },
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn delete_database_privilege(
        &self,
        request: Request<DeleteDatabasePrivilegeRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        crate::services::users::revoke_privilege(
            &self.app_state,
            &req.username,
            PrivilegeTarget::Database {
                db_name: req.db_name,
            },
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn set_table_privilege(
        &self,
        request: Request<SetTablePrivilegeRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let role = data_role_to_domain(req.role)?;
        crate::services::users::grant_privilege(
            &self.app_state,
            &req.username,
            DomainPrivilegeRule::Table {
                db_name: req.db_name,
                table_name: req.table_name,
                role,
            },
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn delete_table_privilege(
        &self,
        request: Request<DeleteTablePrivilegeRequest>,
    ) -> Result<Response<()>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        crate::services::users::revoke_privilege(
            &self.app_state,
            &req.username,
            PrivilegeTarget::Table {
                db_name: req.db_name,
                table_name: req.table_name,
            },
        )
        .await?;
        Ok(Response::new(()))
    }
}
