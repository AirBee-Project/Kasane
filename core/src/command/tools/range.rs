use std::collections::HashMap;

use crate::{
    interface::input::{Prefix, Range},
    user_error::UserError,
};

enum EvalResult {
    Known(Range),   // 完全に評価できた
    Unknown(Range), // FilterValue を含み評価不能な部分がある
}

//ここで値の検証などを行う
fn eval(range: &Range) -> EvalResult {
    todo!()
}
