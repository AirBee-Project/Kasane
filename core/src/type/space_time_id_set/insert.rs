use std::collections::HashSet;

use itertools::iproduct;

use crate::r#type::{
    space_time_id::SpaceTimeId,
    space_time_id_set::{convert_f::convert_f, convert_xy::convert_xy, Index, SpaceTimeIdSet},
};

impl SpaceTimeIdSet {
    pub fn insert(&mut self, id: SpaceTimeId) {
        //まずIDを分解する
        let f_converted: Vec<(u8, i64)> = convert_f(id.z, id.f);
        let x_converted: Vec<(u8, u64)> = convert_xy(id.z, id.x);
        let y_converted: Vec<(u8, u64)> = convert_xy(id.z, id.y);

        let f_encoded = f_converted.iter().map(|f| f_to_bitvec(f.1, f.0));
        let x_encoded=f_converted.iter

        let all_combinations: Vec<_> =
            iproduct!(&f_converted, &x_converted, &y_converted).collect();

        for (f, x, y) in all_combinations {}
    }

    fn search_f(&self) -> HashSet<Index> {
        todo!()
    }
}

pub fn f_to_bitvec(f: i64, z: u8) -> Vec<u8> {
    let mut bits = Vec::with_capacity(z as usize + 1);
    bits.push(z);
    let mut value = f;
    for _ in 0..z {
        bits.push((value & 1) as u8);
        value >>= 1;
    }
    bits
}

pub fn xy_to_bitvec(xy: u64, z: u8) -> Vec<u8> {
    let mut bits = Vec::with_capacity(z as usize + 1);
    bits.push(z);
    let mut value = xy;
    for _ in 0..z {
        bits.push((value & 1) as u8);
        value >>= 1;
    }
    bits
}
