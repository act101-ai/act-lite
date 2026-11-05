//! Position types for source code locations.
//!
//! These types represent positions within source files. They support both
//! line:column addressing (for human display) and byte offsets (for edits).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// A single position within a source file.
///
/// Positions are 1-indexed for lines and columns (matching editor conventions).
/// Byte offsets are 0-indexed.
///
/// # Examples
///
/// ```
/// use act_lite_core::Location;
/// use std::path::PathBuf;
///
/// let loc = Location::new(PathBuf::from("src/main.rs"), 10, 5);
/// assert_eq!(loc.line(), 10);
/// assert_eq!(loc.column(), 5);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    /// Path to the source file (relative to workspace root).
    file: PathBuf,
    /// 1-indexed line number.
    line: u32,
    /// 1-indexed column number (in UTF-8 code units).
    column: u32,
    /// Optional 0-indexed byte offset from start of file.
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_offset: Option<usize>,
}

impl Location {
    /// Creates a new location with line and column.
    ///
    /// # Panics
    ///
    /// Panics if `line` or `column` is 0 (they are 1-indexed).
    pub fn new(file: PathBuf, line: u32, column: u32) -> Self {
        assert!(line > 0, "line must be 1-indexed (got 0)");
        assert!(column > 0, "column must be 1-indexed (got 0)");
        Self {
            file,
            line,
            column,
            byte_offset: None,
        }
    }

    /// Creates a new location with a byte offset.
    pub fn with_byte_offset(file: PathBuf, byte_offset: usize) -> Self {
        Self {
            file,
            line: 0,   // Will be resolved later
            column: 0, // Will be resolved later
            byte_offset: Some(byte_offset),
        }
    }

    /// Creates a location with all fields specified.
    ///
    /// # Panics
    ///
    /// Panics if `line` or `column` is 0.
    pub fn full(file: PathBuf, line: u32, column: u32, byte_offset: usize) -> Self {
        assert!(line > 0, "line must be 1-indexed (got 0)");
        assert!(column > 0, "column must be 1-indexed (got 0)");
        Self {
            file,
            line,
            column,
            byte_offset: Some(byte_offset),
        }
    }

    /// Returns the file path.
    pub fn file(&self) -> &PathBuf {
        &self.file
    }

    /// Returns the 1-indexed line number.
    pub fn line(&self) -> u32 {
        self.line
    }

    /// Returns the 1-indexed column number.
    pub fn column(&self) -> u32 {
        self.column
    }

    /// Returns the byte offset if known.
    pub fn byte_offset(&self) -> Option<usize> {
        self.byte_offset
    }

    /// Sets the byte offset.
    pub fn set_byte_offset(&mut self, offset: usize) {
        self.byte_offset = Some(offset);
    }

    /// Returns true if this location has resolved byte offset.
    pub fn is_resolved(&self) -> bool {
        self.byte_offset.is_some()
    }

    /// Fills the missing half of this location's coordinates against
    /// `content`, in place.
    ///
    /// A location carries two coordinate conventions — 1-indexed
    /// line/column and a 0-indexed byte offset — and each constructor fills
    /// only the half it was given ([`Location::new`] leaves the byte offset
    /// absent; [`Location::with_byte_offset`] leaves line/column at the 0:0
    /// placeholder), so a published payload could otherwise carry a
    /// different convention per edit. The derivation laws:
    ///
    /// * byte → line/column: `line` = newlines before the offset + 1,
    ///   `column` = bytes since the last newline + 1 (byte units — the
    ///   tree-sitter convention). The offset is clamped to the content end
    ///   and walked back to the nearest char boundary.
    /// * line/column → byte: line-start byte + the byte length of the first
    ///   `column − 1` CHARS of that line, clamped to the line end and to
    ///   the content end — exactly where a line/column-based applier cuts.
    ///
    /// Idempotent, and never reconciles: a location that already carries
    /// both conventions is returned unchanged, and the authored half is
    /// never rewritten (the byte offset is canonical; line/column are
    /// informational).
    pub fn resolve_coordinates(&mut self, content: &str) {
        match (self.byte_offset, self.line > 0 && self.column > 0) {
            (Some(offset), false) => {
                let (line, column) = line_col_for_byte(content, offset);
                self.line = line;
                self.column = column;
            }
            (None, true) => {
                self.byte_offset = Some(byte_offset_for_line_col(content, self.line, self.column));
            }
            // Both present (nothing to fill) or neither (nothing to derive
            // from — only reachable via deserialization, which bypasses the
            // constructors' assertions).
            _ => {}
        }
    }
}

