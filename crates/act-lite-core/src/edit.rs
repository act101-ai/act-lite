//! Modification primitives for source code edits.
//!
//! An [`Edit`] represents a single text replacement. Multiple edits combine
//! into a [`ChangeSet`], which is the atomic unit of modification.

use crate::Range;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single text edit — replace the text at `range` with `new_text`.
///
/// Edits are immutable once created. They can be validated and composed
/// into [`ChangeSet`]s.
///
/// # Examples
///
/// ```
/// use act_lite_core::{Edit, Location, Range};
/// use std::path::PathBuf;
///
/// let range = Range::from_byte_offsets(PathBuf::from("src/main.rs"), 10, 20);
/// let edit = Edit::new(range, "replacement".to_string());
///
/// assert_eq!(edit.new_text(), "replacement");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    /// The range of text to replace.
    range: Range,
    /// The new text to insert (empty string for deletion).
    new_text: String,
}

impl Edit {
    /// Creates a new edit.
    pub fn new(range: Range, new_text: String) -> Self {
        Self { range, new_text }
    }

    /// Creates a deletion edit (replaces range with empty string).
    pub fn delete(range: Range) -> Self {
        Self {
            range,
            new_text: String::new(),
        }
    }

    /// Creates an insertion edit at a point location.
    pub fn insert(location: crate::Location, text: String) -> Self {
        Self {
            range: Range::point(location),
            new_text: text,
        }
    }

    /// Returns the range being edited.
    pub fn range(&self) -> &Range {
        &self.range
    }

    /// Returns the new text.
    pub fn new_text(&self) -> &str {
        &self.new_text
    }

    /// Returns the file being edited.
    pub fn file(&self) -> &PathBuf {
        self.range.file()
    }

    /// Returns true if this edit is a deletion (new_text is empty).
    pub fn is_deletion(&self) -> bool {
        self.new_text.is_empty() && !self.range.is_point()
    }

    /// Returns true if this edit is an insertion (range is a point).
    pub fn is_insertion(&self) -> bool {
        self.range.is_point() && !self.new_text.is_empty()
    }

    /// Returns true if this edit is a replacement (non-empty range and text).
    pub fn is_replacement(&self) -> bool {
        !self.range.is_point() && !self.new_text.is_empty()
    }

    /// Returns true if this edit conflicts with another.
    ///
    /// Two edits conflict if their ranges overlap.
    pub fn conflicts_with(&self, other: &Edit) -> bool {
        self.range.overlaps(other.range())
    }

    /// Computes the change in byte length caused by this edit.
    ///
    /// Returns `None` if the range's byte offsets aren't resolved.
    /// Positive values indicate the file grows; negative means it shrinks.
    pub fn byte_delta(&self) -> Option<isize> {
        self.range.byte_len().map(|old_len| {
            let new_len = self.new_text.len();
            new_len as isize - old_len as isize
        })
    }

    /// Fills this edit's missing coordinate halves against `content`.
    ///
    /// See [`crate::Location::resolve_coordinates`] for the derivation laws.
    pub fn resolve_coordinates(&mut self, content: &str) {
        self.range.resolve_coordinates(content);
    }
}

/// A collection of edits to be applied atomically.
///
/// ChangeSets are the unit of modification in act. All edits in a ChangeSet
/// either apply together or none apply. This ensures consistency even when
/// a refactoring touches multiple locations.
///
/// # Invariants
///
/// - Edits within a ChangeSet must not overlap (no conflicts)
/// - Edits are stored in a deterministic order (sorted by file, then byte offset)
///
/// # Examples
///
/// ```
/// use act_lite_core::{ChangeSet, Edit, Range};
/// use std::path::PathBuf;
///
/// let mut changeset = ChangeSet::new("Rename 'foo' to 'bar'".to_string());
///
/// let edit1 = Edit::new(
///     Range::from_byte_offsets(PathBuf::from("src/a.rs"), 10, 13),
///     "bar".to_string(),
/// );
/// let edit2 = Edit::new(
///     Range::from_byte_offsets(PathBuf::from("src/b.rs"), 20, 23),
///     "bar".to_string(),
/// );
///
/// changeset.add_edit(edit1).unwrap();
/// changeset.add_edit(edit2).unwrap();
///
/// assert_eq!(changeset.edit_count(), 2);
/// assert_eq!(changeset.files_affected().len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    /// Human-readable description of this change.
    description: String,
    /// The edits to apply, sorted by (file, start_offset).
    edits: Vec<Edit>,
}

/// Error type for ChangeSet operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChangeSetError {
    /// Two edits have overlapping ranges.
    #[error("edit conflicts with existing edit at {location}")]
    Conflict {
        /// Description of where the conflict occurred.
        location: String,
    },
}

impl ChangeSet {
    /// Creates a new empty ChangeSet with a description.
    pub fn new(description: String) -> Self {
        Self {
            description,
            edits: Vec::new(),
        }
    }

    /// Returns the description of this ChangeSet.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the number of edits.
    pub fn edit_count(&self) -> usize {
        self.edits.len()
    }

