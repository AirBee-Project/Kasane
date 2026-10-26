//! ユーザー・権限まわりの変換。

use tonic::Status;

use super::convert::{enum_from_i32, required};
use super::pb;
use crate::error::AppError;
use crate::models::users::{
    DataRole, PrivilegeRule, PrivilegeTarget, UserDetail, UserRole, UserSummary,
};

impl TryFrom<pb::UserRole> for UserRole {
    type Error = Status;

    fn try_from(value: pb::UserRole) -> Result<Self, Self::Error> {
        match value {
            pb::UserRole::Read => Ok(Self::Read),
            pb::UserRole::Write => Ok(Self::Write),
            pb::UserRole::Manage => Ok(Self::Manage),
            pb::UserRole::Admin => Ok(Self::Admin),
            pb::UserRole::Unspecified => Err(AppError::InvalidArgument {
                reason: "role must be specified".to_string(),
            }
            .into()),
        }
    }
}

impl TryFrom<i32> for UserRole {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        enum_from_i32(value, pb::UserRole::Unspecified).try_into()
    }
}

impl From<UserRole> for pb::UserRole {
    fn from(role: UserRole) -> Self {
        match role {
            UserRole::Read => Self::Read,
            UserRole::Write => Self::Write,
            UserRole::Manage => Self::Manage,
            UserRole::Admin => Self::Admin,
        }
    }
}

impl From<UserRole> for i32 {
    fn from(role: UserRole) -> Self {
        pb::UserRole::from(role) as i32
    }
}

impl TryFrom<pb::DataRole> for DataRole {
    type Error = Status;

    fn try_from(value: pb::DataRole) -> Result<Self, Self::Error> {
        match value {
            pb::DataRole::Read => Ok(Self::Read),
            pb::DataRole::Write => Ok(Self::Write),
            pb::DataRole::Manage => Ok(Self::Manage),
            pb::DataRole::Unspecified => Err(AppError::InvalidArgument {
                reason: "role must be specified".to_string(),
            }
            .into()),
        }
    }
}

impl TryFrom<i32> for DataRole {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        enum_from_i32(value, pb::DataRole::Unspecified).try_into()
    }
}

impl From<DataRole> for pb::DataRole {
    fn from(role: DataRole) -> Self {
        match role {
            DataRole::Read => Self::Read,
            DataRole::Write => Self::Write,
            DataRole::Manage => Self::Manage,
        }
    }
}

impl From<DataRole> for i32 {
    fn from(role: DataRole) -> Self {
        pb::DataRole::from(role) as i32
    }
}

impl TryFrom<pb::PrivilegeRule> for PrivilegeRule {
    type Error = Status;

    fn try_from(rule: pb::PrivilegeRule) -> Result<Self, Self::Error> {
        use pb::privilege_rule::Scope;
        match required(rule.scope, "privilege.scope")? {
            Scope::Global(g) => Ok(Self::Global {
                role: g.role.try_into()?,
            }),
            Scope::Database(d) => Ok(Self::Database {
                db_name: d.db_name,
                role: d.role.try_into()?,
            }),
            Scope::Table(t) => Ok(Self::Table {
                db_name: t.db_name,
                table_name: t.table_name,
                role: t.role.try_into()?,
            }),
        }
    }
}

impl From<PrivilegeRule> for pb::PrivilegeRule {
    fn from(rule: PrivilegeRule) -> Self {
        let scope = match rule {
            PrivilegeRule::Global { role } => {
                pb::privilege_rule::Scope::Global(pb::privilege_rule::Global { role: role.into() })
            }
            PrivilegeRule::Database { db_name, role } => {
                pb::privilege_rule::Scope::Database(pb::privilege_rule::Database {
                    db_name,
                    role: role.into(),
                })
            }
            PrivilegeRule::Table {
                db_name,
                table_name,
                role,
            } => pb::privilege_rule::Scope::Table(pb::privilege_rule::Table {
                db_name,
                table_name,
                role: role.into(),
            }),
        };
        Self { scope: Some(scope) }
    }
}

impl TryFrom<pb::PrivilegeTarget> for PrivilegeTarget {
    type Error = Status;

    fn try_from(target: pb::PrivilegeTarget) -> Result<Self, Self::Error> {
        use pb::privilege_target::Target;
        match required(target.target, "privilege_target.target")? {
            Target::Global(_) => Ok(Self::Global),
            Target::Database(d) => Ok(Self::Database { db_name: d.db_name }),
            Target::Table(t) => Ok(Self::Table {
                db_name: t.db_name,
                table_name: t.table_name,
            }),
        }
    }
}

impl From<PrivilegeTarget> for pb::PrivilegeTarget {
    fn from(target: PrivilegeTarget) -> Self {
        let target = match target {
            PrivilegeTarget::Global => {
                pb::privilege_target::Target::Global(pb::privilege_target::Global {})
            }
            PrivilegeTarget::Database { db_name } => {
                pb::privilege_target::Target::Database(pb::privilege_target::Database { db_name })
            }
            PrivilegeTarget::Table {
                db_name,
                table_name,
            } => pb::privilege_target::Target::Table(pb::privilege_target::Table {
                db_name,
                table_name,
            }),
        };
        Self {
            target: Some(target),
        }
    }
}

impl From<UserDetail> for pb::UserInfo {
    fn from(detail: UserDetail) -> Self {
        Self {
            username: detail.username,
            privileges: detail.privileges.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<UserSummary> for pb::UserSummary {
    fn from(summary: UserSummary) -> Self {
        Self {
            username: summary.username,
            global_role: summary.global_role.map(Into::into),
        }
    }
}
