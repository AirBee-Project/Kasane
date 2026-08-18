use super::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, serde::Serialize, Deserialize)]
pub struct TableMetadata {
    pub id: crate::models::id::TableId,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// 値インデックス（値 → 空間 ID の二次索引）を維持するか。
    ///
    /// **既定は無効。** この索引は格納 `FlexId` 1 件につき 1 キーを持つので、
    /// 書き込み 1 回のキー数――ひいては 1 トランザクションが取る悲観ロックの数――が
    /// ほぼこの索引だけで決まる（シャード本体は 1 リーフ = 最大 1024 件で 1 キー）。
    /// 使わないテーブルにまで払わせる代償としては大きすぎる。
    ///
    /// **作成後は変更できない。** 途中で有効にしても既存データ分の索引は無いため、
    /// 黙って不完全な索引ができあがる。切り替えるなら索引の作り直しが要る。
    ///
    /// 既存のメタデータにはこのフィールドが無いので、`serde(default)` で無効として読む。
    /// 索引を持たないテーブルとして扱われるだけで、シャード本体の読み書きは変わらない。
    #[serde(default)]
    pub value_index: bool,
    /// 時系列データとして扱うかどうか。
    #[serde(default = "crate::models::helpers::default_true")]
    pub has_time: bool,
}