    /// Returns true if this ChangeSet has no edits.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Returns an iterator over the edits.
    pub fn edits(&self) -> impl Iterator<Item = &Edit> {
        self.edits.iter()
    }

    /// Returns the edits as a slice.
    pub fn edits_slice(&self) -> &[Edit] {
        &self.edits
    }

    /// Adds an edit to the ChangeSet.
    ///
    /// Returns an error if the edit conflicts with an existing edit.
    pub fn add_edit(&mut self, edit: Edit) -> Result<(), ChangeSetError> {
        // Check for conflicts
        for existing in &self.edits {
            if edit.conflicts_with(existing) {
                return Err(ChangeSetError::Conflict {
                    location: format!("{}", edit.range()),
                });
            }
        }

        self.edits.push(edit);
        self.sort_edits();
        Ok(())
    }

    /// Adds multiple edits to the ChangeSet.
    ///
    /// Returns an error if any edit conflicts. On error, no edits are added.
    pub fn add_edits(&mut self, edits: Vec<Edit>) -> Result<(), ChangeSetError> {
        // First check all new edits against existing edits
        for edit in &edits {
            for existing in &self.edits {
                if edit.conflicts_with(existing) {
                    return Err(ChangeSetError::Conflict {
                        location: format!("{}", edit.range()),
                    });
                }
            }
        }

        // Check new edits against each other
        for (i, edit) in edits.iter().enumerate() {
            for other in edits.iter().skip(i + 1) {
                if edit.conflicts_with(other) {
                    return Err(ChangeSetError::Conflict {
                        location: format!("{}", edit.range()),
                    });
                }
            }
        }

        self.edits.extend(edits);
        self.sort_edits();
        Ok(())
    }

    /// Returns the set of files affected by this ChangeSet.
    pub fn files_affected(&self) -> Vec<&PathBuf> {
        let mut files: Vec<&PathBuf> = self.edits.iter().map(|e| e.file()).collect();
        files.sort();
        files.dedup();
        files
    }

    /// Returns all edits for a specific file.
    pub fn edits_for_file(&self, file: &PathBuf) -> Vec<&Edit> {
        self.edits.iter().filter(|e| e.file() == file).collect()
    }

    /// Merges another ChangeSet into this one.
    ///
    /// Returns an error if any edits conflict.
    pub fn merge(&mut self, other: ChangeSet) -> Result<(), ChangeSetError> {
        self.add_edits(other.edits)?;
        // Append description
        if !other.description.is_empty() {
            if !self.description.is_empty() {
                self.description.push_str("; ");
            }
            self.description.push_str(&other.description);
        }
        Ok(())
    }

    /// Sorts edits by file path, then by byte offset (descending).
    ///
    /// Descending order allows edits to be applied from end to start,
    /// so earlier edits don't invalidate the offsets of later ones.
    fn sort_edits(&mut self) {
        self.edits.sort_by(|a, b| {
            match a.file().cmp(b.file()) {
                std::cmp::Ordering::Equal => {
                    // Sort by byte offset descending (so we can apply end-to-start)
                    let a_offset = a.range().start().byte_offset().unwrap_or(0);
                    let b_offset = b.range().start().byte_offset().unwrap_or(0);
                    b_offset.cmp(&a_offset)
                }
                other => other,
            }
        });
    }

