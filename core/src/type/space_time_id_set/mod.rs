use std::collections::{BTreeMap, HashMap, HashSet};

use crate::r#type::{bit_vec::BitVec, interval_manager::IntervalManager};

pub mod convert_f;
pub mod convert_xy;
pub mod insert;

pub struct Reverse {
    pub f: BitVec,
    pub x: BitVec,
    pub y: BitVec,
    pub t: (u64, u64),
}

type Index = u64;
pub struct SpaceTimeIdSet {
    f: BTreeMap<BitVec, HashSet<Index>>,
    x: BTreeMap<BitVec, HashSet<Index>>,
    y: BTreeMap<BitVec, HashSet<Index>>,
    t: IntervalManager,
    reverse: HashMap<Index, Reverse>,
    next_index: Index,
}

impl SpaceTimeIdSet {
    pub fn new() -> Self {
        SpaceTimeIdSet {
            f: BTreeMap::new(),
            x: BTreeMap::new(),
            y: BTreeMap::new(),
            t: IntervalManager::new(),
            reverse: HashMap::new(),
            next_index: 0,
        }
    }

    pub fn generate_index(&mut self) -> u64 {
        self.next_index = self.next_index + 1;
        self.next_index - 1
    }
}
