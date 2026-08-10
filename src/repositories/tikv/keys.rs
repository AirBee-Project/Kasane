//! 単一キースペースを持つバックエンド（TiKV）向けのキーレイアウト。
//!
//! LMDB は名前付きの子データベースへ分けて保存できるが、TiKV のキースペースは
//! フラットな 1 本しかない。そこで**先頭 1 バイトの名前空間タグ**で論理テーブルを分ける。
//!
//! ```text
//!   0x01 ‖ name                          -> DatabaseMetadata
//!   0x02 ‖ db_id(16) ‖ table_name        -> TableMetadata
//!   0x03 ‖ db_id(16)                     -> データベース名
//!   0x04 ‖ table_id(16)                  -> テーブル名
//!   0x05 ‖ username                      -> UserMetadata
//!   0x06 ‖ table_id(16) ‖ flex_id        -> シャードエントリ
//!   0x07 ‖ table_id(16) ‖ vkey ‖ flex_id -> 値インデックス（値なし）
//!   0x7F ‖ scope ‖ id                    -> ロック専用キー（値を書かない）
//! ```
//!
//! タグを先頭に置くことで、ある名前空間の全キーは連続した 1 つの範囲に収まり、
//! プレフィックススキャンがそのまま使える。またキー内のバイト順序は LMDB のときと
//! 変わらないため、[`value_index`](super::value_index) の順序保存エンコーディングは
//! そのまま通用する。

use kasane_logic::FlexId;

use crate::models::id::{DatabaseId, TableId};

use crate::repositories::encoding::TABLE_ID_LEN;

/// 論理テーブルを区別する名前空間タグ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Ns {
    Databases = 0x01,
    Tables = 0x02,
    DatabaseIdIndex = 0x03,
    TableIdIndex = 0x04,
    Users = 0x05,
    TablesData = 0x06,
    ValueIndex = 0x07,
    /// ロック取得専用。実データは書かない（[`super::super`] のロック階層を参照）。
    Lock = 0x7F,
}

impl Ns {
    /// この名前空間だけを覆うプレフィックス。
    pub fn prefix(self) -> Vec<u8> {
        vec![self as u8]
    }
}

fn with_ns(ns: Ns, rest: &[u8]) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + rest.len());
    key.push(ns as u8);
    key.extend_from_slice(rest);
    key
}

/// `0x01 ‖ name`
pub fn database(name: &str) -> Vec<u8> {
    with_ns(Ns::Databases, name.as_bytes())
}

/// `0x03 ‖ db_id`
pub fn database_id_index(db_id: DatabaseId) -> Vec<u8> {
    with_ns(Ns::DatabaseIdIndex, &db_id.into_bytes())
}

/// `0x02 ‖ db_id ‖ table_name`
pub fn table(db_id: DatabaseId, table_name: &str) -> Vec<u8> {
    let mut rest = Vec::with_capacity(TABLE_ID_LEN + table_name.len());
    rest.extend_from_slice(&db_id.into_bytes());
    rest.extend_from_slice(table_name.as_bytes());
    with_ns(Ns::Tables, &rest)
}

/// あるデータベース配下の全テーブルを覆うプレフィックス。
pub fn tables_of(db_id: DatabaseId) -> Vec<u8> {
    with_ns(Ns::Tables, &db_id.into_bytes())
}

/// [`tables_of`] で引いたキーからテーブル名を取り出す。
pub fn table_name_from_key(key: &[u8]) -> Result<&str, crate::error::AppError> {
    let head = 1 + TABLE_ID_LEN;
    if key.len() < head {
        return Err(crate::error::AppError::InternalError(
            "table key too short".to_string(),
        ));
    }
    std::str::from_utf8(&key[head..]).map_err(|e| {
        crate::error::AppError::InternalError(format!("table name is not valid utf-8: {e}"))
    })
}

/// `0x04 ‖ table_id`
pub fn table_id_index(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::TableIdIndex, &table_id.into_bytes())
}

/// `0x05 ‖ username`
pub fn user(username: &str) -> Vec<u8> {
    with_ns(Ns::Users, username.as_bytes())
}

/// [`Ns::Users`] のキーからユーザー名を取り出す。
pub fn username_from_key(key: &[u8]) -> Result<&str, crate::error::AppError> {
    std::str::from_utf8(&key[1..]).map_err(|e| {
        crate::error::AppError::InternalError(format!("username is not valid utf-8: {e}"))
    })
}

/// `0x06 ‖ table_id ‖ flex_id`
pub fn shard(table_id: TableId, region: &FlexId) -> Vec<u8> {
    let mut rest = Vec::with_capacity(TABLE_ID_LEN + FlexId::ENCODED_LEN);
    rest.extend_from_slice(&table_id.into_bytes());
    rest.extend_from_slice(&region.encode());
    with_ns(Ns::TablesData, &rest)
}

/// あるテーブルの全シャードを覆うプレフィックス。
pub fn shards_of(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::TablesData, &table_id.into_bytes())
}

/// `0x07 ‖ table_id ‖ vkey ‖ flex_id`
pub fn value_index(table_id: TableId, vkey: &[u8], flex_id: &FlexId) -> Vec<u8> {
    with_ns(
        Ns::ValueIndex,
        &crate::repositories::encoding::value_index::make_key(table_id, vkey, flex_id),
    )
}