    /// Fills every edit's missing coordinate halves, looking each edit's
    /// file content up through `content_for`.
    ///
    /// Best-effort per file: a lookup returning `None` (e.g. an unreadable
    /// creation target) leaves that file's edits with their authored
    /// coordinates — no offset is fabricated from nothing. Edit order is
    /// unchanged. See [`crate::Location::resolve_coordinates`] for the
    /// derivation laws.
    pub fn resolve_coordinates<F>(&mut self, mut content_for: F)
    where
        F: FnMut(&PathBuf) -> Option<String>,
    {
        for edit in &mut self.edits {
            if let Some(content) = content_for(edit.file()) {
                edit.resolve_coordinates(&content);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Location;

    fn make_range(file: &str, start: usize, end: usize) -> Range {
        Range::from_byte_offsets(PathBuf::from(file), start, end)
    }

    #[test]
    fn edit_new() {
        let range = make_range("test.rs", 10, 20);
        let edit = Edit::new(range.clone(), "new".to_string());
        assert_eq!(edit.range(), &range);
        assert_eq!(edit.new_text(), "new");
    }

    #[test]
    fn edit_delete() {
        let range = make_range("test.rs", 10, 20);
        let edit = Edit::delete(range);
        assert!(edit.is_deletion());
        assert!(edit.new_text().is_empty());
    }

    #[test]
    fn edit_insert() {
        let loc = Location::with_byte_offset(PathBuf::from("test.rs"), 10);
        let edit = Edit::insert(loc, "inserted".to_string());
        assert!(edit.is_insertion());
        assert!(edit.range().is_point());
    }

    #[test]
    fn edit_replacement() {
        let range = make_range("test.rs", 10, 20);
        let edit = Edit::new(range, "replaced".to_string());
        assert!(edit.is_replacement());
    }

    #[test]
    fn edit_conflicts_with() {
        let edit1 = Edit::new(make_range("test.rs", 10, 20), "a".to_string());
        let edit2 = Edit::new(make_range("test.rs", 15, 25), "b".to_string());
        let edit3 = Edit::new(make_range("test.rs", 25, 35), "c".to_string());

        assert!(edit1.conflicts_with(&edit2)); // Overlapping
        assert!(!edit1.conflicts_with(&edit3)); // Non-overlapping
    }

    #[test]
    fn edit_byte_delta() {
        let range = make_range("test.rs", 10, 20); // 10 bytes
        let edit = Edit::new(range, "abc".to_string()); // 3 bytes
        assert_eq!(edit.byte_delta(), Some(-7)); // 3 - 10 = -7
    }

    #[test]
    fn changeset_new() {
        let cs = ChangeSet::new("test".to_string());
        assert_eq!(cs.description(), "test");
        assert!(cs.is_empty());
        assert_eq!(cs.edit_count(), 0);
    }

    #[test]
    fn changeset_add_edit() {
        let mut cs = ChangeSet::new("test".to_string());
        let edit = Edit::new(make_range("test.rs", 10, 20), "new".to_string());
        cs.add_edit(edit).unwrap();
        assert_eq!(cs.edit_count(), 1);
    }

    #[test]
    fn changeset_add_conflicting_edit() {
        let mut cs = ChangeSet::new("test".to_string());
        let edit1 = Edit::new(make_range("test.rs", 10, 20), "a".to_string());
        let edit2 = Edit::new(make_range("test.rs", 15, 25), "b".to_string());

        cs.add_edit(edit1).unwrap();
        let result = cs.add_edit(edit2);
        assert!(result.is_err());
        assert_eq!(cs.edit_count(), 1); // Second edit not added
    }

    #[test]
    fn changeset_files_affected() {
        let mut cs = ChangeSet::new("test".to_string());
        cs.add_edit(Edit::new(make_range("a.rs", 0, 10), "x".to_string()))
            .unwrap();
        cs.add_edit(Edit::new(make_range("b.rs", 0, 10), "y".to_string()))
            .unwrap();
        cs.add_edit(Edit::new(make_range("a.rs", 20, 30), "z".to_string()))
            .unwrap();

        let files = cs.files_affected();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn changeset_edits_for_file() {
        let mut cs = ChangeSet::new("test".to_string());
        cs.add_edit(Edit::new(make_range("a.rs", 0, 10), "x".to_string()))
            .unwrap();
        cs.add_edit(Edit::new(make_range("b.rs", 0, 10), "y".to_string()))
            .unwrap();
        cs.add_edit(Edit::new(make_range("a.rs", 20, 30), "z".to_string()))
            .unwrap();

        let a_edits = cs.edits_for_file(&PathBuf::from("a.rs"));
        assert_eq!(a_edits.len(), 2);
    }

    #[test]
    fn changeset_merge() {
        let mut cs1 = ChangeSet::new("first".to_string());
        cs1.add_edit(Edit::new(make_range("a.rs", 0, 10), "x".to_string()))
            .unwrap();

        let mut cs2 = ChangeSet::new("second".to_string());
        cs2.add_edit(Edit::new(make_range("b.rs", 0, 10), "y".to_string()))
            .unwrap();

        cs1.merge(cs2).unwrap();
        assert_eq!(cs1.edit_count(), 2);
        assert_eq!(cs1.description(), "first; second");
    }

    #[test]
    fn changeset_merge_conflict() {
        let mut cs1 = ChangeSet::new("first".to_string());
        cs1.add_edit(Edit::new(make_range("a.rs", 0, 20), "x".to_string()))
            .unwrap();

        let mut cs2 = ChangeSet::new("second".to_string());
        cs2.add_edit(Edit::new(make_range("a.rs", 10, 30), "y".to_string()))
            .unwrap();

        let result = cs1.merge(cs2);
        assert!(result.is_err());
        assert_eq!(cs1.edit_count(), 1); // No merge happened
    }

    #[test]
    fn changeset_sorts_edits_descending() {
        let mut cs = ChangeSet::new("test".to_string());
        cs.add_edit(Edit::new(make_range("a.rs", 0, 10), "first".to_string()))
            .unwrap();
        cs.add_edit(Edit::new(make_range("a.rs", 30, 40), "third".to_string()))
            .unwrap();
        cs.add_edit(Edit::new(make_range("a.rs", 20, 25), "second".to_string()))
            .unwrap();

        let edits: Vec<_> = cs.edits().collect();
        // Should be sorted descending by offset: 30, 20, 0
        assert_eq!(edits[0].range().start().byte_offset(), Some(30));
        assert_eq!(edits[1].range().start().byte_offset(), Some(20));
        assert_eq!(edits[2].range().start().byte_offset(), Some(0));
    }
}
