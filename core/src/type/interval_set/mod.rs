use std::collections::BTreeSet;

use crate::r#type::interval_set::interval::Interval;
pub mod contains;
pub mod insert;
pub mod interval;
pub mod intervals;
pub mod remove;

/// 区間集合: 重なる区間を自動マージ
#[derive(Debug, Default)]
pub struct IntervalSet<T> {
    set: BTreeSet<Interval<T>>,
}

impl<T> IntervalSet<T>
where
    T: PartialOrd + Copy + Ord,
{
    pub fn new() -> Self {
        Self {
            set: BTreeSet::new(),
        }
    }
}
