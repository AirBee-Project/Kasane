use std::collections::{BTreeMap, HashMap, HashSet};

use crate::r#type::{
    bit_vec::BitVec,
    interval_manager::IntervalManager,
    space_time_id::SpaceTimeId,
    space_time_id_set::{
        convert_f::{convert_bitmask_f_multiple, convert_f},
        convert_xy::{convert_bitmask_xy_multiple, convert_xy},
    },
};
pub mod convert_f;
pub mod convert_xy;

struct Reverse {
    f: BitVec,
    x: BitVec,
    y: BitVec,
    t_start: u64,
    t_end: u64,
}

type Index = u64;
pub struct SpaceTimeIdSet {
    f: BTreeMap<BitVec, HashSet<Index>>,
    x: BTreeMap<BitVec, HashSet<Index>>,
    y: BTreeMap<BitVec, HashSet<Index>>,
    t: IntervalManager,
    reverse: HashMap<Index, Reverse>,
    next_index: Index,
}

impl SpaceTimeIdSet {
    pub fn new() -> Self {
        SpaceTimeIdSet {
            f: BTreeMap::new(),
            x: BTreeMap::new(),
            y: BTreeMap::new(),
            t: IntervalManager::new(),
            reverse: HashMap::new(),
            next_index: 0,
        }
    }

    pub fn generate_index(&mut self) -> u64 {
        self.next_index = self.next_index + 1;
        self.next_index - 1
    }

    pub fn insert(&mut self, id: SpaceTimeId) {
        //まずIDを分解する
        let f_converted: Vec<(u8, i64)> = convert_f(id.z, id.f);
        let x_converted: Vec<(u8, u64)> = convert_xy(id.z, id.x);
        let y_converted: Vec<(u8, u64)> = convert_xy(id.z, id.y);

        //各次元をエンコードする
        let f_encode = convert_bitmask_f_multiple(&f_converted);
        let x_encode = convert_bitmask_xy_multiple(&x_converted);
        let y_encode = convert_bitmask_xy_multiple(&y_converted);

        //分解したIDを挿入していく
        for f in &f_encode {
            for x in &x_encode {
                for y in &y_encode {
                    // f, x, y の u8 部分を取り出す
                    let (_, f_val) = f;
                    let (_, x_val) = x;
                    let (_, y_val) = y;

                    // 最小の u8 を求める
                    let min_val = *f_val.min(x_val).min(y_val);

                    // 最小の軸を起点に探索を行う
                    if *f_val == min_val {
                        //まず上位IDの検索を行う
                        //FのPrefixを伸ばしながら検索していく
                        for f_prefix in f.0.generate_top_prefix() {
                            match self.f.get(&f_prefix) {
                                Some(v) => {
                                    comparison_relation_from_f(f.0, x.0, y.0, v, &self.reverse);
                                }
                                None => {
                                    continue;
                                }
                            }
                        }

                        //次に下位IDの検索を行う
                    } else if *x_val == min_val {
                        "x"
                    } else {
                        "y"
                    };
                }
            }
        }
    }

    //上位の次元をを探索する
    pub fn search_top_from_f(value: Vec<u8>) {}
}

///Fにおいて、上位だったIDのHashSetと逆引きのSetを用いてIDを比較する
/// - どのIDも関連がなかった
/// - あるIDに包含されていた
/// - 部分的に包含されていた⇒リターン後に自身を分割

enum Relation {
    Nothing,
    Include,
    PartiallyOverlap(HashMap<Index, Reverse>),
}

fn comparison_relation_from_f(
    f: BitVec,
    x: BitVec,
    y: BitVec,
    hit_index: &HashSet<Index>,
    reverse: &HashMap<Index, Reverse>,
) -> Relation {
    for index in hit_index {
        //まずX軸について検証を行う
        let hit_reverse = reverse.get(&index).unwrap();

        //Xの上位IDの検証
        for x_prefix in x.generate_top_prefix() {
            if hit_reverse.x == x_prefix {
                //X軸において上位のIDが存在する
            }
        }

        //Xの下位IDの検証を行う
    }

    todo!()
}
