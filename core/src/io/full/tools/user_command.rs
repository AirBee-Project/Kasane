use std::collections::HashSet;

use crate::json::input::UserCommand;

impl UserCommand {
    pub fn all() -> HashSet<UserCommand> {
        let mut set = HashSet::new();
        set.insert(UserCommand::CreateUser);
        set.insert(UserCommand::DropUser);
        set.insert(UserCommand::InfoUser);
        set.insert(UserCommand::ShowUsers);
        set.insert(UserCommand::GrantDatabase);
        set.insert(UserCommand::GrantSpace);
        set.insert(UserCommand::GrantKey);
        set.insert(UserCommand::GrauntUser);
        set.insert(UserCommand::RevokeDatabase);
        set.insert(UserCommand::RevokeSpace);
        set.insert(UserCommand::RevokeKey);
        set.insert(UserCommand::RevokeUser);
        set
    }
}
