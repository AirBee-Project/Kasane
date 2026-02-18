use kasane_logic::Segment;
use redb::TableDefinition;

use std::io::Cursor;

use kasane_logic::RoaringTreemap;
use redb::{TypeName, Value as RedbValue};

use crate::Kasane;

//u64を区別するための型のエイリアス
///Fieldの識別子
pub type FiledRank = u64;

///Field内のValueに対する識別子
pub type ValueRank = u64;

///Field内のFlexIdに対する識別子
pub type FlexIdRank = u64;

pub type Value = [u8];

//このファイルではredb関連の型関連定数を定義する
impl Kasane {
    ///フィールド一覧
    pub const FILED: TableDefinition<'static, &str, FiledRank> =
        TableDefinition::new("filed_dictonary");

    ///全体の管理に必要な情報を入れておく
    ///
    /// このTableのKeyになる定数名は必ず`G_`から始まる
    pub const GLOBAL_STATE: TableDefinition<'static, &str, u64> =
        TableDefinition::new("global_state");

    //-----[GLOBAL_STATE]の特定の役割のKeyたち-----

    ///次の[FiledRank]を全体で一意に保つためのSTATE
    pub const G_NEXT_FIELD_RANK: &str = "next_field_id";

    //-----[GLOBAL_STATE]の特定の役割のKeyたち-----

    ///Fのセグメントに関する情報
    pub const F: TableDefinition<
        'static,
        (FiledRank, [u8; Segment::ARRAY_LENGTH]),
        SerializableRoaringTreemap,
    > = TableDefinition::new("f");

    ///Xのセグメントに関する情報
    pub const X: TableDefinition<
        'static,
        (FiledRank, [u8; Segment::ARRAY_LENGTH]),
        SerializableRoaringTreemap,
    > = TableDefinition::new("x");

    ///Yのセグメントに関する情報
    pub const Y: TableDefinition<
        'static,
        (FiledRank, [u8; Segment::ARRAY_LENGTH]),
        SerializableRoaringTreemap,
    > = TableDefinition::new("y");

    ///(FiledRank, FlexIdRank)>(FlexId,ValueRank)の情報
    pub const MAIN: TableDefinition<
        'static,
        (FiledRank, FlexIdRank),
        (
            (
                [u8; Segment::ARRAY_LENGTH],
                [u8; Segment::ARRAY_LENGTH],
                [u8; Segment::ARRAY_LENGTH],
            ),
            ValueRank,
        ),
    > = TableDefinition::new("main");

    ///(FiledRank, Value)>(SerializableRoaringTreemap,ValueRank)の情報
    ///
    /// SerializableRoaringTreemapはこのValueを持つFlexIdRankの集合
    pub const DICTIONARY: TableDefinition<
        'static,
        (FiledRank, &Value),
        (SerializableRoaringTreemap, ValueRank),
    > = TableDefinition::new("dictonary");

    ///(FiledRank, ValueRank)>Valueの情報
    ///
    /// 値の逆引きに使用
    pub const VALUE_REVERSE: TableDefinition<'static, (FiledRank, ValueRank), &Value> =
        TableDefinition::new("reverse");
}

#[derive(Debug, Clone, PartialEq)]
///Redbに読み書きができる[RoaringTreemap]型
pub struct SerializableRoaringTreemap(RoaringTreemap);

impl RedbValue for SerializableRoaringTreemap {
    type SelfType<'a> = SerializableRoaringTreemap;
    type AsBytes<'a> = Vec<u8>;

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Vec<u8> {
        let mut buf = Vec::new();
        value.0.serialize_into(&mut buf).unwrap();
        buf
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let treemap = RoaringTreemap::deserialize_from(Cursor::new(data)).unwrap();
        SerializableRoaringTreemap(treemap)
    }

    fn type_name() -> TypeName {
        TypeName::new("SerializableRoaringTreemap")
    }

    fn fixed_width() -> Option<usize> {
        None
    }
}
