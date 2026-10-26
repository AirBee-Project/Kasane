use tonic::{Request, Response, Status};

use super::auth::authenticate;
use super::pb::{self, user_service_server::UserService};
use crate::Kasane;
use crate::models::users::User;
use crate::models::users::{CreateUserRequest, ListUsersQuery, UpdatePasswordRequest};

/// 更新系の RPC が返す、更新後の利用者情報。
async fn user_info(kasane: &Kasane, user: &User, username: &str) -> Result<pb::UserInfo, Status> {
    Ok(crate::services::users::get_user(kasane, user, username)
        .await?
        .into())
}

#[tonic::async_trait]
impl UserService for Kasane {
    async fn list(
        &self,
        request: Request<pb::ListUsersRequest>,
    ) -> Result<Response<pb::ListUsersResponse>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let query = ListUsersQuery {
            after: req.after,
            limit: req.limit.map(|v| v as usize),
        };
        let result = crate::services::users::list_users(self, &user, &query).await?;
        Ok(Response::new(pb::ListUsersResponse {
            users: result.users.into_iter().map(Into::into).collect(),
            next: result.next,
        }))
    }

    async fn create(
        &self,
        request: Request<pb::CreateUserRequest>,
    ) -> Result<Response<pb::UserInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let privileges = req
            .privileges
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        let domain_req = CreateUserRequest {
            username: req.username.clone(),
            password: req.password,
            privileges,
        };
        crate::services::users::create_user(self, &user, domain_req).await?;
        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }

    async fn get(
        &self,
        request: Request<pb::GetUserRequest>,
    ) -> Result<Response<pb::UserInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }

    async fn delete(
        &self,
        request: Request<pb::DeleteUserRequest>,
    ) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        crate::services::users::delete_user(self, &user, &req.username).await?;
        Ok(Response::new(()))
    }

    async fn update_password(
        &self,
        request: Request<pb::UpdatePasswordRequest>,
    ) -> Result<Response<()>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        crate::services::users::update_password(
            self,
            &user,
            &req.username,
            UpdatePasswordRequest {
                password: req.password,
            },
        )
        .await?;
        Ok(Response::new(()))
    }

    async fn grant_privilege(
        &self,
        request: Request<pb::GrantPrivilegeRequest>,
    ) -> Result<Response<pb::UserInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let privilege = super::convert::required(req.privilege, "privilege")?.try_into()?;

        crate::services::users::grant_privilege(self, &user, &req.username, privilege).await?;
        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }

    async fn revoke_privilege(
        &self,
        request: Request<pb::RevokePrivilegeRequest>,
    ) -> Result<Response<pb::UserInfo>, Status> {
        let user = authenticate(self, &request).await?;
        let req = request.into_inner();

        let target = super::convert::required(req.target, "target")?.try_into()?;

        crate::services::users::revoke_privilege(self, &user, &req.username, target).await?;
        Ok(Response::new(user_info(self, &user, &req.username).await?))
    }
}
