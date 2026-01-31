use crate::{DbError, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

pub struct TypedBucket<V> {
    prefix: Vec<u8>,
    _marker: PhantomData<V>,
}

impl<V> TypedBucket<V> {
    pub fn new(prefix: impl Into<Vec<u8>>) -> Self {
        Self {
            prefix: prefix.into(),
            _marker: PhantomData,
        }
    }

    fn make_key(&self, key: &[u8]) -> Vec<u8> {
        let mut full_key = self.prefix.clone();
        full_key.extend_from_slice(key);
        full_key
    }
}

impl<V> TypedBucket<V>
where
    V: Serialize + DeserializeOwned + Send + Sync,
{
    pub async fn get(
        &self,
        txn: &impl crate::backend::ReadTransaction,
        key: &[u8],
    ) -> Result<Option<V>> {
        let full_key = self.make_key(key);

        match txn.get(&full_key).await? {
            Some(bytes) => {
                let value = bincode::deserialize(&bytes)
                    .map_err(|e| DbError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// 型付きの書き込み
    pub async fn set(
        &self,
        txn: &mut impl crate::backend::WriteTransaction,
        key: &[u8],
        value: &V,
    ) -> Result<()> {
        let full_key = self.make_key(key);
        let bytes = bincode::serialize(value).map_err(|e| DbError::Serialization(e.to_string()))?;

        txn.set(&full_key, &bytes).await
    }
}
