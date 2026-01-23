use kasane_logic::RoaringTreemap;
use redb::{TypeName, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct RedbRoaringTreemap(pub RoaringTreemap);

impl Value for RedbRoaringTreemap {
    type SelfType<'a> = RedbRoaringTreemap;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let map =
            RoaringTreemap::deserialize_from(data).expect("Failed to deserialize RoaringTreemap");
        RedbRoaringTreemap(map)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut vec = Vec::new();
        value
            .0
            .serialize_into(&mut vec)
            .expect("Failed to serialize RoaringTreemap");
        vec
    }

    fn type_name() -> TypeName {
        TypeName::new("RedbRoaringTreemap")
    }
}
