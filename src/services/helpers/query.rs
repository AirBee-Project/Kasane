use kasane_logic::{
    Coordinate, CoverSingleIds, Line, RangeId, SingleId, SpatialIdSet, Sphere, Triangle,
};

use crate::{
    error::AppError,
    models::{
        layer::data::ZoomLevelPolicy,
        query::{Geometry, Query},
    },
};

impl Query {
    pub fn process(
        &self,
        max_zoom_level: u8,
        zoom_level_policy: &ZoomLevelPolicy,
    ) -> Result<SpatialIdSet, AppError> {
        match self {
            Query::Union { left, right } => Ok(left.process(max_zoom_level, zoom_level_policy)?
                | right.process(max_zoom_level, zoom_level_policy)?),

            Query::Intersection { left, right } => Ok(left
                .process(max_zoom_level, zoom_level_policy)?
                & right.process(max_zoom_level, zoom_level_policy)?),

            Query::Difference { base, subtract } => Ok(base
                .process(max_zoom_level, zoom_level_policy)?
                - subtract.process(max_zoom_level, zoom_level_policy)?),

            Query::Geometry { geometry } => {
                Self::process_geometry(geometry, max_zoom_level, zoom_level_policy)
            }

            Query::SpatialIds { ids } => {
                Self::process_spatial_ids(ids, max_zoom_level, zoom_level_policy)
            }
        }
    }

    fn resolve_zoom(
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

    fn process_geometry(
        geometry: &Geometry,
        max_zoom_level: u8,
        policy: &ZoomLevelPolicy,
    ) -> Result<SpatialIdSet, AppError> {
        match geometry {
            Geometry::Coordinate {
                zoomlevel,
                coordinate,
            } => {
                let Some(zoom) = Self::resolve_zoom(*zoomlevel, max_zoom_level, policy)? else {
                    return Ok(SpatialIdSet::new());
                };

                let id = Coordinate::new(
                    coordinate.latitude,
                    coordinate.longitude,
                    coordinate.altitude,
                )?
                .single_id(zoom)?;

                let mut result = SpatialIdSet::new();
                result.insert(id);

                Ok(result)
            }

            Geometry::Line { zoomlevel, points } => {
                let Some(zoom) = Self::resolve_zoom(*zoomlevel, max_zoom_level, policy)? else {
                    return Ok(SpatialIdSet::new());
                };

                let start =
                    Coordinate::new(points[0].latitude, points[0].longitude, points[0].altitude)?;

                let end =
                    Coordinate::new(points[1].latitude, points[1].longitude, points[1].altitude)?;

                let mut result = SpatialIdSet::new();
                for single_id in Line::new([start, end]).cover_single_ids(zoom)? {
                    result.insert(single_id);
                }
                Ok(result)
            }

            Geometry::Triangle { zoomlevel, points } => {
                let Some(zoom) = Self::resolve_zoom(*zoomlevel, max_zoom_level, policy)? else {
                    return Ok(SpatialIdSet::new());
                };

                let coords = [
                    Coordinate::new(points[0].latitude, points[0].longitude, points[0].altitude)?,
                    Coordinate::new(points[1].latitude, points[1].longitude, points[1].altitude)?,
                    Coordinate::new(points[2].latitude, points[2].longitude, points[2].altitude)?,
                ];

                let mut result = SpatialIdSet::new();
                for single_id in Triangle::new(coords).cover_single_ids(zoom)? {
                    result.insert(single_id);
                }
                Ok(result)
            }

            Geometry::Sphere {
                zoomlevel,
                radius_m,
                center,
            } => {
                let Some(zoom) = Self::resolve_zoom(*zoomlevel, max_zoom_level, policy)? else {
                    return Ok(SpatialIdSet::new());
                };

                let center = Coordinate::new(center.latitude, center.longitude, center.altitude)?;

                let mut result = SpatialIdSet::new();
                for single_id in Sphere::new(center, *radius_m)?.cover_single_ids(zoom)? {
                    result.insert(single_id);
                }
                Ok(result)
            }
        }
    }

    fn process_spatial_ids(
        ids: &[crate::models::spatial_id::SpatialId],
        max_zoom_level: u8,
        policy: &ZoomLevelPolicy,
    ) -> Result<SpatialIdSet, AppError> {
        let mut result = SpatialIdSet::new();

        for spatial_id in ids {
            match spatial_id {
                crate::models::spatial_id::SpatialId::SingleId { z, f, x, y } => {
                    let Some(zoom) = Self::resolve_zoom(*z, max_zoom_level, policy)? else {
                        continue;
                    };

                    let id = SingleId::new(*z, *f, *x, *y)?;

                    if zoom == *z {
                        result.insert(id);
                    } else {
                        result.insert(id.spatial_parent_at_zoom(zoom)?);
                    }
                }

                crate::models::spatial_id::SpatialId::RangeId { z, f, x, y } => {
                    let Some(zoom) = Self::resolve_zoom(*z, max_zoom_level, policy)? else {
                        continue;
                    };

                    let id = RangeId::new(*z, *f, *x, *y)?;

                    if zoom == *z {
                        result.insert(id);
                    } else {
                        result.insert(id.spatial_parent_at_zoom(zoom)?);
                    }
                }
            }
        }

        Ok(result)
    }
}
