use crate::r#type::{
    space_time_id::SpaceTimeId,
    space_time_id_set::{
        convert_f::invert_bitmask_f, convert_xy::invert_bitmask_xy, SpaceTimeIdSet,
    },
};

impl SpaceTimeIdSet {
    ///更新されているであろう逆引きSetから集合全体を復元する
    pub fn get_all(&self) -> Vec<SpaceTimeId> {
        //debug部分
        for ele in &self.reverse {
            println!("{:?}", ele.1);
        }

        let mut result = vec![];
        for (index, reverse) in &self.reverse {
            println!("INDEX:{}", index);
            //デコード
            let decode_f = invert_bitmask_f(&reverse.f);
            let decode_x = invert_bitmask_xy(&reverse.x);
            let decode_y = invert_bitmask_xy(&reverse.y);

            println!("decode_f:{:?}", decode_f);
            println!("decode_x:{:?}", decode_x);
            println!("decode_y:{:?}", decode_y);

            //最も粒度が細かいZoomLeveLに合わせる
            let max_z = decode_f.1.max(decode_x.1.max(decode_y.1));

            //スケール
            let scale_f = change_scale_f(decode_f.1, max_z, decode_f.0);
            let scale_x = change_scale_xy(decode_x.1, max_z, decode_x.0);
            let scale_y = change_scale_xy(decode_y.1, max_z, decode_y.0);

            //iとtにはまだバグがある
            result.push(SpaceTimeId {
                z: max_z,
                f: scale_f,
                x: scale_x,
                y: scale_y,
                i: reverse.i,
                t: reverse.t,
            });
        }

        result
    }
}

fn change_scale_xy(now_z: u8, next_z: u8, xy: u64) -> (u64, u64) {
    //もしもnow_zがold_z以下ならばそのまま返す
    println!("now_z:{}", now_z);
    println!("next_z:{}", next_z);
    if now_z >= next_z {
        return (xy, xy);
    } else {
        let diff = next_z - now_z;
        let coef = 2_u64.pow(diff.into());
        println!("coef:{}", coef);

        let start = xy * coef;
        let end = (xy + 1) * coef - 1;
        return (start, end);
    }
}

fn change_scale_f(now_z: u8, next_z: u8, f: i64) -> (i64, i64) {
    if now_z >= next_z {
        return (f, f);
    } else {
        let diff = next_z - now_z;
        let coef = 2_i64.pow(diff.into());
        let start = f * coef;
        let end = (f + 1) * coef - 1;
        return (start, end);
    }
}
