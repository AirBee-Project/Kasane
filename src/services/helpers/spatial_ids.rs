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
        }
    }

    Ok(result)
}
