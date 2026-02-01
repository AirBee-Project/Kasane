use std::cell::RefCell;

use super::{DICTIONARY, F, FORWARD, MAIN, META, REVERSE, X, Y};
use crate::backend::{redb::roaring_treemap::RedbRoaringTreemap, FieldId};
use kasane_logic::{
    Batch, Collection, FlexId, FlexIdRank, KeyValueStore, OrderedKeyValueStore, RoaringTreemap,
    Segment, TableStorage,
};
use redb::ReadableTable;

// --- ストア実装 (Adapter) ---

/// Main Store: FlexIdRank -> FlexId
/// テーブル定義: (FieldId, FlexIdRank) -> (FieldId, ([u8; N], [u8; N], [u8; N]))
pub struct RedbMainStore<'a> {
    txn: &'a RefCell<redb::WriteTransaction>,
    field_id: FieldId,
}

impl<'a> KeyValueStore<FlexIdRank, FlexId> for RedbMainStore<'a> {
    fn get(&self, key: &FlexIdRank) -> Option<FlexId> {
        let txn = self.txn.borrow();
        let table = txn.open_table(MAIN).ok()?;
        // 複合キーで検索
        let (_fid, (f, x, y)) = table.get(&(self.field_id, *key)).ok()??.value();
        unsafe {
            Some(FlexId::from_parts_unchecked(
                Segment::from(f),
                Segment::from(x),
                Segment::from(y),
            ))
        }
    }

    fn batch_get(&self, keys: &[FlexIdRank]) -> Vec<Option<FlexId>> {
        keys.iter().map(|k| self.get(k)).collect()
    }

