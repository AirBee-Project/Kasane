use crate::r#type::interval_set::{Interval, IntervalSet};
use std::ops::Bound::Included;

impl<T> IntervalSet<T>
where
    T: PartialOrd + Copy + Ord,
{
    /// 区間を挿入（重なる区間はマージされる）
    pub fn insert(&mut self, mut new_iv: Interval<T>) {
        // new_iv と重なる区間を収集
        let overlapping: Vec<Interval<T>> = self
            .set
            .range((
                Included(Interval {
                    start: new_iv.start,
                    end: new_iv.start,
                }),
                Included(Interval {
                    start: new_iv.end,
                    end: new_iv.end,
                }),
            ))
            .filter(|iv| iv.overlaps(&new_iv))
            .cloned()
            .collect();

        // 既存の重複を削除 & マージ
        for iv in &overlapping {
            self.set.remove(iv);
            new_iv = new_iv.merge(iv).unwrap();
        }

        self.set.insert(new_iv);
    }
}
