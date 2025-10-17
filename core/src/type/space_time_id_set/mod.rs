use crate::r#type::interval_set::IntervalSet;

pub struct SpaceTimeIdSet {
    xy: Vec<u8>,
    f: IntervalSet,
    t: IntervalSet,
}

impl SpaceTimeIdSet {
    pub fn new() -> Self {
        Self {
            xy: vec![],
            f: IntervalSet::new(),
            t: IntervalSet::new(),
        }
    }

    ///時空間IDを集合に入れる
    pub fn insert() {
        //入ってきたIDを複数の上位IDに置換できないかを試す

        //順番に挿入していく

        todo!()
    }
}
