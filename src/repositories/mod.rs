use std::ops::RangeBounds;

use kasane_logic::IntoFlexIds;
use redb::{ReadTransaction, ReadableTable};

use crate::{db_init::TABLES, error::AppError, models::table::entity::TableMetadata};
pub mod read;
pub mod write;

// ///repositories層でTableの情報を取得するためのヘルパー関数
// fn helper_table_info<'a>(
//     redb_tables: &mut impl ReadableTable<&'a str, TableMetadata>,
//     name: &str,
// ) -> Result<Option<TableMetadata>, AppError> {
//     Ok(redb_tables.get(name)?.map(|v| v.value().clone()))
// }

// pub fn table_info(
//     read_txn: ReadTransaction,
//     name: &str,
// ) -> Result<Option<TableMetadata>, AppError> {
//     let mut redb_tables = read_txn.open_table(TABLES)?;
//     helper_table_info(&mut redb_tables, name)
// }

// ///新規のTableを作成する
// ///同名のTalbeが既に存在する場合はErrorを返す
// pub fn table_create(
//     write_txn: redb::WriteTransaction,
//     name: &str,
//     meta_data: TableMetadata,
// ) -> Result<(), AppError> {
//     let mut redb_table = write_txn.open_table(TABLES)?;
//     if table_info(&mut redb_table, name)?.is_some() {
//         return Err(AppError::TableAlreadyExists {
//             name: name.to_string(),
//         });
//     }
//     redb_table.insert(name, meta_data)?;
//     return Ok(());
// }

// ///新規のTableを削除する
// ///Talbeが存在しない場合はErrorを返す
// pub fn table_remove(write_txn: redb::WriteTransaction, name: &str) -> Result<(), AppError> {
//     todo!()
// }

// ///Tableの指定した時空間IDのValueを取得する
// ///Talbeが存在しない場合はErrorを返す
// pub fn get<I: IntoFlexIds>(&self, table_name: &str, ids: I) -> Result<(), AppError> {
//     todo!()
// }

// ///Tableの指定した時空間IDに値を挿入する。
// ///Talbeが存在しない場合はErrorを返す
// pub fn insert<I: IntoFlexIds>(
//     &self,
//     table_name: &str,
//     ids: I,
//     value: &[u8],
// ) -> Result<(), AppError> {
//     todo!()
// }

// ///Tableの指定した時空間IDを削除する
// ///Talbeが存在しない場合はErrorを返す
// pub fn remove<I: IntoFlexIds>(&self, table_name: &str, ids: I) -> Result<(), AppError> {
//     todo!()
// }

// ///Tableの中から指定した値を持つ時空間IDを取り出す
// ///Talbeが存在しない場合はErrorを返す
// pub fn value_get<I: IntoFlexIds>(&self, table_name: &str, value: &[u8]) -> Result<I, AppError> {
//     todo!()
// }

// ///Tableの中から指定した値の範囲を持つ時空間IDを取り出す
// ///Talbeが存在しない場合はErrorを返す
// pub fn value_range<I: IntoFlexIds, T: RangeBounds<[u8]>>(
//     &self,
//     table_name: &str,
//     value: T,
// ) -> Result<I, AppError> {
//     todo!()
// }
