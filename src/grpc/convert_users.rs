//! ユーザー・権限まわりの変換。

use tonic::Status;

use super::pb;
use crate::models::users::{
    DataRole as DomainDataRole, PrivilegeRule as DomainPrivilegeRule,
    PrivilegeTarget as DomainPrivilegeTarget, UserRole as DomainUserRole,
};

impl TryFrom<pb::UserRole> for DomainUserRole {
    type Error = Status;

    fn try_from(value: pb::UserRole) -> Result<Self, Self::Error> {
        match value {
            pb::UserRole::Read => Ok(Self::Read),
            pb::UserRole::Write => Ok(Self::Write),
            pb::UserRole::Manage => Ok(Self::Manage),
            pb::UserRole::Admin => Ok(Self::Admin),
            pb::UserRole::Unspecified => Err(Status::invalid_argument("role must be specified")),
        }
    }
}

pub fn user_role_to_domain(value: i32) -> Result<DomainUserRole, Status> {
    pb::UserRole::try_from(value)
        .unwrap_or(pb::UserRole::Unspecified)
        .try_into()
}

impl From<DomainUserRole> for pb::UserRole {
    fn from(role: DomainUserRole) -> Self {
        match role {
            DomainUserRole::Read => Self::Read,
            DomainUserRole::Write => Self::Write,
            DomainUserRole::Manage => Self::Manage,
            DomainUserRole::Admin => Self::Admin,
        }
    }
}

pub fn user_role_to_pb(role: DomainUserRole) -> i32 {
    let pb_role: pb::UserRole = role.into();
    pb_role as i32
}

impl TryFrom<pb::DataRole> for DomainDataRole {
    type Error = Status;

    fn try_from(value: pb::DataRole) -> Result<Self, Self::Error> {
        match value {
            pb::DataRole::Read => Ok(Self::Read),
            pb::DataRole::Write => Ok(Self::Write),
            pb::DataRole::Manage => Ok(Self::Manage),
            pb::DataRole::Unspecified => Err(Status::invalid_argument("role must be specified")),
        }
    }
}

pub fn data_role_to_domain(value: i32) -> Result<DomainDataRole, Status> {
    pb::DataRole::try_from(value)
        .unwrap_or(pb::DataRole::Unspecified)
        .try_into()
}

impl From<DomainDataRole> for pb::DataRole {
    fn from(role: DomainDataRole) -> Self {
        match role {
            DomainDataRole::Read => Self::Read,
            DomainDataRole::Write => Self::Write,
            DomainDataRole::Manage => Self::Manage,
        }
    }
}

pub fn data_role_to_pb(role: DomainDataRole) -> i32 {
    let pb_role: pb::DataRole = role.into();
    pb_role as i32
}

impl TryFrom<pb::PrivilegeRule> for DomainPrivilegeRule {
    type Error = Status;

    fn try_from(rule: pb::PrivilegeRule) -> Result<Self, Self::Error> {
        use pb::privilege_rule::Scope;
        match rule
            .scope
            .ok_or_else(|| Status::invalid_argument("privilege.scope must be set"))?
        {
            Scope::Global(g) => Ok(DomainPrivilegeRule::Global {
                role: user_role_to_domain(g.role)?,
            }),
            Scope::Database(d) => Ok(DomainPrivilegeRule::Database {
                db_name: d.db_name,
                role: data_role_to_domain(d.role)?,
            }),
            Scope::Table(t) => Ok(DomainPrivilegeRule::Table {
                db_name: t.db_name,
                table_name: t.table_name,
                role: data_role_to_domain(t.role)?,
            }),
        }
    }
}

pub fn privilege_rules_to_domain(
    rules: Vec<pb::PrivilegeRule>,
) -> Result<Vec<DomainPrivilegeRule>, Status> {
    rules.into_iter().map(TryInto::try_into).collect()
}

impl From<DomainPrivilegeRule> for pb::PrivilegeRule {
    fn from(rule: DomainPrivilegeRule) -> Self {
        let scope = match rule {
            DomainPrivilegeRule::Global { role } => {
                pb::privilege_rule::Scope::Global(pb::privilege_rule::Global {
                    role: user_role_to_pb(role),
                })
            }
            DomainPrivilegeRule::Database { db_name, role } => {
                pb::privilege_rule::Scope::Database(pb::privilege_rule::Database {
                    db_name,
                    role: data_role_to_pb(role),
                })
            }
            DomainPrivilegeRule::Table {
                db_name,
                table_name,
                role,
            } => pb::privilege_rule::Scope::Table(pb::privilege_rule::Table {
                db_name,
                table_name,
                role: data_role_to_pb(role),
            }),
        };
        pb::PrivilegeRule { scope: Some(scope) }
    }
}

impl TryFrom<pb::PrivilegeTarget> for DomainPrivilegeTarget {
    type Error = Status;

    fn try_from(target: pb::PrivilegeTarget) -> Result<Self, Self::Error> {
        use pb::privilege_target::Target;
        match target
            .target
            .ok_or_else(|| Status::invalid_argument("privilege_target.target must be set"))?
        {
            Target::Global(_) => Ok(DomainPrivilegeTarget::Global),
            Target::Database(d) => Ok(DomainPrivilegeTarget::Database { db_name: d.db_name }),
            Target::Table(t) => Ok(DomainPrivilegeTarget::Table {
                db_name: t.db_name,
                table_name: t.table_name,
            }),
        }
    }
}

impl From<DomainPrivilegeTarget> for pb::PrivilegeTarget {
    fn from(target: DomainPrivilegeTarget) -> Self {
        let target = match target {
            DomainPrivilegeTarget::Global => {
                pb::privilege_target::Target::Global(pb::privilege_target::Global {})
            }
            DomainPrivilegeTarget::Database { db_name } => {
                pb::privilege_target::Target::Database(pb::privilege_target::Database { db_name })
            }
            DomainPrivilegeTarget::Table {
                db_name,
                table_name,
            } => pb::privilege_target::Target::Table(pb::privilege_target::Table {
                db_name,
                table_name,
            }),
        };
        pb::PrivilegeTarget {
            target: Some(target),
        }
    }
}
