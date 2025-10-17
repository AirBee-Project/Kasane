use crate::r#type::space_time_id::SpaceTimeId;
use std::collections::{BTreeMap, HashMap};
pub mod insert;
pub struct SpaceTimeIdSet {
    f: BTreeMap<Vec<u8>, u64>,
    x: BTreeMap<Vec<u8>, u64>,
    y: BTreeMap<Vec<u8>, u64>,
    t: HashMap<u64, (u64, u64)>,
}

impl SpaceTimeIdSet {
    pub fn new() -> Self {
        Self {
            f: BTreeMap::new(),
            x: BTreeMap::new(),
            y: BTreeMap::new(),
            t: HashMap::new(),
        }
    }
}
