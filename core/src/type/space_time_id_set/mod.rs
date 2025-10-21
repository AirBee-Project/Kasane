/// 2つのRangeInfo間の3次元重複チェック（静的メソッド）
fn check_overlap_between_infos(info1: &RangeInfo, info2: &RangeInfo) -> bool {
    let max_z = info1.z.max(info2.z);

    let f1 = normalize_range_i64(info1.z, max_z, info1.f);
    let f2 = normalize_range_i64(info2.z, max_z, info2.f);

    let x1 = normalize_range_u64(info1.z, max_z, info1.x);
    let x2 = normalize_range_u64(info2.z, max_z, info2.x);

    let y1 = normalize_range_u64(info1.z, max_z, info1.y);
    let y2 = normalize_range_u64(info2.z, max_z, info2.y);

    ranges_overlap_i64(f1.0, f1.1, f2.0, f2.1)
        && ranges_overlap_u64(x1.0, x1.1, x2.0, x2.1)
        && ranges_overlap_u64(y1.0, y1.1, y2.0, y2.1)
}

/// info1がinfo2を完全に含むかチェック（静的メソッド）
fn info1_contains_info2(info1: &RangeInfo, info2: &RangeInfo) -> bool {
    let max_z = info1.z.max(info2.z);

    let f1 = normalize_range_i64(info1.z, max_z, info1.f);
    let f2 = normalize_range_i64(info2.z, max_z, info2.f);

    let x1 = normalize_range_u64(info1.z, max_z, info1.x);
    let x2 = normalize_range_u64(info2.z, max_z, info2.x);

    let y1 = normalize_range_u64(info1.z, max_z, info1.y);
    let y2 = normalize_range_u64(info2.z, max_z, info2.y);

    f1.0 <= f2.0 && f1.1 >= f2.1 && x1.0 <= x2.0 && x1.1 >= x2.1 && y1.0 <= y2.0 && y1.1 >= y2.1
}

/// 範囲から重複部分を除外した残りの範囲を計算（RangeInfo版）
fn subtract_range_info(base: &RangeInfo, subtract: &RangeInfo) -> Vec<RangeInfo> {
    let mut result = Vec::new();

    // 両方を同じzレベルに正規化
    let max_z = base.z.max(subtract.z);

    let base_f = normalize_range_i64(base.z, max_z, base.f);
    let sub_f = normalize_range_i64(subtract.z, max_z, subtract.f);

    let base_x = normalize_range_u64(base.z, max_z, base.x);
    let sub_x = normalize_range_u64(subtract.z, max_z, subtract.x);

    let base_y = normalize_range_u64(base.z, max_z, base.y);
    let sub_y = normalize_range_u64(subtract.z, max_z, subtract.y);

    // F次元で分割
    if base_f.0 < sub_f.0 {
        result.push(RangeInfo {
            z: max_z,
            f: (base_f.0, sub_f.0 - 1),
            x: base_x,
            y: base_y,
            t: base.t,
        });
    }
    if base_f.1 > sub_f.1 {
        result.push(RangeInfo {
            z: max_z,
            f: (sub_f.1 + 1, base_f.1),
            x: base_x,
            y: base_y,
            t: base.t,
        });
    }

    // F次元の重なり部分でX次元を分割
    let overlap_f = (base_f.0.max(sub_f.0), base_f.1.min(sub_f.1));

    if base_x.0 < sub_x.0 {
        result.push(RangeInfo {
            z: max_z,
            f: overlap_f,
            x: (base_x.0, sub_x.0 - 1),
            y: base_y,
            t: base.t,
        });
    }
    if base_x.1 > sub_x.1 {
        result.push(RangeInfo {
            z: max_z,
            f: overlap_f,
            x: (sub_x.1 + 1, base_x.1),
            y: base_y,
            t: base.t,
        });
    }

    // F, X次元の重なり部分でY次元を分割
    let overlap_x = (base_x.0.max(sub_x.0), base_x.1.min(sub_x.1));

    if base_y.0 < sub_y.0 {
        result.push(RangeInfo {
            z: max_z,
            f: overlap_f,
            x: overlap_x,
            y: (base_y.0, sub_y.0 - 1),
            t: base.t,
        });
    }
    if base_y.1 > sub_y.1 {
        result.push(RangeInfo {
            z: max_z,
            f: overlap_f,
            x: overlap_x,
            y: (sub_y.1 + 1, base_y.1),
            t: base.t,
        });
    }

    result
} // ヘルパー関数

