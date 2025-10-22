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

pub fn invert_bitmask_f(bitmask: &Vec<u8>) -> (u8, i64) {
    if bitmask.is_empty() {
        return (0, 0);
    }

    // まず最初の層の flag_bit と value_bit を確認
    let first_byte = bitmask[0];
    let flag_bit = (first_byte >> 7) & 1;
    let value_bit = (first_byte >> 6) & 1;

    if flag_bit == 0 {
        return (0, 0); // 無効
    }

    // 最初の層だけで Z=0 のケース
    let total_bits = bitmask.len() * 8;
    if total_bits <= 2 {
        let f = if value_bit == 0 { 0 } else { 1 };
        return (0, f);
    }

    // Z>0 の場合
    let mut z = 1; // 最初の層はカウント済み
    let mut f: u64 = value_bit as u64;

    let mut bit_pos = 2; // 1階層2ビット、最初の2ビットは処理済み

    while bit_pos + 1 < total_bits && z < 64 {
        let byte_index = bit_pos / 8;
        let bit_index = 7 - (bit_pos % 8);

        let flag_bit = (bitmask[byte_index] >> bit_index) & 1;
        let value_bit = (bitmask[byte_index] >> (bit_index - 1)) & 1;

        if flag_bit == 0 {
            break; // 階層無効なら終了
        }

        // f を右詰めで復元
        f |= (value_bit as u64) << z;

        z += 1;
        bit_pos += 2;
    }

    (z, f as i64)
}

pub fn convert_bitmask_f(z: u8, f: i64) -> (Vec<u8>, u8) {
    assert!(z <= 32, "z must be <= 32 for safety");

    if z == 0 {
        let value_bit = (f.abs() % 2) as u8; // f の偶奇
        let byte = 0b10_000000 | (value_bit << 6); // MSB=flag, 2番目が value_bit
        return (vec![byte], 0);
    }

    let mut result: Vec<u8> = vec![0; ((z as usize) * 2 + 7) / 8];
    let mut abs_f = f.abs() as u64;

    for i in 0..z {
        let flag_bit = 1;
        let value_bit = (abs_f % 2) as u8;
        abs_f /= 2;

        let bit_pos = i * 2;
        let byte_index = (bit_pos / 8) as usize;
        let bit_index = 7 - (bit_pos % 8) as usize;

        result[byte_index] |= flag_bit << bit_index;
        result[byte_index] |= value_bit << (bit_index - 1);
    }

    (result, z)
}

pub fn convert_bitmask_f_multiple(inputs: &Vec<(u8, i64)>) -> Vec<(Vec<u8>, u8)> {
    inputs
        .iter()
        .map(|(z, f)| convert_bitmask_f(*z, *f))
        .collect()
}
