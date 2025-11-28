use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::interface::input::Range;

///時空間IDの集合に対して特定の条件をかけて加工する
#[derive(Debug, Serialize, Deserialize, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub enum Effect {
    ///集合の内部にある時空間IDのF方向の長さを最大まで引き延ばす
    FStretchMax(Box<Range>),

    ///集合の内部にある時空間IDのX方向の長さを最大まで引き延ばす
    XStretchMax(Box<Range>),

    ///集合の内部にある時空間IDのY方向の長さを最大まで引き延ばす
    YStretchMax(Box<Range>),

    ///集合の内部にある時空間IDのT方向の長さを最大まで引き延ばす
    TStretchMax(Box<Range>),
}
