use std::collections::BTreeSet;
use std::ops::Bound::{Excluded, Included};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Interval<T> {
    pub start: T,
    pub end: T,
}

impl<T> Interval<T>
where
    T: PartialOrd + Copy,
{
    //新しいIntervalの作成
    pub fn new(start: T, end: T) -> Self {
        assert!(start <= end, "Interval start must be <= end");
        Self { start, end }
    }

    //ある値がIntervalに含まれるかを検証する
    pub fn contains(&self, value: T) -> bool {
        self.start <= value && value <= self.end
    }

    //2つのIntervalが重なっているかを判定する
    pub fn overlaps(&self, other: &Self) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    //2つのIntervalを統合する
    //重ならない場合はNoneを返す
    pub fn merge(&self, other: &Self) -> Option<Self> {
        if self.overlaps(other) {
            Some(Self {
                start: if self.start < other.start {
                    self.start
                } else {
                    other.start
                },
                end: if self.end > other.end {
                    self.end
                } else {
                    other.end
                },
            })
        } else {
            None
        }
    }
}

impl<T> Interval<T>
where
    T: Copy + std::ops::Sub<Output = T>,
{
    /// 区間の長さを返す（Tが数値型の場合のみ）
    pub fn len(&self) -> T {
        self.end - self.start
    }
}

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

    /// 指定値が含まれているか
    pub fn contains(&self, x: T) -> bool {
        if let Some(iv) = self.set.range(..=Interval { start: x, end: x }).next_back() {
            iv.contains(x)
        } else {
            false
        }
    }

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

    /// 登録されている全区間を返す
    pub fn intervals(&self) -> impl Iterator<Item = &Interval<T>> {
        self.set.iter()
    }
}
