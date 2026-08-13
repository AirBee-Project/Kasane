//! TiKV のキースペースはフラットな 1 本しかないので、**先頭 1 バイトの名前空間タグ**で
//! 論理テーブルを分ける。
//!
//! ```text
//!   0x00 ‖ "initialized"                 -> クラスタ初期化済みマーカー
//!   0x01 ‖ name                          -> DatabaseMetadata
//!   0x02 ‖ db_id(16) ‖ table_name        -> TableMetadata
//!   0x03 ‖ db_id(16)                     -> データベース名
//!   0x04 ‖ table_id(16)                  -> テーブル名
//!   0x05 ‖ username                      -> UserMetadata
//!   0x06 ‖ table_id(16) ‖ flex_id        -> シャードエントリ
//!   0x07 ‖ table_id(16) ‖ vkey ‖ flex_id -> 値インデックス（値なし）
//!   0x08 ‖ table_id(16) ‖ flex_id        -> シャードの保持件数（u32 LE）
//!   0x09 ‖ table_id(16)                  -> 回収待ちテーブル（値に意味なし）
//!   0x7F ‖ scope ‖ id                    -> ロック専用キー（値を書かない）
//! ```
//!
//! タグが先頭にあると、ある名前空間の全キーが連続した 1 つの範囲に収まる。キー内のバイト
//! 順序は LMDB と変わらないので、順序保存エンコーディングはそのまま通用する。

use kasane_logic::FlexId;

use crate::error::AppError;
use crate::models::id::{DatabaseId, TableId};
use crate::repositories::encoding::UUID_LEN;

/// 与えたプレフィックスで始まる全キーを覆う範囲の終端（排他）。
///
/// バイト辞書順の操作そのものは両バックエンドで同じなので、実体は共通モジュールにある。
pub use crate::repositories::encoding::prefix_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Ns {
    /// クラスタ全体に関わる印。今のところ初期化済みマーカーだけ。
    Meta = 0x00,
    Databases = 0x01,
    Tables = 0x02,
    DatabaseIdIndex = 0x03,
    TableIdIndex = 0x04,
    Users = 0x05,
    TablesData = 0x06,
    ValueIndex = 0x07,
    /// 件数だけを切り出した索引（`kv.rs` の保存の節を参照）。
    ShardCount = 0x08,
    /// 回収待ち行列（`gc.rs` を参照）。
    Garbage = 0x09,
    /// ロック取得専用。実データは書かない。
    Lock = 0x7F,
}

impl Ns {
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

/// 既定ユーザーの投入は**この印の有無**で判定する。
///
/// `root` の有無で判定すると、消した管理者が次の起動で既定パスワードのまま復活してしまう。
pub fn cluster_initialized() -> Vec<u8> {
    with_ns(Ns::Meta, b"initialized")
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
    let mut rest = Vec::with_capacity(UUID_LEN + table_name.len());
    rest.extend_from_slice(&db_id.into_bytes());
    rest.extend_from_slice(table_name.as_bytes());
    with_ns(Ns::Tables, &rest)
}

pub fn tables_of(db_id: DatabaseId) -> Vec<u8> {
    with_ns(Ns::Tables, &db_id.into_bytes())
}

pub fn table_name_from_key(key: &[u8]) -> Result<&str, AppError> {
    let head = 1 + UUID_LEN;
    if key.len() < head {
        return Err(AppError::InternalError("table key too short".to_string()));
    }
    std::str::from_utf8(&key[head..])
        .map_err(|e| AppError::InternalError(format!("table name is not valid utf-8: {e}")))
}

/// `0x04 ‖ table_id`
pub fn table_id_index(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::TableIdIndex, &table_id.into_bytes())
}

/// `0x05 ‖ username`
pub fn user(username: &str) -> Vec<u8> {
    with_ns(Ns::Users, username.as_bytes())
}

pub fn username_from_key(key: &[u8]) -> Result<&str, AppError> {
    name_after_tag(key, "username")
}

pub fn database_name_from_key(key: &[u8]) -> Result<&str, AppError> {
    name_after_tag(key, "database name")
}