fn normalize_range_i64(from_z: u8, to_z: u8, range: (i64, i64)) -> (i64, i64) {
    if to_z >= from_z {
        let shift = (to_z - from_z) as u32;
        (range.0 << shift, range.1 << shift)
    } else {
        let shift = (from_z - to_z) as u32;
        (range.0 >> shift, range.1 >> shift)
    }
}

fn normalize_range_u64(from_z: u8, to_z: u8, range: (u64, u64)) -> (u64, u64) {
    if to_z >= from_z {
        let shift = (to_z - from_z) as u32;
        (range.0 << shift, range.1 << shift)
    } else {
        let shift = (from_z - to_z) as u32;
        (range.0 >> shift, range.1 >> shift)
    }
}

fn ranges_overlap_i64(a_start: i64, a_end: i64, b_start: i64, b_end: i64) -> bool {
    a_start <= b_end && b_start <= a_end
}

fn ranges_overlap_u64(a_start: u64, a_end: u64, b_start: u64, b_end: u64) -> bool {
    a_start <= b_end && b_start <= a_end
}
use std::collections::HashSet;
use std::ops::Bound::{Excluded, Included, Unbounded};
use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
};

use crate::r#type::space_time_id::SpaceTimeId;

#[derive(Debug, Clone)]
pub struct RangeInfo {
    pub z: u8,
    pub f: (i64, i64),
    pub x: (u64, u64),
    pub y: (u64, u64),
    pub t: (u64, u64),
}

#[derive(Debug)]
pub struct SpaceTimeIdSet {
    // Key：(BitMask, ZoomLevel, OriginalZoomLevel)
    // Value : Set内部のSpaceTimeIdを識別するための一意なID
    // OriginalZoomLevelを追加することで、同じビットマスクでも異なる元zレベルを区別
    pub f: BTreeMap<(Vec<u8>, u8, u8), Vec<u64>>,
    pub x: BTreeMap<(Vec<u8>, u8, u8), Vec<u64>>,
    pub y: BTreeMap<(Vec<u8>, u8, u8), Vec<u64>>,
    pub t: HashMap<u64, (u64, u64)>,
    // ★新規：IDごとの元の範囲情報を保持
    pub range_info: HashMap<u64, RangeInfo>,
    next_id: u64,
}

/// bitmask1 が bitmask2 の prefix（上位に含む）かを判定
fn is_prefix_of(bitmask1: &Vec<u8>, z1: u8, bitmask2: &Vec<u8>, z2: u8) -> bool {
    if z1 >= z2 {
        return false;
    }

    for bit_idx in 0..z1 {
        let byte_idx = (bit_idx / 8) as usize;
        let bit_pos = 7 - (bit_idx % 8);

        if byte_idx >= bitmask1.len() || byte_idx >= bitmask2.len() {
            return false;
        }

        let bit1 = (bitmask1[byte_idx] >> bit_pos) & 1;
        let bit2 = (bitmask2[byte_idx] >> bit_pos) & 1;

        if bit1 != bit2 {
            return false;
        }
    }
    true
}

fn shares_prefix(bitmask1: &Vec<u8>, bitmask2: &Vec<u8>, check_bytes: usize) -> bool {
    for i in 0..check_bytes.min(bitmask1.len()).min(bitmask2.len()) {
        if bitmask1[i] != bitmask2[i] {
            return false;
        }
    }
    true
}

