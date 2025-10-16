use crate::r#type::{interval_set::IntervalSet, space_time_id::SpaceTimeId};
pub mod insert;
pub struct SpaceTimeIdSet {
    x: IntervalSet,
    y: IntervalSet,
    f: IntervalSet,
    t: IntervalSet,
    ///内部にある時空間IDの一意なKey
    index: Vec<u64>,
}

impl SpaceTimeIdSet {
    pub fn new() -> Self {
        Self {
            x: IntervalSet::new(),
            y: IntervalSet::new(),
            f: IntervalSet::new(),
            t: IntervalSet::new(),
            index: vec![],
        }
    }
}
