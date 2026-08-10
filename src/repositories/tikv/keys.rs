//! 単一キースペースを持つバックエンド（TiKV）向けのキーレイアウト。
//!
//! LMDB は名前付きの子データベースへ分けて保存できるが、TiKV のキースペースは
//! フラットな 1 本しかない。そこで**先頭 1 バイトの名前空間タグ**で論理テーブルを分ける。
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
//!   0x7F ‖ scope ‖ id                    -> ロック専用キー（値を書かない）
//! ```
//!
//! タグを先頭に置くことで、ある名前空間の全キーは連続した 1 つの範囲に収まり、
//! プレフィックススキャンがそのまま使える。またキー内のバイト順序は LMDB のときと
//! 変わらないため、[`value_index`](super::value_index) の順序保存エンコーディングは
//! そのまま通用する。

use kasane_logic::FlexId;

use crate::error::AppError;
use crate::models::id::{DatabaseId, TableId};
use crate::repositories::encoding::UUID_LEN;

/// 論理テーブルを区別する名前空間タグ。
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
    /// シャードが保持する `FlexId` 件数だけを切り出した索引（`kv.rs` の保存の節を参照）。
    ShardCount = 0x08,
    /// 論理削除されたテーブルの回収待ち行列（`gc.rs` を参照）。
    Garbage = 0x09,
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

/// `0x00 ‖ "initialized"`
///
/// このクラスタが Kasane 用に初期化済みであることを示す印。既定ユーザーの投入は
/// **この印の有無**で判定する（`root` の有無で判定すると、消した管理者が
/// 次の起動で既定パスワードのまま復活してしまう）。
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

/// あるデータベース配下の全テーブルを覆うプレフィックス。
pub fn tables_of(db_id: DatabaseId) -> Vec<u8> {
    with_ns(Ns::Tables, &db_id.into_bytes())
}

/// [`tables_of`] で引いたキーからテーブル名を取り出す。
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

/// [`Ns::Users`] のキーからユーザー名を取り出す。
pub fn username_from_key(key: &[u8]) -> Result<&str, AppError> {
    body_after_tag(key, "user").and_then(|body| {
        std::str::from_utf8(body)
            .map_err(|e| AppError::InternalError(format!("username is not valid utf-8: {e}")))
    })
}

/// [`Ns::Databases`] のキーからデータベース名を取り出す。
pub fn database_name_from_key(key: &[u8]) -> Result<&str, AppError> {
    body_after_tag(key, "database").and_then(|body| {
        std::str::from_utf8(body)
            .map_err(|e| AppError::InternalError(format!("database name is not valid utf-8: {e}")))
    })
}

/// 名前空間タグ 1 バイトを剥がす。
///
/// スキャンで引いたキーは必ずタグを持つが、その前提を暗黙にせず確認する。
/// 添字だけで剥がすと、想定外の短いキーが来たときに panic になる。
fn body_after_tag<'k>(key: &'k [u8], what: &str) -> Result<&'k [u8], AppError> {
    key.split_first()
        .map(|(_tag, body)| body)
        .ok_or_else(|| AppError::InternalError(format!("{what} key is empty")))
}

/// キー内の識別子（名前空間タグ直後の [`UUID_LEN`] バイト）を差し替える。
///
/// シャードと値インデックスのキーはどちらも `タグ ‖ table_id ‖ …` なので、
/// テーブルの複製ではこの部分だけを付け替えれば残りをそのまま流用できる。
pub fn replace_leading_id(key: &mut [u8], id: TableId) -> Result<(), AppError> {
    let end = 1 + UUID_LEN;
    if key.len() < end {
        return Err(AppError::InternalError(
            "key is too short to carry a table id".to_string(),
        ));
    }
    key[1..end].copy_from_slice(&id.into_bytes());
    Ok(())
}

/// `0x06 ‖ table_id ‖ flex_id`
pub fn shard(table_id: TableId, region: &FlexId) -> Vec<u8> {
    let mut rest = Vec::with_capacity(UUID_LEN + FlexId::ENCODED_LEN);
    rest.extend_from_slice(&table_id.into_bytes());
    rest.extend_from_slice(&region.encode());
    with_ns(Ns::TablesData, &rest)
}

/// あるテーブルの全シャードを覆うプレフィックス。
pub fn shards_of(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::TablesData, &table_id.into_bytes())
}