/// 添字だけで剥がすと、想定外の短いキーが来たときに panic になる。
fn name_after_tag<'k>(key: &'k [u8], what: &str) -> Result<&'k str, AppError> {
    let body = key
        .split_first()
        .map(|(_tag, body)| body)
        .ok_or_else(|| AppError::InternalError(format!("{what} key is empty")))?;
    std::str::from_utf8(body)
        .map_err(|e| AppError::InternalError(format!("{what} is not valid utf-8: {e}")))
}

/// シャードと値インデックスのキーはどちらも `タグ ‖ table_id ‖ …` なので、テーブルの複製では
/// この部分だけを付け替えれば残りをそのまま流用できる。
pub fn replace_leading_id(key: &mut [u8], id: TableId) -> Result<(), AppError> {
    let slot = key.get_mut(1..1 + UUID_LEN).ok_or_else(|| {
        AppError::InternalError("key is too short to carry a table id".to_string())
    })?;
    slot.copy_from_slice(&id.into_bytes());
    Ok(())
}

/// シャード本体と件数はレイアウトが同じで、タグだけが違う。
fn region_key(ns: Ns, table_id: TableId, region: &FlexId) -> Vec<u8> {
    let mut key = Vec::with_capacity(1 + UUID_LEN + FlexId::ENCODED_LEN);
    key.push(ns as u8);
    key.extend_from_slice(&table_id.into_bytes());
    key.extend_from_slice(&region.encode());
    key
}

/// `0x06 ‖ table_id ‖ flex_id`
pub fn shard(table_id: TableId, region: &FlexId) -> Vec<u8> {
    region_key(Ns::TablesData, table_id, region)
}

/// キー → 領域の対応表を作らずに済ませるための入口（`tree/node.rs` の `load_nodes`）。
pub fn region_from_shard_key(key: &[u8]) -> Result<FlexId, AppError> {
    let expected = 1 + UUID_LEN + FlexId::ENCODED_LEN;
    let bytes: &[u8; FlexId::ENCODED_LEN] = key
        .get(1 + UUID_LEN..)
        .and_then(|tail| tail.try_into().ok())
        .ok_or_else(|| {
            AppError::InternalError(format!(
                "shard key has an unexpected length (expected {expected}, found {})",
                key.len()
            ))
        })?;
    FlexId::decode(bytes).map_err(|e| AppError::InternalError(format!("flex_id decode: {e}")))
}

pub fn shards_of(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::TablesData, &table_id.into_bytes())
}

/// `0x08 ‖ table_id ‖ flex_id`。本体（[`shard`]）と同じ並びなのでプレフィックスも対応する。
pub fn shard_count(table_id: TableId, region: &FlexId) -> Vec<u8> {
    region_key(Ns::ShardCount, table_id, region)
}

pub fn shard_counts_of(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::ShardCount, &table_id.into_bytes())
}

/// `0x07 ‖ table_id ‖ vkey ‖ flex_id`
pub fn value_index(table_id: TableId, vkey: &[u8], flex_id: &FlexId) -> Vec<u8> {
    with_ns(
        Ns::ValueIndex,
        &crate::repositories::encoding::value_index::make_key(table_id, vkey, flex_id),
    )
}

/// `0x07 ‖ table_id ‖ vkey`
pub fn value_index_prefix(table_id: TableId, vkey: &[u8]) -> Vec<u8> {
    with_ns(
        Ns::ValueIndex,
        &crate::repositories::encoding::value_index::make_prefix(table_id, vkey),
    )
}

pub fn value_index_of(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::ValueIndex, &table_id.into_bytes())
}

/// `0x09 ‖ table_id`。論理削除されたテーブルの回収待ち行列（[`gc`](super::gc)）。
pub fn garbage(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::Garbage, &table_id.into_bytes())
}

pub fn table_id_from_garbage_key(key: &[u8]) -> Result<TableId, AppError> {
    let bytes: [u8; UUID_LEN] = key
        .get(1..1 + UUID_LEN)
        .and_then(|id| id.try_into().ok())
        .ok_or_else(|| {
            AppError::InternalError("garbage key is too short to carry a table id".to_string())
        })?;
    Ok(TableId(uuid::Uuid::from_bytes(bytes)))
}

