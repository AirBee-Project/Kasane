use kasane_logic::{RangeId, SingleId, SpatialIdSet};

use crate::{
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
};

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

pub fn to_spatial_id_set(ids: &[SpatialId]) -> Result<SpatialIdSet, AppError> {
    let mut result = SpatialIdSet::new();

    for spatial_id in ids {
        match spatial_id {
            SpatialId::SingleId(s) => {
                let mut id = SingleId::new(s.z, s.f, s.x, s.y)?;
                if let Some(i) = s.i {
                    let interval = kasane_logic::Interval::new(i)?;
                    if !kasane_logic::AllowedIntervals::calendar().contains(interval) {
                        return Err(AppError::InvalidSpatialId {
                            reason: format!("Interval {} is not allowed", i),
                        });
                    }
                    let t = s.t.ok_or_else(|| AppError::InvalidSpatialId {
                        reason: "t must be provided when i is provided".to_string(),
                    })?;
                    id = id.with_time(interval, t)?;
                } else if s.t.is_some() {
                    return Err(AppError::InvalidSpatialId {
                        reason: "i must be provided when t is provided".to_string(),
                    });
                }
                result.insert(id);
            }
            SpatialId::RangeId(r) => {
                let mut id = RangeId::new(r.z, r.f, r.x, r.y)?;
                if let Some(i) = r.i {
                    let interval = kasane_logic::Interval::new(i)?;
                    if !kasane_logic::AllowedIntervals::calendar().contains(interval) {
                        return Err(AppError::InvalidSpatialId {
                            reason: format!("Interval {} is not allowed", i),
                        });
                    }
                    let t = r.t.ok_or_else(|| AppError::InvalidSpatialId {
                        reason: "t must be provided when i is provided".to_string(),
                    })?;
                    id = id.with_time(interval, t)?;
                } else if r.t.is_some() {
                    return Err(AppError::InvalidSpatialId {
                        reason: "i must be provided when t is provided".to_string(),
                    });
                }
                result.insert(id);
            }
            SpatialId::FlexId(f) => {
                let mut id = kasane_logic::FlexId::new(
                    f.f_zoomlevel,
                    f.f_index,
                    f.x_zoomlevel,
                    f.x_index,
                    f.y_zoomlevel,
                    f.y_index,
                )?;

                if let Some(t_zoomlevel) = f.t_zoomlevel {
                    let t_index = f.t_index.ok_or_else(|| AppError::InvalidSpatialId {
                        reason: "tIndex must be provided when tZoomlevel is provided".to_string(),
                    })?;
                    id = id.with_time(t_zoomlevel, t_index)?;
                } else if f.t_index.is_some() {
                    return Err(AppError::InvalidSpatialId {
                        reason: "tZoomlevel must be provided when tIndex is provided".to_string(),
                    });
                }

                result.insert(id);
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

                let mut id = SingleId::new(single_id.z, single_id.f, single_id.x, single_id.y)?;
                if let Some(i) = single_id.i {
                    let interval = kasane_logic::Interval::new(i)?;
                    if !kasane_logic::AllowedIntervals::calendar().contains(interval) {
                        return Err(AppError::InvalidSpatialId {
                            reason: format!("Interval {} is not allowed", i),
                        });
                    }
                    let t = single_id.t.ok_or_else(|| AppError::InvalidSpatialId {
                        reason: "t must be provided when i is provided".to_string(),
                    })?;
                    id = id.with_time(interval, t)?;
                } else if single_id.t.is_some() {
                    return Err(AppError::InvalidSpatialId {
                        reason: "i must be provided when t is provided".to_string(),
                    });
                }

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

                let mut id = RangeId::new(range_id.z, range_id.f, range_id.x, range_id.y)?;
                if let Some(i) = range_id.i {
                    let interval = kasane_logic::Interval::new(i)?;
                    if !kasane_logic::AllowedIntervals::calendar().contains(interval) {
                        return Err(AppError::InvalidSpatialId {
                            reason: format!("Interval {} is not allowed", i),
                        });
                    }
                    let t = range_id.t.ok_or_else(|| AppError::InvalidSpatialId {
                        reason: "t must be provided when i is provided".to_string(),
                    })?;
                    id = id.with_time(interval, t)?;
                } else if range_id.t.is_some() {
                    return Err(AppError::InvalidSpatialId {
                        reason: "i must be provided when t is provided".to_string(),
                    });
                }

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

                let mut id = kasane_logic::FlexId::new(
                    new_fz,
                    new_fi as i32,
                    new_xz,
                    new_xi as u32,
                    new_yz,
                    new_yi as u32,
                )?;

                if let Some(t_zoomlevel) = flex_id.t_zoomlevel {
                    let t_index = flex_id.t_index.ok_or_else(|| AppError::InvalidSpatialId {
                        reason: "tIndex must be provided when tZoomlevel is provided".to_string(),
                    })?;
                    id = id.with_time(t_zoomlevel, t_index)?;
                } else if flex_id.t_index.is_some() {
                    return Err(AppError::InvalidSpatialId {
                        reason: "tZoomlevel must be provided when tIndex is provided".to_string(),
                    });
                }
                result.insert(id);
            }
        }
    }

    Ok(result)
}
