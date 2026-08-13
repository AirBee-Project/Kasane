use kasane_logic::{
    AllowedIntervals, FlexId, Interval, RangeId, SingleId, SpatialIdSet, ZoomLevel,
};

use crate::{
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
};

/// 時間成分の不正はすべて [`AppError::InvalidSpatialId`] に畳む。
///
/// そのまま `?` で上げると `logic_error` になるが、このモジュール自身が出す不正は
/// `invalid_spatial_id`。同じユーザーミスに 2 つのコードが割り当たるとクライアントが
/// 一様に扱えない。
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
/// `SingleId` と `RangeId` は `{t}` の形だけが違うので、[`apply_interval_time`] を共有する。
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

/// `FlexId` は木のノードアドレスなので `{i}` ではなくズームレベルで時間を指定する。
/// 暦の単位の検証を行わない点が [`apply_interval_time`] と非対称。
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

pub fn resolve_zoom(
    zoom: u8,
    max_zoom_level: u8,
    policy: &ZoomLevelPolicy,
) -> Result<Option<u8>, AppError> {
    if zoom <= max_zoom_level {
        return Ok(Some(zoom));
    }

    match policy {
        ZoomLevelPolicy::Error => Err(AppError::ZoomLevelPolicy {
            max_zoom_level,
            input_zoom_level: zoom,
        }),
        ZoomLevelPolicy::Ignore => Ok(None),
        ZoomLevelPolicy::Normalize => Ok(Some(max_zoom_level)),
    }
}

/// `RawRangeId` の省略された空間軸を、そのズームレベルにおける軸全体へ展開する。
fn range_axes(range_id: &crate::models::spatial_id::RawRangeId) -> Result<RangeAxes, AppError> {
    let zoom = ZoomLevel::new(range_id.z)?;
    Ok(RangeAxes {
        f: range_id.f.unwrap_or([zoom.f_min(), zoom.f_max()]),
        x: range_id.x.unwrap_or([0, zoom.xy_max()]),
        y: range_id.y.unwrap_or([0, zoom.xy_max()]),
    })
}

struct RangeAxes {
    f: [i32; 2],
    x: [u32; 2],
    y: [u32; 2],
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
                let axes = range_axes(r)?;
                let id = RangeId::new(r.z, axes.f, axes.x, axes.y)?;
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

pub fn process_spatial_ids(
    ids: &[SpatialId],
    max_zoom_level: u8,
    policy: &ZoomLevelPolicy,
) -> Result<SpatialIdSet, AppError> {
    let mut result = SpatialIdSet::new();

    for spatial_id in ids {
        match spatial_id {
            SpatialId::SingleId(single_id) => {
                let Some(zoom) = resolve_zoom(single_id.z, max_zoom_level, policy)? else {
                    continue;
                };

                let id = SingleId::new(single_id.z, single_id.f, single_id.x, single_id.y)?;
                // `SingleId` / `RangeId` の縮小は時間を引き継ぐので、先に載せてよい。
                let id = apply_interval_time(id, single_id.i, single_id.t)?;

                if zoom == single_id.z {
                    result.insert(id);
                } else {
                    result.insert(id.spatial_parent_at_zoom(zoom)?);
                }
            }
            SpatialId::RangeId(range_id) => {
                let Some(zoom) = resolve_zoom(range_id.z, max_zoom_level, policy)? else {
                    continue;
                };

                let axes = range_axes(range_id)?;
                let id = RangeId::new(range_id.z, axes.f, axes.x, axes.y)?;
                let id = apply_interval_time(id, range_id.i, range_id.t)?;

                if zoom == range_id.z {
                    result.insert(id);
                } else {
                    result.insert(id.spatial_parent_at_zoom(zoom)?);
                }
            }
            SpatialId::FlexId(flex_id) => {
                let fz = resolve_zoom(flex_id.f_zoomlevel, max_zoom_level, policy)?;
                let xz = resolve_zoom(flex_id.x_zoomlevel, max_zoom_level, policy)?;
                let yz = resolve_zoom(flex_id.y_zoomlevel, max_zoom_level, policy)?;

                if fz.is_none() || xz.is_none() || yz.is_none() {
                    continue;
                }

                let fz = fz.unwrap();
                let xz = xz.unwrap();
                let yz = yz.unwrap();

                let scale_down = |z: u8, target_z: u8, val: i64| -> (u8, i64) {
                    if z > target_z {
                        (target_z, val >> (z - target_z))
                    } else {
                        (z, val)
                    }
                };

                let (new_fz, new_fi) = scale_down(flex_id.f_zoomlevel, fz, flex_id.f_index as i64);
                let (new_xz, new_xi) = scale_down(flex_id.x_zoomlevel, xz, flex_id.x_index as i64);
                let (new_yz, new_yi) = scale_down(flex_id.y_zoomlevel, yz, flex_id.y_index as i64);

                // `FlexId::new` は時間を全時間へ戻すので、逆順にすると指定時間が落ちる。
                let id = FlexId::new(
                    new_fz,
                    new_fi as i32,
                    new_xz,
                    new_xi as u32,
                    new_yz,
                    new_yi as u32,
                )?;
                result.insert(apply_segment_time(
                    id,
                    flex_id.t_zoomlevel,
                    flex_id.t_index,
                )?);
            }
        }
    }

    Ok(result)
}