impl SpaceTimeIdSet {
    pub fn new() -> Self {
        Self {
            f: BTreeMap::new(),
            x: BTreeMap::new(),
            y: BTreeMap::new(),
            t: HashMap::new(),
            range_info: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn generate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn find_overlaps(
        &self,
        tree: &BTreeMap<(Vec<u8>, u8, u8), Vec<u64>>,
        bitmasks: &Vec<(Vec<u8>, u8)>,
        original_z: u8,
    ) -> (
        Vec<((Vec<u8>, u8, u8), Vec<u64>)>,
        Vec<((Vec<u8>, u8, u8), Vec<u64>)>,
    ) {
        let mut overlap_top = vec![];
        let mut overlap_bottom = vec![];

        for side in bitmasks {
            // 上位（粗い）IDを検索: 元のzが同じで、より粗いzレベルのもの
            for z in 0..side.1 {
                let key = (take_n_bits(&side.0, z), z, original_z);
                if let Some(v) = tree.get(&key) {
                    overlap_top.push((key, v.clone()));
                }
            }

            // 下位（詳細な）IDを検索: 元のzが同じで、より詳細なzレベルのもの
            let start_key = (side.0.clone(), side.1, original_z);
            let end_key = (vec![255; side.0.len()], u8::MAX, original_z);
            let check_bytes = (side.1 as usize + 7) / 8;

            for (key, ids) in tree.range((Included(start_key), Included(end_key))) {
                // 元のzが異なる場合はスキップ
                if key.2 != original_z {
                    continue;
                }

                if is_prefix_of(&side.0, side.1, &key.0, key.1) {
                    overlap_bottom.push((key.clone(), ids.clone()));
                } else if !shares_prefix(&side.0, &key.0, check_bytes) {
                    break;
                }
            }
        }

        // ★追加：異なるoriginal_zを持つエントリとの重複もチェック
        // すべてのエントリをスキャンして、3次元での重複を確認
        for ((bitmask, z, orig_z), ids) in tree.iter() {
            if *orig_z == original_z {
                continue; // 既にチェック済み
            }

            // このエントリが表す範囲を取得
            for &id in ids {
                if let Some(existing_info) = self.range_info.get(&id) {
                    // ビットマスクベースではなく、実際の範囲で重複判定
                    // ここでは候補としてoverlapに追加
                    // 実際の3次元重複は後でチェック
                    overlap_bottom.push(((bitmask.clone(), *z, *orig_z), ids.clone()));
                    break; // 同じキーは1回だけ追加
                }
            }
        }

        (overlap_top, overlap_bottom)
    }

    /// 2つの範囲が3次元全てで重なるかチェック
    fn check_3d_overlap(&self, id1: u64, id2: u64) -> bool {
        let info1 = &self.range_info[&id1];
        let info2 = &self.range_info[&id2];

        // 両方の範囲を同じzレベルに正規化
        let max_z = info1.z.max(info2.z);

        let f1 = normalize_range_i64(info1.z, max_z, info1.f);
        let f2 = normalize_range_i64(info2.z, max_z, info2.f);

        let x1 = normalize_range_u64(info1.z, max_z, info1.x);
        let x2 = normalize_range_u64(info2.z, max_z, info2.x);

        let y1 = normalize_range_u64(info1.z, max_z, info1.y);
        let y2 = normalize_range_u64(info2.z, max_z, info2.y);

        ranges_overlap_i64(f1.0, f1.1, f2.0, f2.1)
            && ranges_overlap_u64(x1.0, x1.1, x2.0, x2.1)
            && ranges_overlap_u64(y1.0, y1.1, y2.0, y2.1)
    }

    /// id1がid2を完全に含むかチェック
    fn id1_contains_id2(&self, id1: u64, id2: u64) -> bool {
        let info1 = &self.range_info[&id1];
        let info2 = &self.range_info[&id2];

        let max_z = info1.z.max(info2.z);

        let f1 = normalize_range_i64(info1.z, max_z, info1.f);
        let f2 = normalize_range_i64(info2.z, max_z, info2.f);

        let x1 = normalize_range_u64(info1.z, max_z, info1.x);
        let x2 = normalize_range_u64(info2.z, max_z, info2.x);

        let y1 = normalize_range_u64(info1.z, max_z, info1.y);
        let y2 = normalize_range_u64(info2.z, max_z, info2.y);

        f1.0 <= f2.0 && f1.1 >= f2.1 && x1.0 <= x2.0 && x1.1 >= x2.1 && y1.0 <= y2.0 && y1.1 >= y2.1
    }

    /// 範囲から重複部分を除外した残りの範囲を計算
    fn subtract_range(base: SpaceTimeId, subtract: &RangeInfo) -> Vec<SpaceTimeId> {
        let mut result = Vec::new();

        // 両方を同じzレベルに正規化
        let max_z = base.z.max(subtract.z);

        let base_f = normalize_range_i64(base.z, max_z, base.f);
        let sub_f = normalize_range_i64(subtract.z, max_z, subtract.f);

        let base_x = normalize_range_u64(base.z, max_z, base.x);
        let sub_x = normalize_range_u64(subtract.z, max_z, subtract.x);

        let base_y = normalize_range_u64(base.z, max_z, base.y);
        let sub_y = normalize_range_u64(subtract.z, max_z, subtract.y);

        // F次元で分割
        if base_f.0 < sub_f.0 {
            // 左側の残り
            result.push(SpaceTimeId {
                z: max_z,
                f: (base_f.0, sub_f.0 - 1),
                x: base_x,
                y: base_y,
                t: base.t,
            });
        }
        if base_f.1 > sub_f.1 {
            // 右側の残り
            result.push(SpaceTimeId {
                z: max_z,
                f: (sub_f.1 + 1, base_f.1),
                x: base_x,
                y: base_y,
                t: base.t,
            });
        }

        // F次元の重なり部分でX次元を分割
        let overlap_f = (base_f.0.max(sub_f.0), base_f.1.min(sub_f.1));

        if base_x.0 < sub_x.0 {
            result.push(SpaceTimeId {
                z: max_z,
                f: overlap_f,
                x: (base_x.0, sub_x.0 - 1),
                y: base_y,
                t: base.t,
            });
        }
        if base_x.1 > sub_x.1 {
            result.push(SpaceTimeId {
                z: max_z,
                f: overlap_f,
                x: (sub_x.1 + 1, base_x.1),
                y: base_y,
                t: base.t,
            });
        }

        // F, X次元の重なり部分でY次元を分割
        let overlap_x = (base_x.0.max(sub_x.0), base_x.1.min(sub_x.1));

        if base_y.0 < sub_y.0 {
            result.push(SpaceTimeId {
                z: max_z,
                f: overlap_f,
                x: overlap_x,
                y: (base_y.0, sub_y.0 - 1),
                t: base.t,
            });
        }
        if base_y.1 > sub_y.1 {
            result.push(SpaceTimeId {
                z: max_z,
                f: overlap_f,
                x: overlap_x,
                y: (sub_y.1 + 1, base_y.1),
                t: base.t,
            });
        }

        result
    }

    pub fn insert(&mut self, id: SpaceTimeId) {
        //println!("\n=== INSERT: {} ===", id);

        let converted_f = convert_top_f(id.z, id.f);
        let mut bitmasks_f = convert_bitmask_f_multiple(&converted_f);
        //println!("F converted: {:?}", converted_f);

        let converted_x = convert_top_xy(id.z, id.x);
        let mut bitmasks_x = convert_bitmask_xy_multiple(&converted_x);
        //println!("X converted: {:?}", converted_x);

        let converted_y = convert_top_xy(id.z, id.y);
        let mut bitmasks_y = convert_bitmask_xy_multiple(&converted_y);
        //println!("Y converted: {:?}", converted_y);

        let btree_id = self.generate_id();

        // ステップ1: 同じoriginal_z内でBTreeMapを使って高速検索
        let (overlap_top_f, overlap_bottom_f) = self.find_overlaps(&self.f, &bitmasks_f, id.z);
        let (overlap_top_x, overlap_bottom_x) = self.find_overlaps(&self.x, &bitmasks_x, id.z);
        let (overlap_top_y, overlap_bottom_y) = self.find_overlaps(&self.y, &bitmasks_y, id.z);

        // 同じzレベル内の候補を収集
        let mut candidate_ids = HashSet::new();

        for (_, ids) in overlap_top_f.iter().chain(overlap_bottom_f.iter()) {
            candidate_ids.extend(ids);
        }
        for (_, ids) in overlap_top_x.iter().chain(overlap_bottom_x.iter()) {
            candidate_ids.extend(ids);
        }
        for (_, ids) in overlap_top_y.iter().chain(overlap_bottom_y.iter()) {
            candidate_ids.extend(ids);
        }

        //println!("Same-z candidates: {} IDs", candidate_ids.len());

        // ステップ2: 異なるoriginal_zの範囲も候補に追加（範囲でフィルタ）
        let new_info = RangeInfo {
            z: id.z,
            f: id.f,
            x: id.x,
            y: id.y,
            t: id.t,
        };

        for (&existing_id, existing_info) in &self.range_info {
            if existing_info.z != id.z {
                // 異なるzレベル：ざっくり範囲チェックで候補を絞る
                if Self::might_overlap_rough(&new_info, existing_info) {
                    candidate_ids.insert(existing_id);
                }
            }
        }

        // println!(
        //     "Total candidates (including cross-z): {} IDs",
        //     candidate_ids.len()
        // );

        // ステップ3: 候補に対してのみ3次元重複チェック
        let mut ids_to_delete = Vec::new();
        let mut overlapping_existing_ids = Vec::new();

        for &existing_id in &candidate_ids {
            if let Some(existing_info) = self.range_info.get(&existing_id) {
                if check_overlap_between_infos(&new_info, existing_info) {
                    //println!("Found overlap with existing ID {}", existing_id);

                    if info1_contains_info2(&new_info, existing_info) {
                        // println!(
                        //     "  New range contains existing - will delete ID {}",
                        //     existing_id
                        // );
                        ids_to_delete.push(existing_id);
                    } else if info1_contains_info2(existing_info, &new_info) {
                        //println!("  Existing range contains new - will split new range");
                        overlapping_existing_ids.push(existing_id);
                    } else {
                        //println!("  Partial overlap - will split new range");
                        overlapping_existing_ids.push(existing_id);
                    }
                }
            }
        }

        // 既存IDを削除
        for del_id in ids_to_delete {
            //println!("Deleting ID {}", del_id);
            self.remove_id(del_id);
        }

        // 重複する既存IDがある場合、新しい範囲を分割
        if !overlapping_existing_ids.is_empty() {
            // println!(
            //     "Splitting new range to avoid {} overlapping ranges",
            //     overlapping_existing_ids.len()
            // );

            let mut remaining_ranges = vec![id];

            for existing_id in overlapping_existing_ids {
                let existing_info = &self.range_info[&existing_id].clone();
                let mut next_remaining = Vec::new();

                for range in remaining_ranges {
                    let range_info = RangeInfo {
                        z: range.z,
                        f: range.f,
                        x: range.x,
                        y: range.y,
                        t: range.t,
                    };
                    let subtracted = subtract_range_info(&range_info, existing_info);

                    for sub in subtracted {
                        next_remaining.push(SpaceTimeId {
                            z: sub.z,
                            f: sub.f,
                            x: sub.x,
                            y: sub.y,
                            t: sub.t,
                        });
                    }
                }

                remaining_ranges = next_remaining;
            }

            // println!(
            //     "Split into {} non-overlapping ranges",
            //     remaining_ranges.len()
            // );

            for split_range in remaining_ranges {
                if split_range.f.0 <= split_range.f.1
                    && split_range.x.0 <= split_range.x.1
                    && split_range.y.0 <= split_range.y.1
                {
                    //println!("  Inserting split range: {}", split_range);
                    self.insert(split_range);
                }
            }

            return;
        }

        self.uncheck_insert(bitmasks_f, bitmasks_x, bitmasks_y, btree_id, id);
    }

    /// 粗い範囲チェック（異なるzレベル間の候補絞り込み用）
    /// 正規化せずに、明らかに重ならない場合だけfalseを返す
    fn might_overlap_rough(info1: &RangeInfo, info2: &RangeInfo) -> bool {
        // より粗いzレベルの範囲を計算
        let (coarse_z, coarse_info, fine_info) = if info1.z < info2.z {
            (info1.z, info1, info2)
        } else {
            (info2.z, info2, info1)
        };

        // fine_infoをcoarse_zに変換して比較
        let z_diff = fine_info.z - coarse_z;
        let shift = 1i64 << z_diff;
        let shift_u = 1u64 << z_diff;

        let fine_f_coarse = (fine_info.f.0 >> z_diff, fine_info.f.1 >> z_diff);
        let fine_x_coarse = (fine_info.x.0 >> z_diff, fine_info.x.1 >> z_diff);
        let fine_y_coarse = (fine_info.y.0 >> z_diff, fine_info.y.1 >> z_diff);

        // 粗いレベルで重なりをチェック
        ranges_overlap_i64(
            coarse_info.f.0,
            coarse_info.f.1,
            fine_f_coarse.0,
            fine_f_coarse.1,
        ) && ranges_overlap_u64(
            coarse_info.x.0,
            coarse_info.x.1,
            fine_x_coarse.0,
            fine_x_coarse.1,
        ) && ranges_overlap_u64(
            coarse_info.y.0,
            coarse_info.y.1,
            fine_y_coarse.0,
            fine_y_coarse.1,
        )
    }

    /// IDをBTreeMapとrange_infoから完全に削除
    fn remove_id(&mut self, id: u64) {
        self.f.retain(|_, ids| {
            ids.retain(|&i| i != id);
            !ids.is_empty()
        });
        self.x.retain(|_, ids| {
            ids.retain(|&i| i != id);
            !ids.is_empty()
        });
        self.y.retain(|_, ids| {
            ids.retain(|&i| i != id);
            !ids.is_empty()
        });
        self.range_info.remove(&id);
        self.t.remove(&id);
    }

    pub fn uncheck_insert(
        &mut self,
        f: Vec<(Vec<u8>, u8)>,
        x: Vec<(Vec<u8>, u8)>,
        y: Vec<(Vec<u8>, u8)>,
        id: u64,
        original: SpaceTimeId,
    ) {
        for f_side in f {
            let key = (f_side.0, f_side.1, original.z);
            self.f.entry(key).or_insert_with(Vec::new).push(id);
        }

        for x_side in x {
            let key = (x_side.0, x_side.1, original.z);
            self.x.entry(key).or_insert_with(Vec::new).push(id);
        }

        for y_side in y {
            let key = (y_side.0, y_side.1, original.z);
            self.y.entry(key).or_insert_with(Vec::new).push(id);
        }

        self.t.insert(id, original.t);

        // ★重要：元の範囲情報を保存
        self.range_info.insert(
            id,
            RangeInfo {
                z: original.z,
                f: original.f,
                x: original.x,
                y: original.y,
                t: original.t,
            },
        );
    }

    pub fn get_all(&self) -> Vec<SpaceTimeId> {
        // range_infoから直接復元
        self.range_info
            .values()
            .map(|info| SpaceTimeId {
                z: info.z,
                f: info.f,
                x: info.x,
                y: info.y,
                t: info.t,
            })
            .collect()
    }
}

fn build_map(v: &Vec<((Vec<u8>, u8, u8), Vec<u64>)>) -> HashMap<u64, (Vec<u8>, u8, u8)> {
    let mut map = HashMap::new();
    for ((k, z, orig_z), vals) in v {
        for &u in vals {
            map.insert(u, (k.clone(), *z, *orig_z));
        }
    }
    map
}

fn take_n_bits(data: &[u8], n: u8) -> Vec<u8> {
    let mut result = Vec::new();
    let mut bits_collected = 0u8;
    let mut bits_in_buffer = 0u8;

    for &byte in data {
        for bit_index in (0..8).rev() {
            let bit = (byte >> bit_index) & 1;
            bits_collected = (bits_collected << 1) | bit as u8;
            bits_in_buffer += 1;

            if bits_in_buffer == 8 {
                result.push(bits_collected);
                bits_collected = 0;
                bits_in_buffer = 0;
            }

            if (result.len() * 8) as u8 + bits_in_buffer == n {
                if bits_in_buffer > 0 {
                    bits_collected <<= 8 - bits_in_buffer;
                    result.push(bits_collected);
                }
                return result;
            }
        }
    }

    if bits_in_buffer > 0 {
        bits_collected <<= 8 - bits_in_buffer;
        result.push(bits_collected);
    }

    result
}

pub fn convert_bitmask_f_multiple(inputs: &Vec<(u8, i64)>) -> Vec<(Vec<u8>, u8)> {
    inputs
        .iter()
        .map(|(z, f)| convert_bitmask_f(*z, *f))
        .collect()
}

pub fn convert_bitmask_xy_multiple(inputs: &Vec<(u8, u64)>) -> Vec<(Vec<u8>, u8)> {
    inputs
        .iter()
        .map(|(z, x)| convert_bitmask_xy(*z, *x))
        .collect()
}

pub fn convert_bitmask_xy(z: u8, x: u64) -> (Vec<u8>, u8) {
    assert!(z >= 1 && z <= 64, "z must be between 1 and 64");
    let mut result: Vec<u8> = vec![0; ((z as usize + 7) / 8)];
    for k in 0..z {
        if (x >> k) & 1 != 0 {
            let bit_pos = k;
            let byte_index = (bit_pos / 8) as usize;
            let bit_index = 7 - ((bit_pos % 8) as usize);
            result[byte_index] |= 1 << bit_index;
        }
    }
    (result, z)
}

pub fn invert_bitmask_xy(z: u8, bitmask: &Vec<u8>) -> u64 {
    assert!(z >= 1 && z <= 64, "z must be between 1 and 64");
    let mut value: u64 = 0;
    for k in 0..z {
        let bit_pos = k;
        let byte_index = (bit_pos / 8) as usize;
        let bit_index = 7 - ((bit_pos % 8) as usize);
        let bit = (bitmask[byte_index] >> bit_index) & 1;
        value |= (bit as u64) << k;
    }
    value
}

pub fn convert_bitmask_f(z: u8, f: i64) -> (Vec<u8>, u8) {
    assert!(z >= 1 && z <= 64, "z must be between 1 and 64");
    let mut result: Vec<u8> = vec![0; ((z as usize + 7) / 8)];
    let sign_bit = if f < 0 { 1 } else { 0 };
    let mut abs_f = f.abs() as u64;
    if sign_bit != 0 {
        result[0] |= 1 << 7;
    }
    for k in 0..(z - 1) {
        if abs_f & 1 != 0 {
            let bit_pos = k + 1;
            let byte_index = bit_pos / 8;
            let bit_index = 7 - (bit_pos % 8);
            result[byte_index as usize] |= 1 << bit_index;
        }
        abs_f >>= 1;
    }
    (result, z)
}

pub fn invert_bitmask_f(z: u8, bitmask: &Vec<u8>) -> i64 {
    assert!(z >= 1 && z <= 64, "z must be between 1 and 64");
    let is_negative = (bitmask[0] >> 7) & 1 != 0;
    let mut abs_f: u64 = 0;
    for k in 0..(z - 1) {
        let bit_pos = k + 1;
        let byte_index = (bit_pos / 8) as usize;
        let bit_index = 7 - (bit_pos % 8);
        let bit = (bitmask[byte_index] >> bit_index) & 1;
        abs_f |= (bit as u64) << k;
    }
    if is_negative {
        -(abs_f as i64)
    } else {
        abs_f as i64
    }
}

pub fn convert_top_f(z: u8, dim: (i64, i64)) -> Vec<(u8, i64)> {
    let (mut start, mut end) = dim;
    let mut result = Vec::new();
    let mut current_z = z;

    while start <= end {
        if start > end {
            break;
        }
        if start % 2 == 1 {
            result.push((current_z, start));
            start += 1;
            continue;
        }
        if end > start && end % 2 == 0 {
            result.push((current_z, end));
            end -= 1;
            continue;
        }
        if start == end {
            result.push((current_z, start));
            break;
        }
        if current_z > 0 {
            start /= 2;
            end /= 2;
            current_z -= 1;
        } else {
            while start <= end {
                result.push((current_z, start));
                start += 1;
            }
            break;
        }
    }
    result
}

pub fn convert_top_xy(z: u8, dim: (u64, u64)) -> Vec<(u8, u64)> {
    let (mut start, mut end) = dim;
    let mut result = Vec::new();
    let mut current_z = z;

    while start <= end {
        if start > end {
            break;
        }
        if start % 2 == 1 {
            result.push((current_z, start));
            start += 1;
            continue;
        }
        if end > start && end % 2 == 0 {
            result.push((current_z, end));
            end -= 1;
            continue;
        }
        if start == end {
            result.push((current_z, start));
            break;
        }
        if current_z > 0 {
            start /= 2;
            end /= 2;
            current_z -= 1;
        } else {
            while start <= end {
                result.push((current_z, start));
                start += 1;
            }
            break;
        }
    }
    result
}
