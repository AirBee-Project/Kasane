//! `tables_data`のKey `(TableId, FlexId)`は、
//! 実データを持つ **リーフ**（[`kasane_logic::SpatialIdMap`] のバイト列）か、
//! 分割後に子シャードのキー（領域 [`FlexId`]）を **ポインタ** として並べた **中間ノード**のいずれか。
//!
//! リーフが [`MAX_FLEX_ID_PER_SHARD`] を超えると分割され、親エントリは子へのポインタノードへ置き換わる。
//! ルーティングはこのポインタを辿って対象リーフへ到達する。

use heed::types::Bytes;
use heed::{Database, RoTxn, WithoutTls};
use kasane_logic::{ArchivedSpatialIdMap, FlexId, SpatialIdMap};

use crate::db_init::TableIdAndFlexId;
use crate::error::AppError;
use crate::models::id::TableId;

/// 1つのシャードが保持できる [`FlexId`] 数の上限。これを超えたシャードは動的に分割される。
///
/// # この値の決め方
/// 書き込みは read-modify-write（リーフ全体を `SpatialIdMap::from_bytes` で
/// `Arc` 木へ復元 → 変更 → `to_bytes`）なので、**1回の書き込みコストはリーフサイズに
/// ほぼ線形**に効く。一方リーフを小さくするとリーフ数が増え、ルーティングの降下段数と
/// LMDB エントリ数が増える。
///
/// 実データ（建物ボクセル）でこの上限を掃引して決めた値。大きすぎると書き込みが重くなり、
/// 小さすぎるとリーフ数の増加が効き始める。変更する場合は書き込み・広域検索・点検索の
/// 3つを併せて計測すること（片方だけ見ると逆方向へ最適化しやすい）。
pub const MAX_FLEX_ID_PER_SHARD: usize = 512;

/// 兄弟シャードの合算件数がこの値以下になったら再びmergeして1つのシャードにする。
pub const MERGE_FLEX_ID_THRESHOLD: usize = MAX_FLEX_ID_PER_SHARD / 2;

/// [`FlexId`] の `spatial_encode` のバイト長。
const FLEX_ID_LEN: usize = 14;

const TAG_LEAF: u8 = 0;
const TAG_POINTERS: u8 = 1;

/// Leaf エントリのヘッダ長 = タグ(1) + 件数(u32 LE, 4)。
/// 件数を埋めておくことで `table_count` がリーフを deserialize せず合算できる。
const LEAF_HEADER_LEN: usize = 1 + 4;

/// `tables_data` の値の論理表現。
pub enum ShardEntry {
    /// 実データ（`SpatialIdMap` の rkyv バイト列）。
    Leaf(Vec<u8>),
    /// 子シャードの領域へのポインタたち。
    Pointers(Vec<FlexId>),
}

impl ShardEntry {
    /// 生バイト列を解釈する。
    pub fn decode(bytes: &[u8]) -> Result<Self, AppError> {
        match bytes.first() {
            Some(&TAG_LEAF) => {
                if bytes.len() < LEAF_HEADER_LEN {
                    return Err(AppError::InternalError("truncated leaf entry".to_string()));
                }
                Ok(ShardEntry::Leaf(bytes[LEAF_HEADER_LEN..].to_vec()))
            }
            Some(&TAG_POINTERS) => {
                let body = &bytes[1..];
                if !body.len().is_multiple_of(FLEX_ID_LEN) {
                    return Err(AppError::InternalError(
                        "invalid pointer node length".to_string(),
                    ));
                }
                let mut regions = Vec::with_capacity(body.len() / FLEX_ID_LEN);
                for chunk in body.chunks_exact(FLEX_ID_LEN) {
                    let mut b = [0u8; FLEX_ID_LEN];
                    b.copy_from_slice(chunk);
                    regions
                        .push(FlexId::spatial_decode(&b).map_err(|e| {
                            AppError::InternalError(format!("flex_id decode: {e}"))
                        })?);
                }
                Ok(ShardEntry::Pointers(regions))
            }
            _ => Err(AppError::InternalError("empty shard entry".to_string())),
        }
    }

    /// リーフ（`SpatialIdMap` バイト列）を、保持 [`FlexId`] 件数ヘッダ付きでエンコードする。
    pub fn encode_leaf(flex_id_count: u32, map_bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(LEAF_HEADER_LEN + map_bytes.len());
        out.push(TAG_LEAF);
        out.extend_from_slice(&flex_id_count.to_le_bytes());
        out.extend_from_slice(map_bytes);
        out
    }

