use kasane_logic::{
    Coordinate, CoverSingleIds, Line, RangeId, SingleId, SpatialIdSet, Sphere, Triangle,
};

use crate::{error::AppError, models::table::Query};

impl Query {
    ///[Query]を実行して時空間IDの集合に変換する
    pub fn process(&self, max_zoom_level: u8) -> Result<SpatialIdSet, AppError> {
        match self {
            Query::Union { left, right } => {
                Ok(left.process(max_zoom_level)? | right.process(max_zoom_level)?)
            }
            Query::Intersection { left, right } => {
                Ok(left.process(max_zoom_level)? & right.process(max_zoom_level)?)
            }
            Query::Difference { base, subtract } => {
                Ok(base.process(max_zoom_level)? - subtract.process(max_zoom_level)?)
            }
            Query::GeometryQuery(geometry) => match geometry {
                crate::models::table::Geometry::Coordinate {
                    zoomlevel,
                    coordinate,
                } => {
                    let mut result = SpatialIdSet::new();
                    result.insert(
                        Coordinate::new(
                            coordinate.latitude,
                            coordinate.longitude,
                            coordinate.altitude,
                        )?
                        .single_id(*zoomlevel)?,
                    );
                    Ok(result)
                }
                crate::models::table::Geometry::Line { zoomlevel, points } => {
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
                    for single_id in Line::new([start, end]).cover_single_ids(*zoomlevel)? {
                        result.insert(single_id);
                    }
                    Ok(result)
                }
                crate::models::table::Geometry::Triangle { zoomlevel, points } => {
                    let mut result = SpatialIdSet::new();
                    let mut checked_coords = [Coordinate::default(); 3];
                    for (i, raw) in points.into_iter().enumerate() {
                        let coord = Coordinate::new(raw.latitude, raw.longitude, raw.altitude)?;
                        checked_coords[i] = coord;
                    }
                    for single_id in Triangle::new(checked_coords).cover_single_ids(*zoomlevel)? {
                        result.insert(single_id);
                    }
                    Ok(result)
                }
                crate::models::table::Geometry::Sphere {
                    zoomlevel,
                    radius_m,
                    center,
                } => {
                    let mut result = SpatialIdSet::new();
                    let checked_center =
                        Coordinate::new(center.latitude, center.longitude, center.altitude)?;
                    for single_id in
                        Sphere::new(checked_center, *radius_m)?.cover_single_ids(*zoomlevel)?
                    {
                        result.insert(single_id);
                    }
                    Ok(result)
                }
            },
            Query::SpatialIds(spatial_ids) => {
                let mut result = SpatialIdSet::new();
                for spatial_id in spatial_ids {
                    match spatial_id {
                        crate::models::table::SpatialId::SingleId { z, f, x, y } => {
                            result.insert(SingleId::new(*z, *f, *x, *y)?);
                        }
                        crate::models::table::SpatialId::RangeId { z, f, x, y } => {
                            result.insert(RangeId::new(*z, *f, *x, *y)?);
                        }
                    }
                }
                Ok(result)
            }
        }
    }
}
