//! ユーザー・権限まわりの変換。

use tonic::Status;

use super::pb;
use crate::models::users::{DataRole, PrivilegeRule, UserRole};

pub fn user_role_to_domain(value: i32) -> Result<UserRole, Status> {
    match pb::UserRole::try_from(value).unwrap_or(pb::UserRole::Unspecified) {
        pb::UserRole::Read => Ok(UserRole::Read),
        pb::UserRole::Write => Ok(UserRole::Write),
        pb::UserRole::Manage => Ok(UserRole::Manage),
        pb::UserRole::Admin => Ok(UserRole::Admin),
        pb::UserRole::Unspecified => Err(Status::invalid_argument("role must be specified")),
    }
}

pub fn user_role_to_pb(role: UserRole) -> i32 {
    (match role {
        UserRole::Read => pb::UserRole::Read,
        UserRole::Write => pb::UserRole::Write,
        UserRole::Manage => pb::UserRole::Manage,
        UserRole::Admin => pb::UserRole::Admin,
    }) as i32
}

pub fn data_role_to_domain(value: i32) -> Result<DataRole, Status> {
    match pb::DataRole::try_from(value).unwrap_or(pb::DataRole::Unspecified) {
        pb::DataRole::Read => Ok(DataRole::Read),
        pb::DataRole::Write => Ok(DataRole::Write),
        pb::DataRole::Manage => Ok(DataRole::Manage),
        pb::DataRole::Unspecified => Err(Status::invalid_argument("role must be specified")),
    }
}

pub fn data_role_to_pb(role: DataRole) -> i32 {
    (match role {
        DataRole::Read => pb::DataRole::Read,
        DataRole::Write => pb::DataRole::Write,
        DataRole::Manage => pb::DataRole::Manage,
    }) as i32
}

pub fn privilege_rule_to_domain(rule: pb::PrivilegeRule) -> Result<PrivilegeRule, Status> {
    use pb::privilege_rule::Scope;
    match rule
        .scope
        .ok_or_else(|| Status::invalid_argument("privilege.scope must be set"))?
    {
        Scope::Global(g) => Ok(PrivilegeRule::Global {
            role: user_role_to_domain(g.role)?,
        }),
        Scope::Database(d) => Ok(PrivilegeRule::Database {
            db_name: d.db_name,
            role: data_role_to_domain(d.role)?,
        }),
        Scope::Table(t) => Ok(PrivilegeRule::Table {
            db_name: t.db_name,
            table_name: t.table_name,
            role: data_role_to_domain(t.role)?,
        }),
    }
}

pub fn privilege_rules_to_domain(
    rules: Vec<pb::PrivilegeRule>,
) -> Result<Vec<PrivilegeRule>, Status> {
    rules.into_iter().map(privilege_rule_to_domain).collect()
}

pub fn privilege_rule_to_pb(rule: PrivilegeRule) -> pb::PrivilegeRule {
    let scope = match rule {
        PrivilegeRule::Global { role } => {
            pb::privilege_rule::Scope::Global(pb::privilege_rule::Global {
                role: user_role_to_pb(role),
            })
        }
        PrivilegeRule::Database { db_name, role } => {
            pb::privilege_rule::Scope::Database(pb::privilege_rule::Database {
                db_name,
                role: data_role_to_pb(role),
            })
        }
        PrivilegeRule::Table {
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
