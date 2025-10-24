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
pub fn convert_bitmask_xy(z: u8, xy: u64) -> (BitVec, u8) {
    let length = (((z + 1) * 2 / 8) + 1).max(1) as usize;
    let mut result = vec![0u8; length];

    let mut temp_xy = xy;

    // 階層 z から 1 まで処理（階層0は後で）
    for now_z in (1..=z).rev() {
        let index = ((now_z) * 2 / 8) as usize;
        let in_index = now_z % 4;

        // 有効ビット
        result[index] |= 1 << (7 - in_index * 2);

        // 分岐ビット
        if temp_xy & 1 != 0 {
            result[index] |= 1 << (6 - in_index * 2);
        }

        temp_xy >>= 1;
    }

    // 階層0: 無条件で valid=1, value=0
    result[0] |= 1 << 7; // valid bit
                         // value bit は 0 なので何もしない

    let result = BitVec::from_vec(result);
    println!("-----");
    println!("Convert BitMask XY Z:{} XY:{}", z, xy);
    println!("Result : {}", result);
    println!(
        "Invert BitMask XY Z:{} XY:{}",
        invert_bitmask_xy(&result).0,
        invert_bitmask_xy(&result).1
    );

    (result, z)
}

pub fn invert_bitmask_xy(bitmask: &BitVec) -> (u8, u64) {
    let bytes = &bitmask.0;
    let total_bits = bytes.len() * 8;
    let total_layers = total_bits / 2;

    let mut xy: u64 = 0;
    let mut z: i64 = -1;

    // 上位階層 → 下位階層 (階層1まで、階層0は除外)
    for now_z in (1..total_layers).rev() {
        let index = (now_z * 2) / 8;
        let in_index = now_z % 4;

        let valid_bit_pos = 7 - in_index * 2;
        let branch_bit_pos = 6 - in_index * 2;

        let byte = bytes[index];
        let valid = (byte >> valid_bit_pos) & 1;
        let branch = (byte >> branch_bit_pos) & 1;

        if valid == 1 {
            z = z.max(now_z as i64);
            xy <<= 1;
            xy |= branch as u64;
        }
    }

    // 階層0のチェック（zの更新のみ）
    let valid_layer0 = (bytes[0] >> 7) & 1;
    if valid_layer0 == 1 {
        z = z.max(0);
    }

    (z as u8, xy)
}
