use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// UUID を包むだけの識別子型。中身の扱いはどれも同じなので実装は 1 度だけ書く。
macro_rules! uuid_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn into_bytes(self) -> [u8; 16] {
                self.0.into_bytes()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl From<[u8; 16]> for $name {
            fn from(bytes: [u8; 16]) -> Self {
                Self(Uuid::from_bytes(bytes))
            }
        }
    };
}

uuid_newtype!(DatabaseId);
uuid_newtype!(TableId);
uuid_newtype!(
    /// 権限を持てる主体の識別子。
    ///
    /// 今は利用者と 1 対 1 だが、ACL の鍵はこの型で前置される。グループを足すときも
    /// 同じ識別子空間から採番すれば鍵の形式は変わらない。
    ///
    /// **利用者名ではなくこの ID で ACL を引く。** 名前は可変長なので、`alice` を前置に
    /// した範囲検索が `alicebob` の行まで拾ってしまう。16 バイト固定なら境界が構造的に
    /// 保証される。
    PrincipalId
);

impl TableId {
    /// ACL の鍵で「データベーススコープの行」を表す番兵。
    ///
    /// 実在するテーブルとぶつからないのは、採番が UUIDv7 でありバージョン・バリアント
    /// ビットが必ず立つため（全ゼロは生成されない）。全ゼロなのでデータベース内で
    /// **先頭に並ぶ**。
    const DATABASE_SCOPE: Self = Self(Uuid::nil());
}

/// 権限を付けられる対象。データベース 1 つ、またはその配下のテーブル 1 つ。
///
/// ACL の鍵も行もこの単位で並ぶ。`table_id` が `None` ならデータベース自身を指す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DataTarget {
    pub db_id: DatabaseId,
    pub table_id: Option<TableId>,
}

impl DataTarget {
    pub fn database(db_id: DatabaseId) -> Self {
        Self {
            db_id,
            table_id: None,
        }
    }

    pub fn table(db_id: DatabaseId, table_id: TableId) -> Self {
        Self {
            db_id,
            table_id: Some(table_id),
        }
    }

    /// 鍵に載せる 16 バイト。データベーススコープは
    /// [`TableId::DATABASE_SCOPE`] になる。
    pub fn slot(self) -> TableId {
        self.table_id.unwrap_or(TableId::DATABASE_SCOPE)
    }

    /// 鍵から読み戻す。[`slot`](Self::slot) の逆。
    pub fn from_slot(db_id: DatabaseId, slot: TableId) -> Self {
        Self {
            db_id,
            table_id: (slot != TableId::DATABASE_SCOPE).then_some(slot),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_round_trips_both_scopes() {
        let db = DatabaseId(Uuid::now_v7());
        let table = TableId(Uuid::now_v7());

        for target in [DataTarget::database(db), DataTarget::table(db, table)] {
            assert_eq!(DataTarget::from_slot(db, target.slot()), target);
        }
    }

    /// 番兵が実在のテーブルとぶつからないこと（UUIDv7 は全ゼロにならない）。
    #[test]
    fn the_sentinel_is_never_a_real_table() {
        for _ in 0..64 {
            assert_ne!(TableId(Uuid::now_v7()), TableId::DATABASE_SCOPE);
        }
    }
}
