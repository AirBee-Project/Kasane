//! 書き込みの入口。`TikvWrite` に生やす固有メソッドはここだけ。
//! リーフの中身をどう書き換えるかは [`leaf`](super::leaf) の純粋関数が担う。

use kasane_logic::{FlexId, RangeId, SpatialIdSet};
use rustc_hash::FxHashSet;
use std::collections::BTreeSet;

use super::leaf::{BatchWrite, LeafOp, apply_leaf, merge_children};
use super::node::archived_leaf;
use super::routing::{ParentMap, RoutedLeaf, Routing, route_leaves_batched};
use super::{
    AppError, MERGE_FLEX_ID_THRESHOLD, ShardEntry, TableDataType, TableId, TikvWrite, keys, kv,
};

impl TikvWrite<'_> {
    /// 対象リーフをロックし、ロック時点の内容とともに返す。
    ///
    /// > **リーフ R の内容または到達可能性を変える操作は、必ず R 自身を書くか消す。**
    ///
    /// この不変条件があるので、R をロックすれば R は誰にも変えられず、テーブル全体を排他せずに
    /// 済む。降下はロックなしでよく、古い形で誤ったリーフを選んでもロック後の検証で拾える。
    async fn lock_target_leaves(
        &mut self,
        table_id: TableId,
        flex_ids: &[FlexId],
    ) -> Result<Routing, AppError> {
        let mut routed = route_leaves_batched(&self.txn, table_id, flex_ids).await?;
        let regions: BTreeSet<FlexId> = routed.leaves.iter().map(|leaf| leaf.region).collect();
        let mut locked = kv::lock_shards(&self.txn, table_id, regions).await?;

        for leaf in &mut routed.leaves {
            let entry = locked.remove(&leaf.region).ok_or_else(|| {
                AppError::InternalError("locked shard map is missing a routed region".to_string())
            })?;

            match (&leaf.node, &entry) {
                // ポインタノードになっていた ＝ 降下中に分割された。
                (_, Some(value)) if ShardEntry::child_pointers(value.entry())?.is_some() => {
                    return Err(self.mark_stale().into());
                }
                // 親へ畳まれた可能性がある。区別するよりやり直した方が確実に収束する。
                (Some(_), None) => return Err(self.mark_stale().into()),
                _ => {}
            }

            // 両方を抱えたままにすると、対象リーフのバイト列を二重に持つことになる。
            leaf.node = entry;
        }
        Ok(routed)
    }

    /// **計算は blocking タスクで回す**（理由は [`apply_leaf`]）。
    async fn apply_leaves(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        leaves: Vec<RoutedLeaf>,
        op: LeafOp,
    ) -> Result<(), AppError> {
        if leaves.is_empty() {
            return Ok(());
        }
        // blocking タスクは呼び出し元のスパンを引き継がないので、明示的に渡す。
        let span = tracing::Span::current();
        let mutations = tokio::task::spawn_blocking(move || {
            span.in_scope(|| {
                let mut out = Vec::new();
                for leaf in &leaves {
                    apply_leaf(table_id, index, leaf, &op, &mut out)?;
                }
                Ok::<_, AppError>(out)
            })
        })
        .await
        .map_err(|e| AppError::InternalError(format!("leaf write task: {e}")))??;

        kv::stage(&self.txn, mutations).await;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(in crate::repositories::tikv) async fn data_insert_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let leaves = self.lock_target_leaves(table_id, &flex_ids).await?.leaves;
        self.apply_leaves(table_id, index, leaves, LeafOp::Insert(data.to_vec()))
            .await
    }

    /// **降下・ロック・リーフの書き直しをそれぞれ 1 回で済ませる。** 要素ごとに呼ぶと、
    /// 降下とロックが要素数だけ直列に積み上がる。
    #[tracing::instrument(skip_all, fields(table_id = %table_id, entries = entries.len()))]
    pub(in crate::repositories::tikv) async fn data_insert_many_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        entries: Vec<(SpatialIdSet, Vec<u8>)>,
    ) -> Result<(), AppError> {
        let (batch, flex_ids) = BatchWrite::new(entries);
        if flex_ids.is_empty() {
            return Ok(());
        }
        let leaves = self.lock_target_leaves(table_id, &flex_ids).await?.leaves;
        self.apply_leaves(table_id, index, leaves, LeafOp::InsertMany(batch))
            .await
    }

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(in crate::repositories::tikv) async fn data_upsert_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let leaves = self.lock_target_leaves(table_id, &flex_ids).await?.leaves;
        self.apply_leaves(table_id, index, leaves, LeafOp::Upsert(data.to_vec()))
            .await
    }

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(in crate::repositories::tikv) async fn data_remove_impl(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let flex_ids: Vec<FlexId> = ids.flex_ids().collect();
        let routing = self.lock_target_leaves(table_id, &flex_ids).await?;

        let affected: Vec<FlexId> = routing.leaves.iter().map(|leaf| leaf.region).collect();
        self.apply_leaves(table_id, index, routing.leaves, LeafOp::Remove)
            .await?;

        // 共有しないと、兄弟がそれぞれ上へ辿って先の統合で消えた領域を調べに行く。
        let mut settled = FxHashSet::default();
        for region in affected {
            self.try_merge_up(table_id, index, region, &routing.parents, &mut settled)
                .await?;
        }
        Ok(())
    }

    /// 削除でデータ量が減ったリーフを親へ統合し、可能な限り木を圧縮する。
    ///
    /// 親と**その全子**を書き換えるので、判断の前にまとめてロックする。空の子領域も含めるのは、
    /// 消える予定の空領域へ他者が書き込めてしまうため。
    async fn try_merge_up(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        region: FlexId,
        parents: &ParentMap,
        settled: &mut FxHashSet<FlexId>,
    ) -> Result<(), AppError> {
        let mut region = region;
        loop {
            if settled.contains(&region) {
                break;
            }
            // 対応が無いのはルートに達したとき。
            let Some((parent_region, descended_children)) = parents.get(&region) else {
                break;
            };
            let parent_region = *parent_region;

            // 既に畳んだ子も重なった状態で返る必要がある（[`kv::lock_shards`]）。
            let mut targets: BTreeSet<FlexId> = descended_children.iter().copied().collect();
            targets.insert(parent_region);
            let locked = kv::lock_shards(&self.txn, table_id, targets).await?;

            // 親が今もポインタノードで、子集合が降下時と同じであることを確かめる。
            let child_regions = match locked.get(&parent_region) {
                Some(Some(value)) => match ShardEntry::child_pointers(value.entry())? {
                    Some(children) if children == **descended_children => children,
                    _ => return Err(self.mark_stale().into()),
                },
                _ => return Err(self.mark_stale().into()),
            };

            // 子のいずれかがポインタノードなら、このレベルは統合しない。
            let mut combined = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                // 空領域のキーはそもそも存在しない。
                let Some(Some(value)) = locked.get(cr) else {
                    continue;
                };
                match ShardEntry::leaf_count(value.entry())? {
                    Some(count) => {
                        combined += count as usize;
                        if combined > MERGE_FLEX_ID_THRESHOLD {
                            mergeable = false;
                            break;
                        }
                    }
                    None => {
                        mergeable = false;
                        break;
                    }
                }
            }
            if !mergeable {
                // 兄弟が同じ判定を繰り返しても答えは変わらない。
                settled.extend(child_regions);
                break;
            }

            // ここから先は重い（子マップの復元・統合・直列化）ので blocking タスクへ出す。
            let span = tracing::Span::current();
            let regions = child_regions.clone();
            let mutations = tokio::task::spawn_blocking(move || {
                span.in_scope(|| merge_children(table_id, index, parent_region, &regions, &locked))
            })
            .await
            .map_err(|e| AppError::InternalError(format!("shard merge task: {e}")))??;

            kv::stage(&self.txn, mutations).await;

            // 兄弟がここから上を辿り直さないよう印を付ける。
            settled.extend(child_regions);
            region = parent_region;
        }
        Ok(())
    }

    /// テーブル単位の排他を取らないので**ある一点での検証**にしかならない。すり抜けうるのは
    /// 「古い制約を読んだうえで検証後にコミットされた書き込み」だけ。
    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    pub(in crate::repositories::tikv) async fn validate_existing_data(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        constraints: Option<&crate::models::database::table::TableConstraints>,
    ) -> Result<(), AppError> {
        let entries = kv::scan_shard_prefix(&self.txn, &keys::shards_of(table_id)).await?;
        for (_, value) in entries {
            // ポインタノードには値が無い。
            if ShardEntry::leaf_payload(value.entry())?.is_none() {
                continue;
            }
            let arch = archived_leaf(value.entry())?;
            // 検証したいのは格納値そのものなので、葉を作業木へ組み直す必要はない。
            for range in [FlexId::LOWER_MAX, FlexId::UPPER_MAX].map(RangeId::from) {
                for (_, stored) in arch.get_range(&range) {
                    let restored = crate::services::helpers::value::restore_value(
                        data_type,
                        constraints,
                        stored,
                    )?;
                    crate::services::helpers::value::interpret_value(
                        data_type,
                        constraints,
                        restored,
                    )?;
                }
            }
        }
        Ok(())
    }
}
