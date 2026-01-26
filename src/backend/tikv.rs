#![cfg(all(not(target_arch = "wasm32"), feature = "tikv"))]

use super::{Backend, Transaction};
use async_trait::async_trait;
use tikv_client::{Transaction as TikvTransaction, TransactionClient};

pub struct TikvBackend {
    client: TransactionClient,
}

#[async_trait]
impl Backend for TikvBackend {
    type Tx<'a> = TikvTx;

    // path 引数を PD (Placement Driver) のエンドポイントとして扱う
    // 例: "127.0.0.1:2379"
    async fn new(path: &str) -> anyhow::Result<Self> {
        let client = TransactionClient::new(vec![path]).await?;
        Ok(Self { client })
    }

    async fn begin_read(&self) -> Self::Tx<'_> {
        // TiKVでは読み取りでもトランザクションを開始するのが一般的
        // (Snapshot Readもあるが、IF統一のため楽観的トランザクションを開始)
        let txn = self
            .client
            .begin_optimistic()
            .await
            .expect("Failed to begin tikv txn");
        TikvTx { txn }
    }

    async fn begin_write(&self) -> Self::Tx<'_> {
        // 書き込みも楽観的ロックで開始
        let txn = self
            .client
            .begin_optimistic()
            .await
            .expect("Failed to begin tikv txn");
        TikvTx { txn }
    }
}

pub struct TikvTx {
    txn: TikvTransaction,
}

#[async_trait]
impl Transaction for TikvTx {
    async fn get(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        // TiKVのgetは Option<Vec<u8>> を返すのでそのまま使える
        // キーは Key に変換される
        let res = self.txn.get(key.to_vec()).await?;
        Ok(res)
    }

    async fn set(&mut self, key: &[u8], value: &[u8]) -> anyhow::Result<()> {
        self.txn.put(key.to_vec(), value.to_vec()).await?;
        Ok(())
    }

    async fn commit(mut self) -> anyhow::Result<()> {
        self.txn.commit().await?;
        Ok(())
    }
}
