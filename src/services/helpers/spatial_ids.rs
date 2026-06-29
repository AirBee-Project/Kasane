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

                let id = RangeId::new(range_id.z, range_id.f, range_id.x, range_id.y)?;

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

                let id = kasane_logic::FlexId::new(
                    new_fz,
                    new_fi as i32,
                    new_xz,
                    new_xi as u32,
                    new_yz,
                    new_yi as u32,
                )?;
                result.insert(id);
            }
        }
    }

    Ok(result)
}
