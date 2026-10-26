use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{
    CreateUserRequest, DeleteUserRequest, GetUserRequest, GrantPrivilegeRequest, ListUsersRequest,
    ListUsersResponse, RevokePrivilegeRequest, UpdatePasswordRequest, UserInfo, UserSummary,
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

/// 更新系の RPC が返す、更新後の利用者情報。
async fn user_info(app_state: &AppState, username: &str) -> Result<UserInfo, Status> {
    let user = crate::services::users::get_user(app_state, username).await?;
    Ok(UserInfo {
        username: user.username,
        privileges: user.privileges.into_iter().map(Into::into).collect(),
    })
}

fn summary_to_pb(summary: crate::models::users::UserSummary) -> UserSummary {
    UserSummary {
        username: summary.username,
        global_role: summary.global_role.map(Into::into),
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
    ) -> Result<Response<UserInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let privileges = req
            .privileges
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let domain_req = DomainCreateUserRequest {
            username: req.username.clone(),
            password: req.password,
            privileges,
        };
        crate::services::users::create_user(&self.app_state, domain_req).await?;
        Ok(Response::new(
            user_info(&self.app_state, &req.username).await?,
        ))
    }

    async fn get(&self, request: Request<GetUserRequest>) -> Result<Response<UserInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_self_or_admin(&auth_user, &req.username)?;
        Ok(Response::new(
            user_info(&self.app_state, &req.username).await?,
        ))
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

    async fn grant_privilege(
        &self,
        request: Request<GrantPrivilegeRequest>,
    ) -> Result<Response<UserInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let privilege = super::convert::required(req.privilege, "privilege")?.try_into()?;

        crate::services::users::grant_privilege(&self.app_state, &req.username, privilege).await?;
        Ok(Response::new(
            user_info(&self.app_state, &req.username).await?,
        ))
    }

    async fn revoke_privilege(
        &self,
        request: Request<RevokePrivilegeRequest>,
    ) -> Result<Response<UserInfo>, Status> {
        let auth_user = authenticate(&self.app_state, &request).await?;
        let req = request.into_inner();

        check_global_admin(&auth_user)?;
        let target = super::convert::required(req.target, "target")?.try_into()?;

        crate::services::users::revoke_privilege(&self.app_state, &req.username, target).await?;
        Ok(Response::new(
            user_info(&self.app_state, &req.username).await?,
        ))
    }
}
