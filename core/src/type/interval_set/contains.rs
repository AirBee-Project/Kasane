use crate::r#type::interval_set::{Interval, IntervalSet};
use std::ops::Bound::Included;

impl<T> IntervalSet<T>
where
    T: PartialOrd + Copy + Ord,
{
    /// 指定値が含まれているか
    pub fn contains(&self, x: T) -> bool {
        if let Some(iv) = self.set.range(..=Interval { start: x, end: x }).next_back() {
            iv.contains(x)
        } else {
            false
        }
    }
}
