//! Property-based tests for Edit and ChangeSet composition.

use act_lite_core::{ChangeSet, Edit, Location, Range};
use proptest::prelude::*;
use std::path::PathBuf;

/// Strategy for generating non-overlapping edits for a file.
fn non_overlapping_edits(count: usize) -> impl Strategy<Value = Vec<Edit>> {
    // Generate sorted, non-overlapping offset pairs
    prop::collection::vec((1usize..100, "[a-z]{0,20}"), count).prop_map(|pairs| {
        let mut edits = Vec::new();
        let mut offset = 0usize;
        for (gap, text) in pairs {
            let start = offset + gap;
            let end = start + text.len().max(1);
            let range = Range::from_byte_offsets(PathBuf::from("test.rs"), start, end);
            edits.push(Edit::new(range, text));
            offset = end + 1; // Ensure gap between edits
        }
        edits
    })
}

proptest! {
    /// An edit's byte delta should be consistent.
    #[test]
    fn edit_byte_delta_is_consistent(
        start in 0usize..10000,
        len in 1usize..1000,
        new_text in "[a-z]{0,500}",
    ) {
        let range = Range::from_byte_offsets(PathBuf::from("test.rs"), start, start + len);
        let edit = Edit::new(range, new_text.clone());

        let delta = edit.byte_delta().unwrap();
        let expected = new_text.len() as isize - len as isize;
        prop_assert_eq!(delta, expected);
    }

    /// Non-overlapping edits can always be added to a ChangeSet.
    #[test]
    fn non_overlapping_edits_always_succeed(edits in non_overlapping_edits(10)) {
        let mut changeset = ChangeSet::new("test".to_string());
        for edit in edits {
            prop_assert!(changeset.add_edit(edit).is_ok());
        }
    }

    /// Overlapping edits should always conflict.
    #[test]
    fn overlapping_edits_always_conflict(
        start in 0usize..10000,
        len1 in 10usize..100,
        overlap in 1usize..10,
    ) {
        let edit1 = Edit::new(
            Range::from_byte_offsets(PathBuf::from("test.rs"), start, start + len1),
            "a".to_string(),
        );
        // Second edit starts before first ends
        let edit2 = Edit::new(
            Range::from_byte_offsets(
                PathBuf::from("test.rs"),
                start + len1 - overlap,
                start + len1 + 10,
            ),
            "b".to_string(),
        );

        let mut changeset = ChangeSet::new("test".to_string());
        changeset.add_edit(edit1).unwrap();
        prop_assert!(changeset.add_edit(edit2).is_err());
    }

    /// ChangeSet sorts edits in descending order by offset.
    #[test]
    fn changeset_sorts_descending(edits in non_overlapping_edits(10)) {
        let mut changeset = ChangeSet::new("test".to_string());
        for edit in edits {
            changeset.add_edit(edit).unwrap();
        }

        let offsets: Vec<_> = changeset
            .edits()
            .map(|e| e.range().start().byte_offset().unwrap())
            .collect();

        // Should be sorted descending
        for window in offsets.windows(2) {
            prop_assert!(window[0] >= window[1], "Edits not sorted descending");
        }
    }

    /// Merging two non-conflicting changesets should succeed.
    #[test]
    fn merge_non_conflicting_succeeds(
        edits1 in non_overlapping_edits(5),
        extra_offset in 20000usize..30000,
    ) {
        let mut cs1 = ChangeSet::new("first".to_string());
        for edit in edits1 {
            cs1.add_edit(edit).unwrap();
        }

        // Create second changeset with edits at much higher offsets
        let mut cs2 = ChangeSet::new("second".to_string());
        for i in 0..3 {
            let start = extra_offset + i * 100;
            let edit = Edit::new(
                Range::from_byte_offsets(PathBuf::from("test.rs"), start, start + 10),
                format!("text{}", i),
            );
            cs2.add_edit(edit).unwrap();
        }

        let total_before = cs1.edit_count() + cs2.edit_count();
        prop_assert!(cs1.merge(cs2).is_ok());
        prop_assert_eq!(cs1.edit_count(), total_before);
    }

    /// Location line and column must be positive.
    #[test]
    fn location_requires_positive_line_column(line in 1u32..10000, column in 1u32..10000) {
        let loc = Location::new(PathBuf::from("test.rs"), line, column);
        prop_assert_eq!(loc.line(), line);
        prop_assert_eq!(loc.column(), column);
    }

    /// Range with resolved offsets has consistent byte_len.
    #[test]
    fn range_byte_len_consistent(start in 0usize..10000, len in 0usize..1000) {
        let range = Range::from_byte_offsets(PathBuf::from("test.rs"), start, start + len);
        prop_assert_eq!(range.byte_len(), Some(len));
    }

    /// Range contains works correctly.
    #[test]
    fn range_contains_works(start in 0usize..10000, len in 10usize..1000) {
        let range = Range::from_byte_offsets(PathBuf::from("test.rs"), start, start + len);

        // Point inside should be contained
        let mid = start + len / 2;
        let inside = Location::with_byte_offset(PathBuf::from("test.rs"), mid);
        prop_assert!(range.contains(&inside));

        // Point at start should be contained (inclusive)
        let at_start = Location::with_byte_offset(PathBuf::from("test.rs"), start);
        prop_assert!(range.contains(&at_start));

        // Point at end should NOT be contained (exclusive)
        let at_end = Location::with_byte_offset(PathBuf::from("test.rs"), start + len);
        prop_assert!(!range.contains(&at_end));

        // Point before should not be contained
        if start > 0 {
            let before = Location::with_byte_offset(PathBuf::from("test.rs"), start - 1);
            prop_assert!(!range.contains(&before));
        }

        // Point after should not be contained
        let after = Location::with_byte_offset(PathBuf::from("test.rs"), start + len + 1);
        prop_assert!(!range.contains(&after));
    }
}
