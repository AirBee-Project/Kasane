use std::fs::File;

use kasane_logic::{FlexId, FlexIdRank, Segment, SetOnMemory};
use redb::{ReadableTable, TableDefinition};

use crate::{
    Kasane,
    error::Error,
    scanner::Scanner,
    tables::{FiledRank, SerializableRoaringTreemap, ValueRank},
};

pub struct WriteTx {
    pub tx: redb::WriteTransaction,
}

impl<'txn> Scanner<'txn> for WriteTx {
    fn f(
        &'txn self,
    ) -> Result<
        redb::Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
        redb::TableError,
    > {
        self.tx.open_table(Kasane::F)
    }

    fn x(
        &'txn self,
    ) -> Result<
        redb::Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
        redb::TableError,
    > {
        self.tx.open_table(Kasane::X)
    }

    fn y(
        &'txn self,
    ) -> Result<
        redb::Table<'txn, (FiledRank, [u8; Segment::ARRAY_LENGTH]), SerializableRoaringTreemap>,
        redb::TableError,
    > {
        self.tx.open_table(Kasane::Y)
    }
}

impl WriteTx {
    ///新しいFiledを作成する
    pub fn create_field(&self, filed_name: &str) -> Result<(), Error> {
        let mut filed_dictonary = self.tx.open_table(Kasane::FILED)?;

        //既に同じ名前のFiledが存在する場合
        if filed_dictonary.get(filed_name)?.is_some() {
            return Err(Error::FiledAlreadyExists {
                filed_name: filed_name.to_string(),
            });
        }

        let next_filed_id = self.fetch_filed_id()?;

        filed_dictonary.insert(filed_name, next_filed_id)?;
        Ok(())
    }

    ///次のFiledIdを取得する
    fn fetch_filed_id(&self) -> Result<FiledRank, Error> {
        let mut global_state = self.tx.open_table(Kasane::GLOBAL_STATE)?;

        let current_id = global_state
            .get(Kasane::G_NEXT_FIELD_RANK)?
            .map(|v| v.value())
            .unwrap_or(0);

        let next = current_id.checked_add(1).ok_or(Error::FiledIdOverflow)?;

        global_state.insert(Kasane::G_NEXT_FIELD_RANK, &next)?;

        Ok(next)
    }

    ///時空間IDとValueをInsertする
    pub fn insert(&self, filed_name: &str, range: SetOnMemory, value: &[u8]) -> Result<(), Error> {
        //FiledのRankを取得
        let filed_rank = self
            .filed_rank(filed_name)?
            .ok_or_else(|| Error::FiledAlreadyExists {
                filed_name: filed_name.to_string(),
            })?;

        //既にValueが存在するかを確かめる
        let value_rank = self.find_value(filed_rank, value)?;

        //FlexIdを順番にスキャンしていく
        for flex_id in range.flex_ids() {
            for flex_id_scanner in self.flex_id_scan_plan(filed_rank, flex_id.clone())?.scan() {}
        }

        todo!()
    }

    ///Valueが既にそのFiledに存在するかを検証し、存在すればその[ValueRank]を取得する
    fn find_value(&self, filed_rank: FiledRank, value: &[u8]) -> Result<Option<ValueRank>, Error> {
        let dictionary = self.tx.open_table(Kasane::DICTIONARY)?;

        match dictionary.get((filed_rank, value))? {
            Some(v) => {
                let value_rank = v.value().1;
                return Ok(Some(value_rank));
            }
            None => {
                return Ok(None);
            }
        }
    }

    ///filed_nameから[FiledRank]を取得する
    fn filed_rank(&self, filed_name: &str) -> Result<Option<FiledRank>, Error> {
        let filed = self.tx.open_table(Kasane::FILED)?;
        match filed.get(filed_name)? {
            Some(v) => Ok(Some(v.value())),
            None => Ok(None),
        }
    }

    pub fn commit(self) -> Result<(), Error> {
        Ok(self.tx.commit()?)
    }
}
