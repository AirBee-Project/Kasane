#![cfg(all(not(target_arch = "wasm32"), feature = "redb"))]

use super::{Backend, ReadTransaction, WriteTransaction};
use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};

const TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("main");

pub struct RedbBackend {
    db: Database,
}

#[async_trait]
impl Backend for RedbBackend {
    // Redbのトランザクションは所有権を持つ(Arc)ため、ライフタイム 'a は無視する
    type ReadTx<'a> = RedbReadTx;
    type WriteTx<'a> = RedbWriteTx;

    async fn new(path: &str) -> anyhow::Result<Self> {
        let db = Database::create(path)?;
        // 初回テーブル作成
        let txn = db.begin_write()?;
        {
            txn.open_table(TABLE)?;
        }
        txn.commit()?;
        Ok(Self { db })
    }

    async fn begin_read(&self) -> anyhow::Result<Self::ReadTx<'_>> {
        let txn = self.db.begin_read()?;
        Ok(RedbReadTx(txn))
    }

    async fn begin_write(&self) -> anyhow::Result<Self::WriteTx<'_>> {
        let txn = self.db.begin_write()?;
        Ok(RedbWriteTx(txn))
    }
}

// --- Read Transaction ---
// 修正: ライフタイム <'a> を削除
pub struct RedbReadTx(redb::ReadTransaction);

#[async_trait]
impl ReadTransaction for RedbReadTx {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let table = self.0.open_table(TABLE)?;
        let res = table.get(key)?;
        Ok(res.map(|v| v.value().to_vec()))
    }
}

// --- Write Transaction ---
// 修正: ライフタイム <'a> を削除
pub struct RedbWriteTx(redb::WriteTransaction);

// WriteTxもReadTransactionを実装
#[async_trait]
impl ReadTransaction for RedbWriteTx {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let table = self.0.open_table(TABLE)?;
        let res = table.get(key)?;
        Ok(res.map(|v| v.value().to_vec()))
    }
}

#[async_trait]
impl WriteTransaction for RedbWriteTx {
    async fn set(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        let mut table = self.0.open_table(TABLE)?;
        table.insert(key, value)?;
        Ok(())
    }

    async fn commit(self) -> anyhow::Result<()> {
        self.0.commit()?;
        Ok(())
    }

    async fn rollback(self) -> anyhow::Result<()> {
        self.0.abort()?;
        Ok(())
    }
}
