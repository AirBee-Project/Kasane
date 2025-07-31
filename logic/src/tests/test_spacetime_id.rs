use crate::id::{DimensionRange, SpaceTimeId};

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for SpaceTimeId::new() - Success cases
    #[test]
    fn test_spacetime_id_new_valid() {
        let stid = SpaceTimeId::new(
            4,
            DimensionRange::Single(10),
            DimensionRange::Single(5),
            DimensionRange::Single(3),
            60,
            DimensionRange::Single(100),
        );
        assert!(stid.is_ok());
        
        let stid = stid.unwrap();
        assert_eq!(stid.z(), 4);
        assert_eq!(stid.f(), DimensionRange::Single(10));
        assert_eq!(stid.x(), DimensionRange::Single(5));
        assert_eq!(stid.y(), DimensionRange::Single(3));
        assert_eq!(stid.i(), 60);
        assert_eq!(stid.t(), DimensionRange::Single(100));
    }

    #[test]
    fn test_spacetime_id_new_with_ranges() {
        let stid = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-2, 4),
            DimensionRange::LimitRange(0, 3),
            DimensionRange::LimitRange(1, 5),
            30,
            DimensionRange::LimitRange(10, 20),
        );
        assert!(stid.is_ok());
    }

    #[test]
    fn test_spacetime_id_new_with_any() {
        let stid = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        );
        assert!(stid.is_ok());
    }

    // Tests for SpaceTimeId::new() - Error cases
    #[test]
    fn test_spacetime_id_new_z_too_large() {
        let result = SpaceTimeId::new(
            32,
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Zoom level z must be less than 32"));
    }

    #[test]
    fn test_spacetime_id_new_z_at_boundary() {
        let result = SpaceTimeId::new(
            31,
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_spacetime_id_new_x_out_of_bounds() {
        let result = SpaceTimeId::new(
            2,
            DimensionRange::Single(0), // f
            DimensionRange::Single(4), // x - max is 3 for z=2
            DimensionRange::Single(0), // y
            0,
            DimensionRange::Any,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid x dimension"));
    }

    #[test]
    fn test_spacetime_id_new_y_out_of_bounds() {
        let result = SpaceTimeId::new(
            2,
            DimensionRange::Single(0), // f
            DimensionRange::Single(0), // x
            DimensionRange::Single(5), // y - max is 3 for z=2
            0,
            DimensionRange::Any,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid y dimension"));
    }

    #[test]
    fn test_spacetime_id_new_f_out_of_bounds_positive() {
        let result = SpaceTimeId::new(
            2,
            DimensionRange::Single(4), // f - max is 3 for z=2
            DimensionRange::Single(0), // x
            DimensionRange::Single(0), // y
            0,
            DimensionRange::Any,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid f dimension"));
    }

    #[test]
    fn test_spacetime_id_new_f_out_of_bounds_negative() {
        let result = SpaceTimeId::new(
            2,
            DimensionRange::Single(-5), // min is -4 for z=2
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid f dimension"));
    }

    #[test]
    fn test_spacetime_id_new_range_start_greater_than_end() {
        let result = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(5, 2), // start > end
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("start value 5 cannot be greater than end value 2"));
    }

    #[test]
    fn test_spacetime_id_new_t_not_any_when_i_zero() {
        let result = SpaceTimeId::new(
            2,
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            0,
            DimensionRange::Single(100), // must be Any when i=0
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("t must be Any when time interval i is 0"));
    }

    // Tests for normalization
    #[test]
    fn test_spacetime_id_normalization_equal_range_to_single() {
        let stid = SpaceTimeId::new(
            3,
            DimensionRange::Single(0), // f
            DimensionRange::LimitRange(2, 2), // x - equal start and end
            DimensionRange::Single(0), // y
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.x(), DimensionRange::Single(2));
    }

    #[test]
    fn test_spacetime_id_normalization_full_range_to_any() {
        let stid = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(-4, 3), // full range for z=2
            DimensionRange::LimitRange(0, 3), // full range for z=2
            DimensionRange::Single(1),
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.f(), DimensionRange::Any);
        assert_eq!(stid.x(), DimensionRange::Any);
    }

    #[test]
    fn test_spacetime_id_normalization_start_zero_to_before_unlimit() {
        let stid = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-8, 5), // start is min_f (-8) for z=3
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.f(), DimensionRange::BeforeUnLimitRange(5));
    }

    #[test]
    fn test_spacetime_id_normalization_end_max_to_after_unlimit() {
        let stid = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(2, 7), // end is max (7) for z=3
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.f(), DimensionRange::AfterUnLimitRange(2));
    }

    #[test]
    fn test_spacetime_id_normalization_after_unlimit_start_zero() {
        let stid = SpaceTimeId::new(
            2,
            DimensionRange::AfterUnLimitRange(-4), // start is min_f (-4) for z=2
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.f(), DimensionRange::Any);
    }

    #[test]
    fn test_spacetime_id_normalization_after_unlimit_start_max() {
        let stid = SpaceTimeId::new(
            2,
            DimensionRange::Single(0), // f
            DimensionRange::AfterUnLimitRange(3), // x - start is max (3) for z=2
            DimensionRange::Single(0), // y
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.x(), DimensionRange::Single(3));
    }

    #[test]
    fn test_spacetime_id_normalization_before_unlimit_end_max() {
        let stid = SpaceTimeId::new(
            2,
            DimensionRange::BeforeUnLimitRange(3), // end is max (3) for z=2
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.f(), DimensionRange::Any);
    }

    #[test]
    fn test_spacetime_id_normalization_before_unlimit_end_zero() {
        let stid = SpaceTimeId::new(
            2,
            DimensionRange::Single(0), // f
            DimensionRange::BeforeUnLimitRange(0), // x - end is 0 (minimum for x)
            DimensionRange::Single(0), // y
            60,
            DimensionRange::Single(10),
        ).unwrap();
        
        assert_eq!(stid.x(), DimensionRange::Single(0));
    }

    // Tests for getter functions
    #[test]
    fn test_spacetime_id_getters() {
        let stid = SpaceTimeId::new(
            5,
            DimensionRange::BeforeUnLimitRange(-5),
            DimensionRange::LimitRange(10, 20),
            DimensionRange::AfterUnLimitRange(15),
            120,
            DimensionRange::Any,
        ).unwrap();

        assert_eq!(stid.z(), 5);
        assert_eq!(stid.f(), DimensionRange::BeforeUnLimitRange(-5));
        assert_eq!(stid.x(), DimensionRange::LimitRange(10, 20));
        assert_eq!(stid.y(), DimensionRange::AfterUnLimitRange(15));
        assert_eq!(stid.i(), 120);
        assert_eq!(stid.t(), DimensionRange::Any);
    }

    // Tests for Display trait
    #[test]
    fn test_spacetime_id_display() {
        let stid = SpaceTimeId::new(
            4,
            DimensionRange::Single(5),
            DimensionRange::Single(3),
            DimensionRange::Single(10),
            60,
            DimensionRange::Single(100),
        ).unwrap();

        assert_eq!(stid.to_string(), "4/5/3/10_60/100");
    }

    #[test]
    fn test_spacetime_id_display_with_ranges() {
        let stid = SpaceTimeId::new(
            3,
            DimensionRange::AfterUnLimitRange(-2),
            DimensionRange::LimitRange(1, 5),
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        ).unwrap();

        assert_eq!(stid.to_string(), "3/-2:-/1:5/-_0/-");
    }

    // Tests for equality and cloning
    #[test]
    fn test_spacetime_id_equality() {
        let stid1 = SpaceTimeId::new(
            4,
            DimensionRange::Single(10),
            DimensionRange::Single(5),
            DimensionRange::Single(3),
            60,
            DimensionRange::Single(100),
        ).unwrap();

        let stid2 = SpaceTimeId::new(
            4,
            DimensionRange::Single(10),
            DimensionRange::Single(5),
            DimensionRange::Single(3),
            60,
            DimensionRange::Single(100),
        ).unwrap();

        let stid3 = SpaceTimeId::new(
            4,
            DimensionRange::Single(10),
            DimensionRange::Single(6), // different x
            DimensionRange::Single(3),
            60,
            DimensionRange::Single(100),
        ).unwrap();

        assert_eq!(stid1, stid2);
        assert_ne!(stid1, stid3);
    }

    #[test]
    fn test_spacetime_id_clone() {
        let stid = SpaceTimeId::new(
            4,
            DimensionRange::Single(5),
            DimensionRange::Single(3),
            DimensionRange::Single(10),
            60,
            DimensionRange::Single(100),
        ).unwrap();

        let cloned = stid.clone();
        assert_eq!(stid, cloned);
    }

    // Edge cases and boundary conditions
    #[test]
    fn test_spacetime_id_zero_zoom() {
        let stid = SpaceTimeId::new(
            0,
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        );
        assert!(stid.is_ok());
        
        let stid = stid.unwrap();
        assert_eq!(stid.x(), DimensionRange::Single(0));
        assert_eq!(stid.y(), DimensionRange::Single(0));
        assert_eq!(stid.f(), DimensionRange::Single(0));
    }

    #[test]
    fn test_spacetime_id_max_zoom() {
        let stid = SpaceTimeId::new(
            31,
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        );
        assert!(stid.is_ok());
    }

    #[test]
    fn test_spacetime_id_large_values() {
        let stid = SpaceTimeId::new(
            10,
            DimensionRange::Single(1023), // 2^10 - 1
            DimensionRange::Single(1023),
            DimensionRange::Single(1023), // 2^10 - 1
            u32::MAX,
            DimensionRange::Single(u32::MAX),
        );
        assert!(stid.is_ok());
    }

    #[test]
    fn test_spacetime_id_negative_f_bounds() {
        let stid = SpaceTimeId::new(
            3,
            DimensionRange::Single(-8), // min for z=3 is -8
            DimensionRange::Single(0),
            DimensionRange::Single(0),
            60,
            DimensionRange::Single(0),
        );
        assert!(stid.is_ok());
    }
}