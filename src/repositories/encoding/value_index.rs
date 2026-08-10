use crate::error::AppError;
use crate::models::{database::table::TableDataType, id::TableId};
use kasane_logic::FlexId;

use super::UUID_LEN;

/// 格納バイト列を「バイト辞書順＝値の自然順」になるよう変換する。
///
/// 値の格納形式（`interpret_value` 準拠）：
/// - `Int`   : i64 ビッグエンディアン → 符号ビット反転（負が先）
/// - `Text`  : UTF-8（辞書順そのまま）
/// - `Boolean`: 1 バイト 0/1（そのまま）
pub fn order_preserving(data_type: TableDataType, value: &[u8]) -> Vec<u8> {
    let mut key = value.to_vec();
    match data_type {
        TableDataType::Int => {
            if let Some(b0) = key.first_mut() {
                *b0 ^= 0x80;
            }
        }
        TableDataType::Text
        | TableDataType::Boolean
        | TableDataType::Enum
        | TableDataType::Presence => {}
    }
    key
}

/// インデックスキー `table_id ‖ vkey ‖ flexid` を組み立てる。
pub fn make_key(table_id: TableId, vkey: &[u8], flexid: &FlexId) -> Vec<u8> {
    let encoded = flexid.encode();
    let mut key = Vec::with_capacity(UUID_LEN + vkey.len() + FlexId::ENCODED_LEN);
    key.extend_from_slice(&table_id.into_bytes());
    key.extend_from_slice(vkey);
    key.extend_from_slice(&encoded);
    key
}

/// `table_id ‖ vkey` のプレフィックス（等価スキャン用）。
pub fn make_prefix(table_id: TableId, vkey: &[u8]) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(UUID_LEN + vkey.len());
    prefix.extend_from_slice(&table_id.into_bytes());
    prefix.extend_from_slice(vkey);
    prefix
}

/// インデックスキー末尾 [`FlexId::ENCODED_LEN`] バイトから [`FlexId`] を復元する。
pub fn flexid_from_key(key: &[u8]) -> Result<FlexId, AppError> {
    if key.len() < UUID_LEN + FlexId::ENCODED_LEN {
        return Err(AppError::InternalError(
            "value_index key too short".to_string(),
        ));
    }
    let mut bytes = [0u8; FlexId::ENCODED_LEN];
    bytes.copy_from_slice(&key[key.len() - FlexId::ENCODED_LEN..]);
    FlexId::decode(&bytes).map_err(|e| AppError::InternalError(format!("flex_id decode: {e}")))
}

/// `lo_vkey` 〜 `hi_vkey`（両端含む）を引くために走査すべきキー範囲。
///
/// 返るのは `table_id ‖ …` の並びに対する `(下限（含む）, 上限（排他）)` で、
/// 上限が `None` なら終端まで。**該当キーを必ず含むが、含まないキーも混じる。**
/// 呼び出し側は [`vkey_from_key`] で取り出した値で必ず絞り直すこと。
///
/// # なぜ「過不足なし」にできないのか
///
/// キーは `table_id ‖ vkey ‖ flexid` と値を可変長のまま連結しているので、
/// 該当キーの集合は**バイト順で連続しない**。`vkey` が `hi` の真の接頭辞である行は、
/// 続く flexid のバイト次第で任意に高い位置へ飛ぶためである。
///
/// ```text
/// hi = "bz" のとき
///   該当  : vkey="b"   -> ..."b", 0xFF, ...    flexid 次第でここまで上がる
///   非該当: vkey="bza" -> ..."b","z","a", ...  それより下に来る
/// ```
///
/// 該当行が非該当行より上に来るので、どんな単一レンジも厳密にはならない。
/// そこで「必ず覆う最小の範囲」を返し、厳密な判定は後段のフィルタに任せる。
///
/// # 上限の求め方
///
/// `lo ≤ vkey ≤ hi` なら `vkey` は必ず `lo` と `hi` の共通接頭辞 `c` で始まる
/// （`c` の途中で違えば `lo` 未満か `hi` 超過になる）。よって `c` の次の 1 バイトまで
/// 見れば上限が決まる……のだが、`lo` が `hi` の接頭辞のとき（`lo == c`）だけは
/// `vkey == c` 自身が該当し、そのキーが `c ‖ 0xFF…` まで上がる。この場合は
/// 1 バイト緩めて `c` で止める必要がある。詰めすぎると取りこぼす。
pub fn range_scan_bounds(
    table_id: TableId,
    lo_vkey: &[u8],
    hi_vkey: &[u8],
) -> (Vec<u8>, Option<Vec<u8>>) {
    // 下限は `lo` そのもので厳密。`vkey ≥ lo` なるキーは必ずこれ以上になる。
    let start = make_prefix(table_id, lo_vkey);

    let common = lo_vkey
        .iter()
        .zip(hi_vkey)
        .take_while(|(a, b)| a == b)
        .count();
    let keep = if lo_vkey.len() == common {
        common
    } else {
        common + 1
    };
    let end = super::prefix_end(&make_prefix(table_id, &hi_vkey[..keep.min(hi_vkey.len())]));

    (start, end)
}