/// Derives a 1-indexed (line, byte-column) pair for `offset` in `content`.
///
/// Column is in BYTE units (the tree-sitter convention). The offset is
/// clamped to the content end and walked back to the nearest char boundary
/// so a mid-character offset cannot panic a slice.
fn line_col_for_byte(content: &str, offset: usize) -> (u32, u32) {
    let mut offset = offset.min(content.len());
    while !content.is_char_boundary(offset) {
        offset -= 1;
    }
    let prefix = &content[..offset];
    let line = prefix.matches('\n').count() as u32 + 1;
    let column = match prefix.rfind('\n') {
        Some(newline) => (prefix.len() - newline - 1) as u32 + 1,
        None => prefix.len() as u32 + 1,
    };
    (line, column)
}

/// Derives the byte offset of a 1-indexed (line, char-column) position in
/// `content`: the line-start byte plus the byte length of the first
/// `column − 1` CHARS of that line. Columns beyond the line's end clamp to
/// the line end; lines beyond the content clamp to the content end.
fn byte_offset_for_line_col(content: &str, line: u32, column: u32) -> usize {
    let line_index = line.saturating_sub(1) as usize;
    let mut line_start = content.len();
    let mut newlines = 0usize;
    for (i, byte) in content.bytes().enumerate() {
        if newlines >= line_index {
            line_start = i;
            break;
        }
        if byte == b'\n' {
            newlines += 1;
        }
    }
    let line_end = content[line_start..]
        .find('\n')
        .map_or(content.len(), |pos| line_start + pos);
    let line_text = &content[line_start..line_end];
    let take = (column.saturating_sub(1) as usize).min(line_text.chars().count());
    let column_bytes: usize = line_text.chars().take(take).map(char::len_utf8).sum();
    line_start + column_bytes
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file.display(), self.line, self.column)
    }
}

/// A range within a source file, defined by start and end locations.
///
/// Ranges are half-open: they include the start position but exclude the end.
/// This matches the convention used by LSP and most editors.
///
/// # Examples
///
/// ```
/// use act_lite_core::{Location, Range};
/// use std::path::PathBuf;
///
/// let start = Location::new(PathBuf::from("src/main.rs"), 10, 5);
/// let end = Location::new(PathBuf::from("src/main.rs"), 10, 15);
/// let range = Range::new(start, end);
///
/// assert_eq!(range.start().line(), 10);
/// assert_eq!(range.end().column(), 15);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Range {
    /// Start of the range (inclusive).
    start: Location,
    /// End of the range (exclusive).
    end: Location,
}

impl Range {
    /// Creates a new range from start to end.
    ///
    /// # Panics
    ///
    /// Panics if start and end are in different files.
    pub fn new(start: Location, end: Location) -> Self {
        assert_eq!(
            start.file(),
            end.file(),
            "range start and end must be in the same file"
        );
        Self { start, end }
    }

    /// Creates a range from byte offsets.
    pub fn from_byte_offsets(file: PathBuf, start_offset: usize, end_offset: usize) -> Self {
        Self {
            start: Location::with_byte_offset(file.clone(), start_offset),
            end: Location::with_byte_offset(file, end_offset),
        }
    }

    /// Returns the start location.
    pub fn start(&self) -> &Location {
        &self.start
    }

    /// Returns the end location.
    pub fn end(&self) -> &Location {
        &self.end
    }

