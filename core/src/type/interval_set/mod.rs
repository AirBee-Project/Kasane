use std::collections::BTreeMap;
use std::ops::Bound;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntervalValue {
    pub end: u64,
    pub value: Vec<u8>,
}

pub struct IntervalSet {
    // キー: (prefix, start)のタプル
    // 値: IntervalValue (end と value)
    map: BTreeMap<(Vec<u8>, u64), IntervalValue>,
}

impl IntervalSet {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    fn make_key(prefix: &[u8], start: u64) -> (Vec<u8>, u64) {
        (prefix.to_vec(), start)
    }

    pub fn has_overlap(&self, prefix: &[u8], start: u64, end: u64) -> bool {
        // 左隣接区間をチェック
        if let Some((_s, e, _v)) = self.find_left_neighbor(prefix, start) {
            if e > start {
                return true;
            }
        }

        // 範囲内の最初の区間をチェック
        let prefix_vec = prefix.to_vec();
        let range_start = (prefix_vec.clone(), start);
        let range_end = (prefix_vec, u64::MAX);

        if let Some(((p, s), _)) = self.map.range(range_start..=range_end).next() {
            if p == prefix && *s < end {
                return true;
            }
        }

        false
    }

    fn insert_unchecked(&mut self, prefix: &[u8], start: u64, end: u64, value: Vec<u8>) {
        let mut new_start = start;
        let mut new_end = end;

        // 左隣接区間とのマージチェック
        if let Some((s, e, v)) = self.find_left_neighbor(prefix, start) {
            if e == start && v == value {
                self.map.remove(&Self::make_key(prefix, s));
                new_start = s;
            }
        }

        // 右隣接区間とのマージチェック
        if let Some((s, e, v)) = self.find_right_neighbor(prefix, end) {
            if s == end && v == value {
                self.map.remove(&Self::make_key(prefix, s));
                new_end = e;
            }
        }

        let iv = IntervalValue {
            end: new_end,
            value,
        };
        self.map.insert(Self::make_key(prefix, new_start), iv);
    }

    pub fn insert(&mut self, prefix: &[u8], start: u64, end: u64, value: Vec<u8>) -> bool {
        if start >= end {
            return false; // ゼロ幅の区間は何もしない
        }

        if self.has_overlap(prefix, start, end) {
            return false;
        }

        self.insert_unchecked(prefix, start, end, value);
        true
    }

    pub fn patch(&mut self, prefix: &[u8], start: u64, end: u64, value: Vec<u8>) {
        if start >= end {
            return; // ゼロ幅の区間は何もしない
        }

        // 1. select で重複区間取得
        let overlapping = self.select(prefix, start, end);

        // 2. 重複区間を除いた残差範囲を線形計算
        let mut insert_ranges = vec![];
        let mut cursor = start;
        for (s, e, _) in overlapping {
            if cursor < s {
                insert_ranges.push((cursor, s));
            }
            cursor = cursor.max(e);
        }
        if cursor < end {
            insert_ranges.push((cursor, end));
        }

        // 3. 残差範囲をまとめて batch_insert
        let intervals: Vec<(u64, u64, Vec<u8>)> = insert_ranges
            .into_iter()
            .map(|(s, e)| (s, e, value.clone()))
            .collect();

        self.batch_insert(prefix, intervals);
    }

    pub fn update(&mut self, prefix: &[u8], start: u64, end: u64, value: Vec<u8>) {
        if start >= end {
            return; // ゼロ幅の区間は何もしない
        }
        self.remove(prefix, start, end);
        // オーバーラップチェック不要（removeで既に削除済み）
        self.insert_unchecked(prefix, start, end, value);
    }

    pub fn remove(&mut self, prefix: &[u8], start: u64, end: u64) {
        if start >= end {
            return; // ゼロ幅の区間は何もしない
        }

        // 関連する区間のみを取得
        let overlapping = self.select(prefix, start, end);

        for (s, e, v) in overlapping {
            self.map.remove(&Self::make_key(prefix, s));

            // 左側の残り部分
            if s < start {
                let left = IntervalValue {
                    end: start,
                    value: v.clone(),
                };
                self.map.insert(Self::make_key(prefix, s), left);
            }

            // 右側の残り部分
            if e > end {
                let right = IntervalValue {
                    end: e,
                    value: v.clone(),
                };
                self.map.insert(Self::make_key(prefix, end), right);
            }
        }
    }

    pub fn select(&self, prefix: &[u8], start: u64, end: u64) -> Vec<(u64, u64, Vec<u8>)> {
        if start >= end {
            return Vec::new(); // ゼロ幅のクエリは常に空の結果を返す
        }

        let mut result = Vec::new();
        let prefix_vec = prefix.to_vec();

        // スキャン開始キーを効率的に決定
        let scan_start = if let Some((k, _)) = self
            .map
            .range(..Self::make_key(prefix, start))
            .rev()
            .find(|((p, _), _)| p == &prefix_vec)
        {
            k.clone()
        } else {
            (prefix_vec.clone(), 0)
        };

        let range_end = (prefix_vec.clone(), u64::MAX);

        for ((p, s), iv) in self.map.range(scan_start..=range_end) {
            if p != &prefix_vec {
                continue;
            }

            // 探索範囲が目的の範囲を完全に通り過ぎたら終了
            if *s >= end {
                break;
            }

            // 区間 [s, iv.end) が [start, end) と重なっているかチェック
            if iv.end > start {
                result.push((*s, iv.end, iv.value.clone()));
            }
        }
        result
    }

