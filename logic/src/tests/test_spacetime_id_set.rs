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

    // Tests for SpaceTimeIdSet::new()
    #[test]
    fn test_spacetime_idset_new() {
        let set = SpaceTimeIdSet::new();
        assert!(set.is_empty());
        assert_eq!(set.iter().count(), 0);
    }

    // Tests for SpaceTimeIdSet::is_empty()
    #[test]
    fn test_spacetime_idset_is_empty() {
        let mut set = SpaceTimeIdSet::new();
        assert!(set.is_empty());

        let id = create_test_id_with_any_t(2, 1, 1, 0);
        set.insert(id);
        assert!(!set.is_empty());
    }

    // Tests for SpaceTimeIdSet::iter()
    #[test]
    fn test_spacetime_idset_iter() {
        let mut set = SpaceTimeIdSet::new();
        assert_eq!(set.iter().count(), 0);

        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 0);
        set.insert(id1);
        set.insert(id2);

        assert_eq!(set.iter().count(), 2);
    }

    // Tests for From<SpaceTimeId> trait
    #[test]
    fn test_spacetime_idset_from_spacetime_id() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);

        assert!(!set.is_empty());
        assert_eq!(set.iter().count(), 1);
        assert_eq!(*set.iter().next().unwrap(), id);
    }

    // Tests for SpaceTimeIdSet::insert() - basic functionality
    #[test]
    fn test_spacetime_idset_insert_empty_set() {
        let mut set = SpaceTimeIdSet::new();
        let id = create_test_id_with_any_t(2, 1, 1, 0);

        set.insert(id);
        assert!(!set.is_empty());
        assert_eq!(set.iter().count(), 1);
        assert_eq!(*set.iter().next().unwrap(), id);
    }

    #[test]
    fn test_spacetime_idset_insert_non_overlapping() {
        let mut set = SpaceTimeIdSet::new();
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 0);

        set.insert(id1);
        set.insert(id2);

        assert_eq!(set.iter().count(), 2);
    }

    #[test]
    fn test_spacetime_idset_insert_duplicate() {
        let mut set = SpaceTimeIdSet::new();
        let id = create_test_id_with_any_t(2, 1, 1, 0);

        set.insert(id);
        set.insert(id); // Insert same ID again

        assert_eq!(set.iter().count(), 1); // Should still have only one
    }

    // Tests for Display trait
    #[test]
    fn test_spacetime_idset_display_empty() {
        let set = SpaceTimeIdSet::new();
        assert_eq!(set.to_string(), "");
    }

    #[test]
    fn test_spacetime_idset_display_single_element() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);
        assert_eq!(set.to_string(), "2/0/1/1_0/-");
    }

    #[test]
    fn test_spacetime_idset_display_multiple_elements() {
        let mut set = SpaceTimeIdSet::new();
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        set.insert(id1);
        set.insert(id2);

        let display_str = set.to_string();
        assert!(display_str.contains("2/0/1/1_0/-"));
        assert!(display_str.contains("2/1/2/2_0/-"));
        assert!(display_str.contains(", "));
    }

    // Tests for Debug trait
    #[test]
    fn test_spacetime_idset_debug_empty() {
        let set = SpaceTimeIdSet::new();
        let debug_str = format!("{:?}", set);
        assert_eq!(debug_str, "[]");
    }

    #[test]
    fn test_spacetime_idset_debug_with_element() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);
        let debug_str = format!("{:?}", set);

        // Debug should show individual coordinate combinations
        assert!(debug_str.contains("2/1/1/0"));
    }

    // Tests for IntoIterator trait
    #[test]
    fn test_spacetime_idset_into_iterator() {
        let mut set = SpaceTimeIdSet::new();
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        set.insert(id1);
        set.insert(id2);

        let mut count = 0;
        for _ in set {
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_spacetime_idset_into_iterator_ref() {
        let mut set = SpaceTimeIdSet::new();
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        set.insert(id1);
        set.insert(id2);

        let mut count = 0;
        for _ in &set {
            count += 1;
        }
        assert_eq!(count, 2);

        // Set should still be usable after borrowing
        assert_eq!(set.iter().count(), 2);
    }

    // Tests for FromIterator trait
    #[test]
    fn test_spacetime_idset_from_iterator() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let ids = vec![id1, id2];

        let set: SpaceTimeIdSet = ids.into_iter().collect();
        assert_eq!(set.iter().count(), 2);
    }

    #[test]
    fn test_spacetime_idset_from_iterator_empty() {
        let ids: Vec<SpaceTimeId> = vec![];
        let set: SpaceTimeIdSet = ids.into_iter().collect();
        assert!(set.is_empty());
    }

    #[test]
    fn test_spacetime_idset_from_iterator_with_duplicates() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let ids = vec![id, id, id]; // Same ID multiple times

        let set: SpaceTimeIdSet = ids.into_iter().collect();
        assert_eq!(set.iter().count(), 1); // Should deduplicate
    }

    // Tests for clone trait
    #[test]
    fn test_spacetime_idset_clone() {
        let mut set = SpaceTimeIdSet::new();
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);

        set.insert(id1);
        set.insert(id2);

        let cloned = set.clone();
        assert_eq!(set.iter().count(), cloned.iter().count());
        assert_eq!(set.to_string(), cloned.to_string());
    }

    // Edge cases and corner cases
    #[test]
    fn test_spacetime_idset_with_ranges() {
        let id = SpaceTimeId::new(
            3,
            DimensionRange::AfterUnLimitRange(-1),
            DimensionRange::LimitRange(0, 2),
            DimensionRange::Any,
            60,
            DimensionRange::BeforeUnLimitRange(10),
        )
        .unwrap();

        let set = SpaceTimeIdSet::from(id);
        assert!(!set.is_empty());
        assert_eq!(set.iter().count(), 1);
    }

    #[test]
    fn test_spacetime_idset_with_zero_zoom() {
        let id = create_test_id_with_any_t(0, 0, 0, 0);
        let set = SpaceTimeIdSet::from(id);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_spacetime_idset_with_high_zoom() {
        let id = create_test_id_with_any_t(10, 512, 256, 100);
        let set = SpaceTimeIdSet::from(id);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_spacetime_idset_with_negative_f() {
        let id = create_test_id_with_any_t(3, 1, 1, -5);
        let set = SpaceTimeIdSet::from(id);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_spacetime_idset_with_time_intervals() {
        let id = create_test_id(3, 1, 1, 0, 60, 100);
        let set = SpaceTimeIdSet::from(id);
        assert!(!set.is_empty());
    }

    // Tests for complex insertion scenarios
    #[test]
    fn test_spacetime_idset_insert_overlapping_ranges() {
        let mut set = SpaceTimeIdSet::new();

        // Insert a range
        let id1 = SpaceTimeId::new(
            2,
            DimensionRange::Single(0),
            DimensionRange::LimitRange(0, 2),
            DimensionRange::Single(1),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        // Insert an overlapping range
        let id2 = SpaceTimeId::new(
            2,
            DimensionRange::Single(0),
            DimensionRange::LimitRange(1, 3),
            DimensionRange::Single(1),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        set.insert(id1);
        set.insert(id2);

        // Should handle overlaps appropriately (exact behavior depends on implementation)
        assert!(!set.is_empty());
    }

    #[test]
    fn test_spacetime_idset_insert_contained_range() {
        let mut set = SpaceTimeIdSet::new();

        // Insert a larger range
        let id1 = SpaceTimeId::new(
            3,
            DimensionRange::Single(0),
            DimensionRange::LimitRange(0, 7),
            DimensionRange::Single(1),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        // Insert a smaller contained range
        let id2 = SpaceTimeId::new(
            3,
            DimensionRange::Single(0),
            DimensionRange::LimitRange(2, 4),
            DimensionRange::Single(1),
            0,
            DimensionRange::Any,
        )
        .unwrap();

        set.insert(id1);
        set.insert(id2);

        // The smaller range should not be added as it's contained in the larger one
        assert_eq!(set.iter().count(), 1);
    }

    #[test]
    fn test_spacetime_idset_insert_any_dimension() {
        let mut set = SpaceTimeIdSet::new();

        let id = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Any,
            DimensionRange::Any,
            0,
            DimensionRange::Any,
        )
        .unwrap();

        set.insert(id);
        assert_eq!(set.iter().count(), 1);

        // Try to insert a specific point - should be contained in Any
        let specific_id = create_test_id_with_any_t(2, 1, 1, 0);
        set.insert(specific_id);

        // Should still have only one element as specific is contained in Any
        assert_eq!(set.iter().count(), 1);
    }
}
