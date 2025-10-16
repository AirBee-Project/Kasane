use crate::r#type::{space_time_id::SpaceTimeId, space_time_id_set::SpaceTimeIdSet};

impl SpaceTimeIdSet {
    ///集合に対して時空間IDを追加する
    /// もともと含まれる時空間IDと範囲が重複した場合は自動的にマージする

    pub fn insert(&mut self, space_time_id: SpaceTimeId) {
        let index = (match self.index.last() {
            Some(v) => v,
            None => &0,
        } + 1)
            .to_be_bytes()
            .to_vec();

        self.f.update(
            b"",
            space_time_id.f.start,
            space_time_id.f.end,
            index.clone(),
        );

        self.x.update(
            b"",
            space_time_id.x.start,
            space_time_id.x.end,
            index.clone(),
        );

        self.y.update(
            b"",
            space_time_id.y.start,
            space_time_id.y.end,
            index.clone(),
        );

        self.t
            .update(b"", space_time_id.t.start, space_time_id.t.end, index);
    }
}