    /// エントリがリーフなら、ヘッダに埋めた保持件数を deserialize せず返す。
    /// ポインタノードなら `None`。`table_count` の高速集計に使う。
    pub fn leaf_count(entry: &[u8]) -> Result<Option<u32>, AppError> {
        match entry.first() {
            Some(&TAG_LEAF) => {
                if entry.len() < LEAF_HEADER_LEN {
                    return Err(AppError::InternalError("truncated leaf entry".to_string()));
                }
                let mut b = [0u8; 4];
                b.copy_from_slice(&entry[1..LEAF_HEADER_LEN]);
                Ok(Some(u32::from_le_bytes(b)))
            }
            Some(&TAG_POINTERS) => Ok(None),
            _ => Err(AppError::InternalError("empty shard entry".to_string())),
        }
    }

    /// 子シャード領域へのポインタノードをエンコードする。
    pub fn encode_pointers(regions: &[FlexId]) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + regions.len() * FLEX_ID_LEN);
        out.push(TAG_POINTERS);
        for region in regions {
            out.extend_from_slice(&region.spatial_encode());
        }
        out
    }

    /// ポインタノードなら子領域群を、リーフなら `None` を返す**軽量版**。
    ///
    /// ルーティングはタグだけ見れば十分なので、リーフ本体（`SpatialIdMap` バイト列）を
    /// コピーする [`decode`](Self::decode) を避け、無駄なアロケーションをなくす。
    pub fn child_pointers(bytes: &[u8]) -> Result<Option<Vec<FlexId>>, AppError> {
        match bytes.first() {
            Some(&TAG_LEAF) => Ok(None),
            Some(&TAG_POINTERS) => match ShardEntry::decode(bytes)? {
                ShardEntry::Pointers(children) => Ok(Some(children)),
                ShardEntry::Leaf(_) => unreachable!("tag は POINTERS"),
            },
            _ => Err(AppError::InternalError("empty shard entry".to_string())),
        }
    }
}

// --- ルーティング・ロード（read / write 共用の自由関数） ---

/// 複数の `flex_id` を**一度の木降下**でまとめてルーティングし、
/// `担当リーフ領域 -> そこへ到達した flex_id 群` を返す。
pub fn route_leaves_batched<'a>(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    ids: impl Iterator<Item = &'a FlexId>,
) -> Result<rustc_hash::FxHashMap<FlexId, Vec<FlexId>>, AppError> {
    // f 符号で上下半球に分け、各半球ルートから 1 回ずつ降りる。
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    for f in ids {
        if f.f_index().is_negative() {
            lower.push(f);
        } else {
            upper.push(f);
        }
    }

    let mut out: rustc_hash::FxHashMap<FlexId, Vec<FlexId>> = rustc_hash::FxHashMap::default();
    descend_batched(
        tables_data,
        txn,
        table_id,
        FlexId::LOWER_MAX,
        lower,
        &mut out,
    )?;
    descend_batched(
        tables_data,
        txn,
        table_id,
        FlexId::UPPER_MAX,
        upper,
        &mut out,
    )?;
    Ok(out)
}

/// `range` と重なる**既存のリーフ領域**を、ポインタ木を1回降りて集める。
///
/// [`route_leaves_batched`] が FlexId 群を担当リーフへ振り分ける（書き込み用に未作成キーも返す）のに対し、
/// こちらは範囲クエリ用の読み取り経路であり、**データが存在するリーフだけ**を返す。
pub fn route_leaves_for_range(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    range: &kasane_logic::RangeId,
) -> Result<Vec<FlexId>, AppError> {
    let mut out = Vec::new();
    for root in [FlexId::LOWER_MAX, FlexId::UPPER_MAX] {
        if root.intersects_range(range) {
            descend_range(tables_data, txn, table_id, root, range, &mut out)?;
        }
    }
    Ok(out)
}

/// `region` を根として、`range` と交差する子だけを辿り、到達したリーフ領域を `out` に積む。
fn descend_range(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: FlexId,
    range: &kasane_logic::RangeId,
    out: &mut Vec<FlexId>,
) -> Result<(), AppError> {
    let Some(bytes) = tables_data.get(txn, &(table_id, region.clone()))? else {
        // 未作成領域＝データ無し。読み取りでは辿る必要がない。
        return Ok(());
    };
    match ShardEntry::child_pointers(bytes)? {
        // リーフに到達。
        None => out.push(region),
        Some(children) => {
            for child in children {
                if child.intersects_range(range) {
                    descend_range(tables_data, txn, table_id, child, range, out)?;
                }
            }
        }
    }
    Ok(())
}

