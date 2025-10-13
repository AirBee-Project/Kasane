use crate::r#type::interval_set::{Interval, IntervalSet};
use std::ops::Bound::Included;

impl<T> IntervalSet<T>
where
    T: PartialOrd + Copy + Ord,
{
    /// 区間を削除（残り部分が分割される可能性あり）
    pub fn remove(&mut self, target: Interval<T>)
    where
        T: std::ops::Sub<Output = T> + Copy,
    {
        let overlapping: Vec<Interval<T>> = self
            .set
            .range((
                Included(Interval {
                    start: target.start,
                    end: target.start,
                }),
                Included(Interval {
                    start: target.end,
                    end: target.end,
                }),
            ))
            .filter(|iv| iv.overlaps(&target))
            .cloned()
            .collect();

        for iv in overlapping {
            self.set.remove(&iv);

            // 左に残る部分
            if iv.start < target.start {
                self.set.insert(Interval::new(iv.start, target.start));
            }

            // 右に残る部分
            if iv.end > target.end {
                self.set.insert(Interval::new(target.end, iv.end));
            }
        }
    }
}