    /// Returns the file path (same for start and end).
    pub fn file(&self) -> &PathBuf {
        self.start.file()
    }

    /// Returns true if this range has resolved byte offsets for both endpoints.
    pub fn is_resolved(&self) -> bool {
        self.start.is_resolved() && self.end.is_resolved()
    }

    /// Returns the byte length of this range if both offsets are known.
    pub fn byte_len(&self) -> Option<usize> {
        match (self.start.byte_offset(), self.end.byte_offset()) {
            (Some(start), Some(end)) => Some(end.saturating_sub(start)),
            _ => None,
        }
    }

    /// Returns true if this range contains the given location.
    ///
    /// A location is contained if it's >= start and < end.
    /// Requires byte offsets to be resolved.
    pub fn contains(&self, loc: &Location) -> bool {
        if self.file() != loc.file() {
            return false;
        }
        match (
            self.start.byte_offset(),
            self.end.byte_offset(),
            loc.byte_offset(),
        ) {
            (Some(start), Some(end), Some(loc_offset)) => loc_offset >= start && loc_offset < end,
            _ => {
                // Fall back to line/column comparison
                let after_start = loc.line() > self.start.line()
                    || (loc.line() == self.start.line() && loc.column() >= self.start.column());
                let before_end = loc.line() < self.end.line()
                    || (loc.line() == self.end.line() && loc.column() < self.end.column());
                after_start && before_end
            }
        }
    }

    /// Returns true if this range overlaps with another range.
    ///
    /// Two ranges overlap if they share any positions.
    pub fn overlaps(&self, other: &Range) -> bool {
        if self.file() != other.file() {
            return false;
        }
        // Ranges overlap if neither is entirely before or after the other
        match (
            self.start.byte_offset(),
            self.end.byte_offset(),
            other.start.byte_offset(),
            other.end.byte_offset(),
        ) {
            (Some(s1), Some(e1), Some(s2), Some(e2)) => s1 < e2 && s2 < e1,
            _ => {
                // Fall back to line/column comparison
                let self_before_other = self.end.line() < other.start.line()
                    || (self.end.line() == other.start.line()
                        && self.end.column() <= other.start.column());
                let other_before_self = other.end.line() < self.start.line()
                    || (other.end.line() == self.start.line()
                        && other.end.column() <= self.start.column());
                !self_before_other && !other_before_self
            }
        }
    }

    /// Creates a point range (zero-width) at the given location.
    pub fn point(loc: Location) -> Self {
        Self {
            start: loc.clone(),
            end: loc,
        }
    }

    /// Returns true if this is a point range (zero-width).
    pub fn is_point(&self) -> bool {
        self.start == self.end
    }

