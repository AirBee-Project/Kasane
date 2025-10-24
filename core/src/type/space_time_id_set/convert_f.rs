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
    //配列を初期化する
    let length = (((z + 1) * 2 / 8) + 1).max(1) as usize;
    println!("配列の長さ:{}", length);

    let mut result = vec![0u8; length];

    //処理用のf
    let mut f = f;

    //各階層を順番に処理していく
    for now_z in 0..=z {
        //処理すべきIndexを決定する
        let index = ((now_z + 1) * 2 / 8) as usize;

        //そのIndexの何番目の階層なのかを見る（0から数える）
        let in_index = 3 - now_z % 4;

        //ORを取るための入れるべきBitを作る

        //有効Bitを挿入する
        let mut or_byte: u8 = 1 << (in_index * 2);

        //分岐Bitを挿入する
        if f % 2 != 0 {
            or_byte |= 1 << (in_index * 2 + 1);
        }

        //fを割る
        f = f / 2;

        //当該のIndexのBitを変更する
        result[index] |= or_byte;
    }

    //結果をBitVecに変換して出力
    let result = BitVec::from_vec(result);
    println!("Convert BitMask F Z:{} F:{} Result : {}", z, f, result);
    (result, z)
}

pub fn invert_bitmask_f(z: u8, bitmask: &BitVec) -> i64 {
    assert!(z <= 64, "z must be between 0 and 64");
    let is_negative = (bitmask[0] >> 7) & 1 != 0;
    let mut abs_f: u64 = 0;
    for k in 0..(z - 1) {
        let bit_pos = k + 1;
        let byte_index = (bit_pos / 8) as usize;
        let bit_index = 7 - (bit_pos % 8);
        let bit = (bitmask[byte_index] >> bit_index) & 1;
        abs_f |= (bit as u64) << k;
    }
    if is_negative {
        -(abs_f as i64)
    } else {
        abs_f as i64
    }
}

pub fn convert_bitmask_f_multiple(inputs: &Vec<(u8, i64)>) -> Vec<(BitVec, u8)> {
    inputs
        .iter()
        .map(|(z, f)| convert_bitmask_f(*z, *f))
        .collect()
}
