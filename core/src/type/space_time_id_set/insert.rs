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
use std::process::id;

#[derive(PartialEq)]
///自分の状況
enum Relation {
    ///自分が上位(ここでは同位も含む)である
    Top,

    ///自分が下位である
    Bottom,

    ///無関係である
    Disjoint,
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
            self.insert_encoded(Reverse {
                f: f.0.clone(),
                x: x.0.clone(),
                y: y.0.clone(),
                i: id.i,
                t: id.t,
            });
        }
    }

    fn insert_encoded(&mut self, id: Reverse) {
        let min = id.f.clone().min(id.x.clone().min(id.y.clone()));

        let mut min_dimension_ids: HashSet<u64> = HashSet::new();

        //一番小さい次元を求める
        if id.f == min {
            min_dimension_ids = Self::search(&id.f.clone(), &self.f)
        } else if id.x == min {
            min_dimension_ids = Self::search(&id.x.clone(), &self.x)
        } else if id.y == min {
            min_dimension_ids = Self::search(&id.y.clone(), &self.y)
        }

        for index in min_dimension_ids {
            //Indexを順番に検証していく
            //削除する必要がある下位IDを記録する
            let mut need_delete: Option<Index> = None;
            //処理後に追加が必要なIDを記録する
            let mut need_add: Vec<Reverse> = vec![];

            match self.reverse.get_mut(&index) {
                Some(reverse) => {
                    //どこかの次元が関係なかった時点で次の候補に行く
                    //Disjointを引きそうな順番で探していく（順番は主観的）

                    //時間のRelationを選ぶ
                    let t_relation = match Self::relation_t(id.t, reverse.t) {
                        Relation::Disjoint => {
                            continue;
                        }
                        v => v,
                    };

                    //空間のRelationを選ぶ
                    let y_relation = match Self::relation_fxy(id.y, reverse.y.clone()) {
                        Relation::Disjoint => {
                            continue;
                        }
                        v => v,
                    };
                    let x_relation = match Self::relation_fxy(id.x, reverse.x.clone()) {
                        Relation::Disjoint => {
                            continue;
                        }
                        v => v,
                    };
                    let f_relation = match Self::relation_fxy(id.f, reverse.f.clone()) {
                        Relation::Disjoint => continue,
                        v => v,
                    };

                    //空間において全てが上位か同位の場合は自身が他のIDに完全に含まれる
                    if f_relation == Relation::Top
                        && x_relation == Relation::Top
                        && y_relation == Relation::Top
                    {
                        match t_relation {
                            Relation::Bottom => {
                                //この場合は相手を2つ以下に分割する
                                need_delete = Some(index);
                                for splited_t in Self::split_t(id.t, reverse.t) {
                                    need_add.push(Reverse {
                                        f: reverse.f.clone(),
                                        x: reverse.x.clone(),
                                        y: reverse.y.clone(),
                                        i: reverse.i,
                                        t: splited_t,
                                    });
                                }
                            }
                            _ => continue,
                        }
                    }

                    //全てが下位の場合は相手を削除する必要がある
                    if (f_relation == Relation::Bottom)
                        && (x_relation == Relation::Bottom)
                        && (y_relation == Relation::Bottom)
                    {
                        match t_relation {
                            Relation::Top => {
                                //この場合は自分を2つ以下に分割する
                                for splited_t in Self::split_t(id.t, reverse.t) {
                                    //再帰的に代入して、挿入する
                                }
                            }
                            _ => need_delete = Some(index),
                        }
                    }

                    //空間において多数決で上位と下位を決める

                    //Fのみが独立のパターンを刈り取る
                    if x_relation == y_relation {
                        match f_relation {
                            Relation::Top => {
                                //自身が多数決で負けた場合
                                //つまり自身を削る
                                for splited_f in Self::split_fxy(&id.f, &reverse.f) {
                                    //再帰的に代入して、挿入する
                                }
                            }
                            Relation::Bottom => {
                                //自身が多数決で勝った場合
                                //つまり相手を削る
                                for splited_f in Self::split_fxy(&id.f, &reverse.f) {
                                    need_add.push(Reverse {
                                        f: splited_f,
                                        x: id.x,
                                        y: id.y,
                                        i: id.i,
                                        t: id.t,
                                    });
                                }
                            }
                            Relation::Disjoint => continue,
                        };
                    };

                    //まず状態を見極める
                    // - 無関係（どこかの次元が上位でも下位でもない）OK
                    // - 自身が他のIDに完全に含まれる（相手の全ての次元が上位）OK
                    // - 自身が他のIDを完全に含む（相手の全ての次元が下位）OK
                    // - 自身を一部削る必要がある（多数決で自分が下位）
                    // - 相手を一部削る必要がある（多数決で自分が上位）

                    //各次元の状態を示す
                }
                None => {}
            }

            //やることリストを消費する

            //削除すべきものを削除
            if let Some(v) = need_delete {
                self.uncheck_delete(v);
            }

            //ついかすべきものを追加
            for add in need_add {
                self.uncheck_insert(add);
            }
        }
    }

    ///チェックを行わずにIDを削除する
    /// 内部専用API
    fn uncheck_delete(&mut self, index: Index) {
        let reverse = self.reverse.remove(&index).unwrap();
        self.f.remove(&reverse.f);
        self.x.remove(&reverse.f);
        self.y.remove(&reverse.f);
        self.t.delete(&(index, reverse.i));
    }

    ///チェックを行わずにIDを挿入する
    /// 内部API専用
    fn uncheck_insert(&mut self, reverse: Reverse) {
        let index = self.generate_index();

        //Fについて挿入
        match self.f.get_mut(&reverse.f) {
            Some(v) => {
                v.insert(index);
            }
            None => {
                let mut new_set = HashSet::new();
                new_set.insert(index);
                self.f.insert(reverse.f.clone(), new_set);
            }
        };

        //Xについて挿入
        match self.x.get_mut(&reverse.x) {
            Some(v) => {
                v.insert(index);
            }
            None => {
                let mut new_set = HashSet::new();
                new_set.insert(index);
                self.x.insert(reverse.x.clone(), new_set);
            }
        };

        //Yについて挿入
        match self.y.get_mut(&reverse.y) {
            Some(v) => {
                v.insert(index);
            }
            None => {
                let mut new_set = HashSet::new();
                new_set.insert(index);
                self.y.insert(reverse.y.clone(), new_set);
            }
        };

        //Tについて挿入
        self.t.insert(reverse.t.0, reverse.t.1, (index, reverse.i));

        //逆引きに挿入
        self.reverse.insert(index, reverse);
    }

    ///
    fn split_fxy(me: &BitVec, target: &BitVec) -> Vec<BitVec> {
        //ここで除算の操作が登場する

        //右側と左側に分けて考える

        //

        todo!()
    }

    ///空間において、次元ごとの関係を判定する
    fn relation_fxy(me: BitVec, target: BitVec) -> Relation {
        if me == target {
            return Relation::Top;
        };

        if target.starts_with(&me) {
            return Relation::Top;
        }

        if me.starts_with(&target) {
            return Relation::Bottom;
        }

        return Relation::Disjoint;
    }

    fn split_t(me: (u64, u64), target: (u64, u64)) -> Vec<(u64, u64)> {
        let (me_start, me_end) = me;
        let (target_start, target_end) = target;

        // 結果を格納
        let mut result = Vec::new();

        // Equal
        if me_start == target_start && me_end == target_end {
            result.push(me);
            return result;
        }

        // me が target を含む → me を分割
        if me_start <= target_start && me_end >= target_end {
            if me_start < target_start {
                result.push((me_start, target_start));
            }
            if target_end < me_end {
                result.push((target_end, me_end));
            }
            return result;
        }

        // target が me を含む → target を分割（targetの中でmeを挟む）
        if target_start <= me_start && target_end >= me_end {
            if target_start < me_start {
                result.push((target_start, me_start));
            }
            if me_end < target_end {
                result.push((me_end, target_end));
            }
            return result;
        }

        result
    }

    ///時間において、関係を判定する
    fn relation_t(me: (u64, u64), target: (u64, u64)) -> Relation {
        let (me_start, me_end) = me;
        let (target_start, target_end) = target;

        if me_start == target_start && me_end == target_end {
            Relation::Top
        } else if me_start <= target_start && me_end >= target_end {
            Relation::Top
        } else if me_start >= target_start && me_end <= target_end {
            Relation::Bottom
        } else {
            Relation::Disjoint
        }
    }

    fn search(target: &BitVec, btree: &BTreeMap<BitVec, HashSet<Index>>) -> HashSet<Index> {
        let mut result = HashSet::new();

        // 上位IDの検索
        for f_top in target.generate_top_prefix() {
            if let Some(v) = btree.get(&f_top) {
                result.extend(v.iter().cloned());
            }
        }

        // 下位IDの検索
        let start: BitVec = target.clone();
        let end: BitVec = target.generate_bottom_prefix_end();

        for f_bottom in btree.range(start..end) {
            result.extend(f_bottom.1.iter().cloned());
        }

        result
    }
}
