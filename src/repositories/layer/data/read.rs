use kasane_logic::{SingleId, SpatialIdSet};
use redb::ReadTransaction;

use crate::{db_init::LAYERS, error::AppError, models::layer::Layer};

pub struct SpatialDbRead {
    read_txn: ReadTransaction,
}

impl SpatialDbRead {
    pub fn new(read_txn: ReadTransaction) -> Self {
        Self { read_txn }
    }

    /// Layerの情報を取得する
    pub fn layer_info(&self, name: &str) -> Result<Option<Layer>, AppError> {
        let redb_layers = self.read_txn.open_table(LAYERS)?;
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

    /// TODO: 実際のDB操作を実装する
    pub fn data_get(
        &self,
        _layer_id: u64,
        _ids: SpatialIdSet,
    ) -> Result<Vec<(SingleId, &[u8])>, AppError> {
        Ok(vec![])
    }
}
