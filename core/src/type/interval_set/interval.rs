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
