//! バックエンドを問わず同じ形式を使う。

use spatial_id::{FlexId, collection::SpatialIdMap};

use crate::error::{AppError, Stored};

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

/// 1 シャードが保持できるバイト数の上限。件数が [`MAX_FLEX_ID_PER_SHARD`] 以下でも、値が
/// 大きければこちらで分割する。
///
/// 1 件あたりの値は [`crate::services::helpers::value::MAX_STORED_VALUE_BYTES`] で頭打ちにして
/// あるので、件数 1 の葉がこの上限を超えることは無い（[`shard_needs_split`] の `count > 1` は
/// その前提を保険として残しているだけ）。
pub const MAX_SHARD_BYTES: usize = 512 * 1024;

/// 葉を分割すべきか。件数・バイト数のどちらかが上限を超えたら分割する。
///
/// 件数 1 の葉は幾何分割しても縮まらないので対象から外す（分割不能な巨大値での無限再帰を防ぐ）。
pub fn shard_needs_split(entry_count: usize, encoded_len: usize) -> bool {
    entry_count > 1 && (entry_count > MAX_FLEX_ID_PER_SHARD || encoded_len > MAX_SHARD_BYTES)
}

const TAG_LEAF: u8 = 0;
const TAG_POINTERS: u8 = 1;

/// タグ(1) + 件数(u32 LE, 4)。件数を埋めておくとリーフを deserialize せずに合算できる。
const LEAF_HEADER_LEN: usize = 1 + 4;

pub enum ShardEntry {
    /// [`encode_map`] が書いたバイト列。
    Leaf(Vec<u8>),
    Pointers(Vec<FlexId>),
}

impl ShardEntry {
    pub fn decode(bytes: &[u8]) -> Result<Self, AppError> {
        match bytes.first() {
            Some(&TAG_LEAF) => {
                if bytes.len() < LEAF_HEADER_LEN {
                    return Err(AppError::corrupt(Stored::Shard, "leaf entry is truncated"));
                }
                Ok(Self::Leaf(bytes[LEAF_HEADER_LEN..].to_vec()))
            }
            Some(&TAG_POINTERS) => {
                let body = &bytes[1..];
                if !body.len().is_multiple_of(FlexId::ENCODED_LEN) {
                    return Err(AppError::corrupt(
                        Stored::Shard,
                        "pointer node length is not a multiple of a region",
                    ));
                }
                let mut regions = Vec::with_capacity(body.len() / FlexId::ENCODED_LEN);
                for chunk in body.as_chunks::<{ FlexId::ENCODED_LEN }>().0 {
                    let mut b = [0u8; FlexId::ENCODED_LEN];
                    b.copy_from_slice(chunk);
                    regions
                        .push(FlexId::decode(&b).map_err(|e| AppError::corrupt(Stored::Shard, e))?);
                }
                Ok(Self::Pointers(regions))
            }
            _ => Err(AppError::corrupt(Stored::Shard, "entry is empty")),
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
                    return Err(AppError::corrupt(Stored::Shard, "leaf entry is truncated"));
                }
                let mut b = [0u8; 4];
                b.copy_from_slice(&entry[1..LEAF_HEADER_LEN]);
                Ok(Some(u32::from_le_bytes(b)))
            }
            Some(&TAG_POINTERS) => Ok(None),
            _ => Err(AppError::corrupt(Stored::Shard, "entry is empty")),
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
            _ => Err(AppError::corrupt(Stored::Shard, "entry is empty")),
        }
    }

    /// リーフなら中身への借用、ポインタノードなら `None`。
    pub fn leaf_payload(entry: &[u8]) -> Result<Option<&[u8]>, AppError> {
        match entry.first() {
            Some(&TAG_LEAF) => {
                if entry.len() < LEAF_HEADER_LEN {
                    return Err(AppError::corrupt(Stored::Shard, "leaf entry is truncated"));
                }
                Ok(Some(&entry[LEAF_HEADER_LEN..]))
            }
            Some(&TAG_POINTERS) => Ok(None),
            _ => Err(AppError::corrupt(Stored::Shard, "entry is empty")),
        }
    }
}

