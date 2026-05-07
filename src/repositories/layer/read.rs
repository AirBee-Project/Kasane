use redb::{ReadTransaction, ReadableTable};

use crate::{db_init::LAYERS, error::AppError, models::layer::Layer};

pub struct SpatialDbRead {
    pub read_txn: ReadTransaction,
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

    /// Layerの一覧を取得する
    pub fn layer_list(&self) -> Result<Vec<Layer>, AppError> {
        self.read_txn
            .open_table(LAYERS)?
            .iter()?
            .map(|res| {
                let (k, v) = res.map_err(AppError::from)?;
                let m = v.value();
                Ok(Layer {
                    id: m.id,
                    name: k.value().to_owned(),
                    data_type: m.data_type,
                    max_zoom_level: m.max_zoom_level,
                })
            })
            .collect()
    }
}
