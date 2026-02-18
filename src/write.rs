use kasane_logic::SetOnMemory;
use redb::ReadableTable;

use crate::{Kasane, error::Error, tables::FiledRank};

pub struct WriteTx {
    pub tx: redb::WriteTransaction,
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
    pub fn insert(&self, range: SetOnMemory, value: &[u8]) -> Result<(), Error> {
        todo!()
    }

    pub fn commit(self) -> Result<(), Error> {
        Ok(self.tx.commit()?)
    }
}
