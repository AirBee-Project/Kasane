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

///xyの次元の情報をBitVecに変換する
pub fn convert_bitmask_xy(z: u8, xy: u64) -> (BitVec, u8) {
    let length = ((z * 2 / 8) + 1).max(1) as usize;
    let mut result = vec![0u8; length];

    let bit_count = (z + 1) as u32;
    let mask = if bit_count >= 64 {
        u64::MAX
    } else {
        (1u64 << bit_count) - 1
    };
    let uxy = xy & mask;

    for now_z in (0..=z).rev() {
        let index = ((now_z) * 2 / 8) as usize;
        let in_index = now_z % 4;

        // 有効ビット
        result[index] |= 1 << (7 - in_index * 2);

        // MSB側から取得するように変更
        let bit_position = z - now_z; // now_z が大きいときに上位ビットを取る
        if (uxy >> bit_position) & 1 != 0 {
            result[index] |= 1 << (6 - in_index * 2);
        }
    }

    let result = BitVec::from_vec(result);
    println!("-----");
    println!("Convert BitMask XY Z:{} XY:{}", z, xy);
    println!("Result : {}", result);
    println!(
        "Invert BitMask XY Z:{} XY:{}",
        invert_bitmask_xy(&result).1,
        invert_bitmask_xy(&result).0
    );
    (result, z)
}
pub fn invert_bitmask_xy(bitmask: &BitVec) -> (u64, u8) {
    let bytes = &bitmask.0;
    let total_bits = bytes.len() * 8;
    let total_layers = (total_bits + 1) / 2;

    let mut uxy: u64 = 0;
    let mut max_z: i32 = -1; // 見つかった最大のz

    // now_z=0 から順に処理
    for now_z in 0..total_layers {
        let index = (now_z * 2) / 8;
        let in_index = now_z % 4;

        let byte = bytes[index];
        let valid = (byte >> (7 - in_index * 2)) & 1;
        let branch = (byte >> (6 - in_index * 2)) & 1;

        if valid == 1 {
            max_z = now_z as i32;
            // now_z の位置に branch を配置
            uxy |= (branch as u64) << now_z;
        }
    }

    // uxy を反転（ビットの並びを逆にする）
    let final_z = max_z as u8;
    let mut reversed_uxy = 0u64;
    for i in 0..=final_z {
        let bit = (uxy >> i) & 1;
        reversed_uxy |= bit << (final_z - i);
    }

    (reversed_uxy, final_z)
}
