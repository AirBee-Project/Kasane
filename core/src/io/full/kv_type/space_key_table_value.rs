use std::collections::HashSet;

use redb::{TypeName, Value};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::r#type::uuid::UuidKey;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceKeyTableValue(pub HashSet<UuidKey>);

impl Value for SpaceKeyTableValue {
    type SelfType<'a> = SpaceKeyTableValue;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let mut set = HashSet::new();
        for chunk in data.chunks_exact(16) {
            let uuid = Uuid::from_slice(chunk).expect("invalid uuid bytes");
            set.insert(UuidKey(uuid));
        }
        SpaceKeyTableValue(set)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut v = Vec::with_capacity(value.0.len() * 16);
        for UuidKey(uuid) in &value.0 {
            v.extend_from_slice(uuid.as_bytes());
        }
        v
    }

    fn type_name() -> TypeName {
        TypeName::new("SpaceKeyTableValue")
    }
}
