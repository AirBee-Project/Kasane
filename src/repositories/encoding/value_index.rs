//! 値インデックスのキー組み立てと順序保存エンコード。

use crate::error::{AppError, Stored};
use crate::models::{database::table::TableDataType, id::TableId};
use kasane_logic::FlexId;

use super::UUID_LEN;

/// `Int` だけ符号ビットを反転するのは、i64 BE のままだと負の値が後ろに並ぶため。
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

/// `table_id ‖ vkey ‖ flexid`
pub fn make_key(table_id: TableId, vkey: &[u8], flexid: &FlexId) -> Vec<u8> {
    let encoded = flexid.encode();
    let mut key = Vec::with_capacity(UUID_LEN + vkey.len() + FlexId::ENCODED_LEN);
    key.extend_from_slice(&table_id.into_bytes());
    key.extend_from_slice(vkey);
    key.extend_from_slice(&encoded);
    key
}

/// `table_id ‖ vkey`
pub fn make_prefix(table_id: TableId, vkey: &[u8]) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(UUID_LEN + vkey.len());
    prefix.extend_from_slice(&table_id.into_bytes());
    prefix.extend_from_slice(vkey);
    prefix
}

pub fn flexid_from_key(key: &[u8]) -> Result<FlexId, AppError> {
    if key.len() < UUID_LEN + FlexId::ENCODED_LEN {
        return Err(AppError::corrupt(Stored::ValueIndex, "key is too short"));
    }
    let mut bytes = [0u8; FlexId::ENCODED_LEN];
    bytes.copy_from_slice(&key[key.len() - FlexId::ENCODED_LEN..]);
    FlexId::decode(&bytes).map_err(|e| AppError::corrupt(Stored::ValueIndex, e))
}

/// `lo_vkey` 〜 `hi_vkey`（両端含む）を覆う `(下限（含む）, 上限（排他）)`。
///
/// **該当キーを必ず含むが、含まないキーも混じる。** 呼び出し側は [`vkey_from_key`] で
/// 取り出した値で絞り直すこと。
///
/// 厳密にできないのは、キーが `vkey` を可変長のまま連結していて該当キーの集合が
/// バイト順で連続しないため。`hi = "bz"` に対する `vkey = "b"` の行は、続く flexid
/// 次第で `vkey = "bza"` の行より上に来る。
///
/// 上限は `lo` と `hi` の共通接頭辞 `c` の次の 1 バイトで決まる。ただし `lo == c` の
/// ときは `vkey == c` 自身が該当して `c ‖ 0xFF…` まで上がるので、1 バイト緩めて `c` で
/// 止める。詰めすぎると取りこぼす。
pub fn range_scan_bounds(
    table_id: TableId,
    lo_vkey: &[u8],
    hi_vkey: &[u8],
) -> (Vec<u8>, Option<Vec<u8>>) {
    // 下限は厳密。`vkey ≥ lo` なるキーは必ずこれ以上になる。
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

/// 順序保存エンコード済みの値（`vkey`）部分を取り出す。
///
/// [`range_scan_bounds`] の境界だけでは「先頭が一致しただけの別の値」を落としきれない
/// ので、呼び出し側はこれを境界と直接比べて絞り直す。
pub fn vkey_from_key(key: &[u8]) -> Result<&[u8], AppError> {
    if key.len() < UUID_LEN + FlexId::ENCODED_LEN {
        return Err(AppError::corrupt(Stored::ValueIndex, "key is too short"));
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
        // 最大のキーまで弾けるとは限らない（だから後段の厳密フィルタが要る）。
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
        // vkey="" の行が 0xFF まで上がるので上限はテーブル全体になる。正しい退化。
        let (start, end) = range_scan_bounds(tid(), b"", b"z");
        assert_eq!(start, make_prefix(tid(), b""));
        assert_eq!(end, super::super::prefix_end(&make_prefix(tid(), b"")));
    }
}
