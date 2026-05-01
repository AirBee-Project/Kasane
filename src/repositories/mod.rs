use std::{ops::RangeBounds, sync::Arc};

use kasane_logic::IntoFlexIds;
use redb::WriteTransaction;

use crate::{
    error::AppError,
    models::table::{TableDataType, TableInfo},
};

///onDiskな時空間IDデータベースを抽象化
pub struct SpatialIdDB {}

impl SpatialIdDB {
    ///Tableが存在している場合はTableの情報を返す
    pub fn table_exist(&self, table_name: &str) -> Result<Option<TableInfo>, AppError> {
        todo!()
    }

    ///新規のTableを作成する
    ///同名のTalbeが既に存在する場合はErrorを返す
    pub fn table_create(&self, table_name: &str, r#type: TableDataType) -> Result<(), AppError> {
        //当該のTableの情報を取得する
        let table = match self.table_exist(table_name)? {
            Some(v) => v,
            None => {
                return Err(AppError::TableNotFound {
                    name: table_name.to_string(),
                });
            }
        };

        todo!()
    }

    ///新規のTableを削除する
    ///Talbeが存在しない場合はErrorを返す
    pub fn table_remove(&self, table_name: &str) -> Result<(), AppError> {
        todo!()
    }

    ///Tableの指定した時空間IDのValueを取得する
    ///Talbeが存在しない場合はErrorを返す
    pub fn get<I: IntoFlexIds>(&self, table_name: &str, ids: I) -> Result<(), AppError> {
        todo!()
    }

    ///Tableの指定した時空間IDに値を挿入する。
    ///Talbeが存在しない場合はErrorを返す
    pub fn insert<I: IntoFlexIds>(
        &self,
        table_name: &str,
        ids: I,
        value: &[u8],
    ) -> Result<(), AppError> {
        todo!()
    }

    ///Tableの指定した時空間IDを削除する
    ///Talbeが存在しない場合はErrorを返す
    pub fn remove<I: IntoFlexIds>(&self, table_name: &str, ids: I) -> Result<(), AppError> {
        todo!()
    }

    ///Tableの中から指定した値を持つ時空間IDを取り出す
    ///Talbeが存在しない場合はErrorを返す
    pub fn value_get<I: IntoFlexIds>(&self, table_name: &str, value: &[u8]) -> Result<I, AppError> {
        todo!()
    }

    ///Tableの中から指定した値の範囲を持つ時空間IDを取り出す
    ///Talbeが存在しない場合はErrorを返す
    pub fn value_range<I: IntoFlexIds, T: RangeBounds<[u8]>>(
        &self,
        table_name: &str,
        value: T,
    ) -> Result<I, AppError> {
        todo!()
    }
}
