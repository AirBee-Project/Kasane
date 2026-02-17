use std::io::Cursor;

use kasane_logic::RoaringTreemap;
use redb::{TypeName, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct SerializableRoaringTreemap(RoaringTreemap);

impl Value for SerializableRoaringTreemap {
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
