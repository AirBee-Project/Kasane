//! 書き込みの入口。`TikvWrite` に生やす固有メソッドはここだけ。
//!
//! ネットワーク（降下・ロック・変更の登録）だけを担当し、リーフの中身をどう書き換えるかは
//! [`leaf`](super::leaf) の純粋関数に任せて blocking タスクへ出す。

use kasane_logic::{FlexId, RangeId, SpatialIdSet};
use rustc_hash::FxHashSet;
use std::collections::BTreeSet;

use super::leaf::{BatchWrite, OwnedLeafOp, apply_leaf, merge_children};
use super::node::archived_leaf;
use super::routing::{ParentMap, RoutedLeaf, Routing, route_leaves_batched};
use super::{
    AppError, MERGE_FLEX_ID_THRESHOLD, ShardEntry, TableDataType, TableId, TikvWrite, keys, kv,
};

// --- 書き込み ---
//
// # ロックの取り方
//
// データ書き込みはテーブル全体を排他しない。触るのは「対象の空間 ID が属するリーフ」
// だけで、そのキーを [`kv::lock_shards`]（`batch_get_for_update`）でロックし、
// **ロック時点の最新の内容**を同時に受け取る。別のリーフへの書き込みは、別インスタンス
// からのものであっても並列に流れる。
//
// これが安全なのは、この木が次の不変条件を満たすからである。
//
// > **リーフ R の内容または到達可能性を変える操作は、必ず R 自身を書くか消す。**
//
// - 分割できるのはリーフだけなので、R が分割されるなら R が書き換わる
// - 統合（`try_merge_up`）は親へ畳むときに**子キーを削除**する ＝ R を消す
// - 祖先方向の統合は「子のいずれかがポインタノードなら行わない」ので、R より上を
//   畳むには先に R の親を畳む必要があり、それは R を消すことを意味する
//
// よって R をロックすれば R は誰にも変えられない。木の降下（祖先のポインタノード読み）
// はロックなしのスナップショット読みでよく、古い形で誤ったリーフを選んだ場合は
// ロック後の検証で検出できる。
//
// # 楽観トランザクションにしない理由（実測）
//
// 上の不変条件は楽観でもそのまま通用する。「R は必ず書かれる」以上、構造的な競合は
// R というキーの競合として prewrite の検査に現れるからで、実際に一度そちらへ寄せた。
// 往復は `D+8` から `D+4` へ減り、降下で運んだリーフ本体を `batch_get_for_update` で
// もう一度運ぶ無駄も消える。
//
// **それでも悲観に戻した。** 楽観は競合の検出が prewrite まで遅れるので、1 回の競合で
// 捨てる仕事が「降下・復元・変更・シリアライズの全部」になる。同じ空間領域へ多数の
// 書き込みが集中する使い方（都市データの一括投入など）では、この差が決定的だった。
// トレースでは 6 割超の書き込みが 2 回以上やり直し、prewrite がロック解決の backoff
// （約 1.5 秒）を毎回使い切り、`Failed to resolve lock` が定常的に 500 として出た。
//
// 悲観ならロックの取得は着手前で、競合したその場で判る。捨てる仕事が小さいぶん
// やり直しが収束しやすい。**書き込みが落ちないことのほうが、往復数より優先される。**
//
// 低競合が前提の使い方なら楽観のほうが速い。戻すのであれば
// `super::write_options` を楽観へ変え、`lock_target_leaves` の
// `lock_shards` を素の読みへ差し替えればよい（`lock_options` は悲観のまま残すこと）。
//
// # 検証に失敗したらどうするか
//
// 同一トランザクション内で降下し直しても、`get` は常に `start_ts` を読むので**同じ
// 古い答えが返る**。そこで [`TikvWrite::mark_stale`] でこの試行を捨て、`Storage::write`
// に新しいスナップショットでやり直させる。新しい `start_ts` は他者のコミットより後に
// なるので、必ず前進する。

impl TikvWrite<'_> {
    /// 対象の `flex_id` 群が属するリーフをロックし、最新の内容とともに返す。
    ///
    /// 降下はロックなしで行い、その結果をロック後に検証する。食い違っていたら
    /// この試行ごと捨てて、新しいスナップショットでやり直す（モジュール上部を参照）。
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
                // 降下時はリーフだったのに消えている ＝ 親へ畳まれた可能性がある。
                // 単に空になって消えただけのこともあるが、区別するよりやり直した方が
                // 確実で、やり直しは 1 周で収束する。
                (Some(_), None) => return Err(self.mark_stale().into()),
                _ => {}
            }

            // 降下時に読んだ本体は捨て、ロック時点の内容へ差し替える。
            // 両方を抱えたままにすると、対象リーフのバイト列を二重に持つことになる。
            leaf.node = entry;
        }
        Ok(routed)
    }

    /// ロック済みのリーフ群へ変更を適用し、生じた変更をまとめて溜める。
    ///
    /// **計算は blocking タスクで回す。** リーフ 1 枚ごとに rkyv の復元と直列化が走る
    /// 重い処理で、非同期ワーカー上に置くとハートビートまで巻き添えにする
    /// （リーフの書き換えの節を参照）。ネットワークに触れるのはこの前後だけなので、
    /// 計算をまるごと外へ出せる。
    async fn apply_leaves(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        leaves: Vec<RoutedLeaf>,
        op: OwnedLeafOp,
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
                    apply_leaf(table_id, index, leaf, &op.borrow(), &mut out)?;
                }
                Ok::<_, AppError>(out)
            })
        })
        .await
        .map_err(|e| AppError::InternalError(format!("leaf write task: {e}")))??;

        self.stage(mutations).await
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
        self.apply_leaves(table_id, index, leaves, OwnedLeafOp::Insert(data.to_vec()))
            .await
    }

    /// 値ごとに分かれた書き込みをまとめて適用する。
    ///
    /// **木の降下・ロック・リーフの書き直しをそれぞれ 1 回で済ませる。** 要素ごとに
    /// `data_insert_impl` を呼ぶと、要素の数だけ降下（深さぶんの往復）とロック取得が
    /// 直列に積み上がり、同じリーフを何度も復元・直列化し直すことになる。
    /// 全要素の空間 ID をまとめて振り分けてからリーフごとに適用すれば、コストは
    /// **触れたリーフの数**だけで決まる。
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
        self.apply_leaves(table_id, index, leaves, OwnedLeafOp::InsertMany(batch))
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
        self.apply_leaves(table_id, index, leaves, OwnedLeafOp::Upsert(data.to_vec()))
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
        self.apply_leaves(table_id, index, routing.leaves, OwnedLeafOp::Remove)
            .await?;

        // 同じ親を持つ兄弟がそれぞれ上へ辿ると、先の統合で消えた領域を後から調べに
        // 行くことになる。決着のついた領域を共有して 1 度で終わらせる。
        let mut settled = FxHashSet::default();
        for region in affected {
            self.try_merge_up(table_id, index, region, &routing.parents, &mut settled)
                .await?;
        }
        Ok(())
    }

    /// 溜めた変更をトランザクションへ渡す。
    async fn stage(&mut self, mutations: Vec<kv::Mutation>) -> Result<(), AppError> {
        kv::mutate_many(&self.txn, mutations).await
    }
}

