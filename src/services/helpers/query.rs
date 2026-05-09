use kasane_logic::{
    Coordinate, CoverSingleIds, Line, RangeId, SingleId, SpatialIdSet, Sphere, Triangle,
    spatial_id::single_id,
};
use redb::Range;

use crate::{
    error::AppError,
    models::{layer::data::ZoomLevelPolicy, query::Query},
};

impl Query {
    ///[Query]を実行して時空間IDの集合に変換する
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
            Query::Geometry { geometry } => match geometry {
                crate::models::query::Geometry::Coordinate {
                    zoomlevel,
                    coordinate,
                } => {
                    let actual_zoom = if *zoomlevel > max_zoom_level {
                        match zoom_level_policy {
                            ZoomLevelPolicy::Error => {
                                return Err(AppError::ZoomLevelPolicy {
                                    max_zoom_level,
                                    input_zoom_level: *zoomlevel,
                                });
                            }
                            ZoomLevelPolicy::Ignore => return Ok(SpatialIdSet::new()),
                            ZoomLevelPolicy::Normalize => max_zoom_level,
                        }
                    } else {
                        *zoomlevel
                    };

                    let single_id = Coordinate::new(
                        coordinate.latitude,
                        coordinate.longitude,
                        coordinate.altitude,
                    )?
                    .single_id(actual_zoom)?;

                    let mut result = SpatialIdSet::new();
                    result.insert(single_id);

                    Ok(result)
                }
                crate::models::query::Geometry::Line { zoomlevel, points } => {
                    let actual_zoom = if *zoomlevel > max_zoom_level {
                        match zoom_level_policy {
                            ZoomLevelPolicy::Error => {
                                return Err(AppError::ZoomLevelPolicy {
                                    max_zoom_level,
                                    input_zoom_level: *zoomlevel,
                                });
                            }
                            ZoomLevelPolicy::Ignore => return Ok(SpatialIdSet::new()),
                            ZoomLevelPolicy::Normalize => max_zoom_level,
                        }
                    } else {
                        *zoomlevel
                    };

                    let mut result = SpatialIdSet::new();

                    let start = Coordinate::new(
                        points[0].latitude,
                        points[0].longitude,
                        points[0].altitude,
                    )?;
                    let end = Coordinate::new(
                        points[1].latitude,
                        points[1].longitude,
                        points[1].altitude,
                    )?;

                    for single_id in Line::new([start, end]).cover_single_ids(actual_zoom)? {
                        result.insert(single_id);
                    }
                    Ok(result)
                }
                crate::models::query::Geometry::Triangle { zoomlevel, points } => {
                    let actual_zoom = if *zoomlevel > max_zoom_level {
                        match zoom_level_policy {
                            ZoomLevelPolicy::Error => {
                                return Err(AppError::ZoomLevelPolicy {
                                    max_zoom_level,
                                    input_zoom_level: *zoomlevel,
                                });
                            }
                            ZoomLevelPolicy::Ignore => return Ok(SpatialIdSet::new()),
                            ZoomLevelPolicy::Normalize => max_zoom_level,
                        }
                    } else {
                        *zoomlevel
                    };

                    let mut result = SpatialIdSet::new();
                    let mut checked_coords = [Coordinate::default(); 3];
                    for (i, raw) in points.into_iter().enumerate() {
                        let coord = Coordinate::new(raw.latitude, raw.longitude, raw.altitude)?;
                        checked_coords[i] = coord;
                    }
                    for single_id in Triangle::new(checked_coords).cover_single_ids(actual_zoom)? {
                        result.insert(single_id);
                    }
                    Ok(result)
                }
                crate::models::query::Geometry::Sphere {
                    zoomlevel,
                    radius_m,
                    center,
                } => {
                    let mut result = SpatialIdSet::new();

                    let actual_zoom = if *zoomlevel > max_zoom_level {
                        match zoom_level_policy {
                            ZoomLevelPolicy::Error => {
                                return Err(AppError::ZoomLevelPolicy {
                                    max_zoom_level,
                                    input_zoom_level: *zoomlevel,
                                });
                            }
                            ZoomLevelPolicy::Ignore => return Ok(SpatialIdSet::new()),
                            ZoomLevelPolicy::Normalize => max_zoom_level,
                        }
                    } else {
                        *zoomlevel
                    };

                    let checked_center =
                        Coordinate::new(center.latitude, center.longitude, center.altitude)?;
                    for single_id in
                        Sphere::new(checked_center, *radius_m)?.cover_single_ids(actual_zoom)?
                    {
                        result.insert(single_id);
                    }
                    Ok(result)
                }
            },
            Query::SpatialIds { ids } => {
                let mut result = SpatialIdSet::new();

                for spatial_id in ids {
                    match spatial_id {
                        crate::models::spatial_id::SpatialId::SingleId { z, f, x, y } => {
                            if *z > max_zoom_level {
                                match zoom_level_policy {
                                    ZoomLevelPolicy::Error => {
                                        return Err(AppError::ZoomLevelPolicy {
                                            max_zoom_level,
                                            input_zoom_level: *z,
                                        });
                                    }
                                    ZoomLevelPolicy::Ignore => {
                                        continue;
                                    }
                                    ZoomLevelPolicy::Normalize => {
                                        let mini_id = SingleId::new(*z, *f, *x, *y)?;
                                        let normalize_id =
                                            mini_id.spatial_parent_at_zoom(max_zoom_level)?;
                                        result.insert(normalize_id);
                                    }
                                }
                            } else {
                                result.insert(SingleId::new(*z, *f, *x, *y)?);
                            };
                        }
                        crate::models::spatial_id::SpatialId::RangeId { z, f, x, y } => {
                            if *z > max_zoom_level {
                                match zoom_level_policy {
                                    ZoomLevelPolicy::Error => {
                                        return Err(AppError::ZoomLevelPolicy {
                                            max_zoom_level,
                                            input_zoom_level: *z,
                                        });
                                    }
                                    ZoomLevelPolicy::Ignore => {
                                        continue;
                                    }
                                    ZoomLevelPolicy::Normalize => {
                                        let mini_id = RangeId::new(*z, *f, *x, *y)?;
                                        let normalize_id =
                                            mini_id.spatial_parent_at_zoom(max_zoom_level)?;
                                        result.insert(normalize_id);
                                    }
                                }
                            } else {
                                result.insert(RangeId::new(*z, *f, *x, *y)?);
                            };
                        }
                    }
                }
                Ok(result)
            }
        }
    }
}
