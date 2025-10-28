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

pub fn convert_bitmask_f(z: u8, f: i64) -> (BitVec, u8) {
    let length = ((z * 2 / 8) + 1) as usize;
    let mut result = vec![0u8; length];

    // 符号を保存するためにfの絶対値を使用
    let is_negative = f < 0;
    let mut f_abs = f.abs();

    // 最初の階層(z=0)の処理 - 符号を格納
    result[0] |= 1 << 7; // 有効ビット
    if is_negative {
        result[0] |= 1 << 6; // 分岐ビット（負の場合）
    }

    // それ以降の階層では各ビットを順に格納
    for now_z in 1..=z {
        let index = ((now_z) * 2 / 8) as usize;
        let in_index = now_z % 4;

        // 有効ビット
        result[index] |= 1 << (7 - in_index * 2);

        // 分岐ビット - f_absの最下位ビットを使用
        if f_abs % 2 != 0 {
            result[index] |= 1 << (6 - in_index * 2);
        }

        f_abs >>= 1; // 次のビットへ
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
    let total_layers = (total_bits + 1) / 2;

    let mut f: i64 = 0;
    let mut max_z: i32 = -1;
    let mut is_negative = false;

    for now_z in 0..total_layers {
        let index = (now_z * 2) / 8;
        let in_index = now_z % 4;

        let byte = bytes[index];
        let valid = (byte >> (7 - in_index * 2)) & 1;
        let branch = (byte >> (6 - in_index * 2)) & 1;

        if valid == 1 {
            max_z = now_z as i32;

            if now_z == 0 {
                // 最初の階層は符号情報
                is_negative = branch == 1;
            } else {
                // それ以降は各ビットを復元
                // 下位ビットから順に格納されているので、適切な位置に配置
                f |= (branch as i64) << (now_z - 1);
            }
        }
    }

    // 符号を適用
    if is_negative {
        f = -f;
    }

    (f, max_z as u8)
}
