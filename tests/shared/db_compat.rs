//! DBフォーマット互換性テストの共有シナリオ。
//!
//! `populate` が本物の書き込み経路（[`WriteRepository`]）でデータを作り、`read_actual` が
//! 読み取り経路だけを使って [`Expected`] を組み立てる。
//!
//! - フィクスチャ生成時（`tests/gen_db_compat_fixture.rs`）は `populate` → `read_actual` を
//!   直後に通し、その結果を「正解」として書き出す。
//! - 互換性テスト（`tests/db_compat_lmdb.rs` / `tests/tikv.rs`）は、過去に書き出された生バイト列を
//!   注入したうえで `read_actual` だけを呼び、記録された正解と突き合わせる。
//!
//! 名前をタグから作らず固定にしてあるのは、フィクスチャの生バイト列（データベース名や
//! テーブル名を含む）と、それを読み直す側の名前が常に一致していないといけないため。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use kasane::models::database::table::TableDataType;
use kasane::models::id::{PrincipalId, TableId};
use kasane::models::users::{DataRole, PrivilegeRule};
use kasane::repositories::{CatalogRepository, ReadRepository, Storage, WriteRepository};
use kasane_logic::{FlexId, SingleId, SpatialIdSet};

pub const DB_NAME: &str = "fmtcompat_db";
pub const INT_TABLE: &str = "int";
pub const TEXT_TABLE: &str = "text";
pub const COPY_TABLE: &str = "int_copy";
pub const USER_NAME: &str = "fmtcompat_user";

/// 分割が起きる規模（既存テストの `shard_split_preserves_all_flex_ids` と同じ桁）。
const N: u32 = 1500;
/// 併合を誘発する削除件数。
const REMOVED: u32 = 300;
/// 生き残る値の代表点。
const PROBE_HIT: u32 = 777;
/// 削除済みの代表点（`REMOVED` 未満なので必ず消えている）。
const PROBE_MISS: u32 = 100;

/// 1 テーブルぶんの書き込みを分割する単位（既存テストの `insert_flex_ids` と同じ）。
const CHUNK: usize = 500;

// --- フィクスチャの入れ物 ---

/// 1 版ぶんのフィクスチャ。過去に書かれた生バイト列と、そこから読める「正解」の組。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    pub schema_version: u32,
    pub entries: Vec<FixtureEntry>,
    pub expected: Expected,
}

/// 生のキー・バリュー 1 組。
///
/// `db` は LMDB の named database 名。TiKV はキー空間が 1 本なので常に `None`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureEntry {
    pub db: Option<String>,
    pub key: String,
    pub value: String,
}

impl FixtureEntry {
    pub fn key_bytes(&self) -> Vec<u8> {
        from_hex(&self.key)
    }
    pub fn value_bytes(&self) -> Vec<u8> {
        from_hex(&self.value)
    }
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn from_hex(s: &str) -> Vec<u8> {
    debug_assert!(s.len().is_multiple_of(2), "16進文字列の長さが奇数: {s}");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("不正な16進文字列"))
        .collect()
}

/// `tests/fixtures/db_compat/<backend>/` の絶対パス。
pub fn fixtures_root(backend: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/db_compat")
        .join(backend)
}

pub fn fixture_path(backend: &str, schema_version: u32) -> std::path::PathBuf {
    fixtures_root(backend).join(format!("v{schema_version}/fixture.json"))
}

/// `tests/fixtures/db_compat/<backend>/v*/fixture.json` を全部読む。
pub fn load_all_fixtures(backend: &str) -> Vec<Fixture> {
    let root = fixtures_root(backend);
    let Ok(read_dir) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read_dir.flatten() {
        let path = entry.path().join("fixture.json");
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let fixture: Fixture =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        out.push(fixture);
    }
    out
}

// --- シナリオ本体 ---

