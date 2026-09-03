use tonic::{Request, Response, Status};

use super::auth_ctx::authenticate;
use super::convert_users::{privilege_rules_to_domain, user_role_to_pb};
use super::pb::{
    CreateUserRequest, CreateUserResponse, DeleteUserRequest, DeleteUserResponse,
    GetPrivilegesRequest, GetUserRequest, GrantPrivilegeRequest, GrantPrivilegeResponse,
    ListUsersRequest, ListUsersResponse, PrivilegesResponse, RevokePrivilegeRequest,
    RevokePrivilegeResponse, UpdatePasswordRequest, UpdatePasswordResponse, UserInfo, UserSummary,
    user_service_server::UserService,
};
use crate::AppState;
use crate::models::users::{
    CreateUserRequest as DomainCreateUserRequest, ListUsersQuery,
    UpdatePasswordRequest as DomainUpdatePasswordRequest,
};
use crate::services::auth::{check_global_admin, check_self_or_admin};

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

    async fn create(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let domain_req = DomainCreateUserRequest {
            username: req.username,
            password: req.password,
            privileges: privilege_rules_to_domain(req.privileges)?,
        };
        crate::services::users::create_user(&self.app_state, domain_req).await?;
        Ok(Response::new(CreateUserResponse {}))
    }

    async fn get(&self, request: Request<GetUserRequest>) -> Result<Response<UserInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_self_or_admin(&auth_user, &req.username)?;
        let user = crate::services::users::get_user(&self.app_state, &req.username).await?;
        Ok(Response::new(UserInfo {
            username: user.username,
            privileges: user.privileges.into_iter().map(Into::into).collect(),
        }))
    }

    async fn delete(
        &self,
        request: Request<DeleteUserRequest>,
    ) -> Result<Response<DeleteUserResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        crate::services::users::delete_user(&self.app_state, &req.username).await?;
        Ok(Response::new(DeleteUserResponse {}))
    }

    async fn update_password(
        &self,
        request: Request<UpdatePasswordRequest>,
    ) -> Result<Response<UpdatePasswordResponse>, Status> {
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
        Ok(Response::new(UpdatePasswordResponse {}))
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
            privileges: privileges.into_iter().map(Into::into).collect(),
        }))
    }

    async fn grant_privilege(
        &self,
        request: Request<GrantPrivilegeRequest>,
    ) -> Result<Response<GrantPrivilegeResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let privilege = req
            .privilege
            .ok_or_else(|| Status::invalid_argument("privilege must be set"))?
            .try_into()?;

        crate::services::users::grant_privilege(&self.app_state, &req.username, privilege).await?;
        Ok(Response::new(GrantPrivilegeResponse {}))
    }

    async fn revoke_privilege(
        &self,
        request: Request<RevokePrivilegeRequest>,
    ) -> Result<Response<RevokePrivilegeResponse>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let target = req
            .target
            .ok_or_else(|| Status::invalid_argument("target must be set"))?
            .try_into()?;

        crate::services::users::revoke_privilege(&self.app_state, &req.username, target).await?;
        Ok(Response::new(RevokePrivilegeResponse {}))
    }
}