    fn apply_batch(&mut self, batch: Batch<FlexIdRank, FlexId>) {
        let mut txn = self.txn.borrow_mut();
        if let Ok(mut table) = txn.open_table(MAIN) {
            for (rank, flex_id) in batch.puts {
                let (f, x, y): (
                    [u8; Segment::ARRAY_LENGTH],
                    [u8; Segment::ARRAY_LENGTH],
                    [u8; Segment::ARRAY_LENGTH],
                ) = flex_id.into();

                let val = (self.field_id, (f, x, y));

                let _ = table.insert(&(self.field_id, rank), val);
            }
            for rank in batch.deletes {
                let _ = table.remove(&(self.field_id, rank));
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (FlexIdRank, FlexId)> {
        let txn = self.txn.borrow();
        let table = txn.open_table(MAIN).expect("open main");
        let mut res = Vec::new();

        let start = (self.field_id, 0);
        let end = (self.field_id, u64::MAX);

        if let Ok(iter) = table.range(start..=end) {
            for item in iter {
                if let Ok((k, v)) = item {
                    let (_, rank) = k.value();
                    let (_, (f, x, y)) = v.value();
                    unsafe {
                        let flex_id = FlexId::from_parts_unchecked(
                            Segment::from_be_bytes(f),
                            Segment::from_be_bytes(x),
                            Segment::from_be_bytes(y),
                        );
                        res.push((rank, flex_id));
                    }
                }
            }
        }
        res.into_iter()
    }

    fn len(&self) -> usize {
        self.iter().count()
    }
}

/// Dimension Store: Segment -> RoaringTreemap
pub struct RedbDimStore<'a> {
    txn: &'a RefCell<redb::WriteTransaction>,
    field_id: FieldId,
    // F, X, Y どのテーブルを使うか
    table_def:
        redb::TableDefinition<'static, (FieldId, [u8; Segment::ARRAY_LENGTH]), RedbRoaringTreemap>,
}

impl<'a> KeyValueStore<Segment, RoaringTreemap> for RedbDimStore<'a> {
    fn get(&self, key: &Segment) -> Option<RoaringTreemap> {
        let txn = self.txn.borrow();
        let table = txn.open_table(self.table_def).ok()?;
        let k = (self.field_id, key.to_be_bytes());
        let v = table.get(&k).ok()??;
        Some(v.value().0)
    }

    fn batch_get(&self, keys: &[Segment]) -> Vec<Option<RoaringTreemap>> {
        keys.iter().map(|k| self.get(k)).collect()
    }

    fn apply_batch(&mut self, batch: Batch<Segment, RoaringTreemap>) {
        let mut txn = self.txn.borrow_mut();
        if let Ok(mut table) = txn.open_table(self.table_def) {
            for (seg, bitmap) in batch.puts {
                let k = (self.field_id, seg.to_be_bytes());
                let _ = table.insert(&k, RedbRoaringTreemap(bitmap));
            }
            for seg in batch.deletes {
                let k = (self.field_id, seg.to_be_bytes());
                let _ = table.remove(&k);
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (Segment, RoaringTreemap)> {
        let txn = self.txn.borrow();
        let table = txn.open_table(self.table_def).expect("open dim");
        let mut res = Vec::new();

        // Range Scan
        let start = (self.field_id, [0u8; Segment::ARRAY_LENGTH]);
        let end = (self.field_id, [0xFFu8; Segment::ARRAY_LENGTH]);

        if let Ok(iter) = table.range(start..=end) {
            for item in iter {
                if let Ok((k, v)) = item {
                    let (_, bytes) = k.value();
                    let seg = Segment::from_be_bytes(bytes);
                    res.push((seg, v.value().0));
                }
            }
        }
        res.into_iter()
    }

    fn len(&self) -> usize {
        self.iter().count()
    }
}

impl<'a> OrderedKeyValueStore<Segment, RoaringTreemap> for RedbDimStore<'a> {
    fn scan<R>(&self, _range: R) -> Box<dyn Iterator<Item = (Segment, RoaringTreemap)> + '_>
    where
        R: std::ops::RangeBounds<Segment>,
    {
        let iter = self.iter();
        Box::new(iter)
    }

    fn last_key(&self) -> Option<Segment> {
        self.iter().map(|(k, _)| k).max()
    }

    fn first_key(&self) -> Option<Segment> {
        self.iter().map(|(k, _)| k).min()
    }
}

/// Forward: FlexIdRank -> ValueRank
pub struct RedbForwardStore<'a> {
    txn: &'a RefCell<redb::WriteTransaction>,
    field_id: FieldId,
}

impl<'a> KeyValueStore<FlexIdRank, u64> for RedbForwardStore<'a> {
    fn get(&self, key: &FlexIdRank) -> Option<u64> {
        let txn = self.txn.borrow();
        let table = txn.open_table(FORWARD).ok()?;
        let v = table.get(&(self.field_id, *key)).ok()??;
        Some(v.value())
    }

    fn batch_get(&self, keys: &[FlexIdRank]) -> Vec<Option<u64>> {
        keys.iter().map(|k| self.get(k)).collect()
    }

    fn apply_batch(&mut self, batch: Batch<FlexIdRank, u64>) {
        let mut txn = self.txn.borrow_mut();
        if let Ok(mut table) = txn.open_table(FORWARD) {
            for (k, v) in batch.puts {
                let _ = table.insert(&(self.field_id, k), v);
            }
            for k in batch.deletes {
                let _ = table.remove(&(self.field_id, k));
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (FlexIdRank, u64)> {
        let txn = self.txn.borrow();
        let table = txn.open_table(FORWARD).expect("open forward");
        let mut res = Vec::new();
        let start = (self.field_id, 0);
        let end = (self.field_id, u64::MAX);
        if let Ok(iter) = table.range(start..=end) {
            for item in iter {
                if let Ok((k, v)) = item {
                    res.push((k.value().1, v.value()));
                }
            }
        }
        res.into_iter()
    }

    fn len(&self) -> usize {
        self.iter().count()
    }
}

/// Dictionary: ValueRank -> Value
pub struct RedbDictStore<'a> {
    txn: &'a RefCell<redb::WriteTransaction>,
    field_id: FieldId,
}

impl<'a> KeyValueStore<u64, Vec<u8>> for RedbDictStore<'a> {
    fn get(&self, key: &u64) -> Option<Vec<u8>> {
        let txn = self.txn.borrow();
        let table = txn.open_table(DICTIONARY).ok()?;
        let v = table.get(&(self.field_id, *key)).ok()??;
        Some(v.value().to_vec())
    }

    fn batch_get(&self, keys: &[u64]) -> Vec<Option<Vec<u8>>> {
        keys.iter().map(|k| self.get(k)).collect()
    }

    fn apply_batch(&mut self, batch: Batch<u64, Vec<u8>>) {
        let mut txn = self.txn.borrow_mut();
        if let Ok(mut table) = txn.open_table(DICTIONARY) {
            for (k, v) in batch.puts {
                let _ = table.insert(&(self.field_id, k), v);
            }
            for k in batch.deletes {
                let _ = table.remove(&(self.field_id, k));
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (u64, Vec<u8>)> {
        let txn = self.txn.borrow();
        let table = txn.open_table(DICTIONARY).expect("open dict");
        let mut res = Vec::new();
        let start = (self.field_id, 0);
        let end = (self.field_id, u64::MAX);
        if let Ok(iter) = table.range(start..=end) {
            for item in iter {
                if let Ok((k, v)) = item {
                    res.push((k.value().1, v.value()));
                }
            }
        }
        res.into_iter()
    }

    fn len(&self) -> usize {
        self.iter().count()
    }
}

/// Reverse: Value -> ValueRank
pub struct RedbReverseStore<'a> {
    txn: &'a RefCell<redb::WriteTransaction>,
    field_id: FieldId,
}

impl<'a> KeyValueStore<Vec<u8>, u64> for RedbReverseStore<'a> {
    fn get(&self, key: &Vec<u8>) -> Option<u64> {
        let txn = self.txn.borrow();
        let table = txn.open_table(REVERSE).ok()?;
        let v = table.get(&(self.field_id, key.clone())).ok()??;
        Some(v.value())
    }

    fn batch_get(&self, keys: &[Vec<u8>]) -> Vec<Option<u64>> {
        keys.iter().map(|k| self.get(k)).collect()
    }

    fn apply_batch(&mut self, batch: Batch<Vec<u8>, u64>) {
        let mut txn = self.txn.borrow_mut();
        if let Ok(mut table) = txn.open_table(REVERSE) {
            for (k, v) in batch.puts {
                let _ = table.insert(&(self.field_id, k), v);
            }
            for k in batch.deletes {
                let _ = table.remove(&(self.field_id, k));
            }
        }
    }

    fn iter(&self) -> impl Iterator<Item = (Vec<u8>, u64)> {
        let txn = self.txn.borrow();
        let table = txn.open_table(REVERSE).expect("open reverse");
        let mut res = Vec::new();
        // Vec<u8>のRangeは難しいため、全件スキャンしてFieldIdマッチを行う
        if let Ok(iter) = table.iter() {
            for item in iter {
                if let Ok((k, v)) = item {
                    let (fid, val) = k.value();
                    if fid == self.field_id {
                        res.push((val, v.value()));
                    }
                }
            }
        }
        res.into_iter()
    }

    fn len(&self) -> usize {
        self.iter().count()
    }
}

// --- 本体: RedbSingleField ---

pub struct RedbSingleField<'a> {
    txn: &'a RefCell<redb::WriteTransaction>,
    field_id: FieldId,

    // Cache stores
    main: RedbMainStore<'a>,
    f: RedbDimStore<'a>,
    x: RedbDimStore<'a>,
    y: RedbDimStore<'a>,

    forward: RedbForwardStore<'a>,
    dict: RedbDictStore<'a>,
    reverse: RedbReverseStore<'a>,
}

impl<'a> RedbSingleField<'a> {
    pub fn new(txn: &'a RefCell<redb::WriteTransaction>, field_id: FieldId) -> Self {
        Self {
            txn,
            field_id,
            main: RedbMainStore { txn, field_id },
            f: RedbDimStore {
                txn,
                field_id,
                table_def: F,
            },
            x: RedbDimStore {
                txn,
                field_id,
                table_def: X,
            },
            y: RedbDimStore {
                txn,
                field_id,
                table_def: Y,
            },
            forward: RedbForwardStore { txn, field_id },
            dict: RedbDictStore { txn, field_id },
            reverse: RedbReverseStore { txn, field_id },
        }
    }
}

impl<'a> Collection for RedbSingleField<'a> {
    type Main = RedbMainStore<'a>;
    type Dimension = RedbDimStore<'a>;

    fn main(&self) -> &Self::Main {
        &self.main
    }
    fn main_mut(&mut self) -> &mut Self::Main {
        &mut self.main
    }
    fn f(&self) -> &Self::Dimension {
        &self.f
    }
    fn f_mut(&mut self) -> &mut Self::Dimension {
        &mut self.f
    }
    fn x(&self) -> &Self::Dimension {
        &self.x
    }
    fn x_mut(&mut self) -> &mut Self::Dimension {
        &mut self.x
    }
    fn y(&self) -> &Self::Dimension {
        &self.y
    }
    fn y_mut(&mut self) -> &mut Self::Dimension {
        &mut self.y
    }

    fn fetch_flex_rank(&mut self) -> u64 {
        let mut txn = self.txn.borrow_mut();
        let mut meta = txn.open_table(META).expect("meta");
        // キーを "next_rank_<field_id>" のようにユニークにするか、
        // METAテーブル定義が (FieldId, &str) -> u64 ならタプルキーを使う
        // ここでは mod.rs の定義に合わせて文字列表現で回避する例
        let key = format!("next_flex_rank_{}", self.field_id);
        let next = meta
            .get(key.as_str())
            .unwrap()
            .map(|v| v.value())
            .unwrap_or(0);
        meta.insert(key.as_str(), next + 1).unwrap();
        next
    }

    fn return_flex_rank(&mut self, _rank: u64) {}
    fn move_flex_rank(&self) -> u64 {
        0
    }
    fn move_flex_rank_free_list(&self) -> Vec<u64> {
        vec![]
    }
}

impl<'a> TableStorage for RedbSingleField<'a> {
    type Value = Vec<u8>;

    type Forward = RedbForwardStore<'a>;
    type Dictionary = RedbDictStore<'a>;
    type Reverse = RedbReverseStore<'a>;

    fn forward(&self) -> &Self::Forward {
        &self.forward
    }
    fn forward_mut(&mut self) -> &mut Self::Forward {
        &mut self.forward
    }

    fn dictionary(&self) -> &Self::Dictionary {
        &self.dict
    }
    fn dictionary_mut(&mut self) -> &mut Self::Dictionary {
        &mut self.dict
    }

    fn reverse(&self) -> &Self::Reverse {
        &self.reverse
    }
    fn reverse_mut(&mut self) -> &mut Self::Reverse {
        &mut self.reverse
    }

    fn fetch_value_rank(&mut self) -> u64 {
        todo!()
    }

    fn return_value_rank(&mut self, rank: u64) {
        todo!()
    }

    fn move_value_rank(&self) -> u64 {
        todo!()
    }

    fn move_value_rank_free_list(&self) -> Vec<u64> {
        todo!()
    }

    fn insert_value(&mut self, value: &Self::Value, flex_id_ranks: Vec<FlexIdRank>) -> ValueRank {
        let value_rank = if let Some(rank) = self.reverse().get(&value) {
            rank
        } else {
            let new_rank = self.fetch_value_rank();
            let mut dict_batch = Batch::new();
            dict_batch.put(new_rank, value.clone());
            self.dictionary_mut().apply_batch(dict_batch);
            let mut rev_batch = Batch::new();
            rev_batch.put(value.clone(), new_rank);
            self.reverse_mut().apply_batch(rev_batch);
            new_rank
        };

        let mut fwd_batch = Batch::new();
        for id_rank in flex_id_ranks {
            fwd_batch.put(id_rank, value_rank);
        }
        self.forward_mut().apply_batch(fwd_batch);

        value_rank
    }
}
