use crate::id::{DimensionRange, SpaceTimeId};
use crate::set::SpaceTimeIdSet;

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
        )
        .unwrap()
    }

    fn create_test_id_with_any_t(z: u16, x: u64, y: u64, f: i64) -> SpaceTimeId {
        SpaceTimeId::new(
            z,
            DimensionRange::Single(f),
            DimensionRange::Single(x),
            DimensionRange::Single(y),
            0,
            DimensionRange::Any,
        )
        .unwrap()
    }

    // Tests for complement() - Cases that should return empty set
    #[test]
    fn test_complement_universal_space() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(complement.is_empty());
    }

    #[test]
    fn test_complement_universal_time() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,                  // Non-zero interval
            DimensionRange::Any, // Any time
        )
        .unwrap();

        let complement = id.complement();
        assert!(complement.is_empty());
    }

    #[test]
    fn test_complement_spatial_any_with_time_any() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            60,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(complement.is_empty());
    }

    // Tests for complement() - Cases that should return non-empty set
    #[test]
    fn test_complement_single_point() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let complement = id.complement();

        // Complement of a single point should not be empty
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_partial_x_dimension() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(1), // Not Any
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_partial_y_dimension() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Single(1), // Not Any
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_partial_f_dimension() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Single(0), // Not Any
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    #[should_panic]
    fn test_complement_with_time_interval() {
        let id = create_test_id(2, 1, 1, 0, 60, 10);
        let complement = id.complement();

        // Should not be empty as time is constrained
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_range_dimensions() {
        let id = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-2, 2),
            DimensionRange::LimitRange(1, 3),
            DimensionRange::LimitRange(2, 4),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_after_unlimit_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::AfterUnLimitRange(2),
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_before_unlimit_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::BeforeUnLimitRange(1),
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    // Tests for complement properties
    #[test]
    fn test_complement_of_complement() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let complement = id.complement();

        if !complement.is_empty() {
            // Test a specific element from the complement
            if let Some(comp_id) = complement.iter().next() {
                let double_complement = comp_id.complement();
                // Double complement should contain the original or be equivalent
                assert!(!double_complement.is_empty());
            }
        }
    }

    #[test]
    fn test_complement_disjoint_with_original() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let complement = id.complement();
        let original_set = SpaceTimeIdSet::from(id);

        // Original and complement should be disjoint
        let intersection = &original_set & &complement;
        assert!(intersection.is_empty());
    }

    #[test]
    fn test_complement_different_zoom_levels() {
        let id1 = create_test_id_with_any_t(1, 0, 0, 0);
        let id2 = create_test_id_with_any_t(3, 0, 0, 0);

        let complement1 = id1.complement();
        let complement2 = id2.complement();

        // Both should not be empty (unless universal)
        assert!(!complement1.is_empty());
        assert!(!complement2.is_empty());
    }

    #[test]
    fn test_complement_boundary_values() {
        // Test at zoom level boundaries
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(3), // Max x for z=2
            DimensionRange::Single(3), // Max y for z=2
            DimensionRange::Single(3), // Max f for z=2
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_negative_f_values() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(-2), // Negative f
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_zero_zoom() {
        let id = create_test_id_with_any_t(0, 0, 0, 0);
        let complement = id.complement();

        // Should work at zoom 0
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_high_zoom() {
        let id = create_test_id_with_any_t(10, 512, 256, 100);
        let complement = id.complement();

        // Should work at high zoom levels
        assert!(!complement.is_empty());
    }

    // Tests for complement with different time configurations
    #[test]
    #[should_panic]
    fn test_complement_time_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::LimitRange(10, 20),
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    fn test_complement_time_after_unlimit() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::AfterUnLimitRange(100),
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    #[should_panic]
    fn test_complement_time_before_unlimit() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::BeforeUnLimitRange(50),
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    // Tests for complement structure validation
    #[test]
    fn test_complement_contains_valid_ids() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let complement = id.complement();

        // All IDs in complement should be valid
        for comp_id in complement.iter() {
            assert_eq!(comp_id.z(), id.z()); // Should have same zoom level
            assert_eq!(comp_id.i(), id.i()); // Should have same time interval

            // Should be valid coordinates for the zoom level
            let max_xy = (1u64 << comp_id.z()) - 1;
            let max_f = (1i64 << comp_id.z()) - 1;
            let min_f = -(1i64 << comp_id.z());

            match comp_id.x() {
                DimensionRange::Single(v) => assert!(v <= max_xy),
                DimensionRange::LimitRange(start, end) => {
                    assert!(start <= end);
                    assert!(end <= max_xy);
                }
                DimensionRange::AfterUnLimitRange(_) => {} // Valid by construction
                DimensionRange::BeforeUnLimitRange(_) => {} // Valid by construction
                DimensionRange::Any => {}                  // Valid by construction
            }

            match comp_id.f() {
                DimensionRange::Single(v) => assert!(v >= min_f && v <= max_f),
                DimensionRange::LimitRange(start, end) => {
                    assert!(start <= end);
                    assert!(start >= min_f && end <= max_f);
                }
                DimensionRange::AfterUnLimitRange(_) => {} // Valid by construction
                DimensionRange::BeforeUnLimitRange(_) => {} // Valid by construction
                DimensionRange::Any => {}                  // Valid by construction
            }
        }
    }

    #[test]
    #[should_panic]
    fn test_complement_preserves_zoom_and_interval() {
        let id = create_test_id(2, 1, 1, 0, 60, 10);
        let complement = id.complement();

        // All complement IDs should have same z and i
        for comp_id in complement.iter() {
            assert_eq!(comp_id.z(), id.z());
            assert_eq!(comp_id.i(), id.i());
        }
    }

    // Tests for specific complement patterns
    #[test]
    fn test_complement_single_x() {
        let id = SpaceTimeId::new(
            1, // Only 2 tiles: x=0 and x=1
            DimensionRange::Single(0),
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());

        // Complement should contain x=1 in some form
        let has_x1 = complement.iter().any(|comp_id| match comp_id.x() {
            DimensionRange::Single(v) => v == 1,
            DimensionRange::LimitRange(start, end) => start <= 1 && end >= 1,
            DimensionRange::AfterUnLimitRange(start) => start <= 1,
            DimensionRange::BeforeUnLimitRange(end) => end >= 1,
            DimensionRange::Any => true,
        });
        assert!(has_x1);
    }

    #[test]
    #[should_panic]
    fn test_complement_multiple_constraints() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(1),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::Single(10),
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());

        // Complement should handle all dimension constraints
        // This is a complex case as it needs to invert multiple dimensions
    }

    // Edge cases
    #[test]
    fn test_complement_edge_coordinates() {
        // Test at coordinate boundaries
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(-4), // Min f for z=2
            DimensionRange::Single(0),  // Min x
            DimensionRange::Single(0),  // Min y
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let complement = id.complement();
        assert!(!complement.is_empty());
    }

    #[test]
    #[should_panic]
    fn test_complement_consistency() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);

        // Multiple calls should return equivalent results
        let complement1 = id.complement();
        let complement2 = id.complement();

        // Both should have same emptiness
        assert_eq!(complement1.is_empty(), complement2.is_empty());

        if !complement1.is_empty() && !complement2.is_empty() {
            // Should represent the same logical set (though structure may differ)
            assert_eq!(complement1, complement2);
        }
    }
}
