use crate::r#type::{space_time_id::SpaceTimeId, space_time_id_set::SpaceTimeIdSet};

impl SpaceTimeIdSet {
  ///更新されているであろう逆引きSetから集合全体を復元する
    pub fn get_all(&self) -> Vec<SpaceTimeId> {
        let mut result = vec![];
        for (index, reverse) in self.reverse {
            SpaceTimeId {
                z: reverse.,
                f: todo!(),
                x: todo!(),
                y: todo!(),
                i: todo!(),
                t: todo!(),
            }
        }

        result
    }
}
