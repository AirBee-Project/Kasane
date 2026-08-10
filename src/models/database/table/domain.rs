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
}

impl Table {
    /// 保存されているメタデータと、キー側にある名前から組み立てる。
    ///
    /// 名前はメタデータには含まれない（キーの一部）ので、読み出したどの経路でも
    /// この 2 つを突き合わせる必要がある。
    pub fn from_meta(name: &str, meta: super::TableMetadata) -> Self {
        Self {
            id: meta.id,
            name: name.to_string(),
            data_type: meta.data_type,
            max_zoom_level: meta.max_zoom_level,
            constraints: meta.constraints,
            description: meta.description,
        }
    }
}
