pub mod domain;
pub mod entity;
pub mod privilege;
pub mod request;
pub mod response;

pub use domain::{Grant, User};
pub use entity::{AclEntry, DataRole, MAX_PRIVILEGES_PER_USER, UserRecord, UserRole, UserSummary};
pub use privilege::{PrivilegeRule, PrivilegeTarget, ResolvedPrivilege, ResolvedTarget, Scope};
pub use request::{
    CreateUserRequest, ListUsersQuery, SetDataPrivilegeRequest, SetGlobalPrivilegeRequest,
    UpdatePasswordRequest,
};
pub use response::{PrivilegesResponse, UserInfoResponse, UserListResponse};
