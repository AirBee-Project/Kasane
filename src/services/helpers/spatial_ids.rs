use kasane_logic::{FlexId, SpatialId as _, SpatialIdSet};

use crate::{
    error::AppError,
    models::{database::table::data::ZoomLevelPolicy, spatial_id::SpatialId},
};

fn invalid_time_reason(reason: impl Into<String>) -> AppError {
    AppError::InvalidSpatialId {
        reason: reason.into(),
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

pub fn to_spatial_id_set(ids: &[SpatialId]) -> Result<SpatialIdSet, AppError> {
    let mut result = SpatialIdSet::new();

    for spatial_id in ids {
        match spatial_id {
            SpatialId::SingleId(s) => {
                result.insert(s.clone());
            }
            SpatialId::RangeId(r) => {
                result.insert(r.clone());
            }
            SpatialId::FlexId(f) => {
                result.insert(*f);
            }
        }
    }

    Ok(result)
}

pub fn process_spatial_ids(
    ids: &[SpatialId],
    max_zoom_level: u8,
    policy: &ZoomLevelPolicy,
    enforce_no_time: bool,
) -> Result<SpatialIdSet, AppError> {
    let mut result = SpatialIdSet::new();

    for spatial_id in ids {
        match spatial_id {
            SpatialId::SingleId(single_id) => {
                if enforce_no_time && !single_id.is_whole_time() {
                    return Err(invalid_time_reason(
                        "This table does not accept time-indexed spatial IDs for write operations",
                    ));
                }
                let Some(zoom) = resolve_zoom(single_id.z(), max_zoom_level, policy)? else {
                    continue;
                };

                if zoom == single_id.z() {
                    result.insert(single_id.clone());
                } else {
                    result.insert(single_id.spatial_parent_at_zoom(zoom)?);
                }
            }
            SpatialId::RangeId(range_id) => {
                if enforce_no_time && !range_id.is_whole_time() {
                    return Err(invalid_time_reason(
                        "This table does not accept time-indexed spatial IDs for write operations",
                    ));
                }
                let Some(zoom) = resolve_zoom(range_id.z(), max_zoom_level, policy)? else {
                    continue;
                };

                if zoom == range_id.z() {
                    result.insert(range_id.clone());
                } else {
                    result.insert(range_id.spatial_parent_at_zoom(zoom)?);
                }
            }
            SpatialId::FlexId(flex_id) => {
                if enforce_no_time && !flex_id.is_whole_time() {
                    return Err(invalid_time_reason(
                        "This table does not accept time-indexed spatial IDs for write operations",
                    ));
                }
                let fz = resolve_zoom(flex_id.f_zoomlevel(), max_zoom_level, policy)?;
                let xz = resolve_zoom(flex_id.x_zoomlevel(), max_zoom_level, policy)?;
                let yz = resolve_zoom(flex_id.y_zoomlevel(), max_zoom_level, policy)?;

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

                let (new_fz, new_fi) =
                    scale_down(flex_id.f_zoomlevel(), fz, flex_id.f_index() as i64);
                let (new_xz, new_xi) =
                    scale_down(flex_id.x_zoomlevel(), xz, flex_id.x_index() as i64);
                let (new_yz, new_yi) =
                    scale_down(flex_id.y_zoomlevel(), yz, flex_id.y_index() as i64);

                let id = FlexId::new(
                    new_fz,
                    new_fi as i32,
                    new_xz,
                    new_xi as u32,
                    new_yz,
                    new_yi as u32,
                )?;
                if flex_id.is_whole_time() {
                    result.insert(id);
                } else {
                    result.insert(id.with_time(flex_id.t_zoomlevel(), flex_id.t())?);
                }
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kasane_logic::{FlexId, Interval, RangeId, SingleId};

    #[test]
    fn test_process_spatial_ids_enforce_no_time() {
        let single_with_time = SpatialId::SingleId(
            SingleId::new(0, 0, 0, 0)
                .unwrap()
                .with_time(Interval::new(1).unwrap(), 2)
                .unwrap(),
        );
        let single_no_time = SpatialId::SingleId(SingleId::new(0, 0, 0, 0).unwrap());
        let range_with_time = SpatialId::RangeId(
            RangeId::new(0, [-1, 0], [0, 0], [0, 0])
                .unwrap()
                .with_time(Interval::new(1).unwrap(), [0, 1])
                .unwrap(),
        );
        let flex_with_time = SpatialId::FlexId(
            FlexId::new(0, 0, 0, 0, 0, 0)
                .unwrap()
                .with_time(1, 0)
                .unwrap(),
        );

        // 1. enforce_no_time = true (Reject time)
        let res_single_reject = process_spatial_ids(
            std::slice::from_ref(&single_with_time),
            0,
            &ZoomLevelPolicy::Ignore,
            true,
        );
        assert!(res_single_reject.is_err());

        let res_range_reject = process_spatial_ids(
            std::slice::from_ref(&range_with_time),
            0,
            &ZoomLevelPolicy::Ignore,
            true,
        );
        assert!(res_range_reject.is_err());

        let res_flex_reject = process_spatial_ids(
            std::slice::from_ref(&flex_with_time),
            0,
            &ZoomLevelPolicy::Ignore,
            true,
        );
        assert!(res_flex_reject.is_err());

        // 2. enforce_no_time = true (Accept no time)
        let res_single_accept = process_spatial_ids(
            std::slice::from_ref(&single_no_time),
            0,
            &ZoomLevelPolicy::Ignore,
            true,
        );
        assert!(res_single_accept.is_ok());

        // 3. enforce_no_time = false (Accept time)
        let res_single_allow_time =
            process_spatial_ids(&[single_with_time], 0, &ZoomLevelPolicy::Ignore, false);
        assert!(res_single_allow_time.is_ok());

        let res_range_allow_time =
            process_spatial_ids(&[range_with_time], 0, &ZoomLevelPolicy::Ignore, false);
        assert!(res_range_allow_time.is_ok());

        let res_flex_allow_time =
            process_spatial_ids(&[flex_with_time], 0, &ZoomLevelPolicy::Ignore, false);
        assert!(res_flex_allow_time.is_ok());
    }
}
