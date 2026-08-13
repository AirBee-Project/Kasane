//! バックエンドを問わず同じ形式を使う。

use kasane_logic::FlexId;

use crate::error::AppError;

/// 1 シャードが保持できる [`FlexId`] 数の上限。超えたら分割する。
///
/// このツリーは 1 件の変更でもリーフを丸ごと書き直すので、上限がそのまま 1 回の書き込み量に
/// なる。256 ならリーフは 90KB 前後（1024 では 350KB に達した）。
///
/// バイト数ではなく件数で見るのは、隣接して同じ値を持つ領域が結合されるため。連続データは
/// いくら広くても件数が増えないが、散らばったデータは 1 件あたり 350 バイト前後になる。
pub const MAX_FLEX_ID_PER_SHARD: usize = 256;

/// 兄弟シャードの合算件数がこの値以下になったら 1 つへ統合する。
pub const MERGE_FLEX_ID_THRESHOLD: usize = MAX_FLEX_ID_PER_SHARD / 2;

const TAG_LEAF: u8 = 0;
const TAG_POINTERS: u8 = 1;

/// タグ(1) + 件数(u32 LE, 4)。件数を埋めておくとリーフを deserialize せずに合算できる。
const LEAF_HEADER_LEN: usize = 1 + 4;

pub enum ShardEntry {
    /// `SpatialIdMap` の rkyv バイト列。
    Leaf(Vec<u8>),
    Pointers(Vec<FlexId>),
}

impl ShardEntry {
    pub fn decode(bytes: &[u8]) -> Result<Self, AppError> {
        match bytes.first() {
            Some(&TAG_LEAF) => {
                if bytes.len() < LEAF_HEADER_LEN {
                    return Err(AppError::InternalError("truncated leaf entry".to_string()));
                }
                Ok(Self::Leaf(bytes[LEAF_HEADER_LEN..].to_vec()))
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
                Ok(Self::Pointers(regions))
            }
            _ => Err(AppError::InternalError("empty shard entry".to_string())),
        }
    }

    pub fn encode_leaf(flex_id_count: u32, map_bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(LEAF_HEADER_LEN + map_bytes.len());
        out.push(TAG_LEAF);
        out.extend_from_slice(&flex_id_count.to_le_bytes());
        out.extend_from_slice(map_bytes);
        out
    }

    /// ヘッダの件数をそのまま返す（deserialize しない）。ポインタノードなら `None`。
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

    pub fn encode_pointers(regions: &[FlexId]) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + regions.len() * FlexId::ENCODED_LEN);
        out.push(TAG_POINTERS);
        for region in regions {
            out.extend_from_slice(&region.encode());
        }
        out
    }

    /// ポインタノードなら子領域群、リーフなら `None`。
    ///
    /// ルーティングはタグだけ見れば足りるので、本体をコピーする [`decode`](Self::decode)
    /// を避ける。
    pub fn child_pointers(bytes: &[u8]) -> Result<Option<Vec<FlexId>>, AppError> {
        match bytes.first() {
            Some(&TAG_LEAF) => Ok(None),
            Some(&TAG_POINTERS) => match Self::decode(bytes)? {
                Self::Pointers(children) => Ok(Some(children)),
                Self::Leaf(_) => unreachable!("tag は POINTERS"),
            },
            _ => Err(AppError::InternalError("empty shard entry".to_string())),
        }
    }

    /// リーフなら中身への借用、ポインタノードなら `None`。
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