/// `0x08 ‖ table_id ‖ flex_id`
///
/// 本体（[`shard`]）と同じ並びにしてあるので、テーブル単位のプレフィックスも対応する。
pub fn shard_count(table_id: TableId, region: &FlexId) -> Vec<u8> {
    let mut rest = Vec::with_capacity(UUID_LEN + FlexId::ENCODED_LEN);
    rest.extend_from_slice(&table_id.into_bytes());
    rest.extend_from_slice(&region.encode());
    with_ns(Ns::ShardCount, &rest)
}

/// あるテーブルの全シャード件数を覆うプレフィックス。
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

/// `0x09 ‖ table_id`
///
/// 論理削除されたテーブルの回収待ち行列。テーブル削除はカタログ項目を消して
/// ここへ積むだけで完了し、シャード実体の削除は [`gc`](super::gc) が後から行う。
/// こうすることで、削除がテーブル全体の排他を必要とせず、1 トランザクションの
/// サイズ上限にも縛られない。
pub fn garbage(table_id: TableId) -> Vec<u8> {
    with_ns(Ns::Garbage, &table_id.into_bytes())
}

/// [`garbage`] のキーから [`TableId`] を復元する。
pub fn table_id_from_garbage_key(key: &[u8]) -> Result<TableId, AppError> {
    let end = 1 + UUID_LEN;
    if key.len() < end {
        return Err(AppError::InternalError(
            "garbage key is too short to carry a table id".to_string(),
        ));
    }
    let bytes: [u8; UUID_LEN] = key[1..end].try_into().expect("長さは確認済み");
    Ok(TableId(uuid::Uuid::from_bytes(bytes)))
}

/// あるテーブルのデータ実体を覆うプレフィックス群（カタログ項目は含まない）。
///
/// 回収も複製も「この 3 つを対象にする」で足りるよう、1 箇所にまとめてある。
pub fn table_data_prefixes(table_id: TableId) -> [Vec<u8>; 3] {
    [
        shards_of(table_id),
        shard_counts_of(table_id),
        value_index_of(table_id),
    ]
}

/// ロック階層のスコープ。粒度ごとに別のキーを取ることで、無関係な書き込み同士が
/// ロックを奪い合わないようにする。
///
/// ここに残るのは**範囲スキャンから変更を導く操作**、つまりキー単位のロックでは
/// ファントムを防げないものだけ。シャードの読み書きはキー単位で完結するので、
/// 明示ロックではなく `batch_get_for_update`（ロックと最新値の取得を同時に行う）
/// を使う（`super::mod` の「データ経路」の節）。
///
/// **判別値の並び順に意味がある。** ロックキーは `0x7F ‖ scope ‖ id` で、取得は
/// 常にキーのバイト昇順で行われるため（`super::mod` のデッドロックの節）、この
/// 判別値の大小がそのままロック階層の取得順になる。粗い粒度から順に並べること。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LockScope {
    /// データベースのテーブル集合を触る操作（テーブルの作成・削除・複製、DB 削除）。
    ///
    /// テーブル一覧は範囲スキャンで得るので、その集合を変える操作と読む操作が
    /// 同じデータベース名のロックを取ることでファントムを防ぐ。
    Database = 0x01,
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
/// バイト辞書順の操作そのものは両バックエンドで同じなので、実体は
/// [`encoding::prefix_end`](crate::repositories::encoding::prefix_end) にある。
pub fn prefix_end(prefix: &[u8]) -> Option<Vec<u8>> {
    crate::repositories::encoding::prefix_end(prefix)
}

/// 値インデックスの範囲検索で走査すべきキー範囲（名前空間タグ付き）。
///
/// 境界の求め方と、なぜ厳密にならないのかは
/// [`value_index::range_scan_bounds`](crate::repositories::encoding::value_index::range_scan_bounds)
/// を参照。ここはそこへ名前空間タグを被せるだけ。
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
        // 別々の名前空間のキーは、先頭タグだけで必ず区別できる。
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
    fn short_keys_are_rejected_instead_of_panicking() {
        // スキャン由来なら必ず十分な長さがあるが、その前提が崩れても panic させない。
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
        // この並びが崩れると、BTreeSet 経由の取得順がロック階層と食い違う。
        assert!(LockScope::Database < LockScope::User);
    }

    #[test]
    fn prefix_end_rolls_over_trailing_max_bytes() {
        assert_eq!(prefix_end(&[0x01, 0x02]), Some(vec![0x01, 0x03]));
        assert_eq!(prefix_end(&[0x01, 0xFF]), Some(vec![0x02]));
        assert_eq!(prefix_end(&[0xFF, 0xFF]), None);
    }
}