impl TikvWrite<'_> {
    /// 削除でデータ量が減ったリーフを親へ統合し、可能な限り木を圧縮する。
    ///
    /// 統合は親と**その全子**を書き換えるので、判断の前にその集合をまとめてロックする。
    /// 空の子領域もロックの対象に含める（キーが無くてもロックは取れる）。そうしないと、
    /// 統合で消える予定の空領域へ他者が書き込めてしまう。
    ///
    /// 親子関係は降下で判ったもの（`parents`）をそのまま使い、ルートから引き直さない。
    /// 引き直しても同じ `start_ts` を読むので答えは変わらず、下の検証が「降下時と同じか」
    /// を確かめる以上、根拠として過不足がない。
    ///
    /// `settled` は決着のついた領域（畳まれて消えた、あるいは畳めないと判った）を
    /// 呼び出し間で共有する。これが無いと、兄弟ごとに同じ親を調べ直したり、
    /// 自分で消した領域を調べに行って `mark_stale` で無駄にやり直したりする。
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
            // 対応が無いのはルートに達したとき（ルート自身に親はない）。
            let Some((parent_region, descended_children)) = parents.get(&region) else {
                break;
            };
            let parent_region = *parent_region;

            // 親と全子をロックし、ロック時点の内容を得る。子には**このループで既に
            // 畳んだもの**が含まれるので、自分の変更が重なった状態で返る必要がある
            // （`kv::lock_shards` を参照）。
            let mut targets: BTreeSet<FlexId> = descended_children.iter().copied().collect();
            targets.insert(parent_region);
            let locked = kv::lock_shards(&self.txn, table_id, targets).await?;

            // 親が今もポインタノードで、子集合が降下時と同じであることを確かめる。
            // 違っていれば降下が古かったので、新しいスナップショットでやり直す。
            let child_regions = match locked.get(&parent_region) {
                Some(Some(value)) => match ShardEntry::child_pointers(value.entry())? {
                    Some(children) if children == **descended_children => children,
                    _ => return Err(self.mark_stale().into()),
                },
                _ => return Err(self.mark_stale().into()),
            };

            // 子を走査し、いずれかがポインタノードならこのレベルは統合しない。
            // 全リーフで合算が閾値以下なら 1 リーフへ畳み込む。
            let mut combined = 0usize;
            let mut mergeable = true;
            for cr in &child_regions {
                // 空領域のキーはそもそも存在しないのでスキップ。
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
                // この親はもう畳めない。兄弟が同じ判定を繰り返しても答えは変わらない。
                settled.extend(child_regions);
                break;
            }

            // ここから先は重い（子マップの復元・統合・直列化）ので blocking タスクへ出す。
            // 非同期ワーカー上に置くとハートビートを巻き添えにする
            // （リーフの書き換えの節を参照）。
            let span = tracing::Span::current();
            let regions = child_regions.clone();
            let mutations = tokio::task::spawn_blocking(move || {
                span.in_scope(|| merge_children(table_id, index, parent_region, &regions, &locked))
            })
            .await
            .map_err(|e| AppError::InternalError(format!("shard merge task: {e}")))??;

            self.stage(mutations).await?;

            // 子はもう存在しない。兄弟がここから上を辿り直さないよう印を付ける。
            settled.extend(child_regions);

            // 親が新たなリーフになった → さらに上へ伝播。
            region = parent_region;
        }
        Ok(())
    }

    /// 制約変更時に、既存の格納値が新しい制約を満たすか検証する。
    ///
    /// テーブル全体をスナップショットで走査する。データ書き込みはテーブル単位の
    /// 排他を取らないので、これは**ある一点での検証**であり、検証中に走った書き込みは
    /// 対象に入らない。新しい制約はコミット後の書き込みには適用されるので、
    /// すり抜けうるのは「古い制約を読んだうえで検証後にコミットされた書き込み」だけ。
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
            // 全域を覆う範囲で舐める。検証したいのは格納値そのものなので、
            // 葉を作業木へ組み直す必要はない。
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
