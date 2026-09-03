//! ユーザー・権限まわりの変換。

use tonic::Status;

use super::convert::enum_from_i32;
use super::pb;
use crate::models::users::{
    DataRole as DomainDataRole, PrivilegeRule as DomainPrivilegeRule,
    PrivilegeTarget as DomainPrivilegeTarget, UserRole as DomainUserRole,
};

// ---------------------------------------------------------------------------
// UserRole
// ---------------------------------------------------------------------------

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

impl TryFrom<i32> for DomainUserRole {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        enum_from_i32(value, pb::UserRole::Unspecified).try_into()
    }
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

impl From<DomainUserRole> for i32 {
    fn from(role: DomainUserRole) -> Self {
        pb::UserRole::from(role) as i32
    }
}

// ---------------------------------------------------------------------------
// DataRole
// ---------------------------------------------------------------------------

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

impl TryFrom<i32> for DomainDataRole {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        enum_from_i32(value, pb::DataRole::Unspecified).try_into()
    }
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

impl From<DomainDataRole> for i32 {
    fn from(role: DomainDataRole) -> Self {
        pb::DataRole::from(role) as i32
    }
}

// ---------------------------------------------------------------------------
// PrivilegeRule
// ---------------------------------------------------------------------------

impl TryFrom<pb::PrivilegeRule> for DomainPrivilegeRule {
    type Error = Status;

    fn try_from(rule: pb::PrivilegeRule) -> Result<Self, Self::Error> {
        use pb::privilege_rule::Scope;
        match rule
            .scope
            .ok_or_else(|| Status::invalid_argument("privilege.scope must be set"))?
        {
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

impl From<DomainPrivilegeRule> for pb::PrivilegeRule {
    fn from(rule: DomainPrivilegeRule) -> Self {
        let scope = match rule {
            DomainPrivilegeRule::Global { role } => {
                pb::privilege_rule::Scope::Global(pb::privilege_rule::Global { role: role.into() })
            }
            DomainPrivilegeRule::Database { db_name, role } => {
                pb::privilege_rule::Scope::Database(pb::privilege_rule::Database {
                    db_name,
                    role: role.into(),
                })
            }
            DomainPrivilegeRule::Table {
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

// ---------------------------------------------------------------------------
// PrivilegeTarget
// ---------------------------------------------------------------------------

impl TryFrom<pb::PrivilegeTarget> for DomainPrivilegeTarget {
    type Error = Status;

    fn try_from(target: pb::PrivilegeTarget) -> Result<Self, Self::Error> {
        use pb::privilege_target::Target;
        match target
            .target
            .ok_or_else(|| Status::invalid_argument("privilege_target.target must be set"))?
        {
            Target::Global(_) => Ok(Self::Global),
            Target::Database(d) => Ok(Self::Database { db_name: d.db_name }),
            Target::Table(t) => Ok(Self::Table {
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
        Self {
            target: Some(target),
        }
    }
}
