use crate::r#type::interval_set::{Interval, IntervalSet};
use std::ops::Bound::Included;

impl<T> IntervalSet<T>
where
    T: PartialOrd + Copy + Ord,
{
    /// 登録されている全区間を返す
    pub fn intervals(&self) -> impl Iterator<Item = &Interval<T>> {
        self.set.iter()
    }
}
