use kasane_logic::{IterSingleIds, SingleId, SpatialIdSet};
use rayon::prelude::*;

use crate::{error::AppError, repositories::KasaneDbRead};

type SpatialDataResult<'txn> = Result<Option<(SingleId, &'txn [u8])>, AppError>;
type SpatialDataListResult<'txn> = Result<Option<Vec<(SingleId, &'txn [u8])>>, AppError>;

/// この件数以上の SingleId を引く場合は rayon で並列にファンアウトする。
/// 小さなクエリではスレッド分配のオーバーヘッドが勝つため直列で処理する。
const PARALLEL_FANOUT_THRESHOLD: usize = 512;

impl<'a> KasaneDbRead<'a> {
    /// 空間IDごとに格納値を解決し、`decode` で復元した値を返す。
    ///
    /// `decode` は「ストレージ上のバイト列 → 任意の復元値」への変換関数で、
    /// 呼び出し側（サービス層）が `restore_value` などを渡す。これにより
    /// リポジトリ層はデータ型の解釈に依存せず、かつ **復元を txn スコープ内・
    /// 並列ワーカー内で行える**（borrow したスライスを `Vec<u8>` に一旦コピー
    /// せず、その場で所有値へ復元できる）。
    pub fn data_get<T, F>(
        &self,
        table_id: uuid::Uuid,
        ids: SpatialIdSet,
        decode: F,
    ) -> Result<Vec<(SingleId, T)>, AppError>
    where
        F: Fn(&[u8]) -> Result<T, AppError> + Sync,
        T: Send,
    {
        let spatial_db = self.db.spatialid_to_value;
        let single_ids: Vec<SingleId> = ids.iter_single_ids().collect();

        if single_ids.len() < PARALLEL_FANOUT_THRESHOLD {
            // 直列パス: 既存の read スナップショットをそのまま使う。
            let mut result = Vec::with_capacity(single_ids.len());
            for single_id in &single_ids {
                Self::lookup_one(
                    &spatial_db,
                    &self.read_txn,
                    table_id,
                    single_id,
                    &decode,
                    &mut result,
                )?;
            }
            return Ok(result);
        }

        // 並列パス: 各 lookup は独立なので rayon でファンアウトする。
        //
        // LMDB の read トランザクションは（NOTLS でも）スレッド間で *同時* 共有できないため、
        // 1つの txn を共有するのではなく、ワーカーごとに自分の read txn を開く。
        // チャンク単位に分割して txn の生成コストを償却する。
        // 各 txn は独立スナップショットになるが、読み取りクエリでは許容できる。
        let env = &self.db.env;
        let chunk_size = single_ids
            .len()
            .div_ceil(rayon::current_num_threads())
            .max(1);

        let nested = single_ids
            .par_chunks(chunk_size)
            .map(|chunk| -> Result<Vec<(SingleId, T)>, AppError> {
                let txn = env.read_txn()?;
                let mut out = Vec::with_capacity(chunk.len());
                for single_id in chunk {
                    Self::lookup_one(&spatial_db, &txn, table_id, single_id, &decode, &mut out)?;
                }
                Ok(out)
            })
            .collect::<Result<Vec<Vec<(SingleId, T)>>, AppError>>()?;

        let total: usize = nested.iter().map(Vec::len).sum();
        let mut result = Vec::with_capacity(total);
        for chunk_result in nested {
            result.extend(chunk_result);
        }
        Ok(result)
    }

    /// 1つの SingleId に対して equal → parent → children の順で値を解決し、
    /// `decode` で復元した結果を `out` に push する。
    ///
    /// 復元を借用スライスに対してその場で行うため、生バイト列のコピーが発生しない。
    fn lookup_one<'txn, 'env, T, F>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RoTxn<'env, heed::WithoutTls>,
        table_id: uuid::Uuid,
        single_id: &SingleId,
        decode: &F,
        out: &mut Vec<(SingleId, T)>,
    ) -> Result<(), AppError>
    where
        F: Fn(&[u8]) -> Result<T, AppError>,
    {
        if let Some(data) = Self::overlap_equal(spatial_db, txn, table_id, single_id)? {
            out.push((single_id.clone(), decode(data)?));
            return Ok(());
        }

        if let Some((_, data)) = Self::overlap_parent(spatial_db, txn, table_id, single_id)? {
            out.push((single_id.clone(), decode(data)?));
            return Ok(());
        }

        if let Some(children) = Self::overlap_children(spatial_db, txn, table_id, single_id)? {
            for (child, value) in children {
                out.push((child, decode(value)?));
            }
        }

        Ok(())
    }

    pub(self) fn overlap_equal<'txn, 'env>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RoTxn<'env, heed::WithoutTls>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> Result<Option<&'txn [u8]>, AppError> {
        if let Some(val) = spatial_db.get(txn, &(table_id.into_bytes(), target.spatial_encode()))? {
            return Ok(Some(val));
        }
        Ok(None)
    }

    pub(self) fn overlap_parent<'txn, 'env>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RoTxn<'env, heed::WithoutTls>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> SpatialDataResult<'txn> {
        for parent in target.spatial_parents() {
            if let Some(val) =
                spatial_db.get(txn, &(table_id.into_bytes(), parent.spatial_encode()))?
            {
                return Ok(Some((parent, val)));
            }
        }
        Ok(None)
    }

    pub(self) fn overlap_children<'txn, 'env>(
        spatial_db: &heed::Database<crate::db_init::TableIdAndSpatialId, heed::types::Bytes>,
        txn: &'txn heed::RoTxn<'env, heed::WithoutTls>,
        table_id: uuid::Uuid,
        target: &SingleId,
    ) -> SpatialDataListResult<'txn> {
        let mut result = Vec::new();

        let start = (table_id.into_bytes(), target.spatial_encode());
        let end = (table_id.into_bytes(), target.spatial_encode_prefix_max());

        for item in spatial_db.range(txn, &(start..=end))? {
            let (key, value) = item?;
            let single_id_encode = key.1;

            if SingleId::spatial_decode(&single_id_encode)? == *target {
                continue;
            }

            result.push((SingleId::spatial_decode(&single_id_encode)?, value));
        }

        if result.is_empty() {
            return Ok(None);
        }

        Ok(Some(result))
    }
}