/// テーブルのデータ実体（カタログ項目は含まない）。
///
/// 回収も複製も同じ集合を使うので、名前空間が増えたときに片方だけ追随することがないよう
/// 1 箇所にまとめてある。
pub fn table_data_prefixes(table_id: TableId) -> [Vec<u8>; 3] {
    [
        shards_of(table_id),
        shard_counts_of(table_id),
        value_index_of(table_id),
    ]
}

/// ロック階層のスコープ。
///
/// ここに並ぶのは**範囲スキャンから変更を導く操作**、つまりキー単位の競合検査ではファントムを
/// 防げないものだけ（シャードの読み書きは明示ロックを取らない）。
///
/// **判別値の並び順に意味がある。** 取得は常にキーのバイト昇順なので、この大小がそのまま
/// ロック階層の取得順になる。粗い粒度から順に並べること。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LockScope {
    /// テーブル一覧は範囲スキャンで得るので、その集合を変える操作と読む操作が同じ名前の
    /// ロックを取ることでファントムを防ぐ。
    Database = 0x01,
    /// 範囲スキャンを伴わないので独立した単位でよい。
    User = 0x03,
}

/// `0x7F ‖ scope ‖ id`。`id` は対象の識別子そのもの。
///
/// 名前をハッシュへ潰さないので、別の名前が同じロックキーへ衝突することがない。このキーには
/// **値を書かない**――解放は常に rollback なので MVCC のバージョンも作られない。
pub fn lock(scope: LockScope, id: &[u8]) -> Vec<u8> {
    let mut rest = Vec::with_capacity(1 + id.len());
    rest.push(scope as u8);
    rest.extend_from_slice(id);
    with_ns(Ns::Lock, &rest)
}

/// 名前空間タグを被せるだけ。境界の求め方は
/// [`value_index::range_scan_bounds`](crate::repositories::encoding::value_index::range_scan_bounds)。
pub fn value_index_scan_bounds(
    table_id: TableId,
    lo_vkey: &[u8],
    hi_vkey: &[u8],
) -> (Vec<u8>, Option<Vec<u8>>) {
    let (start, end) =
        crate::repositories::encoding::value_index::range_scan_bounds(table_id, lo_vkey, hi_vkey);
    (
        with_ns(Ns::ValueIndex, &start),
        // 上限が無い（値側が全部 0xFF）なら、名前空間の終端で止める。
        Some(match end {
            Some(end) => with_ns(Ns::ValueIndex, &end),
            None => prefix_end(&Ns::ValueIndex.prefix()).expect("0x07 は 0xFF ではない"),
        }),
    )
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
        let keys = [
            cluster_initialized(),
            database("x"),
            table(db_id(1), "x"),
            database_id_index(db_id(1)),
            table_id_index(table_id(1)),
            user("x"),
            shards_of(table_id(1)),
            shard_counts_of(table_id(1)),
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

        // 順序保存エンコーディングが名前空間タグを付けても壊れないこと。
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
    fn short_keys_are_rejected_instead_of_panicking() {
        assert!(table_name_from_key(&[Ns::Tables as u8]).is_err());
        assert!(username_from_key(&[]).is_err());
        assert!(database_name_from_key(&[]).is_err());

        let mut too_short = vec![Ns::TablesData as u8, 0x00];
        assert!(replace_leading_id(&mut too_short, table_id(1)).is_err());
    }

    #[test]
    fn replacing_the_leading_id_keeps_the_rest_of_the_key() {
        let region = FlexId::UPPER_MAX;
        let mut key = shard(table_id(1), &region);
        replace_leading_id(&mut key, table_id(2)).unwrap();
        assert_eq!(key, shard(table_id(2), &region));
    }

    #[test]
    fn names_round_trip_through_their_keys() {
        assert_eq!(username_from_key(&user("alice")).unwrap(), "alice");
        assert_eq!(database_name_from_key(&database("mydb")).unwrap(), "mydb");
    }

    #[test]
    fn lock_scopes_are_ordered_from_coarse_to_fine() {
        // 崩れると BTreeSet 経由の取得順がロック階層と食い違う。
        assert!(LockScope::Database < LockScope::User);
    }
}