/// インデックスキーから、順序保存エンコード済みの値（`vkey`）部分を取り出す。
///
/// 値は可変長のまま連結されているため、範囲スキャンの境界だけでは
/// 「先頭が一致しただけの別の値」を落としきれない。取り出した `vkey` を
/// 境界と直接比べることで、型の幅によらず正確に絞り込める。
pub fn vkey_from_key(key: &[u8]) -> Result<&[u8], AppError> {
    if key.len() < UUID_LEN + FlexId::ENCODED_LEN {
        return Err(AppError::InternalError(
            "value_index key too short".to_string(),
        ));
    }
    Ok(&key[UUID_LEN..key.len() - FlexId::ENCODED_LEN])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tid() -> TableId {
        TableId(uuid::Uuid::from_bytes([7u8; UUID_LEN]))
    }

    /// 走査範囲に入るか。`None` の上限は「終端まで」を意味する。
    fn covered(bounds: &(Vec<u8>, Option<Vec<u8>>), key: &[u8]) -> bool {
        let above_start = key >= bounds.0.as_slice();
        let below_end = match &bounds.1 {
            Some(end) => key < end.as_slice(),
            None => true,
        };
        above_start && below_end
    }

    /// `vkey` の行が取りうる**最大**のキー（flexid が全部 0xFF のとき）。
    fn highest_key(vkey: &[u8]) -> Vec<u8> {
        let mut key = make_prefix(tid(), vkey);
        key.extend_from_slice(&[0xFF; FlexId::ENCODED_LEN]);
        key
    }

    /// `vkey` の行が取りうる**最小**のキー（flexid が全部 0x00 のとき）。
    fn lowest_key(vkey: &[u8]) -> Vec<u8> {
        let mut key = make_prefix(tid(), vkey);
        key.extend_from_slice(&[0x00; FlexId::ENCODED_LEN]);
        key
    }

    /// 該当する `vkey` の行は、flexid が何であっても必ず走査範囲に入ること。
    ///
    /// ここを外すと、範囲検索が黙って行を取りこぼす。
    #[track_caller]
    fn assert_covers(lo: &[u8], hi: &[u8], vkey: &[u8]) {
        let bounds = range_scan_bounds(tid(), lo, hi);
        for key in [lowest_key(vkey), highest_key(vkey)] {
            assert!(
                covered(&bounds, &key),
                "lo={lo:?} hi={hi:?} の範囲が vkey={vkey:?} を取りこぼした"
            );
        }
    }

    #[test]
    fn covers_every_qualifying_value() {
        // 共通接頭辞あり。
        assert_covers(b"tokyo-a", b"tokyo-z", b"tokyo-a");
        assert_covers(b"tokyo-a", b"tokyo-z", b"tokyo-m");
        assert_covers(b"tokyo-a", b"tokyo-z", b"tokyo-z");
        assert_covers(b"tokyo-a", b"tokyo-z", b"tokyo-mm");

        // lo が hi の接頭辞。vkey == lo 自身のキーが 0xFF まで上がる場合を含む。
        assert_covers(b"b", b"bz", b"b");
        assert_covers(b"b", b"bz", b"bm");
        assert_covers(b"b", b"bz", b"bz");

        // 共通接頭辞なし。
        assert_covers(b"a", b"z", b"a");
        assert_covers(b"a", b"z", b"m");
        assert_covers(b"a", b"z", b"z");
        assert_covers(b"a", b"z", b"mmmm");

        // 単一値（lo == hi）。
        assert_covers(b"x", b"x", b"x");

        // 下限が空 = 上限以下すべて。
        assert_covers(b"", b"z", b"");
        assert_covers(b"", b"z", b"z");

        // 固定長（Int の順序保存エンコード相当）。
        let lo = order_preserving(TableDataType::Int, &10i64.to_be_bytes());
        let hi = order_preserving(TableDataType::Int, &20i64.to_be_bytes());
        for v in [10i64, 15, 20] {
            let vkey = order_preserving(TableDataType::Int, &v.to_be_bytes());
            assert_covers(&lo, &hi, &vkey);
        }
    }

    #[test]
    fn excludes_values_outside_the_range() {
        // 範囲外の値は、少なくとも「最小のキー」では弾けていること
        // （最大のキーまで弾けるとは限らない ＝ だから後段の厳密フィルタが要る）。
        let bounds = range_scan_bounds(tid(), b"tokyo-a", b"tokyo-z");
        assert!(!covered(&bounds, &lowest_key(b"osaka")));
        assert!(!covered(&bounds, &lowest_key(b"yokohama")));
        assert!(!covered(&bounds, &highest_key(b"yokohama")));
    }

    #[test]
    fn narrows_the_scan_well_below_the_whole_table() {
        // 共通接頭辞があるときは、その接頭辞のぶんだけで止まる。
        let (start, end) = range_scan_bounds(tid(), b"tokyo-a", b"tokyo-z");
        assert_eq!(start, make_prefix(tid(), b"tokyo-a"));
        assert_eq!(end, Some(make_prefix(tid(), b"tokyo-{")));

        // lo が hi の接頭辞なら 1 バイト緩める（`c ‖ 0xFF…` を覆うため）。
        let (_, end) = range_scan_bounds(tid(), b"b", b"bz");
        assert_eq!(end, Some(make_prefix(tid(), b"c")));

        // 共通接頭辞が無くても、先頭 1 バイトぶんは絞れる。
        let (_, end) = range_scan_bounds(tid(), b"a", b"z");
        assert_eq!(end, Some(make_prefix(tid(), b"{")));
    }

    #[test]
    fn an_empty_lower_bound_still_produces_a_usable_range() {
        // vkey="" の行が 0xFF まで上がるので、上限はテーブル全体になる。
        // 取りこぼさないことが優先で、これは正しい退化。
        let (start, end) = range_scan_bounds(tid(), b"", b"z");
        assert_eq!(start, make_prefix(tid(), b""));
        assert_eq!(end, super::super::prefix_end(&make_prefix(tid(), b"")));
    }
}
