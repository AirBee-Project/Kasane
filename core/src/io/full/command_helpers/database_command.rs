use std::collections::HashSet;

use crate::interface::input::DatabaseCommand;

impl DatabaseCommand {
    pub fn all() -> HashSet<DatabaseCommand> {
        let mut set = HashSet::new();
        set.insert(DatabaseCommand::CreateSpace);
        set.insert(DatabaseCommand::DropSpace);
        set.insert(DatabaseCommand::ShowSpaces);
        set.insert(DatabaseCommand::Version);
        set.insert(DatabaseCommand::InfoSpace);
        set
    }
}
