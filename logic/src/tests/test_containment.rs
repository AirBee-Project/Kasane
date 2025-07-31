use crate::id::{DimensionRange, SpaceTimeId};
use crate::id::contain::Containment;

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a simple SpaceTimeId for testing
    fn create_test_id(z: u16, x: u64, y: u64, f: i64, i: u32, t: u32) -> SpaceTimeId {
        SpaceTimeId::new(
            z,
            DimensionRange::Single(f),
            DimensionRange::Single(x),
            DimensionRange::Single(y),
            i,
            DimensionRange::Single(t),
        ).unwrap()
    }

    fn create_test_id_with_any_t(z: u16, x: u64, y: u64, f: i64) -> SpaceTimeId {
        SpaceTimeId::new(
            z,
            DimensionRange::Single(f),
            DimensionRange::Single(x),
            DimensionRange::Single(y),
            0,
            DimensionRange::Any,
        ).unwrap()
    }

    // Tests for Containment enum
    #[test]
    fn test_containment_equality() {
        assert_eq!(Containment::Full, Containment::Full);
        assert_eq!(Containment::None, Containment::None);
        
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        assert_eq!(Containment::Partial(id), Containment::Partial(id));
        
        assert_ne!(Containment::Full, Containment::None);
    }

    #[test]
    fn test_containment_debug() {
        let debug_str = format!("{:?}", Containment::Full);
        assert_eq!(debug_str, "Full");
        
        let debug_str = format!("{:?}", Containment::None);
        assert_eq!(debug_str, "None");
        
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let debug_str = format!("{:?}", Containment::Partial(id));
        assert!(debug_str.starts_with("Partial("));
    }

    // Tests for containment_relation() - Full containment
    #[test]
    fn test_containment_relation_identical_ids() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 1, 1, 0);
        
        let result = id1.containment_relation(&id2);
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_point_in_any() {
        let any_id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let point_id = create_test_id_with_any_t(2, 1, 1, 0);
        
        let result = any_id.containment_relation(&point_id);
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_point_in_range() {
        let range_id = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-5, 5),
            DimensionRange::LimitRange(0, 5),
            DimensionRange::LimitRange(0, 5),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let point_id = create_test_id_with_any_t(3, 2, 3, 1);
        
        let result = range_id.containment_relation(&point_id);
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_range_in_larger_range() {
        let large_range = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-7, 7),
            DimensionRange::LimitRange(0, 7),
            DimensionRange::LimitRange(0, 7),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let small_range = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-2, 2),
            DimensionRange::LimitRange(2, 5),
            DimensionRange::LimitRange(1, 4),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let result = large_range.containment_relation(&small_range);
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_after_unlimit_contains_point() {
        let after_range = SpaceTimeId::new(
            3,
            DimensionRange::AfterUnLimitRange(3),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let point_id = create_test_id_with_any_t(3, 5, 1, 0);
        
        let result = after_range.containment_relation(&point_id);
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_before_unlimit_contains_point() {
        let before_range = SpaceTimeId::new(
            3,
            DimensionRange::BeforeUnLimitRange(5),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let point_id = create_test_id_with_any_t(3, 3, 1, 0);
        
        let result = before_range.containment_relation(&point_id);
        assert_eq!(result, Containment::Full);
    }

    // Tests for containment_relation() - No containment
    #[test]
    fn test_containment_relation_disjoint_points() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        
        let result = id1.containment_relation(&id2);
        assert_eq!(result, Containment::None);
    }

    #[test]
    fn test_containment_relation_disjoint_ranges() {
        let range1 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(0, 2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let range2 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(4, 6),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let result = range1.containment_relation(&range2);
        assert_eq!(result, Containment::None);
    }

    #[test]
    fn test_containment_relation_point_outside_range() {
        let range_id = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(0, 3),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let point_id = create_test_id_with_any_t(3, 5, 1, 0);
        
        let result = range_id.containment_relation(&point_id);
        assert_eq!(result, Containment::None);
    }

    #[test]
    fn test_containment_relation_different_y_dimension() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 1, 2, 0);
        
        let result = id1.containment_relation(&id2);
        assert_eq!(result, Containment::None);
    }

    #[test]
    fn test_containment_relation_different_f_dimension() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 1, 1, 1);
        
        let result = id1.containment_relation(&id2);
        assert_eq!(result, Containment::None);
    }

    #[test]
    fn test_containment_relation_different_time() {
        let id1 = create_test_id(2, 1, 1, 0, 60, 100);
        let id2 = create_test_id(2, 1, 1, 0, 60, 200);
        
        let result = id1.containment_relation(&id2);
        assert_eq!(result, Containment::None);
    }

    // Tests for containment_relation() - Partial containment
    #[test]
    fn test_containment_relation_overlapping_ranges() {
        let range1 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(0, 4),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let range2 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(2, 6),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let result = range1.containment_relation(&range2);
        match result {
            Containment::Partial(intersection) => {
                assert_eq!(intersection.f(), DimensionRange::LimitRange(2, 4));
                assert_eq!(intersection.x(), DimensionRange::Single(1));
                assert_eq!(intersection.y(), DimensionRange::Single(0));
            }
            _ => panic!("Expected Partial containment"),
        }
    }

    #[test]
    fn test_containment_relation_partial_after_unlimit() {
        let after_range = SpaceTimeId::new(
            3,
            DimensionRange::AfterUnLimitRange(3),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let limit_range = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(1, 5),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let result = after_range.containment_relation(&limit_range);
        match result {
            Containment::Partial(intersection) => {
                assert_eq!(intersection.x(), DimensionRange::LimitRange(3, 5));
            }
            _ => panic!("Expected Partial containment"),
        }
    }

    #[test]
    fn test_containment_relation_partial_before_unlimit() {
        let before_range = SpaceTimeId::new(
            3,
            DimensionRange::BeforeUnLimitRange(5),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let limit_range = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(3, 7),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let result = before_range.containment_relation(&limit_range);
        match result {
            Containment::Partial(intersection) => {
                assert_eq!(intersection.x(), DimensionRange::LimitRange(3, 5));
            }
            _ => panic!("Expected Partial containment"),
        }
    }

    // Tests for different zoom levels and intervals
    #[test]
    fn test_containment_relation_different_zoom_levels() {
        let low_zoom = create_test_id_with_any_t(2, 1, 1, 0);
        let high_zoom = create_test_id_with_any_t(3, 2, 2, 0); // Should map to same logical area
        
        let result = low_zoom.containment_relation(&high_zoom);
        // Should handle scaling appropriately
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_different_intervals() {
        let coarse_interval = create_test_id(2, 1, 1, 0, 120, 10);
        let fine_interval = create_test_id(2, 1, 1, 0, 60, 20); // Same logical time range
        
        let result = coarse_interval.containment_relation(&fine_interval);
        // Should handle time scaling appropriately
        assert_eq!(result, Containment::Full);
    }

    // Tests for edge cases
    #[test]
    fn test_containment_relation_zero_zoom() {
        let id1 = create_test_id_with_any_t(0, 0, 0, 0);
        let id2 = create_test_id_with_any_t(0, 0, 0, 0);
        
        let result = id1.containment_relation(&id2);
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_high_zoom() {
        let id1 = create_test_id_with_any_t(10, 512, 256, 100); // Use valid values for zoom 10
        let id2 = create_test_id_with_any_t(10, 512, 256, 100);
        
        let result = id1.containment_relation(&id2);
        assert_eq!(result, Containment::Full);
    }

    #[test]
    fn test_containment_relation_negative_f_values() {
        let range1 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-5, -2),
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let range2 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-7, -1),
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let result = range1.containment_relation(&range2);
        match result {
            Containment::Partial(intersection) => {
                assert_eq!(intersection.f(), DimensionRange::LimitRange(-5, -2));
            }
            _ => panic!("Expected Partial containment"),
        }
    }

    #[test]
    fn test_containment_relation_boundary_values() {
        let range1 = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(0, 3), // Full range for z=2
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let any_range = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let result = any_range.containment_relation(&range1);
        assert_eq!(result, Containment::Full);
    }

    // Tests for complex multi-dimensional overlaps
    #[test]
    fn test_containment_relation_multi_dimensional_partial() {
        let range1 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-4, 4),
            DimensionRange::LimitRange(0, 4),
            DimensionRange::LimitRange(0, 4),
            60,
            DimensionRange::LimitRange(10, 20),
        ).unwrap();
        
        let range2 = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-2, 6),
            DimensionRange::LimitRange(2, 6),
            DimensionRange::LimitRange(2, 6),
            60,
            DimensionRange::LimitRange(15, 25),
        ).unwrap();
        
        let result = range1.containment_relation(&range2);
        match result {
            Containment::Partial(intersection) => {
                assert_eq!(intersection.x(), DimensionRange::LimitRange(2, 4));
                assert_eq!(intersection.y(), DimensionRange::LimitRange(2, 4));
                assert_eq!(intersection.f(), DimensionRange::LimitRange(-2, 4));
                assert_eq!(intersection.t(), DimensionRange::LimitRange(15, 20));
            }
            _ => panic!("Expected Partial containment"),
        }
    }
}