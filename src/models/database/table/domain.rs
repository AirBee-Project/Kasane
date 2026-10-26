use crate::models::database::table::TableDataType;
use crate::models::database::table::data_type::TableConstraints;

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub id: crate::models::id::TableId,
    pub name: String,
    pub data_type: TableDataType,
    pub max_zoom_level: u8,
    pub constraints: Option<TableConstraints>,
    pub description: Option<String>,
    /// 時間データ（時系列データ）として扱うかどうか。
    pub is_temporal: bool,
}

/// テーブルと、保持する空間ID(FlexId)の総数。テーブル情報を返す RPC はすべてこれを返す。
#[derive(Debug, Clone, PartialEq)]
pub struct TableInfo {
    pub table: Table,
    pub count: u64,
}

impl Table {
    /// 名前はメタデータに含まれない（キーの一部）ので、読み出し側で突き合わせる。
    pub fn from_meta(name: &str, meta: super::TableMetadata) -> Self {
        Self {
            id: meta.id,
            name: name.to_string(),
            data_type: meta.data_type,
            max_zoom_level: meta.max_zoom_level,
            constraints: meta.constraints,
            description: meta.description,
            is_temporal: meta.is_temporal,
        }
    }
}