    /// Fills each endpoint's missing coordinate half against `content`.
    ///
    /// See [`Location::resolve_coordinates`] for the derivation laws; the
    /// endpoints are filled independently (a range commonly mixes
    /// conventions only at construction time, and each endpoint carries its
    /// own halves).
    pub fn resolve_coordinates(&mut self, content: &str) {
        self.start.resolve_coordinates(content);
        self.end.resolve_coordinates(content);
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start.line() == self.end.line() {
            write!(
                f,
                "{}:{}:{}-{}",
                self.start.file().display(),
                self.start.line(),
                self.start.column(),
                self.end.column()
            )
        } else {
            write!(
                f,
                "{}:{}:{}-{}:{}",
                self.start.file().display(),
                self.start.line(),
                self.start.column(),
                self.end.line(),
                self.end.column()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn location_new() {
        let loc = Location::new(PathBuf::from("test.rs"), 10, 5);
        assert_eq!(loc.line(), 10);
        assert_eq!(loc.column(), 5);
        assert_eq!(loc.file(), &PathBuf::from("test.rs"));
        assert!(!loc.is_resolved());
    }

    #[test]
    fn location_with_byte_offset() {
        let loc = Location::with_byte_offset(PathBuf::from("test.rs"), 100);
        assert_eq!(loc.byte_offset(), Some(100));
        assert!(loc.is_resolved());
    }

    #[test]
    fn location_full() {
        let loc = Location::full(PathBuf::from("test.rs"), 10, 5, 100);
        assert_eq!(loc.line(), 10);
        assert_eq!(loc.column(), 5);
        assert_eq!(loc.byte_offset(), Some(100));
    }

    #[test]
    #[should_panic(expected = "line must be 1-indexed")]
    fn location_zero_line_panics() {
        Location::new(PathBuf::from("test.rs"), 0, 5);
    }

    #[test]
    #[should_panic(expected = "column must be 1-indexed")]
    fn location_zero_column_panics() {
        Location::new(PathBuf::from("test.rs"), 10, 0);
    }

    #[test]
    fn location_display() {
        let loc = Location::new(PathBuf::from("src/main.rs"), 10, 5);
        assert_eq!(format!("{}", loc), "src/main.rs:10:5");
    }

    #[test]
    fn range_new() {
        let start = Location::new(PathBuf::from("test.rs"), 10, 5);
        let end = Location::new(PathBuf::from("test.rs"), 10, 15);
        let range = Range::new(start, end);
        assert_eq!(range.start().column(), 5);
        assert_eq!(range.end().column(), 15);
    }

    #[test]
    #[should_panic(expected = "same file")]
    fn range_different_files_panics() {
        let start = Location::new(PathBuf::from("a.rs"), 1, 1);
        let end = Location::new(PathBuf::from("b.rs"), 1, 1);
        Range::new(start, end);
    }

    #[test]
    fn range_byte_len() {
        let range = Range::from_byte_offsets(PathBuf::from("test.rs"), 10, 20);
        assert_eq!(range.byte_len(), Some(10));
    }

    #[test]
    fn range_contains() {
        let range = Range::from_byte_offsets(PathBuf::from("test.rs"), 10, 20);
        let inside = Location::with_byte_offset(PathBuf::from("test.rs"), 15);
        let before = Location::with_byte_offset(PathBuf::from("test.rs"), 5);
        let after = Location::with_byte_offset(PathBuf::from("test.rs"), 25);
        let at_start = Location::with_byte_offset(PathBuf::from("test.rs"), 10);
        let at_end = Location::with_byte_offset(PathBuf::from("test.rs"), 20);

        assert!(range.contains(&inside));
        assert!(range.contains(&at_start)); // Inclusive start
        assert!(!range.contains(&at_end)); // Exclusive end
        assert!(!range.contains(&before));
        assert!(!range.contains(&after));
    }

    #[test]
    fn range_overlaps() {
        let r1 = Range::from_byte_offsets(PathBuf::from("test.rs"), 10, 20);
        let r2 = Range::from_byte_offsets(PathBuf::from("test.rs"), 15, 25);
        let r3 = Range::from_byte_offsets(PathBuf::from("test.rs"), 25, 35);
        let r4 = Range::from_byte_offsets(PathBuf::from("test.rs"), 5, 10);

        assert!(r1.overlaps(&r2)); // Partial overlap
        assert!(!r1.overlaps(&r3)); // No overlap
        assert!(!r1.overlaps(&r4)); // Adjacent, no overlap
    }

    #[test]
    fn range_display_single_line() {
        let start = Location::new(PathBuf::from("test.rs"), 10, 5);
        let end = Location::new(PathBuf::from("test.rs"), 10, 15);
        let range = Range::new(start, end);
        assert_eq!(format!("{}", range), "test.rs:10:5-15");
    }

    #[test]
    fn range_display_multi_line() {
        let start = Location::new(PathBuf::from("test.rs"), 10, 5);
        let end = Location::new(PathBuf::from("test.rs"), 15, 10);
        let range = Range::new(start, end);
        assert_eq!(format!("{}", range), "test.rs:10:5-15:10");
    }

    #[test]
    fn range_point() {
        let loc = Location::new(PathBuf::from("test.rs"), 10, 5);
        let range = Range::point(loc);
        assert!(range.is_point());
    }
}
