use crate::id::{DimensionRange, SpaceTimeId};
use crate::id::points::Point;

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

    // Tests for Point struct
    #[test]
    fn test_point_debug() {
        let point = Point {
            latitude: 45.0,
            longitude: -122.0,
            altitude: 100.0,
        };
        
        let debug_str = format!("{:?}", point);
        assert!(debug_str.contains("latitude"));
        assert!(debug_str.contains("longitude"));
        assert!(debug_str.contains("altitude"));
        assert!(debug_str.contains("45.0"));
        assert!(debug_str.contains("-122.0"));
        assert!(debug_str.contains("100.0"));
    }

    // Tests for center() method
    #[test]
    fn test_center_single_point() {
        let id = create_test_id(2, 1, 1, 0);
        let center = id.center();
        
        // Center should be within valid coordinate ranges
        assert!(center.latitude >= -90.0 && center.latitude <= 90.0);
        assert!(center.longitude >= -180.0 && center.longitude <= 180.0);
        
        // For a single point, center should be the midpoint of the tile
        let coords = id.coordinates();
        let expected_lat = (coords.latitude.0 + coords.latitude.1) / 2.0;
        let expected_lng = (coords.longitude.0 + coords.longitude.1) / 2.0;
        let expected_alt = (coords.altitude.0 + coords.altitude.1) / 2.0;
        
        assert!((center.latitude - expected_lat).abs() < 1e-10);
        assert!((center.longitude - expected_lng).abs() < 1e-10);
        assert!((center.altitude - expected_alt).abs() < 1e-10);
    }

    #[test]
    fn test_center_with_ranges() {
        let id = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-2, 2),
            DimensionRange::LimitRange(1, 3),
            DimensionRange::LimitRange(2, 4),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let center = id.center();
        
        // Center should be within valid ranges
        assert!(center.latitude >= -90.0 && center.latitude <= 90.0);
        assert!(center.longitude >= -180.0 && center.longitude <= 180.0);
        
        // Should be the average of the coordinate bounds
        let coords = id.coordinates();
        let expected_lat = (coords.latitude.0 + coords.latitude.1) / 2.0;
        let expected_lng = (coords.longitude.0 + coords.longitude.1) / 2.0;
        let expected_alt = (coords.altitude.0 + coords.altitude.1) / 2.0;
        
        assert!((center.latitude - expected_lat).abs() < 1e-10);
        assert!((center.longitude - expected_lng).abs() < 1e-10);
        assert!((center.altitude - expected_alt).abs() < 1e-10);
    }

    #[test]
    fn test_center_with_any_dimensions() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Single(1),
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let center = id.center();
        
        // Should handle Any dimensions gracefully
        assert!(center.latitude >= -90.0 && center.latitude <= 90.0);
        assert!(center.longitude >= -180.0 && center.longitude <= 180.0);
    }

    #[test]
    fn test_center_different_zoom_levels() {
        let low_zoom = create_test_id(1, 0, 0, 0);
        let high_zoom = create_test_id(4, 0, 0, 0);
        
        let center_low = low_zoom.center();
        let center_high = high_zoom.center();
        
        // Both should be valid coordinates
        assert!(center_low.latitude >= -90.0 && center_low.latitude <= 90.0);
        assert!(center_low.longitude >= -180.0 && center_low.longitude <= 180.0);
        assert!(center_high.latitude >= -90.0 && center_high.latitude <= 90.0);
        assert!(center_high.longitude >= -180.0 && center_high.longitude <= 180.0);
        
        // Different zoom levels may have different centers
        // (exact relationship depends on coordinate transformation)
    }

    #[test]
    fn test_center_negative_f_values() {
        let id = create_test_id(2, 1, 1, -2);
        let center = id.center();
        
        // Negative f should result in negative altitude
        assert!(center.altitude < 0.0);
        assert!(center.latitude >= -90.0 && center.latitude <= 90.0);
        assert!(center.longitude >= -180.0 && center.longitude <= 180.0);
    }

    #[test]
    fn test_center_positive_f_values() {
        let id = create_test_id(2, 1, 1, 2);
        let center = id.center();
        
        // Positive f should result in positive altitude
        assert!(center.altitude > 0.0);
        assert!(center.latitude >= -90.0 && center.latitude <= 90.0);
        assert!(center.longitude >= -180.0 && center.longitude <= 180.0);
    }

    // Tests for vertex() method
    #[test]
    fn test_vertex_count() {
        let id = create_test_id(2, 1, 1, 0);
        let vertices = id.vertex();
        
        // Should return exactly 8 vertices
        assert_eq!(vertices.len(), 8);
    }

    #[test]
    fn test_vertex_coordinates_valid() {
        let id = create_test_id(2, 1, 1, 0);
        let vertices = id.vertex();
        
        // All vertices should have valid coordinates
        for vertex in &vertices {
            assert!(vertex.latitude >= -90.0 && vertex.latitude <= 90.0);
            assert!(vertex.longitude >= -180.0 && vertex.longitude <= 180.0);
        }
    }

    #[test]
    fn test_vertex_covers_bounds() {
        let id = create_test_id(2, 1, 1, 0);
        let vertices = id.vertex();
        let coords = id.coordinates();
        
        // Find min/max values from vertices
        let mut min_lat = f64::INFINITY;
        let mut max_lat = f64::NEG_INFINITY;
        let mut min_lng = f64::INFINITY;
        let mut max_lng = f64::NEG_INFINITY;
        let mut min_alt = f64::INFINITY;
        let mut max_alt = f64::NEG_INFINITY;
        
        for vertex in &vertices {
            min_lat = min_lat.min(vertex.latitude);
            max_lat = max_lat.max(vertex.latitude);
            min_lng = min_lng.min(vertex.longitude);
            max_lng = max_lng.max(vertex.longitude);
            min_alt = min_alt.min(vertex.altitude);
            max_alt = max_alt.max(vertex.altitude);
        }
        
        // Vertices should span the coordinate bounds
        assert!((min_lat - coords.latitude.0.min(coords.latitude.1)).abs() < 1e-10);
        assert!((max_lat - coords.latitude.0.max(coords.latitude.1)).abs() < 1e-10);
        assert!((min_lng - coords.longitude.0).abs() < 1e-10);
        assert!((max_lng - coords.longitude.1).abs() < 1e-10);
        assert!((min_alt - coords.altitude.0).abs() < 1e-10);
        assert!((max_alt - coords.altitude.1).abs() < 1e-10);
    }

    #[test]
    fn test_vertex_all_combinations() {
        let id = create_test_id(2, 1, 1, 1);
        let vertices = id.vertex();
        let coords = id.coordinates();
        
        // Should have vertices at all 8 combinations of min/max lat/lng/alt
        let lat_values = [coords.latitude.0, coords.latitude.1];
        let lng_values = [coords.longitude.0, coords.longitude.1];
        let alt_values = [coords.altitude.0, coords.altitude.1];
        
        // Count how many vertices match each expected combination
        let mut found_combinations = 0;
        
        for &lat in &lat_values {
            for &lng in &lng_values {
                for &alt in &alt_values {
                    let found = vertices.iter().any(|v| {
                        (v.latitude - lat).abs() < 1e-10 &&
                        (v.longitude - lng).abs() < 1e-10 &&
                        (v.altitude - alt).abs() < 1e-10
                    });
                    if found {
                        found_combinations += 1;
                    }
                }
            }
        }
        
        assert_eq!(found_combinations, 8);
    }

    #[test]
    fn test_vertex_with_ranges() {
        let id = SpaceTimeId::new(
            3,
            DimensionRange::LimitRange(-1, 1),
            DimensionRange::LimitRange(1, 2),
            DimensionRange::LimitRange(2, 3),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let vertices = id.vertex();
        
        // Should still have 8 vertices
        assert_eq!(vertices.len(), 8);
        
        // All should be valid coordinates
        for vertex in &vertices {
            assert!(vertex.latitude >= -90.0 && vertex.latitude <= 90.0);
            assert!(vertex.longitude >= -180.0 && vertex.longitude <= 180.0);
        }
    }

    #[test]
    fn test_vertex_with_any_dimensions() {
        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let vertices = id.vertex();
        
        // Should have 8 vertices covering the entire space
        assert_eq!(vertices.len(), 8);
        
        // Should span the full coordinate range
        let mut has_min_lng = false;
        let mut has_max_lng = false;
        
        for vertex in &vertices {
            if (vertex.longitude - (-180.0)).abs() < 1e-10 {
                has_min_lng = true;
            }
            if (vertex.longitude - 180.0).abs() < 1e-10 {
                has_max_lng = true;
            }
        }
        
        assert!(has_min_lng);
        assert!(has_max_lng);
    }

    #[test]
    fn test_vertex_negative_altitude() {
        let id = create_test_id(2, 1, 1, -2);
        let vertices = id.vertex();
        
        // All vertices should have negative altitude
        for vertex in &vertices {
            assert!(vertex.altitude < 0.0);
        }
    }

    #[test]
    fn test_vertex_zero_zoom() {
        let id = create_test_id(0, 0, 0, 0);
        let vertices = id.vertex();
        
        // Should work even at zoom 0
        assert_eq!(vertices.len(), 8);
        
        // Should cover the entire world
        let has_full_longitude_range = vertices.iter().any(|v| (v.longitude - (-180.0)).abs() < 1e-10) &&
                                      vertices.iter().any(|v| (v.longitude - 180.0).abs() < 1e-10);
        assert!(has_full_longitude_range);
    }

    #[test]
    fn test_vertex_high_zoom() {
        let id = create_test_id(10, 512, 256, 100);
        let vertices = id.vertex();
        
        // Should work at high zoom levels
        assert_eq!(vertices.len(), 8);
        
        // All vertices should be valid
        for vertex in &vertices {
            assert!(vertex.latitude >= -90.0 && vertex.latitude <= 90.0);
            assert!(vertex.longitude >= -180.0 && vertex.longitude <= 180.0);
        }
    }

    // Tests for relationship between center and vertex
    #[test]
    fn test_center_within_vertex_bounds() {
        let id = create_test_id(3, 2, 3, 1);
        let center = id.center();
        let vertices = id.vertex();
        
        // Find bounds from vertices
        let min_lat = vertices.iter().map(|v| v.latitude).fold(f64::INFINITY, f64::min);
        let max_lat = vertices.iter().map(|v| v.latitude).fold(f64::NEG_INFINITY, f64::max);
        let min_lng = vertices.iter().map(|v| v.longitude).fold(f64::INFINITY, f64::min);
        let max_lng = vertices.iter().map(|v| v.longitude).fold(f64::NEG_INFINITY, f64::max);
        let min_alt = vertices.iter().map(|v| v.altitude).fold(f64::INFINITY, f64::min);
        let max_alt = vertices.iter().map(|v| v.altitude).fold(f64::NEG_INFINITY, f64::max);
        
        // Center should be within the vertex bounds
        assert!(center.latitude >= min_lat && center.latitude <= max_lat);
        assert!(center.longitude >= min_lng && center.longitude <= max_lng);
        assert!(center.altitude >= min_alt && center.altitude <= max_alt);
    }

    #[test]
    fn test_center_is_actual_midpoint() {
        let id = create_test_id(2, 1, 1, 0);
        let center = id.center();
        let vertices = id.vertex();
        
        // Calculate expected center from vertices
        let avg_lat = vertices.iter().map(|v| v.latitude).sum::<f64>() / 8.0;
        let avg_lng = vertices.iter().map(|v| v.longitude).sum::<f64>() / 8.0;
        let avg_alt = vertices.iter().map(|v| v.altitude).sum::<f64>() / 8.0;
        
        // Center should be the average of all vertices
        assert!((center.latitude - avg_lat).abs() < 1e-10);
        assert!((center.longitude - avg_lng).abs() < 1e-10);
        assert!((center.altitude - avg_alt).abs() < 1e-10);
    }

    // Tests for consistency
    #[test]
    fn test_points_consistency() {
        let id = create_test_id(3, 2, 3, 1);
        
        // Multiple calls should return same results
        let center1 = id.center();
        let center2 = id.center();
        let vertices1 = id.vertex();
        let vertices2 = id.vertex();
        
        assert!((center1.latitude - center2.latitude).abs() < 1e-10);
        assert!((center1.longitude - center2.longitude).abs() < 1e-10);
        assert!((center1.altitude - center2.altitude).abs() < 1e-10);
        
        for i in 0..8 {
            assert!((vertices1[i].latitude - vertices2[i].latitude).abs() < 1e-10);
            assert!((vertices1[i].longitude - vertices2[i].longitude).abs() < 1e-10);
            assert!((vertices1[i].altitude - vertices2[i].altitude).abs() < 1e-10);
        }
    }
}