/// このシナリオが作る名前を、以前の実行の残骸ごと消す。
///
/// フィクスチャ生成前の掃除にも、互換性テストが読む対象を汚さないための事前確認にも使う。
pub async fn teardown<S: Storage>(db: &S) {
    let _ = db
        .write(async move |w| w.delete_user(USER_NAME).await)
        .await;
    let _ = db
        .write(async move |w| w.database_remove(DB_NAME).await)
        .await;
}

/// 本物の書き込み経路でシナリオを構築する。
///
/// カタログ・シャード分割/併合・値インデックス（固定長/可変長の両方）・テーブル複製・
/// ユーザーと ACL（データベーススコープ/テーブルスコープの両方）をひととおり触る。
pub async fn populate<S: Storage>(db: &S) {
    teardown(db).await;

    db.write(async move |w| {
        w.database_create(DB_NAME, Some("format compat fixture".into()))
            .await?;
        w.table_create(
            DB_NAME,
            INT_TABLE,
            TableDataType::Int,
            20,
            None,
            None,
            true,
            true,
        )
        .await?;
        w.table_create(
            DB_NAME,
            TEXT_TABLE,
            TableDataType::Text,
            20,
            None,
            None,
            true,
            true,
        )
        .await?;
        Ok(())
    })
    .await
    .unwrap();

    let int_id = db
        .read(async move |r| r.table_info(DB_NAME, INT_TABLE).await)
        .await
        .unwrap()
        .unwrap()
        .id;

    insert_int_data(db, int_id).await;
    insert_text_data(db, text_table_id(db).await).await;

    db.write(async move |w| w.table_copy(DB_NAME, INT_TABLE, DB_NAME, COPY_TABLE).await)
        .await
        .unwrap();

    db.write(async move |w| {
        w.create_user(
            USER_NAME,
            PrincipalId(uuid::Uuid::now_v7()),
            "fixture-hash".to_string(),
            &[
                PrivilegeRule::Database {
                    db_name: DB_NAME.to_string(),
                    role: DataRole::Read,
                },
                PrivilegeRule::Table {
                    db_name: DB_NAME.to_string(),
                    table_name: INT_TABLE.to_string(),
                    role: DataRole::Manage,
                },
            ],
        )
        .await
    })
    .await
    .unwrap();
}

async fn text_table_id<S: Storage>(db: &S) -> TableId {
    db.read(async move |r| r.table_info(DB_NAME, TEXT_TABLE).await)
        .await
        .unwrap()
        .unwrap()
        .id
}

/// `(20, 0, x, 0)` に `x` を書き込み、シャード分割を起こしたうえで一部を削除して併合も起こす。
async fn insert_int_data<S: Storage>(db: &S, table_id: TableId) {
    let xs: Vec<u32> = (0..N).collect();
    for chunk in xs.chunks(CHUNK) {
        let entries: Vec<(SpatialIdSet, Vec<u8>)> = chunk
            .iter()
            .map(|&x| {
                let mut ids = SpatialIdSet::new();
                ids.insert(SingleId::new(20, 0, x, 0).unwrap());
                (ids, (x as i64).to_be_bytes().to_vec())
            })
            .collect();
        db.write(async move |w| {
            w.table_create(
                DB_NAME,
                INT_TABLE,
                TableDataType::Int,
                20,
                None,
                None,
                true,
                true,
            )
            .await
        })
        .await
        .unwrap();
        db.write(async move |w| {
            w.data_insert_many(table_id, Some(TableDataType::Int), entries)
                .await
        })
        .await
        .unwrap();
    }

    // 先頭 REMOVED 件を消して併合を誘発する。
    let remove_xs: Vec<u32> = (0..REMOVED).collect();
    for chunk in remove_xs.chunks(CHUNK) {
        let chunk = chunk.to_vec();
        db.write(async move |w| {
            for &x in &chunk {
                let mut ids = SpatialIdSet::new();
                ids.insert(SingleId::new(20, 0, x, 0).unwrap());
                w.data_remove(table_id, Some(TableDataType::Int), ids)
                    .await?;
            }
            Ok(())
        })
        .await
        .unwrap();
    }
}

