use redb::{ReadableTable, WriteTransaction};

use crate::{
    db_init::{LAYER_IDS, LAYER_IDS_KEY, LAYERS},
    error::AppError,
    models::layer::{Layer, LayerDataType, LayerMetadata},
};

pub struct SpatialDbWrite {
    write_txn: WriteTransaction,
}

impl SpatialDbWrite {
    pub fn new(write_txn: WriteTransaction) -> Self {
        Self { write_txn }
    }

    /// Layerの情報を取得する
    pub fn layer_info(&self, name: &str) -> Result<Option<Layer>, AppError> {
        let redb_layers = self.write_txn.open_table(LAYERS)?;
        if let Some(meta_data) = redb_layers.get(name)? {
            let m = meta_data.value();
            Ok(Some(Layer {
                id: m.id,
                name: name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
            }))
        } else {
            Ok(None)
        }
    }

    /// Layerを作成する
    pub fn layer_create(
        &self,
        name: &str,
        data_type: LayerDataType,
        max_zoom_level: u8,
    ) -> Result<Layer, AppError> {
        let id = self.increment_layer_id()?;
        let meta = LayerMetadata {
            id,
            data_type: data_type.clone(),
            max_zoom_level,
        };
        let mut redb_layers = self.write_txn.open_table(LAYERS)?;
        let _ = redb_layers.insert(name, meta)?;
        Ok(Layer {
            id,
            name: name.to_string(),
            data_type,
            max_zoom_level,
        })
    }

    /// Layerを削除する
    pub fn layer_remove(&self, name: &str) -> Result<(), AppError> {
        let mut redb_layers = self.write_txn.open_table(LAYERS)?;
        let removed = redb_layers.remove(name)?;
        if removed.is_none() {
            return Err(AppError::LayerNotFound {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    /// 次のLayerに対して割り当てるIDを返す
    fn increment_layer_id(&self) -> Result<u64, AppError> {
        let mut redb_ids = self.write_txn.open_table(LAYER_IDS)?;
        let current_id = match redb_ids.get(LAYER_IDS_KEY)? {
            Some(id) => id.value(),
            None => 0,
        };
        let _ = redb_ids.insert(LAYER_IDS_KEY, current_id + 1)?;
        Ok(current_id)
    }

    /// 変更の内容を永続化する
    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}
