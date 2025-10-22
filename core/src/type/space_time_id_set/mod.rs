use std::collections::{BTreeMap, HashMap, HashSet};

use crate::r#type::{
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
    f: Vec<u8>,
    x: Vec<u8>,
    y: Vec<u8>,
    t_start: u64,
    t_end: u64,
}

type Index = u64;
pub struct SpaceTimeIdSet {
    f: BTreeMap<Vec<u8>, Index>,
    x: BTreeMap<Vec<u8>, Index>,
    y: BTreeMap<Vec<u8>, Index>,
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
                        //FのPrefixを伸ばしながら検索していく
                        for f_depth in f.1..=0 {
                            let prefix = take_n_bits_min(&f.0, (f_depth + 1).into());

                            let hit = match self.f.get(&prefix) {
                                Some(index) => self.reverse.get(index).unwrap(),
                                None => {
                                    continue;
                                }
                            };

                            //hitしたものと他の次元を見比べる
                        }
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

pub fn take_n_bits_min(input: &Vec<u8>, n: usize) -> Vec<u8> {
    if n == 0 || input.is_empty() {
        return vec![];
    }

    // 必要なバイト数 = ceil(n / 8)
    let num_bytes = (n + 7) / 8;
    let mut result = vec![0u8; num_bytes];

    for i in 0..n {
        let src_byte = i / 8;
        let src_bit = 7 - (i % 8);

        let dst_byte = i / 8;
        let dst_bit = 7 - (i % 8);

        let bit = (input[src_byte] >> src_bit) & 1;
        result[dst_byte] |= bit << dst_bit;
    }

    result
}
