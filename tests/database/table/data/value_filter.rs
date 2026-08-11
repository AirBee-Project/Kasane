//! 値インデックス（値フィルタ）のエンドツーエンド検証。
//!
//! 1) 分割（>MAX）を跨いでも等価/範囲フィルタが正しい FlexId を返すこと、
//! 2) 上書き・削除でインデックスが正しく差分維持されること、を確認する。

use std::collections::HashSet;

use kasane::models::database::table::TableDataType;
use kasane::models::id::TableId;
use kasane::repositories::lmdb::initialize_database;
use kasane::repositories::lmdb::{KasaneDbRead, KasaneDbWrite};
use kasane_logic::{SingleId, SpatialIdSet};

/// i32 を `interpret_value` と同じ格納形式（ビッグエンディアン）へ。
fn enc(v: i32) -> Vec<u8> {
    v.to_be_bytes().to_vec()
}

/// FlexId 群を、含まれる SingleId の x 集合へ展開する。
fn xs(flex_ids: &[kasane_logic::FlexId]) -> HashSet<u32> {
    flex_ids
        .iter()
        .flat_map(|f| (*f).single_ids().map(|s| s.x()))
        .collect()
}

#[test]
#[ignore]
fn value_filter_eq_and_range_after_split() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Int;

    // 分割閾値を超える数の FlexId を、各々別の値で挿入する（高カーディナリティ数値）。
    let n: i32 = 5000;
    {
        let wtxn = db.env.write_txn().unwrap();
        let mut w = KasaneDbWrite::new(wtxn, &db);
        for i in 0..n {
            let mut set = SpatialIdSet::new();
            set.insert(SingleId::new(20, 0, (i as u32) * 4, 0).unwrap());
            w.data_insert_impl(table_id, Some(dt), set, &enc(i)).unwrap();
        }
        w.commit().unwrap();
    }

    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);

    let eq = r.data_filter_eq_impl(table_id, dt, &enc(1234)).unwrap();
    assert_eq!(xs(&eq), HashSet::from([1234u32 * 4]));

    // 範囲: 10 <= value <= 20 → 11 FlexId 。順序保存エンコードが効くことを確認。
    let rng = r
        .data_filter_range_impl(table_id, dt, &enc(10), &enc(20))
        .unwrap();
    let expected: HashSet<u32> = (10u32..=20).map(|i| i * 4).collect();
    assert_eq!(xs(&rng), expected);
}

#[test]
fn value_filter_reflects_overwrite_and_remove() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Int;
    let id = SingleId::new(20, 0, 100, 0).unwrap();

    let insert = |value: i32| {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut set = SpatialIdSet::new();
        set.insert(id.clone());
        w.data_insert_impl(table_id, Some(dt), set, &enc(value)).unwrap();
        w.commit().unwrap();
    };

    // 値 7 を挿入 → フィルタで引ける。
    insert(7);
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let eq_7 = r.data_filter_eq_impl(table_id, dt, &enc(7)).unwrap();
        assert_eq!(eq_7.len(), 1);
    }

    // 値 8 で上書き → 旧値 7 は消え、8 が引ける。
    insert(8);
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let eq_7 = r.data_filter_eq_impl(table_id, dt, &enc(7)).unwrap();
        assert!(
            eq_7.is_empty(),
            "overwritten value must be gone from the index"
        );
        let eq_8 = r.data_filter_eq_impl(table_id, dt, &enc(8)).unwrap();
        assert_eq!(eq_8.len(), 1);
    }

    // 削除 → 8 も消える。
    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        let mut set = SpatialIdSet::new();
        set.insert(id.clone());
        w.data_remove_impl(table_id, Some(dt), set).unwrap();
        w.commit().unwrap();
    }
    {
        let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);
        let eq_8 = r.data_filter_eq_impl(table_id, dt, &enc(8)).unwrap();
        assert!(eq_8.is_empty(), "removed value must be gone from the index");
    }
}

