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
    match range {
        Range::FilterValue(_) => {
            return EvalResult::Unknown(range.clone());
        }

        Range::Ids(_) | Range::Function(_) => {
            return EvalResult::Known(range.clone());
        }

        Range::Prefix(Prefix::AND(children)) => {
            let mut new_children = Vec::new();
            let mut has_unknown = false;

            for child in children {
                match eval(child) {
                    EvalResult::Known(r) => {
                        // Known の AND 内でできる簡約をしたければここでする
                        new_children.push(r);
                    }
                    EvalResult::Unknown(r) => {
                        has_unknown = true;
                        new_children.push(r);
                    }
                }
            }

            let new = Range::Prefix(Prefix::AND(new_children));
            if has_unknown {
                EvalResult::Unknown(new)
            } else {
                EvalResult::Known(new)
            }
        }

        Range::Prefix(Prefix::OR(children)) => {
            // AND と同様に Unknown を保持しつつ Known は評価する
            let mut new_children = Vec::new();
            let mut has_unknown = false;

            for child in children {
                match eval(child) {
                    EvalResult::Known(r) => new_children.push(r),
                    EvalResult::Unknown(r) => {
                        has_unknown = true;
                        new_children.push(r);
                    }
                }
            }

            let new = Range::Prefix(Prefix::OR(new_children));
            if has_unknown {
                EvalResult::Unknown(new)
            } else {
                EvalResult::Known(new)
            }
        }

        Range::Prefix(Prefix::NOT(child)) => match eval(child) {
            EvalResult::Known(r) => EvalResult::Known(Range::Prefix(Prefix::NOT(Box::new(r)))),
            EvalResult::Unknown(r) => EvalResult::Unknown(Range::Prefix(Prefix::NOT(Box::new(r)))),
        },
    }
}
