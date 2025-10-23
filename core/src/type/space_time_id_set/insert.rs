use crate::r#type::bit_vec::BitVec;
use crate::r#type::space_time_id_set::convert_f::convert_bitmask_f;
use crate::r#type::space_time_id_set::convert_xy::convert_bitmask_xy;
use crate::r#type::{
    space_time_id::SpaceTimeId,
    space_time_id_set::{convert_f::convert_f, convert_xy::convert_xy, Index, SpaceTimeIdSet},
};
use itertools::{iproduct, Iterate};
use std::collections::{HashMap, HashSet};

impl SpaceTimeIdSet {
    pub fn insert(&mut self, id: SpaceTimeId) {
        let f_converted: Vec<(u8, i64)> = convert_f(id.z, id.f);
        let x_converted: Vec<(u8, u64)> = convert_xy(id.z, id.x);
        let y_converted: Vec<(u8, u64)> = convert_xy(id.z, id.y);

        // 各要素を BitVec に変換してベクタにする
        let f_encoded: Vec<_> = f_converted
            .iter()
            .map(|f| convert_bitmask_f(f.0, f.1))
            .collect();
        let x_encoded: Vec<_> = x_converted
            .iter()
            .map(|x| convert_bitmask_xy(x.0, x.1))
            .collect();
        let y_encoded: Vec<_> = y_converted
            .iter()
            .map(|y| convert_bitmask_xy(y.0, y.1))
            .collect();

        // 3つのベクタの直積を取る
        for (f, x, y) in iproduct!(&f_encoded, &x_encoded, &y_encoded) {
            let min = f.min(x.min(y));

            let mut min_dimension_ids;

            //一番小さい次元を求める
            if f == min {
                min_dimension_ids = Self::search_f(&self, f.0)
            } else if x == min {
            } else if y == min {
            }
        }
    }

    fn search_f(&self, f: BitVec) -> HashSet<Index> {
        let mut result = HashSet::new();

        // 上位IDの検索
        for f_top in f.generate_top_prefix() {
            if let Some(v) = self.f.get(&f_top) {
                result.extend(v.iter().cloned());
            }
        }

        // 下位IDの検索
        let start: BitVec = f.clone();
        let end: BitVec = f.generate_bottom_prefix_end();

        for f_bottom in self.f.range(start..end) {
            result.extend(f_bottom.1.iter().cloned());
        }

        result
    }
}
