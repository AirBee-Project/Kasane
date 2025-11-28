use bincode::{Decode, Encode};
use redb::{Key, TypeName, Value};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, convert::TryInto};
use uuid::Uuid; // for try_into()

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UuidKey(pub [u8; 16]);

impl Value for UuidKey {
    type SelfType<'a> = UuidKey;
    type AsBytes<'a> = &'a [u8];

    fn fixed_width() -> Option<usize> {
        Some(16) // UUIDは常に16バイト
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        &value.0
    }

    fn type_name() -> redb::TypeName {
        TypeName::new("UuidKey")
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        UuidKey(data.try_into().expect("Input data must be 16 bytes"))
    }
}

impl Key for UuidKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        data1.cmp(data2)
    }
}

impl UuidKey {
    pub fn new() -> Self {
        UuidKey(*Uuid::new_v4().as_bytes())
    }

    // UuidKey から Uuid を復元するメソッド
    pub fn as_uuid(&self) -> Uuid {
        Uuid::from_bytes(self.0)
    }
}

// Uuid から UuidKey への変換
impl From<Uuid> for UuidKey {
    fn from(uuid: Uuid) -> Self {
        UuidKey(*uuid.as_bytes())
    }
}

// UuidKey から Uuid への変換
impl From<UuidKey> for Uuid {
    fn from(key: UuidKey) -> Self {
        Uuid::from_bytes(key.0)
    }
}
