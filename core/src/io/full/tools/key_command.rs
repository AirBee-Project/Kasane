use std::collections::HashSet;

use crate::json::input::KeyCommand;

impl KeyCommand {
    pub fn all() -> HashSet<KeyCommand> {
        let mut set = HashSet::new();
        set.insert(KeyCommand::InsertValue);
        set.insert(KeyCommand::PatchValue);
        set.insert(KeyCommand::UpdateValue);
        set.insert(KeyCommand::DropKey);
        set.insert(KeyCommand::SelectValue);
        set.insert(KeyCommand::InfoKey);
        set.insert(KeyCommand::ShowValues);
        set.insert(KeyCommand::FilterValue);
        set
    }
}