/// **可変長（Text）の範囲検索が、境界を絞っても取りこぼさないこと。**
///
/// キーは `table_id ‖ 値 ‖ flexid` と値を可変長のまま連結するので、該当キーは
/// バイト順で連続しない。`vkey` が上限の真の接頭辞になっている行は、続く flexid の
/// バイト次第で `hi ‖ 0xFF…` を超えた位置へ飛ぶ。走査範囲を詰めすぎると、
/// そういう行だけが黙って消える。
///
/// 「上限の接頭辞になる値」と「上限を 1 文字超える値」を両方入れて、
/// 前者が残り後者が落ちることを確かめる。
#[test]
fn text_range_filter_keeps_values_that_prefix_the_upper_bound() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let table_id = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Text;

    // 値と、それを置く FlexId の x 座標。
    let rows: [(&str, u32); 7] = [
        ("a", 1),     // 範囲外（下限未満）
        ("b", 2),     // 該当。"bz" の真の接頭辞 ＝ 取りこぼしやすい行
        ("bm", 3),    // 該当
        ("bz", 4),    // 該当（上限そのもの）
        ("bza", 5),   // 範囲外（上限を超える）
        ("c", 6),     // 範囲外
        ("tokyo", 7), // 範囲外
    ];

    {
        let mut w = KasaneDbWrite::new(db.env.write_txn().unwrap(), &db);
        for (value, x) in rows {
            let mut set = SpatialIdSet::new();
            set.insert(SingleId::new(20, 0, x, 0).unwrap());
            w.data_insert_impl(table_id, Some(dt), set, value.as_bytes())
                .unwrap();
        }
        w.commit().unwrap();
    }

    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);

    // "b" 〜 "bz"（両端含む）。
    let got = r.data_filter_range_impl(table_id, dt, b"b", b"bz").unwrap();
    assert_eq!(
        xs(&got),
        HashSet::from([2u32, 3, 4]),
        "上限の接頭辞になる値を取りこぼしたか、範囲外を拾っている"
    );

    // 共通接頭辞がない範囲でも同じこと。
    let got = r.data_filter_range_impl(table_id, dt, b"a", b"c").unwrap();
    assert_eq!(xs(&got), HashSet::from([1u32, 2, 3, 4, 5, 6]));

    // 単一値の範囲は等価フィルタと一致する。
    let got = r
        .data_filter_range_impl(table_id, dt, b"bm", b"bm")
        .unwrap();
    assert_eq!(xs(&got), HashSet::from([3u32]));

    // 下限が上限より大きい（空範囲）。
    let got = r.data_filter_range_impl(table_id, dt, b"z", b"a").unwrap();
    assert!(xs(&got).is_empty(), "空範囲が何かを返している");
}

/// 値インデックスを維持しないテーブル（`index = None`）では索引キーが一切作られず、
/// それでいてシャード本体の読み書きは変わらないこと。
///
/// 索引キーは格納 `FlexId` 1 件につき 1 つ増えるので、1 回の書き込みが触るキー数――
/// ひいては悲観トランザクションが取るロック数――のほとんどをこれが占める。
/// 既定で無効になっていることが崩れると、そこが黙って元に戻る。
#[test]
fn writes_without_indexing_leave_the_value_index_empty() {
    let tmp = tempfile::TempDir::new().unwrap();
    let db = initialize_database(tmp.path().to_str().unwrap());
    let indexed = TableId(uuid::Uuid::now_v7());
    let plain = TableId(uuid::Uuid::now_v7());
    let dt = TableDataType::Int;

    let ids = |i: u32| {
        let mut set = SpatialIdSet::new();
        set.insert(SingleId::new(20, 0, i, 0).unwrap());
        set
    };

    {
        let wtxn = db.env.write_txn().unwrap();
        let mut w = KasaneDbWrite::new(wtxn, &db);
        for i in 0..64u32 {
            // 同じ内容を、索引ありとなしの 2 つのテーブルへ書く。
            w.data_insert_impl(indexed, Some(dt), ids(i), &enc(i as i32))
                .unwrap();
            w.data_insert_impl(plain, None, ids(i), &enc(i as i32))
                .unwrap();
        }
        w.commit().unwrap();
    }

    let r = KasaneDbRead::new(db.env.read_txn().unwrap(), &db);

    // 索引ありのテーブルは今までどおり引ける。
    let eq = r.data_filter_eq_impl(indexed, dt, &enc(7)).unwrap();
    assert_eq!(xs(&eq), HashSet::from([7u32]));

    // 索引なしのテーブルには索引キーが 1 つも無い。
    let eq = r.data_filter_eq_impl(plain, dt, &enc(7)).unwrap();
    assert!(
        eq.is_empty(),
        "索引を維持しないテーブルに索引キーが作られている"
    );

    // それでも本体は普通に読める（索引の有無はデータの見え方を変えない）。
    let groups = r.data_get_impl(plain, ids(7), None).unwrap();
    let values: Vec<&Vec<u8>> = groups.iter().map(|(value, _)| value).collect();
    assert_eq!(values, vec![&enc(7)], "本体の読み出しが索引の有無に依存している");
}