/// 可変長の値インデックス（範囲検索）を確かめるための行。既存の text_range テストと同じ配置。
async fn insert_text_data<S: Storage>(db: &S, table_id: TableId) {
    let rows: [(&str, u32); 6] = [
        ("a", 1),
        ("b", 2),
        ("bm", 3),
        ("bz", 4),
        ("bza", 5),
        ("c", 6),
    ];
    for (value, x) in rows {
        let value = value.as_bytes().to_vec();
        db.write(async move |w| {
            let mut ids = SpatialIdSet::new();
            ids.insert(SingleId::new(20, 0, x, 0).unwrap());
            w.data_insert(table_id, Some(TableDataType::Text), ids, &value)
                .await
        })
        .await
        .unwrap();
    }
}

// --- 期待結果 ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expected {
    pub int_table_count: u64,
    pub int_copy_table_count: u64,
    pub table_names: Vec<String>,
    /// 生き残っている値（`PROBE_HIT`）の座標。`(z, f, x, y)`。
    pub filter_eq_hit: Vec<(u8, i32, u32, u32)>,
    /// 削除済みの値（`PROBE_MISS`）が索引から引けないこと。
    pub filter_eq_miss_is_empty: bool,
    /// `PROBE_HIT` の位置に実際に入っている値。
    pub data_get_value: Option<Vec<u8>>,
    /// テキスト範囲検索 `"b"..="bz"` で引ける x 座標。
    pub text_range_b_bz: Vec<u32>,
    /// テキスト範囲検索 `"a"..="c"` で引ける x 座標（全件）。
    pub text_range_a_c: Vec<u32>,
    /// 描画した権限（`Debug` 表現をソート）。
    pub privileges: Vec<String>,
    pub user_exists: bool,
    pub user_global_role_is_none: bool,
}

/// 読み取り経路だけでシナリオの状態を組み立てる。
///
/// `populate` の直後にも、フィクスチャを注入した直後にも、どちらでも呼べる
/// （書き込みには一切触れない）。
pub async fn read_actual<S: Storage>(db: &S) -> Expected {
    let int_id = db
        .read(async move |r| r.table_info(DB_NAME, INT_TABLE).await)
        .await
        .unwrap()
        .unwrap()
        .id;
    let copy_id = db
        .read(async move |r| r.table_info(DB_NAME, COPY_TABLE).await)
        .await
        .unwrap()
        .unwrap()
        .id;
    let text_id = text_table_id(db).await;

    let int_table_count = db
        .read(async move |r| r.table_count(int_id).await)
        .await
        .unwrap();
    let int_copy_table_count = db
        .read(async move |r| r.table_count(copy_id).await)
        .await
        .unwrap();

    let mut table_names: Vec<String> = db
        .read(async move |r| r.table_list(DB_NAME).await)
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    table_names.sort();

    let hit_value = (PROBE_HIT as i64).to_be_bytes().to_vec();
    let hit_hits: Vec<FlexId> = {
        let v = hit_value.clone();
        db.read(async move |r| r.data_filter_eq(int_id, TableDataType::Int, &v).await)
            .await
            .unwrap()
    };
    let filter_eq_hit: Vec<(u8, i32, u32, u32)> = hit_hits
        .iter()
        .flat_map(|f| (*f).single_ids().map(|s| (s.z(), s.f(), s.x(), s.y())))
        .collect();

    let miss_value = (PROBE_MISS as i64).to_be_bytes().to_vec();
    let filter_eq_miss_is_empty = {
        let v = miss_value.clone();
        db.read(async move |r| r.data_filter_eq(int_id, TableDataType::Int, &v).await)
            .await
            .unwrap()
    }
    .is_empty();

    let data_get_value = {
        let mut ids = SpatialIdSet::new();
        ids.insert(SingleId::new(20, 0, PROBE_HIT, 0).unwrap());
        let groups = db
            .read(async move |r| r.data_get(int_id, ids.clone(), None).await)
            .await
            .unwrap();
        groups.into_iter().next().map(|(value, _)| value)
    };

    let text_range_b_bz = xs_of(db, text_id, "b", "bz").await;
    let text_range_a_c = xs_of(db, text_id, "a", "c").await;

    let mut privileges: Vec<String> = db
        .read(async move |r| {
            let record = r.require_user_record(USER_NAME).await?;
            let entries = r.acl_entries(record.id).await?;
            r.render_privileges(record.global_role, &entries).await
        })
        .await
        .unwrap()
        .into_iter()
        .map(|rule| format!("{rule:?}"))
        .collect();
    privileges.sort();

    let user_record = db
        .read(async move |r| r.user_record(USER_NAME).await)
        .await
        .unwrap();

    Expected {
        int_table_count,
        int_copy_table_count,
        table_names,
        filter_eq_hit,
        filter_eq_miss_is_empty,
        data_get_value,
        text_range_b_bz,
        text_range_a_c,
        privileges,
        user_exists: user_record.is_some(),
        user_global_role_is_none: user_record
            .map(|r| r.global_role.is_none())
            .unwrap_or(false),
    }
}