/// リーフの中身を、値の辞書と `(FlexId, 辞書番号)` の列として書く。
///
/// 値を辞書へ寄せるのは、隣り合わない領域が同じ値を持つことが多く、値ごとに書くと重複するため。
///
/// 形式: `辞書の件数 u32` → `(値の長さ u32, 値)` × 辞書の件数 → `(FlexId, 辞書番号 u32)` × 残り。
pub fn encode_map(map: &SpatialIdMap<Vec<u8>>) -> Vec<u8> {
    let mut dictionary: Vec<&[u8]> = Vec::new();
    let mut index_of: rustc_hash::FxHashMap<&[u8], u32> = rustc_hash::FxHashMap::default();
    let entries: Vec<(FlexId, u32)> = map
        .iter()
        .map(|(id, value)| {
            let index = *index_of.entry(value.as_slice()).or_insert_with(|| {
                dictionary.push(value.as_slice());
                (dictionary.len() - 1) as u32
            });
            (id, index)
        })
        .collect();

    let mut out = Vec::new();
    out.extend_from_slice(&(dictionary.len() as u32).to_le_bytes());
    for value in &dictionary {
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value);
    }
    for (id, index) in entries {
        out.extend_from_slice(&id.encode());
        out.extend_from_slice(&index.to_le_bytes());
    }
    out
}

/// [`encode_map`] が書いたバイト列を、値を複製せずに読む。
pub struct LeafView<'a> {
    dictionary: Vec<&'a [u8]>,
    pub entries: Vec<(FlexId, u32)>,
}

impl<'a> LeafView<'a> {
    pub fn parse(mut bytes: &'a [u8]) -> Result<Self, AppError> {
        let truncated = || AppError::corrupt(Stored::Shard, "leaf body is truncated");
        let read_u32 = |bytes: &mut &'a [u8]| -> Result<u32, AppError> {
            let (head, rest) = bytes.split_first_chunk::<4>().ok_or_else(truncated)?;
            *bytes = rest;
            Ok(u32::from_le_bytes(*head))
        };

        let dictionary_len = read_u32(&mut bytes)? as usize;
        let mut dictionary = Vec::with_capacity(dictionary_len);
        for _ in 0..dictionary_len {
            let len = read_u32(&mut bytes)? as usize;
            let (value, rest) = bytes.split_at_checked(len).ok_or_else(truncated)?;
            dictionary.push(value);
            bytes = rest;
        }

        const ENTRY_LEN: usize = FlexId::ENCODED_LEN + 4;
        let (chunks, rest) = bytes.as_chunks::<ENTRY_LEN>();
        if !rest.is_empty() {
            return Err(truncated());
        }
        let entries = chunks
            .iter()
            .map(|chunk| {
                let (id, index) = chunk
                    .split_first_chunk::<{ FlexId::ENCODED_LEN }>()
                    .unwrap();
                let id = FlexId::decode(id).map_err(|e| AppError::corrupt(Stored::Shard, e))?;
                let index = u32::from_le_bytes(index.try_into().unwrap());
                if index as usize >= dictionary.len() {
                    return Err(AppError::corrupt(
                        Stored::Shard,
                        "value index is out of range",
                    ));
                }
                Ok((id, index))
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            dictionary,
            entries,
        })
    }

    pub fn value(&self, index: u32) -> &'a [u8] {
        self.dictionary[index as usize]
    }

    pub fn values(&self) -> &[&'a [u8]] {
        &self.dictionary
    }

    pub fn to_map(&self) -> SpatialIdMap<Vec<u8>> {
        self.entries
            .iter()
            .map(|&(id, index)| (id, self.value(index).to_vec()))
            .collect()
    }
}
