use std::collections::HashSet;

use crate::function::triangle::triangle as function_triangle;
use crate::json::input::Triangle;
use crate::r#type::spacetimeid::SpaceTimeId;

pub fn triangle(v: Triangle) -> HashSet<SpaceTimeId> {
    function_triangle(v.zoom, v.point1, v.point2, v.point3)
}