/// 範囲検索で引けた FlexId 群を x 座標の集合へ（既存の text_range テストと同じ形）。
async fn xs_of<S: Storage>(db: &S, table_id: TableId, lo: &str, hi: &str) -> Vec<u32> {
    let (lo, hi) = (lo.as_bytes().to_vec(), hi.as_bytes().to_vec());
    let hits = db
        .read(async move |r| {
            r.data_filter_range(table_id, TableDataType::Text, &lo, &hi)
                .await
        })
        .await
        .unwrap();
    let mut xs: Vec<u32> = hits
        .iter()
        .flat_map(|f| (*f).single_ids().map(|s| s.x()))
        .collect();
    xs.sort_unstable();
    xs
}

// --- LMDB: 生バイト列の出し入れ ---
//
// `AppDb` が開く named database の一覧（`src/repositories/lmdb/mod.rs` の `AppDb` と同じ順）。
// フィールドが増えたらここも合わせること。

#[cfg(feature = "backend-lmdb")]
pub const LMDB_DB_NAMES: &[&str] = &[
    "meta",
    "databases",
    "tables",
    "database_id_index",
    "table_id_index",
    "users",
    "acl",
    "acl_by_object",
    "tables_data",
    "value_index",
];

#[cfg(feature = "backend-lmdb")]
const LMDB_MAX_DBS: u32 = 24;
#[cfg(feature = "backend-lmdb")]
const LMDB_MAP_SIZE: usize = 1024 * 1024 * 1024;

/// 生バイト列だけを扱う一時的な `Env`。型付きの `AppDb` とは別の `Env` インスタンスとして開く。
///
/// 同じ名前つき DB を同一 `Env` の中で型を変えて開き直すと heed が型不一致として拒否するので、
/// 「生で読む／書く」専用の `Env` と「`AppDb` として型付きで開く」`Env` を混ぜてはいけない
/// （呼び出し側は必ず開いたあとに drop してから次を開くこと）。
#[cfg(feature = "backend-lmdb")]
fn open_raw_env(path: &std::path::Path) -> heed::Env<heed::WithoutTls> {
    unsafe {
        heed::EnvOpenOptions::new()
            .read_txn_without_tls()
            .map_size(LMDB_MAP_SIZE)
            .max_dbs(LMDB_MAX_DBS)
            .open(path)
            .unwrap_or_else(|e| panic!("failed to open LMDB env at {}: {e}", path.display()))
    }
}

