use crate::models::database::table::TableDataType;
use crate::models::database::table::data_type::TableConstraints;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Table {
    pub id: crate::models::id::TableId,
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// 値インデックスを維持するか（[`TableMetadata::value_index`](super::TableMetadata) を参照）。
    pub value_index: bool,
    /// 時間データ（時系列データ）として扱うかどうか。
    pub is_temporal: bool,
}

impl Table {
    /// 書き込みが値インデックスへ反映すべき型。無効なら `None`。
    ///
    /// 1 つの値にまとめてあるので、索引が無効なテーブルへ誤って型を渡す余地がない。
    pub fn value_indexing(&self) -> Option<TableDataType> {
        self.value_index.then_some(self.data_type)
    }
    /// 名前はメタデータに含まれない（キーの一部）ので、読み出し側で突き合わせる。
    pub fn from_meta(name: &str, meta: super::TableMetadata) -> Self {
        Self {
            id: meta.id,
            name: name.to_string(),
            data_type: meta.data_type,
            max_zoom_level: meta.max_zoom_level,
            constraints: meta.constraints,
            description: meta.description,
            value_index: meta.value_index,
            is_temporal: meta.is_temporal,
        }
    }
}
