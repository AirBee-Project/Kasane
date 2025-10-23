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

        //先にエンコードする
        let f_encoded;
        let x_encoded;
        let y_encoded;

        let all_combinations: Vec<_> =
            iproduct!(&f_converted, &x_converted, &y_converted).collect();

        for (f, x, y) in all_combinations {}
    }

    fn search_f(&self) -> HashSet<Index> {
        todo!()
    }
}
