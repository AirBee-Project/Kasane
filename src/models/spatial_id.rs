use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// 通常の時空間IDを表す型。
///
/// 時間を指定する場合は `i` と `t` を必ず両方指定すること。
/// 両方省略した場合は「全時間」を表す。
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Hash, Eq)]
pub struct RawSingleId {
    /// ズームレベル。
    pub z: u8,
    /// 高さ方向のインデックス。
    pub f: i32,
    /// 経度方向のインデックス。
    pub x: u32,
    /// 緯度方向のインデックス。
    pub y: u32,
    /// 時間間隔（単位: 秒）。1970-01-01T00:00:00Z を起点とする。任意の値は指定できず、 次の暦の単位のいずれかのみ指定できる: `1`、`60`、`3600`、`86400`、`34359738368`（システムの全時間）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i: Option<u64>,
    /// 時間インデックス。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<u64>,
}

/// 区間表現の時空間IDを表す型。省略された次元はその次元全体を表す。
/// 時間を指定する場合は `i` と `t` を必ず両方指定すること。
/// 両方省略した場合は「全時間」を表す。
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct RawRangeId {
    /// ズームレベル。
    pub z: u8,
    /// 高さ方向の範囲 `[min, max]`（両端を含む）。
    pub f: Option<[i32; 2]>,
    /// 経度方向の範囲 `[min, max]`（両端を含む）。
    pub x: Option<[u32; 2]>,
    /// 緯度方向の範囲 `[min, max]`（両端を含む）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<[u32; 2]>,

    /// 時間間隔（単位: 秒）。1970-01-01T00:00:00Z を起点とする。任意の値は指定できず、 次の暦の単位のいずれかのみ指定できる: `1`、`60`、`3600`、`86400`、`34359738368`（システムの全時間）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i: Option<u64>,
    /// 時間インデックスの区間表現。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<[u64; 2]>,
}

/// 高さ・経度・緯度・時間の4軸すべてを「ズームレベル + インデックス」で表す拡張時空間IDを表す型。
/// 時間を指定する場合は `t_zoomlevel` と `t_index` を必ず両方指定すること。
/// 両方省略した場合は「全時間」を表す。
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawFlexId {
    /// 高さ方向のズームレベル。
    pub f_zoomlevel: u8,
    /// 高さ方向のインデックス。
    pub f_index: i32,
    /// 経度方向のズームレベル。
    pub x_zoomlevel: u8,
    /// 経度方向のインデックス。
    pub x_index: u32,
    /// 緯度方向のズームレベル。
    pub y_zoomlevel: u8,
    /// 緯度方向のインデックス。
    pub y_index: u32,
    /// 時間軸のズームレベル
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_zoomlevel: Option<u8>,
    /// 時間軸のインデックス。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t_index: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SpatialId {
    #[serde(rename = "singleId")]
    SingleId(RawSingleId),
    #[serde(rename = "rangeId")]
    RangeId(RawRangeId),
    #[serde(rename = "flexId")]
    FlexId(RawFlexId),
}