    pub fn get_all(&self, prefix: &[u8]) -> Vec<(u64, u64, Vec<u8>)> {
        let prefix_vec = prefix.to_vec();
        let mut result = Vec::new();

        let range_start = (prefix_vec.clone(), 0);
        let range_end = (prefix_vec.clone(), u64::MAX);

        for ((p, s), iv) in self.map.range(range_start..=range_end) {
            if p != &prefix_vec {
                continue;
            }
            result.push((*s, iv.end, iv.value.clone()));
        }
        result
    }

    pub fn batch_insert(&mut self, prefix: &[u8], intervals: Vec<(u64, u64, Vec<u8>)>) -> usize {
        let mut sorted = intervals;
        sorted.sort_by_key(|i| i.0);

        // メモリ上でマージ
        let mut merged = Vec::new();
        for (start, end, value) in sorted {
            if let Some(last) = merged.last_mut() {
                let (ls, le, lv): &mut (u64, u64, Vec<u8>) = last;
                if *le >= start && *lv == value {
                    *le = (*le).max(end);
                    continue;
                }
            }
            merged.push((start, end, value));
        }

        // まとめて挿入
        let mut inserted = 0;
        for (start, end, value) in merged {
            if !self.has_overlap(prefix, start, end) {
                self.insert_unchecked(prefix, start, end, value);
                inserted += 1;
            }
        }

        inserted
    }

    fn find_left_neighbor(&self, prefix: &[u8], start: u64) -> Option<(u64, u64, Vec<u8>)> {
        let prefix_vec = prefix.to_vec();
        let key = Self::make_key(prefix, start);

        self.map
            .range(..key)
            .rev()
            .find(|((p, _), _)| p == &prefix_vec)
            .map(|((_, s), iv)| (*s, iv.end, iv.value.clone()))
    }

    fn find_right_neighbor(&self, prefix: &[u8], end: u64) -> Option<(u64, u64, Vec<u8>)> {
        let prefix_vec = prefix.to_vec();
        let key = Self::make_key(prefix, end);
        let range_end = (prefix_vec.clone(), u64::MAX);

        self.map
            .range(key..=range_end)
            .find(|((p, s), _)| p == &prefix_vec && *s >= end)
            .map(|((_, s), iv)| (*s, iv.end, iv.value.clone()))
    }
}

impl Default for IntervalSet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_select() {
        let mut set = IntervalSet::new();
        let prefix = b"test";

        assert!(set.insert(prefix, 0, 10, vec![1]));
        assert!(set.insert(prefix, 20, 30, vec![2]));

        let result = set.select(prefix, 0, 30);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], (0, 10, vec![1]));
        assert_eq!(result[1], (20, 30, vec![2]));
    }

    #[test]
    fn test_overlap_detection() {
        let mut set = IntervalSet::new();
        let prefix = b"test";

        set.insert(prefix, 10, 20, vec![1]);
        assert!(!set.insert(prefix, 15, 25, vec![2])); // オーバーラップ
        assert!(set.insert(prefix, 20, 30, vec![2])); // 隣接は可能
    }

    #[test]
    fn test_merge_adjacent() {
        let mut set = IntervalSet::new();
        let prefix = b"test";

        set.insert(prefix, 0, 10, vec![1]);
        set.insert(prefix, 10, 20, vec![1]); // 同じ値で隣接

        let result = set.get_all(prefix);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], (0, 20, vec![1]));
    }

    #[test]
    fn test_remove() {
        let mut set = IntervalSet::new();
        let prefix = b"test";

        set.insert(prefix, 0, 30, vec![1]);
        set.remove(prefix, 10, 20);

        let result = set.get_all(prefix);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], (0, 10, vec![1]));
        assert_eq!(result[1], (20, 30, vec![1]));
    }

    #[test]
    fn test_update() {
        let mut set = IntervalSet::new();
        let prefix = b"test";

        set.insert(prefix, 0, 30, vec![1]);
        set.update(prefix, 10, 20, vec![2]);

        let result = set.get_all(prefix);
        assert_eq!(result.len(), 3);
        assert_eq!(result[1], (10, 20, vec![2]));
    }

    #[test]
    fn test_patch() {
        let mut set = IntervalSet::new();
        let prefix = b"test";

        set.insert(prefix, 5, 15, vec![1]);
        set.insert(prefix, 20, 30, vec![1]);
        set.patch(prefix, 0, 25, vec![2]);

        let result = set.get_all(prefix);
        // [0,5), [5,15), [15,20), [20,30) のうち
        // [5,15) と [20,30) は既存、[0,5) と [15,20) が追加される
        assert!(
            result
                .iter()
                .any(|&(s, e, ref v)| s == 0 && e == 5 && v == &vec![2])
        );
        assert!(
            result
                .iter()
                .any(|&(s, e, ref v)| s == 15 && e == 20 && v == &vec![2])
        );
    }
}
