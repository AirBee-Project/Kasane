use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{
    CreateUserRequest, DeleteUserRequest, GetUserRequest, GrantPrivilegeRequest, ListUsersRequest,
    ListUsersResponse, RevokePrivilegeRequest, UpdatePasswordRequest, UserInfo, UserSummary,
    user_service_server::UserService,
};
use crate::Kasane;
use crate::models::users::User;
use crate::models::users::{
    CreateUserRequest as DomainCreateUserRequest, ListUsersQuery,
    UpdatePasswordRequest as DomainUpdatePasswordRequest,
};

/// 更新系の RPC が返す、更新後の利用者情報。
async fn user_info(kasane: &Kasane, user: &User, username: &str) -> Result<UserInfo, Status> {
    let info = crate::services::users::get_user(kasane, user, username).await?;
    Ok(UserInfo {
        username: info.username,
        privileges: info.privileges.into_iter().map(Into::into).collect(),
    })
}

fn summary_to_pb(summary: crate::models::users::UserSummary) -> UserSummary {
    UserSummary {
        username: summary.username,
        global_role: summary.global_role.map(Into::into),
    }
}

#[tonic::async_trait]
impl UserService for Kasane {
    async fn list(
        &self,
        request: Request<ListUsersRequest>,
    ) -> Result<Response<ListUsersResponse>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let query = ListUsersQuery {
            after: req.after,
            limit: req.limit.map(|v| v as usize),
        };
        let result = crate::services::users::list_users(self, &user, &query).await?;
        Ok(Response::new(ListUsersResponse {
            users: result.users.into_iter().map(summary_to_pb).collect(),
            next: result.next,
        }))
    }

    async fn create(
        &self,
        request: Request<CreateUserRequest>,
    ) -> Result<Response<UserInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

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
        crate::services::users::create_user(self, &user, domain_req).await?;
        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }

    async fn get(&self, request: Request<GetUserRequest>) -> Result<Response<UserInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }

    async fn delete(&self, request: Request<DeleteUserRequest>) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        crate::services::users::delete_user(self, &user, &req.username).await?;
        Ok(Response::new(()))
    }

    async fn update_password(
        &self,
        request: Request<UpdatePasswordRequest>,
    ) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        crate::services::users::update_password(
            self,
            &user,
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
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let privilege = super::convert::required(req.privilege, "privilege")?.try_into()?;

        crate::services::users::grant_privilege(self, &user, &req.username, privilege).await?;
        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }

    async fn revoke_privilege(
        &self,
        request: Request<RevokePrivilegeRequest>,
    ) -> Result<Response<UserInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let target = super::convert::required(req.target, "target")?.try_into()?;

        crate::services::users::revoke_privilege(self, &user, &req.username, target).await?;
        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }
}
