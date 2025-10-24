use crate::r#type::bit_vec::BitVec;

//XYの次元について入れられた値を可能な限り高いZの集合として表す
pub fn convert_f(z: u8, dim: (i64, i64)) -> Vec<(u8, i64)> {
    let mut current_range = Some(dim);
    let mut now_z = z;
    let mut result = Vec::new();

    while let Some(mut target) = current_range {
        // 終了条件：範囲が縮退した or z=0
        if target.0 >= target.1 {
            result.push((now_z, target.0));
            break;
        }

        if now_z == 0 {
            result.push((now_z, target.0));
            result.push((now_z, target.1));
            break;
        }

        // 左端が奇数なら個別処理
        if target.0 % 2 != 0 {
            result.push((now_z, target.0));
            target.0 += 1;
        }

        // 右端が偶数なら個別処理
        if target.1 % 2 == 0 {
            result.push((now_z, target.1));
            target.1 -= 1;
        }

        // 範囲が逆転したら終了
        if target.0 > target.1 {
            break;
        }

        // 次のズームレベルに範囲を縮小
        let a = target.0 / 2;
        let b = target.1 / 2;

        if a == b {
            result.push((now_z - 1, a));
            break;
        }

        current_range = Some((a.min(b), a.max(b)));
        now_z -= 1;
    }

    result
}

pub fn convert_bitmask_f(z: u8, mut f: i64) -> (BitVec, u8) {
    let length = (((z + 1) * 2 / 8) + 1).max(1) as usize;
    let mut result = vec![0u8; length];

    // z+1 ビットだけを使用（上位ビットをマスク）
    let bit_count = (z + 1) as u32;
    let mask = if bit_count >= 64 {
        u64::MAX
    } else {
        (1u64 << bit_count) - 1
    };
    let mut uf = (f as u64) & mask;

    for now_z in (0..=z).rev() {
        let index = ((now_z) * 2 / 8) as usize;
        let in_index = now_z % 4;

        // 有効ビット
        result[index] |= 1 << (7 - in_index * 2);

        // 分岐ビット
        if uf & 1 != 0 {
            result[index] |= 1 << (6 - in_index * 2);
        }

        uf >>= 1;
    }

    let result = BitVec::from_vec(result);
    println!("-----");
    println!("Convert BitMask F Z:{} F:{}", z, f);
    println!("Result : {}", result);
    println!(
        "Invert BitMask F Z:{} F:{}",
        invert_bitmask_f(&result).1,
        invert_bitmask_f(&result).0
    );
    (result, z)
}
pub fn invert_bitmask_f(bitmask: &BitVec) -> (i64, u8) {
    let bytes = &bitmask.0;
    let total_bits = bytes.len() * 8;
    let total_layers = total_bits / 2;

    let mut uf: u64 = 0;
    let mut z: i64 = -1;

    // 上位階層 (z) → 下位階層 (0) の順で復元
    for now_z in (0..total_layers).rev() {
        let index = (now_z * 2) / 8;
        let in_index = now_z % 4;

        let valid_bit_pos = 7 - in_index * 2;
        let branch_bit_pos = 6 - in_index * 2;

        let byte = bytes[index];
        let valid = (byte >> valid_bit_pos) & 1;
        let branch = (byte >> branch_bit_pos) & 1;

        if valid == 1 {
            z = z.max(now_z as i64);
            uf <<= 1; // valid なビットの時だけシフト
            uf |= branch as u64;
        }
    }

    // 符号拡張: z+1 ビットの符号付き整数として扱う
    let bit_count = (z + 1) as u32;
    let sign_bit_pos = z as u32;
    let is_negative = (uf >> sign_bit_pos) & 1 == 1;

    let mut result = if is_negative && bit_count < 64 {
        // 上位ビットを1で埋める（符号拡張）
        let sign_extension = !((1u64 << bit_count) - 1);
        (uf | sign_extension) as i64
    } else {
        uf as i64
    };

    if result >= 0 {
        result = result * 2
    }

    (result, z as u8)
}
