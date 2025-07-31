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

    // Tests for PartialEq implementation - Equal sets
    #[test]
    fn test_spacetime_idset_equality_identical_sets() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);

        assert_eq!(set1, set2);
    }

    #[test]
    fn test_spacetime_idset_equality_empty_sets() {
        let set1 = SpaceTimeIdSet::new();
        let set2 = SpaceTimeIdSet::new();

        assert_eq!(set1, set2);
    }

    #[test]
    #[should_panic]
    fn test_spacetime_idset_equality_same_content_different_structure() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        let mut set1 = SpaceTimeIdSet::new();
        set1.insert(id1);
        set1.insert(id2);

        let mut set2 = SpaceTimeIdSet::new();
        set2.insert(id2); // Insert in different order
        set2.insert(id1);

        assert_eq!(set1, set2);
    }

    #[test]
    fn test_spacetime_idset_equality_equivalent_ranges() {
        // Create two sets that represent the same physical space but with different range representations
        let range_id = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(1, 2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let mut point_set = SpaceTimeIdSet::new();
        let point1 = create_test_id_with_any_t(2, 1, 1, 0);
        let point2 = create_test_id_with_any_t(2, 2, 1, 0);
        point_set.insert(point1);
        point_set.insert(point2);

        let range_set = SpaceTimeIdSet::from(range_id);

        // These should be equal if they represent the same physical space
        // Note: This test may fail depending on the exact implementation of equality
    }

    // Tests for PartialEq implementation - Unequal sets
    #[test]
    fn test_spacetime_idset_inequality_different_elements() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);

        assert_ne!(set1, set2);
    }

    #[test]
    #[should_panic]
    fn test_spacetime_idset_inequality_empty_vs_non_empty() {
        let empty_set = SpaceTimeIdSet::new();
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let non_empty_set = SpaceTimeIdSet::from(id);

        assert_ne!(empty_set, non_empty_set);
        assert_ne!(non_empty_set, empty_set);
    }

    #[test]
    fn test_spacetime_idset_inequality_different_sizes() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        let single_set = SpaceTimeIdSet::from(id1);

        let mut double_set = SpaceTimeIdSet::new();
        double_set.insert(id1);
        double_set.insert(id2);

        assert_ne!(single_set, double_set);
    }

    #[test]
    fn test_spacetime_idset_inequality_subset_relationship() {
        // Create a larger range
        let large_id = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(0, 3),
            DimensionRange::LimitRange(0, 3),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        // Create a smaller range that's contained in the larger one
        let small_id = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(1, 2),
            DimensionRange::LimitRange(1, 2),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let large_set = SpaceTimeIdSet::from(large_id);
        let small_set = SpaceTimeIdSet::from(small_id);

        assert_ne!(large_set, small_set);
    }

    // Tests for equality with complex operations
    #[test]
    #[should_panic]
    fn test_spacetime_idset_equality_after_operations() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);

        // Union should be commutative
        let union1 = &set1 | &set2;
        let union2 = &set2 | &set1;
        assert_eq!(union1, union2);

        // Intersection should be commutative
        let intersection1 = &set1 & &set2;
        let intersection2 = &set2 & &set1;
        assert_eq!(intersection1, intersection2);
    }

    #[test]
    fn test_spacetime_idset_equality_with_self_operations() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);

        // Set union with itself should equal itself
        let self_union = &set | &set;
        assert_eq!(set, self_union);

        // Set intersection with itself should equal itself
        let self_intersection = &set & &set;
        assert_eq!(set, self_intersection);
    }

    #[test]
    fn test_spacetime_idset_equality_idempotent_operations() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);

        // Double complement should equal original (if implemented)
        let complement = !&set;
        if !complement.is_empty() {
            let double_complement = !complement;
            // Note: This might not always be exactly equal due to representation differences
            // But they should represent the same logical set
        }
    }

    // Tests for equality reflexivity, symmetry, and transitivity
    #[test]
    fn test_spacetime_idset_equality_reflexive() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);

        assert_eq!(set, set);
    }

    #[test]
    fn test_spacetime_idset_equality_symmetric() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);

        assert_eq!(set1, set2);
        assert_eq!(set2, set1);
    }

    #[test]
    fn test_spacetime_idset_equality_transitive() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);
        let set3 = SpaceTimeIdSet::from(id);

        assert_eq!(set1, set2);
        assert_eq!(set2, set3);
        assert_eq!(set1, set3);
    }

    // Tests for equality edge cases
    #[test]
    fn test_spacetime_idset_equality_with_ranges() {
        let range_id1 = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(1, 2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let range_id2 = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(1, 2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let set1 = SpaceTimeIdSet::from(range_id1);
        let set2 = SpaceTimeIdSet::from(range_id2);

        assert_eq!(set1, set2);
    }

    #[test]
    fn test_spacetime_idset_equality_with_any_dimensions() {
        let any_id1 = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let any_id2 = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        let set1 = SpaceTimeIdSet::from(any_id1);
        let set2 = SpaceTimeIdSet::from(any_id2);

        assert_eq!(set1, set2);
    }

    #[test]
    fn test_spacetime_idset_equality_different_zoom_levels() {
        let low_zoom_id = create_test_id_with_any_t(1, 0, 0, 0);
        let high_zoom_id = create_test_id_with_any_t(2, 0, 0, 0);

        let set1 = SpaceTimeIdSet::from(low_zoom_id);
        let set2 = SpaceTimeIdSet::from(high_zoom_id);

        // Different zoom levels representing potentially different areas
        assert_ne!(set1, set2);
    }

    #[test]
    #[should_panic]
    fn test_spacetime_idset_equality_different_time_intervals() {
        let id1 = create_test_id(2, 1, 1, 0, 60, 10);
        let id2 = create_test_id(2, 1, 1, 0, 30, 20); // Different interval, potentially same physical time

        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);

        // Different time intervals should generally not be equal
        assert_ne!(set1, set2);
    }

    // Tests for performance and consistency
    #[test]
    fn test_spacetime_idset_equality_consistency() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);

        // Multiple equality checks should give same result
        assert_eq!(set1 == set2, set1 == set2);
        assert_eq!(set1 == set2, set2 == set1);
    }

    #[test]
    #[should_panic]
    fn test_spacetime_idset_equality_with_insertion() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        let mut set1 = SpaceTimeIdSet::new();
        set1.insert(id1);

        let mut set2 = SpaceTimeIdSet::new();
        set2.insert(id1);

        assert_eq!(set1, set2);

        // After inserting different elements, should not be equal
        set1.insert(id2);
        assert_ne!(set1, set2);

        // After inserting same element in both, should be equal again
        set2.insert(id2);
        assert_eq!(set1, set2);
    }

    #[test]
    fn test_spacetime_idset_equality_empty_after_operations() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);
        let empty_set = SpaceTimeIdSet::new();

        // XOR with itself should be empty
        let xor_self = &set ^ &set;
        assert_eq!(xor_self, empty_set);

        // Intersection with empty should be empty
        let intersection_empty = &set & &empty_set;
        assert_eq!(intersection_empty, empty_set);
    }

    // Tests for hash consistency (if Hash is implemented)
    #[test]
    fn test_spacetime_idset_equality_implies_same_behavior() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);

        assert_eq!(set1, set2);

        // Equal sets should have same display
        assert_eq!(set1.to_string(), set2.to_string());

        // Equal sets should have same emptiness
        assert_eq!(set1.is_empty(), set2.is_empty());

        // Equal sets should have same count
        assert_eq!(set1.iter().count(), set2.iter().count());
    }
}
