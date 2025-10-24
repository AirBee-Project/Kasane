use crate::r#type::bit_vec::BitVec;

pub fn convert_xy(z: u8, dim: (u64, u64)) -> Vec<(u8, u64)> {
    let mut current_range = Some(dim);
    let mut now_z = z;
    let mut result = Vec::new();

    while let Some(mut target) = current_range {
        if target.0 == target.1 {
            // 終端 → これ以上分割しない
            result.push((now_z, target.0));
            break;
        }

        // 左端が奇数なら個別に処理
        if target.0 % 2 != 0 {
            result.push((now_z, target.0));
            target.0 += 1;
        }

        // 右端が偶数なら個別に処理
        if target.1 % 2 == 0 {
            result.push((now_z, target.1));
            target.1 -= 1;
        }

        // 範囲が逆転したら終了
        if target.0 > target.1 {
            break;
        }

        // 次のズームレベルへ（範囲を半分に縮小）
        current_range = Some((target.0 / 2, target.1 / 2));
        if now_z == 0 {
            break; // z=0で終了
        }
        now_z -= 1;
    }

    result
}
pub fn convert_bitmask_xy(z: u8, mut xy: u64) -> (BitVec, u8) {
    if z == 0 {
        // 階層がない場合でも最初の層は無条件で10

        return (BitVec::from_vec(vec![0b10_000000]), 0);
    }

    // 必要バイト数: 1層2ビット × z, 切り上げ

    let mut result = BitVec::from_vec(vec![0; ((z as usize) * 2 + 7) / 8]);

    // 最初の層は無条件で10
    result[0] |= 1 << 7; // flag_bit = 1
                         // value_bit = 0 はすでに0なので不要

    // 残りの階層（i=1からスタート）
    for i in 1..z {
        let flag_bit = 1;
        let value_bit = (xy % 2) as u8;
        xy /= 2;

        let bit_pos = i * 2;
        let byte_index = (bit_pos / 8) as usize;
        let bit_index = 7 - (bit_pos % 8) as usize;

        result[byte_index] |= flag_bit << bit_index;
        result[byte_index] |= value_bit << (bit_index - 1);
    }

    println!("Convert BitMask XY Z:{} XY:{} Result : {}", z, xy, result);

    (result, z)
}

pub fn invert_bitmask_xy(bitmask: &BitVec) -> (u8, u64) {
    if bitmask.is_empty() {
        return (0, 0);
    }

    let mut z = 1; // 最初の層は無条件で10
    let mut xy = 0u64;

    let total_bits = bitmask.len() * 8;

    let mut bit_pos = 2; // 0,1 は最初の層

    while bit_pos + 1 < total_bits && z < 64 {
        let byte_index = bit_pos / 8;
        let bit_index = 7 - (bit_pos % 8);

        let flag_bit = (bitmask[byte_index] >> bit_index) & 1;
        let value_bit = (bitmask[byte_index] >> (bit_index - 1)) & 1;

        if flag_bit == 0 {
            break; // 階層無効なら終了
        }

        // xy に右詰めで格納
        xy |= (value_bit as u64) << (z - 1);

        z += 1;
        bit_pos += 2; // 1層2ビットなので次の層へ
    }

    (z, xy)
}

pub fn convert_bitmask_xy_multiple(inputs: &Vec<(u8, u64)>) -> Vec<(BitVec, u8)> {
    inputs
        .iter()
        .map(|(z, x)| convert_bitmask_xy(*z, *x))
        .collect()
}
