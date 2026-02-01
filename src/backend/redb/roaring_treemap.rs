use std::io::Cursor;

use kasane_logic::RoaringTreemap;
use redb::Value;

#[derive(Debug)]
///RedbのValueとしてRoaringTreemapを管理するためのラッパー
pub struct RedbRoaringTreemap(pub RoaringTreemap);

impl Value for RedbRoaringTreemap {
    type SelfType<'a>
        = RedbRoaringTreemap
    where
        Self: 'a;

    type AsBytes<'a>
        = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut cursor = Cursor::new(data);
        let inner = RoaringTreemap::deserialize_from(&mut cursor)
            .expect("failed to deserialize RoaringTreemap");
        RedbRoaringTreemap(inner)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut buf = Vec::new();
        value
            .0
            .serialize_into(&mut buf)
            .expect("failed to serialize RoaringTreemap");
        buf
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("RoaringTreemap")
    }
}