/// `region` を根として `ids` を子へ振り分けながら降り、リーフ（または未作成キー）へ到達した
/// flex_id 群を `out` に積む。
fn descend_batched<'a>(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: FlexId,
    ids: Vec<&'a FlexId>,
    out: &mut rustc_hash::FxHashMap<FlexId, Vec<FlexId>>,
) -> Result<(), AppError> {
    if ids.is_empty() {
        return Ok(());
    }
    match tables_data.get(txn, &(table_id, region.clone()))? {
        // 未作成リーフ or 実データリーフ → ここへ到達した全 flex_id が担当。
        None => {
            out.entry(region)
                .or_default()
                .extend(ids.into_iter().cloned());
        }
        Some(bytes) => match ShardEntry::child_pointers(bytes)? {
            None => {
                out.entry(region)
                    .or_default()
                    .extend(ids.into_iter().cloned());
            }
            Some(children) => {
                for child in children {
                    let bucket: Vec<&'a FlexId> = ids
                        .iter()
                        .filter(|&&f| child.intersection(f).is_some())
                        .copied()
                        .collect();
                    descend_batched(tables_data, txn, table_id, child, bucket, out)?;
                }
            }
        },
    }
    Ok(())
}

/// `region`を直接の子に持つ**親ポインタノード**を見つけ、`(親領域, 親の全子領域)` を返す。
/// `region`が `0/0/0/0` or `0/-1/0/0`などのルートや、ポインタ配下でないなら `None`。
pub fn find_parent_pointer(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<Option<(FlexId, Vec<FlexId>)>, AppError> {
    let root = if region.f_index().is_negative() {
        FlexId::LOWER_MAX
    } else {
        FlexId::UPPER_MAX
    };
    // ルート自身に親はない。
    if region == &root {
        return Ok(None);
    }

    let mut cur = root;
    loop {
        match tables_data.get(txn, &(table_id, cur.clone()))? {
            Some(bytes) => match ShardEntry::child_pointers(bytes)? {
                Some(children) => {
                    // region が直接の子なら、cur が親。
                    if children.iter().any(|c| c == region) {
                        return Ok(Some((cur, children)));
                    }
                    // region を含む子へ降りる（region ⊆ child）。
                    match children
                        .into_iter()
                        .find(|c| c.intersection(region) == Some(region.clone()))
                    {
                        Some(child) => cur = child,
                        None => return Ok(None),
                    }
                }
                None => return Ok(None),
            },
            None => return Ok(None),
        }
    }
}

/// エントリ生バイト列がリーフなら、その中身（`SpatialIdMap` バイト列）への借用を返す。
/// ポインタノードなら `None`、不正なら `Err`。
fn leaf_payload(entry: &[u8]) -> Result<Option<&[u8]>, AppError> {
    match entry.first() {
        Some(&TAG_LEAF) => {
            if entry.len() < LEAF_HEADER_LEN {
                return Err(AppError::InternalError("truncated leaf entry".to_string()));
            }
            Ok(Some(&entry[LEAF_HEADER_LEN..]))
        }
        Some(&TAG_POINTERS) => Ok(None),
        _ => Err(AppError::InternalError("empty shard entry".to_string())),
    }
}

/// リーフ領域 `region` を **ZeroCopy archived リーダ**として開く（`Arc`を再構築しない）。
/// 未作成なら `Ok(None)`、ポインタノードに当たったら `Err`。読み取り専用。
pub fn load_leaf_archived<'txn>(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &'txn RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<Option<ArchivedSpatialIdMap<'txn>>, AppError> {
    match tables_data.get(txn, &(table_id, region.clone()))? {
        Some(entry) => match leaf_payload(entry)? {
            // 自分自身の to_bytes が書いた正当なバイト列。
            // 形式バージョンだけは検証されるので、古い形式のデータは黙って誤読されず
            // ここでエラーになる。
            Some(map_bytes) => Ok(Some(
                unsafe { ArchivedSpatialIdMap::access(map_bytes) }
                    .map_err(|e| AppError::InternalError(format!("leaf format: {e}")))?,
            )),
            None => Err(AppError::InternalError(
                "routed to a pointer node".to_string(),
            )),
        },
        None => Ok(None),
    }
}

/// リーフ領域 `region` の [`SpatialIdMap`] をロードする。未作成なら空の[`SpatialIdMap`]を作成する。
pub fn load_leaf_map(
    tables_data: &Database<TableIdAndFlexId, Bytes>,
    txn: &RoTxn<WithoutTls>,
    table_id: TableId,
    region: &FlexId,
) -> Result<SpatialIdMap<Vec<u8>>, AppError> {
    match tables_data.get(txn, &(table_id, region.clone()))? {
        Some(bytes) => match ShardEntry::decode(bytes)? {
            ShardEntry::Leaf(map_bytes) => {
                unsafe { SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes) }
                    .map_err(|e| AppError::InternalError(format!("rkyv deserialize: {e}")))
            }
            ShardEntry::Pointers(_) => Err(AppError::InternalError(
                "routed to a pointer node".to_string(),
            )),
        },
        None => Ok(SpatialIdMap::new_in_shard(region.clone())),
    }
}
