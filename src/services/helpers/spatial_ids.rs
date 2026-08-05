use kasane_logic::{AllowedIntervals, FlexId, Interval, RangeId, SingleId, SpatialIdSet};

use crate::{
    error::AppError,
    models::spatial_id::SpatialId,
};

/// 時間成分の不正はすべて [`AppError::InvalidSpatialId`] に畳む。
///
/// `Interval::new` や `with_time` は [`kasane_logic::Error`] を返し、そのまま `?` で
/// 上げると `logic_error` になる。一方このモジュール自身が出す「暦の単位ではない」
/// 「`i` と `t` が片方だけ」は `invalid_spatial_id` である。同じ「時間指定が不正」
/// という1つのユーザーミスに2つのコードが割り当たると、クライアント側で一様に
/// 扱えないため、ここで `invalid_spatial_id` へ統一する。
fn invalid_time(error: kasane_logic::Error) -> AppError {
    AppError::InvalidSpatialId {
        reason: error.to_string(),
    }
}

fn invalid_time_reason(reason: impl Into<String>) -> AppError {
    AppError::InvalidSpatialId {
        reason: reason.into(),
    }
}

/// `{i}`（時間間隔）と `{t}`（時間インデックス）で時間を指定できる ID。
///
/// `SingleId` と `RangeId` は `{t}` の形だけが違う（単一値 / 区間）ので、
/// [`apply_interval_time`] をこの1つのトレイト越しに共有する。
trait WithIntervalTime: Sized {
    /// `{t}` の表現。`SingleId` は `u64`、`RangeId` は `[u64; 2]`。
    type Index;

    fn with_interval_time(
        self,
        interval: Interval,
        t: Self::Index,
    ) -> Result<Self, kasane_logic::Error>;
}

impl WithIntervalTime for SingleId {
    type Index = u64;

    fn with_interval_time(self, interval: Interval, t: u64) -> Result<Self, kasane_logic::Error> {
        self.with_time(interval, t)
    }
}

impl WithIntervalTime for RangeId {
    type Index = [u64; 2];

    fn with_interval_time(
        self,
        interval: Interval,
        t: [u64; 2],
    ) -> Result<Self, kasane_logic::Error> {
        self.with_time(interval, t)
    }
}

/// `{i}` / `{t}` を検証して ID へ適用する。
///
/// `i` と `t` は「両方指定」か「両方省略」のいずれかで、省略時は全時間を表す。
/// `i` は暦の単位（[`AllowedIntervals::calendar`]）のみ受け付ける。
fn apply_interval_time<Id: WithIntervalTime>(
    id: Id,
    i: Option<u64>,
    t: Option<Id::Index>,
) -> Result<Id, AppError> {
    match (i, t) {
        (None, None) => Ok(id),
        (Some(i), Some(t)) => {
            let interval = Interval::new(i).map_err(invalid_time)?;
            if !AllowedIntervals::calendar().contains(interval) {
                // 許可値はここで数え上げず候補集合から作る（定義がずれても文言が追従する）。
                let allowed = AllowedIntervals::calendar()
                    .iter()
                    .map(|unit| unit.seconds().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(invalid_time_reason(format!(
                    "interval {i} is not allowed; i must be one of: {allowed}"
                )));
            }
            id.with_interval_time(interval, t).map_err(invalid_time)
        }
        (Some(_), None) => Err(invalid_time_reason("t must be provided when i is provided")),
        (None, Some(_)) => Err(invalid_time_reason("i must be provided when t is provided")),
    }
}

/// `FlexId` の時間（ズームレベル + インデックスの2分岐Segment）を適用する。
///
/// `FlexId` は木のノードアドレスなので `{i}` ではなくズームレベルで時間を指定する。
/// そのため暦の単位の検証は行わない（[`apply_interval_time`] と非対称なのはこのため）。
fn apply_segment_time(
    id: FlexId,
    t_zoomlevel: Option<u8>,
    t_index: Option<u64>,
) -> Result<FlexId, AppError> {
    match (t_zoomlevel, t_index) {
        (None, None) => Ok(id),
        (Some(t_zoomlevel), Some(t_index)) => {
            id.with_time(t_zoomlevel, t_index).map_err(invalid_time)
        }
        (Some(_), None) => Err(invalid_time_reason(
            "tIndex must be provided when tZoomlevel is provided",
        )),
        (None, Some(_)) => Err(invalid_time_reason(
            "tZoomlevel must be provided when tIndex is provided",
        )),
    }
}


pub fn to_spatial_id_set(ids: &[SpatialId]) -> Result<SpatialIdSet, AppError> {
    let mut result = SpatialIdSet::new();

    for spatial_id in ids {
        match spatial_id {
            SpatialId::SingleId(s) => {
                let id = SingleId::new(s.z, s.f, s.x, s.y)?;
                result.insert(apply_interval_time(id, s.i, s.t)?);
            }
            SpatialId::RangeId(r) => {
                let id = RangeId::new(r.z, r.f, r.x, r.y)?;
                result.insert(apply_interval_time(id, r.i, r.t)?);
            }
            SpatialId::FlexId(f) => {
                let id = FlexId::new(
                    f.f_zoomlevel,
                    f.f_index,
                    f.x_zoomlevel,
                    f.x_index,
                    f.y_zoomlevel,
                    f.y_index,
                )?;
                result.insert(apply_segment_time(id, f.t_zoomlevel, f.t_index)?);
            }
        }
    }

    Ok(result)
}

