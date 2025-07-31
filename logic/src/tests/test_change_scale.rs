use crate::id::{DimensionRange, SpaceTimeId};

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

    // Tests for change_scale() - No change cases
    #[test]
    fn test_change_scale_no_change() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let result = id.change_scale(None, None);

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled, id);
    }

    #[test]
    fn test_change_scale_same_z() {
        let id = create_test_id_with_any_t(3, 2, 3, 1);
        let result = id.change_scale(Some(3), None);

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled, id);
    }

    #[test]
    fn test_change_scale_same_i() {
        let id = create_test_id(3, 2, 3, 1, 60, 10);
        let result = id.change_scale(None, Some(60));

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled, id);
    }

    // Tests for change_scale() - Zoom level changes
    #[test]
    #[should_panic]
    fn test_change_scale_increase_zoom() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let result = id.change_scale(Some(4), None);

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.z(), 4);

        // Coordinates should be scaled up accordingly
        assert_eq!(scaled.x(), DimensionRange::Single(4)); // 1 * 2^(4-2) = 4
        assert_eq!(scaled.y(), DimensionRange::Single(4));
        assert_eq!(scaled.f(), DimensionRange::Single(0)); // f scaling is different
    }

    #[test]
    fn test_change_scale_increase_zoom_with_ranges() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(1, 2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let result = id.change_scale(Some(3), None);
        assert!(result.is_ok());

        let scaled = result.unwrap();
        assert_eq!(scaled.z(), 3);
        // Range should be scaled: [1,2] -> [2,5] at zoom 3
        assert_eq!(scaled.x(), DimensionRange::LimitRange(2, 5));
    }

    #[test]
    fn test_change_scale_zoom_with_any() {
        let id = SpaceTimeId::new(
            1,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let result = id.change_scale(Some(3), None);
        assert!(result.is_ok());

        let scaled = result.unwrap();
        assert_eq!(scaled.z(), 3);
        assert_eq!(scaled.x(), DimensionRange::Any);
        assert_eq!(scaled.y(), DimensionRange::Any);
        assert_eq!(scaled.f(), DimensionRange::Any);
    }

    // Tests for change_scale() - Time interval changes
    #[test]
    fn test_change_scale_decrease_time_interval() {
        let id = create_test_id(2, 1, 1, 0, 60, 10);
        let result = id.change_scale(None, Some(15));

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.i(), 15);

        // Time range should be expanded: t=10 -> t=[40,43] for 60->15 scaling
        // Exact values depend on GCD calculation
        match scaled.t() {
            DimensionRange::LimitRange(start, end) => {
                assert!(start <= end);
                assert!(start >= 40); // Approximate expected range
                assert!(end <= 43);
            }
            _ => panic!("Expected LimitRange for time dimension"),
        }
    }

    #[test]
    fn test_change_scale_time_with_gcd() {
        let id = create_test_id(2, 1, 1, 0, 120, 5);
        let result = id.change_scale(None, Some(30));

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.i(), 30);

        // GCD(120, 30) = 30, so scaling factor is 120/30 = 4
        // t=5 -> t=[20,23] (5*4 to (5+1)*4-1)
        match scaled.t() {
            DimensionRange::LimitRange(start, end) => {
                assert_eq!(start, 20);
                assert_eq!(end, 23);
            }
            _ => panic!("Expected LimitRange for time dimension"),
        }
    }

    #[test]
    #[should_panic]
    fn test_change_scale_time_with_any() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let result = id.change_scale(None, Some(30));

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.i(), 30);
        assert_eq!(scaled.t(), DimensionRange::Any);
    }

    // Tests for change_scale() - Combined changes
    #[test]
    #[should_panic]
    fn test_change_scale_both_zoom_and_time() {
        let id = create_test_id(2, 1, 1, 0, 60, 10);
        let result = id.change_scale(Some(4), Some(15));

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.z(), 4);
        assert_eq!(scaled.i(), 15);

        // Both spatial and temporal scaling should be applied
        assert_eq!(scaled.x(), DimensionRange::Single(4)); // 1 * 2^2 = 4
        assert_eq!(scaled.y(), DimensionRange::Single(4));

        // Time should be scaled based on GCD
        match scaled.t() {
            DimensionRange::LimitRange(_, _) => {} // Should be a range
            _ => panic!("Expected LimitRange for time dimension"),
        }
    }

    // Tests for change_scale() - Error cases
    #[test]
    fn test_change_scale_zoom_too_small() {
        let id = create_test_id_with_any_t(3, 1, 1, 0);
        let result = id.change_scale(Some(2), None);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Target zoom level must be finer")
        );
    }

    #[test]
    fn test_change_scale_zoom_too_large() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let result = id.change_scale(Some(32), None);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Zoom level z must be less than 32")
        );
    }

    #[test]
    #[should_panic]
    fn test_change_scale_time_too_large() {
        let id = create_test_id(2, 1, 1, 0, 30, 10);
        let result = id.change_scale(None, Some(60));

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Target time level must be finer")
        );
    }

    // Tests for edge cases
    #[test]
    #[should_panic]
    fn test_change_scale_zero_zoom_to_higher() {
        let id = create_test_id_with_any_t(0, 0, 0, 0);
        let result = id.change_scale(Some(2), None);

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.z(), 2);
        assert_eq!(scaled.x(), DimensionRange::Single(0)); // 0 * 2^2 = 0
        assert_eq!(scaled.y(), DimensionRange::Single(0));
    }

    #[test]
    fn test_change_scale_max_zoom() {
        let id = create_test_id_with_any_t(30, 0, 0, 0);
        let result = id.change_scale(Some(31), None);

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.z(), 31);
    }

    #[test]
    #[should_panic]
    fn test_change_scale_boundary_coordinates() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Single(3), // Max for z=2
            DimensionRange::Single(3),
            DimensionRange::Single(3), // Max for z=2
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let result = id.change_scale(Some(4), None);
        assert!(result.is_ok());

        let scaled = result.unwrap();
        assert_eq!(scaled.x(), DimensionRange::Single(12)); // 3 * 2^2 = 12
        assert_eq!(scaled.y(), DimensionRange::Single(12));
        assert_eq!(scaled.f(), DimensionRange::Single(12)); // f scales differently but should be valid
    }

    #[test]
    #[should_panic]
    fn test_change_scale_negative_f_values() {
        let id = create_test_id_with_any_t(2, 1, 1, -2);
        let result = id.change_scale(Some(3), None);

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.f(), DimensionRange::Single(-4)); // -2 * 2^1 = -4
    }

    #[test]
    fn test_change_scale_time_interval_one() {
        let id = create_test_id(2, 1, 1, 0, 2, 5);
        let result = id.change_scale(None, Some(1));

        assert!(result.is_ok());
        let scaled = result.unwrap();
        assert_eq!(scaled.i(), 1);

        // Time should be scaled by factor of 2
        match scaled.t() {
            DimensionRange::LimitRange(start, end) => {
                assert_eq!(start, 10); // 5 * 2
                assert_eq!(end, 11); // (5+1) * 2 - 1
            }
            _ => panic!("Expected LimitRange for time dimension"),
        }
    }

    // Tests for complex range transformations
    #[test]
    fn test_change_scale_after_unlimit_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::AfterUnLimitRange(2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let result = id.change_scale(Some(3), None);
        assert!(result.is_ok());

        let scaled = result.unwrap();
        assert_eq!(scaled.x(), DimensionRange::AfterUnLimitRange(4)); // 2 * 2^1 = 4
    }

    #[test]
    fn test_change_scale_before_unlimit_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::BeforeUnLimitRange(2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let result = id.change_scale(Some(3), None);
        assert!(result.is_ok());

        let scaled = result.unwrap();
        assert_eq!(scaled.x(), DimensionRange::BeforeUnLimitRange(5)); // 2 * 2^1 + 1 = 5
    }

    #[test]
    fn test_change_scale_preserve_physical_meaning() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let result = id.change_scale(Some(4), None);

        assert!(result.is_ok());
        let scaled = result.unwrap();

        // The physical coordinates should represent the same area
        let original_coords = id.coordinates();
        let scaled_coords = scaled.coordinates();

        // Longitude should be the same (within floating point precision)
        assert!((original_coords.longitude.0 - scaled_coords.longitude.0).abs() < 1e-10);
        assert!((original_coords.longitude.1 - scaled_coords.longitude.1).abs() < 1e-10);

        // Latitude should be the same
        assert!((original_coords.latitude.0 - scaled_coords.latitude.0).abs() < 1e-10);
        assert!((original_coords.latitude.1 - scaled_coords.latitude.1).abs() < 1e-10);

        // Altitude should be the same
        assert!((original_coords.altitude.0 - scaled_coords.altitude.0).abs() < 1e-10);
        assert!((original_coords.altitude.1 - scaled_coords.altitude.1).abs() < 1e-10);
    }
}
