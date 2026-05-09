use std::collections::HashMap;

use redb::{ReadableTable, WriteTransaction};

use crate::{
    db_init::{LAYER_ID_INDEX, LAYERS},
    error::AppError,
    models::layer::{Layer, LayerDataType, LayerMetadata},
};
use uuid::Uuid;

pub struct SpatialDbWrite {
    pub write_txn: WriteTransaction,
    ///[Layer]に関する情報をキャッシュしておくレイヤー
    ///[layer]の追加や削除があったときに編集する
    layer_caches: HashMap<String, LayerMetadata>,
}

impl SpatialDbWrite {
    pub fn new(write_txn: WriteTransaction) -> Self {
        let layer_caches: HashMap<String, LayerMetadata> = HashMap::new();
        Self {
            write_txn,
            layer_caches,
        }
    }

    /// Layerの情報を取得する
    pub fn layer_info(&self, layer_name: &str) -> Result<Option<Layer>, AppError> {
        //まずはcacheを検索する
        if let Some(meta_data) = self.layer_caches.get(layer_name) {
            return Ok(Some(Layer {
                id: meta_data.id,
                name: layer_name.to_string(),
                data_type: meta_data.data_type,
                max_zoom_level: meta_data.max_zoom_level,
            }));
        }

        //キャッシュに存在しなければデータベースを検索する
        let redb_layers = self.write_txn.open_table(LAYERS)?;
        if let Some(meta_data) = redb_layers.get(layer_name)? {
            let meta_data = meta_data.value();
            Ok(Some(Layer {
                id: meta_data.id,
                name: layer_name.to_string(),
                data_type: meta_data.data_type,
                max_zoom_level: meta_data.max_zoom_level,
            }))
        } else {
            Ok(None)
        }
    }

    /// Layerを作成する
    pub fn layer_create(
        &mut self,
        layer_name: &str,
        data_type: LayerDataType,
        max_zoom_level: u8,
    ) -> Result<Layer, AppError> {
        //同じ名前のLayerがないかを検索する
        if self.layer_info(layer_name)?.is_some() {
            return Err(AppError::LayerAlreadyExists {
                name: layer_name.to_string(),
            });
        }

        let mut redb_layer_ids = self.write_txn.open_table(LAYER_ID_INDEX)?;

        // UUIDv7の生成と衝突フォールバック
        let mut id = Uuid::now_v7();
        loop {
            if redb_layer_ids.get(id.into_bytes())?.is_none() {
                break;
            }
            id = Uuid::now_v7();
        }

        //[LayerMetadata]を作成
        let meta = LayerMetadata {
            id,
            data_type: data_type.clone(),
            max_zoom_level,
        };

        //データベースを開いて挿入
        let mut redb_layers = self.write_txn.open_table(LAYERS)?;
        redb_layers.insert(layer_name, meta)?;
        redb_layer_ids.insert(id.into_bytes(), ())?;

        //キャッシュに対して挿入
        self.layer_caches.insert(layer_name.to_string(), meta);

        //結果を返却
        Ok(Layer {
            id,
            name: layer_name.to_string(),
            data_type,
            max_zoom_level,
        })
    }

    /// Layerを削除する
    pub fn layer_remove(&mut self, layer_name: &str) -> Result<(), AppError> {
        //存在検証
        let layer_meta = if let Some(meta) = self.layer_info(layer_name)? {
            meta
        } else {
            return Err(AppError::LayerNotFound {
                name: layer_name.to_string(),
            });
        };

        //データベースを開いて削除
        let mut redb_layers = self.write_txn.open_table(LAYERS)?;
        redb_layers.remove(layer_name)?;

        //IDインデックスからも削除
        let mut redb_layer_ids = self.write_txn.open_table(LAYER_ID_INDEX)?;
        redb_layer_ids.remove(layer_meta.id.into_bytes())?;

        //キャッシュから削除
        self.layer_caches.remove(layer_name);

        return Ok(());
    }

    /// 変更の内容を永続化する
    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }

    /// 変更の内容を元に戻す
    pub fn abort(self) -> Result<(), AppError> {
        self.write_txn.abort()?;
        Ok(())
    }
}
