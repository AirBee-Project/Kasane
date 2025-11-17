use std::collections::HashSet;

use crate::interface::input::SpaceCommand;

impl SpaceCommand {
    pub fn all() -> HashSet<SpaceCommand> {
        let mut set = HashSet::new();
        set.insert(SpaceCommand::CreateKey);
        set.insert(SpaceCommand::DropKey);
        set.insert(SpaceCommand::ShowKeys);
        set.insert(SpaceCommand::InfoKey);
        set
    }
}
