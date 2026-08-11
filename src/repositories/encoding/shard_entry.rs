//! シャードツリーのノード（リーフ／ポインタ）のバイト表現。
//!
//! 保存先がどのバックエンドでも同じ形式を使う。ここには符号化・復号だけを置き、
//! ツリーの探索や分割・統合はバックエンド側（トランザクションを持つ層）が担う。

use kasane_logic::FlexId;

use crate::error::AppError;

/// 1 つのシャードが保持できる [`FlexId`] 数の上限。これを超えたシャードは動的に分割される。
///
/// **バイト数ではなく件数で見ている点に注意。** 隣接して同じ値を持つ領域は木の中で
/// 結合されるので、連続したデータはいくら広くても件数が増えず、リーフも小さいままになる。
/// 一方**散らばったデータは結合されず、1 件あたり 350 バイト前後**になる（実測）。
///
/// この値を 1024 にしていたときのリーフは最大 350KB に達していた。このツリーは
/// **1 件の変更でもリーフを丸ごと書き直す**ので、その大きさがそのまま 1 回の書き込み量に
/// なる。KV ストアの 1 値としても、更新のたびに書き直す対象としても大きすぎる。
///
/// 256 にするとリーフは 90KB 前後に収まる。木は 2 段深くなるが、降下は 1 段 1 往復
/// （TiKV では 1〜2ms）なので、書き込み量が 1/4 になる利益のほうがはるかに大きい。
///
/// 下げると 1 リクエストが跨るリーフ数は増える。そちらは同時書き込みを 1 つの
/// トランザクションへ畳むこと（`services::database::table::data::coalesce`）で相殺する。
pub const MAX_FLEX_ID_PER_SHARD: usize = 256;

/// 兄弟シャードの合算件数がこの値以下になったら再び merge して 1 つのシャードにする。
pub const MERGE_FLEX_ID_THRESHOLD: usize = MAX_FLEX_ID_PER_SHARD / 2;

const TAG_LEAF: u8 = 0;
const TAG_POINTERS: u8 = 1;

/// Leaf エントリのヘッダ長 = タグ(1) + 件数(u32 LE, 4)。
/// 件数を埋めておくことで `table_count` がリーフを deserialize せず合算できる。
const LEAF_HEADER_LEN: usize = 1 + 4;

/// シャードツリーのノードの論理表現。
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
                if !body.len().is_multiple_of(FlexId::ENCODED_LEN) {
                    return Err(AppError::InternalError(
                        "invalid pointer node length".to_string(),
                    ));
                }
                let mut regions = Vec::with_capacity(body.len() / FlexId::ENCODED_LEN);
                for chunk in body.as_chunks::<{ FlexId::ENCODED_LEN }>().0 {
                    let mut b = [0u8; FlexId::ENCODED_LEN];
                    b.copy_from_slice(chunk);
                    regions
                        .push(FlexId::decode(&b).map_err(|e| {
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
        let mut out = Vec::with_capacity(1 + regions.len() * FlexId::ENCODED_LEN);
        out.push(TAG_POINTERS);
        for region in regions {
            out.extend_from_slice(&region.encode());
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

    /// エントリ生バイト列がリーフなら、その中身（`SpatialIdMap` バイト列）への借用を返す。
    /// ポインタノードなら `None`、不正なら `Err`。
    pub fn leaf_payload(entry: &[u8]) -> Result<Option<&[u8]>, AppError> {
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
}