/// `path` にある LMDB 環境の全 named database を生バイト列でダンプする。
///
/// 呼び出し前に、対象の `Env`（`AppDb` 経由で開いたもの）を完全に drop しておくこと。
#[cfg(feature = "backend-lmdb")]
pub fn dump_lmdb(path: &std::path::Path) -> Vec<FixtureEntry> {
    let env = open_raw_env(path);
    let rtxn = env.read_txn().unwrap();
    let mut out = Vec::new();
    for &name in LMDB_DB_NAMES {
        let db = env
            .open_database::<heed::types::Bytes, heed::types::Bytes>(&rtxn, Some(name))
            .unwrap();
        let Some(db) = db else { continue };
        for item in db.iter(&rtxn).unwrap() {
            let (key, value) = item.unwrap();
            out.push(FixtureEntry {
                db: Some(name.to_string()),
                key: to_hex(key),
                value: to_hex(value),
            });
        }
    }
    out
}

/// フィクスチャの生バイト列を、`path` にある新規の LMDB 環境へそのまま注入する。
///
/// 注入後は必ず drop してから `initialize_database` で開き直すこと（同上）。
#[cfg(feature = "backend-lmdb")]
pub fn load_lmdb(path: &std::path::Path, entries: &[FixtureEntry]) {
    let env = open_raw_env(path);
    let mut wtxn = env.write_txn().unwrap();
    for entry in entries {
        let name = entry
            .db
            .as_deref()
            .expect("LMDBのフィクスチャ行にはdatabase名が必要");
        let db = env
            .create_database::<heed::types::Bytes, heed::types::Bytes>(&mut wtxn, Some(name))
            .unwrap();
        db.put(&mut wtxn, &entry.key_bytes(), &entry.value_bytes())
            .unwrap();
    }
    wtxn.commit().unwrap();
}

// --- TiKV: 生バイト列の出し入れ ---

/// このシナリオが触れる可能性のある名前空間タグの範囲。
///
/// `src/repositories/tikv/keys.rs` の `Ns` と同じ値（`keys` モジュールは非公開なので
/// ここに複製する）。クラスタ全体をスキャンして前後の差分を取る都合上、他のテストが
/// たまたま同じタグ範囲へ同時に書いていないこと（＝生成器を専有クラスタに対して
/// 単独で走らせること）が前提。
#[cfg(feature = "backend-tikv")]
pub async fn tikv_full_scan(
    client: &tikv_client::TransactionClient,
) -> std::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    use std::ops::Bound;

    let mut txn = client.begin_optimistic().await.unwrap();
    let mut out = std::collections::BTreeMap::new();
    let mut from: Bound<tikv_client::Key> = Bound::Unbounded;
    const PAGE: u32 = 4096;
    loop {
        let range = tikv_client::BoundRange::new(from.clone(), Bound::Unbounded);
        let page: Vec<tikv_client::KvPair> = txn.scan(range, PAGE).await.unwrap().collect();
        let got = page.len();
        let Some(last) = page.last().map(|p| p.key().clone()) else {
            break;
        };
        for pair in page {
            let (key, value): (tikv_client::Key, Vec<u8>) = pair.into();
            out.insert(key.into(), value);
        }
        if got < PAGE as usize {
            break;
        }
        from = Bound::Excluded(last);
    }
    let _ = txn.rollback().await;
    out
}

/// 2 回のスキャン結果の差分（`before` に無く `after` にある、あるいは値が変わったキー）。
#[cfg(feature = "backend-tikv")]
pub fn tikv_diff(
    before: &std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    after: &std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
) -> Vec<FixtureEntry> {
    after
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, v)| FixtureEntry {
            db: None,
            key: to_hex(k),
            value: to_hex(v),
        })
        .collect()
}

/// フィクスチャの生バイト列を、そのままクラスタへ注入する。
#[cfg(feature = "backend-tikv")]
pub async fn load_tikv(client: &tikv_client::TransactionClient, entries: &[FixtureEntry]) {
    let mut txn = client.begin_optimistic().await.unwrap();
    for entry in entries {
        txn.put(entry.key_bytes(), entry.value_bytes())
            .await
            .unwrap();
    }
    txn.commit().await.unwrap();
}
