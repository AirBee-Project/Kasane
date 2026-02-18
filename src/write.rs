use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    mem,
};

use kasane_logic::{FlexId, FlexIdRank, RoaringTreemap, Segment, SetOnMemory};
use redb::{ReadableTable, TableDefinition};

use crate::{
    Kasane,
    error::Error,
    scanner::Scanner,
    tables::{FiledRank, SerializableRoaringTreemap, Value, ValueRank},
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
        let exist_value_rank = self.find_value(filed_rank, value)?;

        //必要なTableを開いておく
        let main = self.tx.open_table(Kasane::MAIN)?;
        let value_reverse = self.tx.open_table(Kasane::VALUE_REVERSE)?;

        //FlexIdを順番にスキャンしていく
        for flex_id in range.flex_ids() {
            //後でまとめて処理する
            let mut need_insert: HashMap<Vec<u8>, Vec<FlexId>> = HashMap::new();
            let mut need_delete_ranks = RoaringTreemap::new();

            for flex_id_scanner in self.flex_id_scan_plan(filed_rank, flex_id.clone())?.scan() {
                if let Some(parent_rank) = flex_id_scanner.parent()? {
                    let (parent_flex_id, parent_value_rank) =
                        main.get((filed_rank, parent_rank))?.unwrap().value();

                    if let Some(value_rank) = exist_value_rank {
                        if value_rank == parent_value_rank {
                            continue;
                        }
                    }

                    let parent_splited =
                        FlexId::from(parent_flex_id).difference(&flex_id_scanner.flex_id());

                    let parent_value = value_reverse
                        .get((filed_rank, parent_value_rank))?
                        .unwrap()
                        .value()
                        .to_vec();

                    need_delete_ranks.insert(parent_rank);

                    for splited in parent_splited {
                        need_insert
                            .entry(parent_value.clone())
                            .or_default()
                            .push(splited);
                    }

                    continue;
                }

                need_delete_ranks |= flex_id_scanner.children()?;

                let partial_overlaps = flex_id_scanner.partial_overlaps()?;

                if partial_overlaps.is_empty() {
                    need_insert
                        .entry(value.to_vec())
                        .or_default()
                        .push(flex_id_scanner.flex_id().clone());
                    continue;
                }

                need_delete_ranks |= flex_id_scanner.partial_overlaps()?;

                for partial_overlap_rank in partial_overlaps {
                    let (partial_overlap_flex_id, overlap_val_rank) = main
                        .get((filed_rank, partial_overlap_rank))?
                        .unwrap()
                        .value();
                    let overlap_splited = FlexId::from(partial_overlap_flex_id)
                        .difference(&flex_id_scanner.flex_id());

                    let overlap_value = value_reverse
                        .get((filed_rank, overlap_val_rank))?
                        .unwrap()
                        .value()
                        .to_vec();

                    for splited in overlap_splited {
                        need_insert
                            .entry(overlap_value.clone())
                            .or_default()
                            .push(splited);
                    }
                }
            }

            //削除フェーズ
            for need_delete_rank in need_delete_ranks {}
        }

        todo!()
    }

    fn remove_from_rank(
        &self,
        filed_rank: FiledRank,
        flex_id_rank: FlexIdRank,
    ) -> Result<(), Error> {
        let mut main = self.tx.open_table(Kasane::MAIN)?;
        let mut f = self.tx.open_table(Kasane::F)?;
        let mut x = self.tx.open_table(Kasane::X)?;
        let mut y = self.tx.open_table(Kasane::Y)?;
        let mut dictionary = self.tx.open_table(Kasane::DICTIONARY)?;
        let mut value_reverse = self.tx.open_table(Kasane::VALUE_REVERSE)?;

        let (flex_id_bytes, value_rank) = main.remove((filed_rank, flex_id_rank))?.unwrap().value();

        f.get_mut((filed_rank, flex_id_bytes.0))?
            .unwrap()
            .value()
            .mut_treemap()
            .remove(flex_id_rank);

        x.get_mut((filed_rank, flex_id_bytes.1))?
            .unwrap()
            .value()
            .mut_treemap()
            .remove(flex_id_rank);

        y.get_mut((filed_rank, flex_id_bytes.2))?
            .unwrap()
            .value()
            .mut_treemap()
            .remove(flex_id_rank);

        let binding = value_reverse.remove((filed_rank, value_rank))?.unwrap();

        let value = binding.value().to_vec();

        drop(binding);

        //dictionaryから削除
        dictionary
            .get_mut((filed_rank, value.as_slice()))?
            .unwrap()
            .value()
            .0
            .mut_treemap()
            .remove(flex_id_rank);

        //そのValueを参照しているFlexIdが0になった場合はRoaringTreemapを削除して、ValueRankを返却する
        if dictionary
            .get((filed_rank, value.as_slice()))?
            .unwrap()
            .value()
            .0
            .as_treemap()
            .is_empty()
        {
            dictionary.remove((filed_rank, value.as_slice()))?.unwrap();
            value_reverse.remove((filed_rank, value_rank)).unwrap();
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

    fn return_value_rank(&mut self, filed_rank: FiledRank, value_rank: ValueRank) {}

    pub fn commit(self) -> Result<(), Error> {
        Ok(self.tx.commit()?)
    }
}
