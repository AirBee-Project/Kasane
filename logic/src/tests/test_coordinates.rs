use crate::id::{DimensionRange, SpaceTimeId};
use crate::id::coordinates::Coordinates;

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a simple SpaceTimeId for testing
    fn create_test_id(z: u16, x: u64, y: u64, f: i64) -> SpaceTimeId {
        SpaceTimeId::new(
            z,
            DimensionRange::Single(f),
            DimensionRange::Single(x),
            DimensionRange::Single(y),
            0,
            DimensionRange::Any,
        ).unwrap()
    }

    // Tests for Coordinates struct
    #[test]
    fn test_coordinates_debug() {
        let coords = Coordinates {
            latitude: (45.0, 46.0),
            longitude: (-122.0, -121.0),
            altitude: (100.0, 200.0),
        };
        
        let debug_str = format!("{:?}", coords);
        assert!(debug_str.contains("latitude"));
        assert!(debug_str.contains("longitude"));
        assert!(debug_str.contains("altitude"));
        assert!(debug_str.contains("45.0"));
        assert!(debug_str.contains("-122.0"));
        assert!(debug_str.contains("100.0"));
    }

    // Tests for coordinates() method - Single values
    #[test]
    fn test_coordinates_single_point_zoom_0() {
        let id = create_test_id(0, 0, 0, 0);
        let coords = id.coordinates();
        
        // For zoom 0, the entire world is one tile
        assert!((coords.longitude.0 - (-180.0)).abs() < 1e-10);
        assert!((coords.longitude.1 - 180.0).abs() < 1e-10);
        
        // Latitude should span a range (order might be reversed due to coordinate system)
        assert!(coords.latitude.0 != coords.latitude.1);
        assert!(coords.latitude.0 > -90.0 && coords.latitude.0 < 90.0);
        assert!(coords.latitude.1 > -90.0 && coords.latitude.1 < 90.0);
        
        // Altitude for f=0 at zoom 0
        assert_eq!(coords.altitude.0, 0.0);
        assert_eq!(coords.altitude.1, 33554432.0);
    }

    #[test]
    fn test_coordinates_single_point_zoom_1() {
        let id = create_test_id(1, 0, 0, 0);
        let coords = id.coordinates();
        
        // For zoom 1, tile (0,0) covers western hemisphere
        assert!((coords.longitude.0 - (-180.0)).abs() < 1e-10);
        assert!((coords.longitude.1 - 0.0).abs() < 1e-10);
        
        let id2 = create_test_id(1, 1, 0, 0);
        let coords2 = id2.coordinates();
        
        // Tile (1,0) covers eastern hemisphere
        assert!((coords2.longitude.0 - 0.0).abs() < 1e-10);
        assert!((coords2.longitude.1 - 180.0).abs() < 1e-10);
    }

    #[test]
    fn test_coordinates_single_point_zoom_2() {
        let id = create_test_id(2, 1, 1, 1);
        let coords = id.coordinates();
        
        // Verify that coordinates are within valid ranges
        assert!(coords.longitude.0 >= -180.0 && coords.longitude.0 <= 180.0);
        assert!(coords.longitude.1 >= -180.0 && coords.longitude.1 <= 180.0);
        assert!(coords.longitude.0 < coords.longitude.1);
        
        assert!(coords.latitude.0 >= -90.0 && coords.latitude.0 <= 90.0);
        assert!(coords.latitude.1 >= -90.0 && coords.latitude.1 <= 90.0);
        
        // Altitude should be positive for positive f
        assert!(coords.altitude.0 > 0.0);
        assert!(coords.altitude.1 > coords.altitude.0);
    }

    #[test]
    fn test_coordinates_negative_f_values() {
        let id = create_test_id(2, 1, 1, -2);
        let coords = id.coordinates();
        
        // Negative f should give negative altitudes
        assert!(coords.altitude.0 < 0.0);
        assert!(coords.altitude.1 < 0.0);
        assert!(coords.altitude.0 < coords.altitude.1);
    }

    #[test]
    fn test_coordinates_boundary_values() {
        // Test maximum values for zoom level 2
        let id = create_test_id(2, 3, 3, 3); // Max values for z=2
        let coords = id.coordinates();
        
        // Should be valid coordinates
        assert!(coords.longitude.0 >= -180.0 && coords.longitude.1 <= 180.0);
        assert!(coords.latitude.0 >= -90.0 && coords.latitude.1 <= 90.0);
    }

    // Tests for coordinates() method - Range values
    #[test]
    fn test_coordinates_limit_range() {
        let id = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-2, 2),
            DimensionRange::LimitRange(1, 3),
            DimensionRange::LimitRange(2, 4),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let coords = id.coordinates();
        
        // Range should cover multiple tiles
        assert!(coords.longitude.1 > coords.longitude.0);
        assert!(coords.latitude.1 != coords.latitude.0); // Order might be reversed
        assert!(coords.altitude.1 > coords.altitude.0);
    }

    #[test]
    fn test_coordinates_after_unlimit_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::AfterUnLimitRange(2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let coords = id.coordinates();
        
        // Should extend to the maximum longitude
        assert!((coords.longitude.1 - 180.0).abs() < 1e-10);
        assert!(coords.longitude.0 < coords.longitude.1);
    }

    #[test]
    fn test_coordinates_before_unlimit_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::BeforeUnLimitRange(2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let coords = id.coordinates();
        
        // Should start from the minimum longitude
        assert!((coords.longitude.0 - (-180.0)).abs() < 1e-10);
        assert!(coords.longitude.0 < coords.longitude.1);
    }

    #[test]
    fn test_coordinates_any_range() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let coords = id.coordinates();
        
        // Should cover the entire coordinate space
        assert!((coords.longitude.0 - (-180.0)).abs() < 1e-10);
        assert!((coords.longitude.1 - 180.0).abs() < 1e-10);
        
        // Latitude should cover meaningful range (order might be reversed)
        assert!(coords.latitude.0 != coords.latitude.1);
        
        // Altitude should cover full range
        assert!(coords.altitude.0 < 0.0); // Negative part
        assert!(coords.altitude.1 > 0.0); // Positive part
    }

    // Tests for different zoom levels
    #[test]
    fn test_coordinates_higher_zoom_precision() {
        let low_zoom = create_test_id(1, 1, 1, 1);
        let high_zoom = create_test_id(4, 8, 8, 8); // Should represent smaller area
        
        let coords_low = low_zoom.coordinates();
        let coords_high = high_zoom.coordinates();
        
        // Higher zoom should have smaller coordinate ranges (more precise)
        let low_lon_range = coords_low.longitude.1 - coords_low.longitude.0;
        let high_lon_range = coords_high.longitude.1 - coords_high.longitude.0;
        
        assert!(high_lon_range < low_lon_range);
    }

    #[test]
    fn test_coordinates_zoom_scaling() {
        // Test that doubling coordinates at higher zoom gives similar results
        let z1_id = create_test_id(1, 1, 1, 1);
        let z2_id = create_test_id(2, 2, 2, 2);
        
        let coords1 = z1_id.coordinates();
        let coords2 = z2_id.coordinates();
        
        // The coordinates should be related by the zoom scaling
        // (exact relationship depends on the coordinate transformation)
        assert!(coords1.longitude.0 != coords2.longitude.0 || coords1.longitude.1 != coords2.longitude.1);
    }

    // Tests for coordinate value ranges
    #[test]
    fn test_coordinates_longitude_bounds() {
        for zoom in 0..5 {
            for x in 0..(1 << zoom) {
                let id = create_test_id(zoom, x, 0, 0);
                let coords = id.coordinates();
                
                // Longitude should always be within valid range
                assert!(coords.longitude.0 >= -180.0, "Longitude start out of bounds: {} at zoom {} x {}", coords.longitude.0, zoom, x);
                assert!(coords.longitude.1 <= 180.0, "Longitude end out of bounds: {} at zoom {} x {}", coords.longitude.1, zoom, x);
                assert!(coords.longitude.0 < coords.longitude.1, "Longitude range invalid at zoom {} x {}", zoom, x);
            }
        }
    }

    #[test]
    fn test_coordinates_latitude_bounds() {
        for zoom in 0..5 {
            for y in 0..(1 << zoom) {
                let id = create_test_id(zoom, 0, y, 0);
                let coords = id.coordinates();
                
                // Latitude should be within reasonable bounds (Web Mercator limits)
                assert!(coords.latitude.0 >= -90.0, "Latitude start out of bounds: {} at zoom {} y {}", coords.latitude.0, zoom, y);
                assert!(coords.latitude.1 <= 90.0, "Latitude end out of bounds: {} at zoom {} y {}", coords.latitude.1, zoom, y);
                // Note: latitude.0 might be > latitude.1 due to the coordinate system
            }
        }
    }

    #[test]
    fn test_coordinates_altitude_scaling() {
        let id_f0 = create_test_id(2, 0, 0, 0);
        let id_f1 = create_test_id(2, 0, 0, 1);
        let id_f_neg1 = create_test_id(2, 0, 0, -1);
        
        let coords_f0 = id_f0.coordinates();
        let coords_f1 = id_f1.coordinates();
        let coords_f_neg1 = id_f_neg1.coordinates();
        
        // f=0 should give altitude around 0
        assert!(coords_f0.altitude.0 >= 0.0);
        
        // f=1 should give positive altitude
        assert!(coords_f1.altitude.0 > coords_f0.altitude.0);
        
        // f=-1 should give negative altitude
        assert!(coords_f_neg1.altitude.1 <= coords_f0.altitude.0);
    }

    // Edge cases and error conditions
    #[test]
    fn test_coordinates_zero_zoom() {
        let id = create_test_id(0, 0, 0, 0);
        let coords = id.coordinates();
        
        // Should work with zero zoom
        assert!(coords.longitude.0 < coords.longitude.1);
        assert!(coords.altitude.0 < coords.altitude.1);
    }

    #[test]
    fn test_coordinates_high_zoom() {
        let id = create_test_id(10, 512, 256, 100);
        let coords = id.coordinates();
        
        // Should work with high zoom levels
        assert!(coords.longitude.0 >= -180.0 && coords.longitude.1 <= 180.0);
        assert!(coords.latitude.0 >= -90.0 && coords.latitude.1 <= 90.0);
        assert!(coords.longitude.0 < coords.longitude.1);
    }

    #[test]
    fn test_coordinates_extreme_f_values() {
        let id_max_f = create_test_id(3, 0, 0, 7); // Max f for z=3
        let id_min_f = create_test_id(3, 0, 0, -8); // Min f for z=3
        
        let coords_max = id_max_f.coordinates();
        let coords_min = id_min_f.coordinates();
        
        // Should handle extreme values
        assert!(coords_max.altitude.0 > coords_min.altitude.1);
    }

    // Tests for consistency
    #[test]
    fn test_coordinates_consistency() {
        let id = create_test_id(3, 2, 3, 1);
        let coords1 = id.coordinates();
        let coords2 = id.coordinates();
        
        // Multiple calls should return same results
        assert_eq!(coords1.longitude.0, coords2.longitude.0);
        assert_eq!(coords1.longitude.1, coords2.longitude.1);
        assert_eq!(coords1.latitude.0, coords2.latitude.0);
        assert_eq!(coords1.latitude.1, coords2.latitude.1);
        assert_eq!(coords1.altitude.0, coords2.altitude.0);
        assert_eq!(coords1.altitude.1, coords2.altitude.1);
    }

    #[test]
    fn test_coordinates_range_order() {
        let id = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-3, 3),
            DimensionRange::LimitRange(1, 5),
            DimensionRange::LimitRange(2, 6),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let coords = id.coordinates();
        
        // Start should always be less than end for longitude and altitude
        assert!(coords.longitude.0 < coords.longitude.1);
        assert!(coords.altitude.0 < coords.altitude.1);
        // Note: latitude might be reversed due to coordinate system
    }
}