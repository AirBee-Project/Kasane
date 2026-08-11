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
}

impl Table {
    /// 書き込みが値インデックスへ反映すべき型。無効なら `None`。
    ///
    /// 書き込み経路が `data_type` を必要とするのは索引キーの順序保存エンコードのためだけ
    /// なので、「索引するか」と「どう索引するか」を 1 つの値にまとめてある。
    /// こうしておくと、索引が無効なテーブルへ誤って型を渡してしまう余地がない。
    pub fn value_indexing(&self) -> Option<TableDataType> {
        self.value_index.then_some(self.data_type)
    }
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
            value_index: meta.value_index,
        }
    }
}