/// 値インデックスの等価スキャン用プレフィックス（`0x07 ‖ table_id ‖ vkey`）。
pub fn value_index_prefix(table_id: TableId, vkey: &[u8]) -> Vec<u8> {
    with_ns(
        Ns::ValueIndex,
        &crate::repositories::encoding::value_index::make_prefix(table_id, vkey),
    )
}

/// あるテーブルの値インデックス全体を覆うプレフィックス。
pub fn value_index_of(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::ValueIndex, &table_id.into_bytes())
}

/// ロック階層のスコープ。粒度ごとに別のキーを取ることで、無関係な書き込み同士が
/// ロックを奪い合わないようにする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LockScope {
    /// データベースのテーブル集合を触る操作（テーブルの作成・削除・複製、DB 削除）。
    Database = 0x01,
    /// テーブルのシャード集合を触る操作（データ書き込み、テーブル削除・複製）。
    Table = 0x02,
    /// 1 ユーザーの権限・資格情報。範囲スキャンを伴わないので独立した単位でよい。
    User = 0x03,
}

/// `0x7F ‖ scope ‖ id`
///
/// `id` は排他したい対象の識別子そのもの（テーブルなら `TableId` のバイト列、
/// データベースやユーザーなら名前のバイト列）。名前をハッシュへ潰さないので、
/// 別の名前が同じロックキーへ衝突することがない。
///
/// このキーには**値を書かない**。悲観ロックの取得対象にするだけで、
/// 解放は常に rollback で行うため MVCC のバージョンは作られない。
pub fn lock(scope: LockScope, id: &[u8]) -> Vec<u8> {
    let mut rest = Vec::with_capacity(1 + id.len());
    rest.push(scope as u8);
    rest.extend_from_slice(id);
    with_ns(Ns::Lock, &rest)
}

/// 与えたプレフィックスで始まる全キーを覆う範囲の終端（排他）。
///
/// 末尾のバイトを繰り上げて「次のプレフィックス」を作る。全バイトが 0xFF の場合は
/// 上限が存在しないので `None`（そのときは名前空間の終端まで読めばよい）。
pub fn prefix_end(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end = prefix.to_vec();
    while let Some(last) = end.last_mut() {
        if *last < 0xFF {
            *last += 1;
            return Some(end);
        }
        end.pop();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_id(n: u8) -> DatabaseId {
        DatabaseId(uuid::Uuid::from_bytes([n; 16]))
    }

    fn table_id(n: u8) -> TableId {
        TableId(uuid::Uuid::from_bytes([n; 16]))
    }

    #[test]
    fn namespaces_do_not_overlap() {
        // 別々の名前空間のキーは、先頭タグだけで必ず区別できる。
        let keys = [
            database("x"),
            table(db_id(1), "x"),
            database_id_index(db_id(1)),
            table_id_index(table_id(1)),
            user("x"),
            shards_of(table_id(1)),
            value_index_of(table_id(1)),
            lock(LockScope::Database, b"some-db"),
        ];
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                assert_ne!(a[0], b[0], "名前空間タグが重複している");
            }
        }
    }

    #[test]
    fn table_prefix_covers_only_its_database() {
        let mine = table(db_id(1), "t");
        let other = table(db_id(2), "t");
        let prefix = tables_of(db_id(1));

        assert!(mine.starts_with(&prefix));
        assert!(!other.starts_with(&prefix));
    }

    #[test]
    fn table_name_roundtrips_through_the_key() {
        let key = table(db_id(7), "my_table");
        assert_eq!(table_name_from_key(&key).unwrap(), "my_table");
    }

    #[test]
    fn shard_keys_sort_within_their_table() {
        // 同一テーブルのシャードは連続し、別テーブルのものは範囲外に出る。
        let prefix = shards_of(table_id(3));
        let end = prefix_end(&prefix).unwrap();

        let region = FlexId::UPPER_MAX;
        let mine = shard(table_id(3), &region);
        let other = shard(table_id(4), &region);

        assert!(mine.as_slice() >= prefix.as_slice() && mine.as_slice() < end.as_slice());
        assert!(!(other.as_slice() >= prefix.as_slice() && other.as_slice() < end.as_slice()));
    }

    #[test]
    fn value_index_keeps_value_ordering() {
        use crate::models::database::table::TableDataType;

        // 順序保存エンコーディングが名前空間タグを付けても壊れないこと（負→正の順）。
        let t = table_id(1);
        let region = FlexId::UPPER_MAX;
        let enc = |v: i64| {
            crate::repositories::encoding::value_index::order_preserving(
                TableDataType::Int,
                &v.to_be_bytes(),
            )
        };

        let neg = value_index(t, &enc(-5), &region);
        let zero = value_index(t, &enc(0), &region);
        let pos = value_index(t, &enc(5), &region);

        assert!(neg < zero, "負の値が 0 より後ろに並んでいる");
        assert!(zero < pos, "0 が正の値より後ろに並んでいる");
    }

    #[test]
    fn prefix_end_rolls_over_trailing_max_bytes() {
        assert_eq!(prefix_end(&[0x01, 0x02]), Some(vec![0x01, 0x03]));
        assert_eq!(prefix_end(&[0x01, 0xFF]), Some(vec![0x02]));
        assert_eq!(prefix_end(&[0xFF, 0xFF]), None);
    }
}
