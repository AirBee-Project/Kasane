use redb::{Key, TypeName, Value};
use std::fmt;
use uuid::Uuid;

// =========================================================================
// Newtype パターン: Uuidをラップした新しい型を定義
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UuidKey(pub Uuid);

// Uuidとの相互変換を簡単にする
impl From<Uuid> for UuidKey {
    fn from(uuid: Uuid) -> Self {
        UuidKey(uuid)
    }
}

impl From<UuidKey> for Uuid {
    fn from(key: UuidKey) -> Self {
        key.0
    }
}

impl AsRef<Uuid> for UuidKey {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

// 便利なメソッドを追加
impl UuidKey {
    pub fn new_v4() -> Self {
        UuidKey(Uuid::new_v4())
    }

    pub fn inner(&self) -> &Uuid {
        &self.0
    }

    pub fn into_inner(self) -> Uuid {
        self.0
    }
}

// Display実装で使いやすく
impl fmt::Display for UuidKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =========================================================================
// redb::Value トレイトの実装
// =========================================================================

impl Value for UuidKey {
    type SelfType<'a>
        = Self
    where
        Self: 'a;

    type AsBytes<'a>
        = [u8; 16]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(16)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self
    where
        Self: 'a,
    {
        let bytes: [u8; 16] = data.try_into().expect("UUID must be 16 bytes");
        UuidKey(Uuid::from_bytes(bytes))
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> [u8; 16]
    where
        Self: 'a,
        Self: 'b,
    {
        *value.0.as_bytes()
    }

    fn type_name() -> TypeName {
        TypeName::new("UuidKey")
    }
}

// =========================================================================
// redb::Key トレイトの実装
// KeyトレイトにはSelfTypeやAsBytesは不要（Valueトレイトから継承される）
// =========================================================================

impl Key for UuidKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}
