use std::collections::HashSet;

use kasane_logic::{
    function::triangle::triangle as other_triangle, id::SpaceTimeId, set::SpaceTimeIdSet,
};

use crate::json::input::Triangle;

pub fn triangle(v: Triangle) -> HashSet<SpaceTimeId> {
    other_triangle(v.zoom, v.point1, v.point2, v.point3)
}
