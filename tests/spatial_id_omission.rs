//! LMDB バックエンド向けの結合テスト。TiKV バックエンドのビルドでは対象外。
#![cfg(feature = "backend-lmdb")]

use kasane::models::spatial_id::{RawFlexId, RawRangeId, SpatialId};
use kasane::services::helpers::spatial_ids::to_spatial_id_set;

fn range(z: u8, f: Option<[i32; 2]>, x: Option<[u32; 2]>, y: Option<[u32; 2]>) -> SpatialId {
    SpatialId::RangeId(RawRangeId {
        z,
        f,
        x,
        y,
        i: None,
        t: None,
    })
}

fn flex(fz: u8, fi: i32, xz: u8, xi: u32, yz: u8, yi: u32) -> SpatialId {
    SpatialId::FlexId(RawFlexId {
        f_zoomlevel: fz,
        f_index: fi,
        x_zoomlevel: xz,
        x_index: xi,
        y_zoomlevel: yz,
        y_index: yi,
        t_zoomlevel: None,
        t_index: None,
    })
}

#[test]
/// x を省略すると、その軸だけズーム 0 にした FlexId と同じ集合になる。
fn omitting_x_covers_the_whole_axis() {
    let omitted = to_spatial_id_set(&[range(3, Some([0, 0]), None, Some([2, 2]))]).unwrap();
    let explicit = to_spatial_id_set(&[flex(3, 0, 0, 0, 3, 2)]).unwrap();
    assert_eq!(omitted, explicit);
}

#[test]
/// y も同じ。
fn omitting_y_covers_the_whole_axis() {
    let omitted = to_spatial_id_set(&[range(3, Some([0, 0]), Some([2, 2]), None)]).unwrap();
    let explicit = to_spatial_id_set(&[flex(3, 0, 3, 2, 0, 0)]).unwrap();
    assert_eq!(omitted, explicit);
}

#[test]
/// f は符号付き軸なので、全域はズーム 0 の 2 セル（-1 と 0）になる。
fn omitting_f_covers_both_signs() {
    let omitted = to_spatial_id_set(&[range(3, None, Some([2, 2]), Some([2, 2]))]).unwrap();
    let explicit = to_spatial_id_set(&[flex(0, -1, 3, 2, 3, 2), flex(0, 0, 3, 2, 3, 2)]).unwrap();
    assert_eq!(omitted, explicit);
}

#[test]
/// 3 軸すべて省略すると空間全体を表す。
fn omitting_every_axis_covers_everything() {
    let omitted = to_spatial_id_set(&[range(5, None, None, None)]).unwrap();
    let explicit = to_spatial_id_set(&[flex(0, -1, 0, 0, 0, 0), flex(0, 0, 0, 0, 0, 0)]).unwrap();
    assert_eq!(omitted, explicit);
}

#[test]
/// 省略した軸の展開コストがズームレベルに依存しないこと。
fn whole_space_is_zoom_independent() {
    let shallow = to_spatial_id_set(&[range(1, None, None, None)]).unwrap();
    for z in [5u8, 12, 20, 30] {
        let deep = to_spatial_id_set(&[range(z, None, None, None)]).unwrap();
        assert_eq!(deep, shallow, "z={z} で空間全体の表現が変わった");
    }
}

#[test]
/// 明示指定した場合の意味は従来どおり変わらない。
fn explicit_axes_are_unchanged() {
    let explicit =
        to_spatial_id_set(&[range(3, Some([0, 0]), Some([1, 2]), Some([3, 4]))]).unwrap();
    let expected = to_spatial_id_set(&[
        flex(3, 0, 3, 1, 3, 3),
        flex(3, 0, 3, 1, 3, 4),
        flex(3, 0, 3, 2, 3, 3),
        flex(3, 0, 3, 2, 3, 4),
    ])
    .unwrap();
    assert_eq!(explicit, expected);
}
