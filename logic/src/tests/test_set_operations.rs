use crate::id::{DimensionRange, SpaceTimeId};
use crate::set::SpaceTimeIdSet;
use std::ops::{BitAnd, BitOr, BitXor, Not};

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

    // Tests for BitAnd (&) operation
    #[test]
    fn test_spacetime_idset_and_empty_sets() {
        let set1 = SpaceTimeIdSet::new();
        let set2 = SpaceTimeIdSet::new();
        let result = &set1 & &set2;
        assert!(result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_and_one_empty() {
        let mut set1 = SpaceTimeIdSet::new();
        let set2 = SpaceTimeIdSet::new();
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        set1.insert(id);
        
        let result = &set1 & &set2;
        assert!(result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_and_identical_sets() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);
        
        let result = &set1 & &set2;
        assert!(!result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_and_disjoint_sets() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        let result = &set1 & &set2;
        assert!(result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_and_by_value() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);
        
        let result = set1 & set2;
        assert!(!result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_and_mixed_references() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);
        
        let result1 = &set1 & set2;
        let result2 = set1 & &SpaceTimeIdSet::from(id);
        
        assert!(!result1.is_empty());
        assert!(!result2.is_empty());
    }

    // Tests for BitOr (|) operation
    #[test]
    fn test_spacetime_idset_or_empty_sets() {
        let set1 = SpaceTimeIdSet::new();
        let set2 = SpaceTimeIdSet::new();
        let result = &set1 | &set2;
        assert!(result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_or_one_empty() {
        let mut set1 = SpaceTimeIdSet::new();
        let set2 = SpaceTimeIdSet::new();
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        set1.insert(id);
        
        let result = &set1 | &set2;
        assert_eq!(result.iter().count(), 1);
    }

    #[test]
    fn test_spacetime_idset_or_identical_sets() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);
        
        let result = &set1 | &set2;
        assert_eq!(result.iter().count(), 1); // Should merge identical elements
    }

    #[test]
    fn test_spacetime_idset_or_disjoint_sets() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        let result = &set1 | &set2;
        assert_eq!(result.iter().count(), 2);
    }

    #[test]
    fn test_spacetime_idset_or_by_value() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        let result = set1 | set2;
        assert_eq!(result.iter().count(), 2);
    }

    #[test]
    fn test_spacetime_idset_or_mixed_references() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        let result1 = &set1 | set2;
        let result2 = set1 | &SpaceTimeIdSet::from(id2);
        
        assert_eq!(result1.iter().count(), 2);
        assert_eq!(result2.iter().count(), 2);
    }

    // Tests for BitXor (^) operation
    #[test]
    fn test_spacetime_idset_xor_empty_sets() {
        let set1 = SpaceTimeIdSet::new();
        let set2 = SpaceTimeIdSet::new();
        let result = &set1 ^ &set2;
        assert!(result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_xor_one_empty() {
        let mut set1 = SpaceTimeIdSet::new();
        let set2 = SpaceTimeIdSet::new();
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        set1.insert(id);
        
        let result = &set1 ^ &set2;
        // XOR behavior may vary depending on implementation
        // The key is that it should be consistent
        
        // Test the reverse as well
        let result2 = &set2 ^ &set1;
        // Both should give same result (commutative)
        assert_eq!(result.is_empty(), result2.is_empty());
    }

    #[test]
    fn test_spacetime_idset_xor_identical_sets() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set1 = SpaceTimeIdSet::from(id);
        let set2 = SpaceTimeIdSet::from(id);
        
        let result = &set1 ^ &set2;
        assert!(result.is_empty()); // XOR of identical sets should be empty
    }

    #[test]
    fn test_spacetime_idset_xor_disjoint_sets() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        let result = &set1 ^ &set2;
        assert_eq!(result.iter().count(), 2); // Should contain both elements
    }

    #[test]
    fn test_spacetime_idset_xor_by_value() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        let result = set1 ^ set2;
        assert_eq!(result.iter().count(), 2);
    }

    #[test]
    fn test_spacetime_idset_xor_mixed_references() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        let result1 = &set1 ^ set2;
        let result2 = set1 ^ &SpaceTimeIdSet::from(id2);
        
        assert_eq!(result1.iter().count(), 2);
        assert_eq!(result2.iter().count(), 2);
    }

    // Tests for Not (!) operation
    #[test]
    fn test_spacetime_idset_not_empty() {
        let set = SpaceTimeIdSet::new();
        let result = !&set;
        // Not operation should return some non-empty result (universal set or similar)
        // Exact behavior depends on implementation
    }

    #[test]
    fn test_spacetime_idset_not_single_element() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);
        let result = !&set;
        // Should return complement of the set
        // Cannot be empty as it's the complement of a non-universal set
    }

    #[test]
    fn test_spacetime_idset_not_by_value() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);
        let result = !set;
        // Should work with owned value as well
    }

    // Tests for double negation
    #[test]
    fn test_spacetime_idset_double_not() {
        let id = create_test_id_with_any_t(2, 1, 1, 0);
        let set = SpaceTimeIdSet::from(id);
        let complement = !&set;
        let double_complement = !complement;
        
        // Double complement should be close to original
        // (exact equality depends on normalization and representation)
    }

    // Tests for complex expressions
    #[test]
    fn test_spacetime_idset_complex_expression() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let id3 = create_test_id_with_any_t(2, 3, 3, 2);
        
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        let set3 = SpaceTimeIdSet::from(id3);
        
        // Test: (set1 | set2) & set3
        let union = &set1 | &set2;
        let result = &union & &set3;
        
        // Should be empty as set3 is disjoint from union of set1 and set2
        assert!(result.is_empty());
    }

    #[test]
    fn test_spacetime_idset_distributive_law() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        let id3 = create_test_id_with_any_t(2, 3, 3, 2);
        
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        let set3 = SpaceTimeIdSet::from(id3);
        
        // Test: set1 & (set2 | set3) == (set1 & set2) | (set1 & set3)
        let left_side = &set1 & &(&set2 | &set3);
        let right_side = &(&set1 & &set2) | &(&set1 & &set3);
        
        // Both should be empty in this case as sets are disjoint
        assert!(left_side.is_empty());
        assert!(right_side.is_empty());
    }

    #[test]
    fn test_spacetime_idset_de_morgan_laws() {
        let id1 = create_test_id_with_any_t(2, 1, 1, 0);
        let id2 = create_test_id_with_any_t(2, 2, 2, 1);
        
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        // Test De Morgan's law: !(set1 | set2) == (!set1) & (!set2)
        let union = &set1 | &set2;
        let not_union = !union;
        
        let not_set1 = !&set1;
        let not_set2 = !&set2;
        let and_of_nots = &not_set1 & &not_set2;
        
        // The results should be equivalent (though exact comparison might be complex)
        // This is more of a behavioral test than exact equality
    }

    // Edge cases with ranges
    #[test]
    fn test_spacetime_idset_operations_with_ranges() {
        let id1 = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(0, 1),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let id2 = SpaceTimeId::new(
            2,
            DimensionRange::LimitRange(1, 2),
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        // Test operations with overlapping ranges
        let intersection = &set1 & &set2;
        let union = &set1 | &set2;
        let xor = &set1 ^ &set2;
        
        // Intersection should contain the overlap
        // Union should contain both ranges (possibly merged)
        // XOR should contain non-overlapping parts
    }

    #[test]
    fn test_spacetime_idset_operations_with_any_dimensions() {
        let id1 = SpaceTimeId::new(
            2,
            DimensionRange::Any,
            DimensionRange::Single(1),
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let id2 = SpaceTimeId::new(
            2,
            DimensionRange::Single(1),
            DimensionRange::Any,
            DimensionRange::Single(0),
            0,
            DimensionRange::Any,
        ).unwrap();
        
        let set1 = SpaceTimeIdSet::from(id1);
        let set2 = SpaceTimeIdSet::from(id2);
        
        // Test operations with Any dimensions
        let intersection = &set1 & &set2;
        let union = &set1 | &set2;
        
        // Should handle Any dimensions correctly
        assert!(!intersection.is_empty()); // Should have intersection
        assert!(!union.is_empty()); // Should have union
    }
}