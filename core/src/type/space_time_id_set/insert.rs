use crate::r#type::bit_vec::BitVec;
use crate::r#type::space_time_id_set::convert_f::convert_bitmask_f;
use crate::r#type::space_time_id_set::convert_xy::convert_bitmask_xy;
use crate::r#type::{
    space_time_id::SpaceTimeId,
    space_time_id_set::{
        convert_f::convert_f, convert_xy::convert_xy, Index, Reverse, SpaceTimeIdSet,
    },
};
use itertools::{iproduct, Iterate};
use std::collections::{BTreeMap, HashMap, HashSet};

enum Relation {
    ///自分が上位である
    Top = 1,

    ///相手が下位である
    Bottom = 2,

    ///同位である
    Equal = 3,

    ///無関係である
    Disjoint = 4,
}

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

            let mut min_dimension_ids: HashSet<u64> = HashSet::new();

            ///挿入するIDから見た各次元の状態
            //一番小さい次元を求める
            if f == min {
                min_dimension_ids = Self::search(&self, &f.0, &self.f)
            } else if x == min {
                min_dimension_ids = Self::search(&self, &x.0, &self.x)
            } else if y == min {
                min_dimension_ids = Self::search(&self, &y.0, &self.y)
            }

            for index in min_dimension_ids {
                //Indexを順番に検証していく
                match self.reverse.get_mut(&index) {
                    Some(reverse) => {
                        //FのRelationを選ぶ
                        if reverse.f {}

                        //まず状態を見極める
                        // - 無関係（どこかの次元が上位でも下位でもない）
                        // - 自身が他のIDに完全に含まれる（相手の全ての次元が上位）
                        // - 自身が他のIDを完全に含む（相手の全ての次元が下位）
                        // - 自身を一部削る必要がある（多数決で自分が下位）
                        // - 相手を一部削る必要がある（多数決で自分が上位）

                        //各次元の状態を示す
                    }
                    None => {}
                }
            }
        }
    }

    fn relation(me: BitVec, target: BitVec) -> Relation {
        if me == target {
            return Relation::Equal;
        };

        if me < target {
            return Relation::Top;
        };
    }

    fn search(&self, target: &BitVec, btree: &BTreeMap<BitVec, HashSet<Index>>) -> HashSet<Index> {
        let mut result = HashSet::new();

        // 上位IDの検索
        for f_top in target.generate_top_prefix() {
            if let Some(v) = self.f.get(&f_top) {
                result.extend(v.iter().cloned());
            }
        }

        // 下位IDの検索
        let start: BitVec = target.clone();
        let end: BitVec = target.generate_bottom_prefix_end();

        for f_bottom in self.f.range(start..end) {
            result.extend(f_bottom.1.iter().cloned());
        }

        result
    }
}
