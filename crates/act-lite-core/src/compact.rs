//! Compact serialization types for token-efficient output.
//!
//! [`CompactRange`] serializes as `"line:col-line:col"` instead of nested JSON objects,
//! reducing token count in LLM-consumed output.

use crate::{Location, Range};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

/// A range that serializes as `"line:col-line:col"` for compact JSON output.
///
/// Unlike [`Range`], this type does not carry file path or byte offset information.
/// It is intended for output contexts where the file path is already known from
/// a parent object, and only line/column positions are needed.
///
/// # Examples
///
/// ```
/// use act_lite_core::{CompactRange, Location, Range};
/// use std::path::PathBuf;
///
/// let start = Location::new(PathBuf::from("test.ts"), 8, 1);
/// let end = Location::new(PathBuf::from("test.ts"), 28, 2);
/// let compact = CompactRange::from(Range::new(start, end));
/// let json = serde_json::to_string(&compact).unwrap();
/// assert_eq!(json, r#""8:1-28:2""#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompactRange {
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
}

impl CompactRange {
    /// Creates a new `CompactRange` from explicit line/column values.
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Returns the start line (1-indexed).
    pub fn start_line(&self) -> u32 {
        self.start_line
    }

    /// Returns the start column (1-indexed).
    pub fn start_col(&self) -> u32 {
        self.start_col
    }

    /// Converts this compact range back into a full [`Range`] with the given file path.
    pub fn into_range(self, file: PathBuf) -> Range {
        let start = Location::new(file.clone(), self.start_line, self.start_col);
        let end = Location::new(file, self.end_line, self.end_col);
        Range::new(start, end)
    }
}

impl From<Range> for CompactRange {
    fn from(range: Range) -> Self {
        Self {
            start_line: range.start().line(),
            start_col: range.start().column(),
            end_line: range.end().line(),
            end_col: range.end().column(),
        }
    }
}

impl From<&Range> for CompactRange {
    fn from(range: &Range) -> Self {
        Self {
            start_line: range.start().line(),
            start_col: range.start().column(),
            end_line: range.end().line(),
            end_col: range.end().column(),
        }
    }
}

impl Serialize for CompactRange {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = format!(
            "{}:{}-{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        );
        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for CompactRange {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(serde::de::Error::custom(
                "expected format: line:col-line:col",
            ));
        }
        let start: Vec<u32> = parts[0]
            .split(':')
            .map(|p| p.parse().map_err(serde::de::Error::custom))
            .collect::<Result<_, _>>()?;
        let end: Vec<u32> = parts[1]
            .split(':')
            .map(|p| p.parse().map_err(serde::de::Error::custom))
            .collect::<Result<_, _>>()?;
        if start.len() != 2 || end.len() != 2 {
            return Err(serde::de::Error::custom(
                "expected format: line:col-line:col",
            ));
        }
        Ok(Self::new(start[0], start[1], end[0], end[1]))
    }
}

impl std::fmt::Display for CompactRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}-{}:{}",
            self.start_line, self.start_col, self.end_line, self.end_col
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Location, Range};
    use std::path::PathBuf;

    #[test]
    fn compact_range_serializes_as_string() {
        let start = Location::new(PathBuf::from("test.ts"), 8, 1);
        let end = Location::new(PathBuf::from("test.ts"), 28, 2);
        let compact = CompactRange::from(Range::new(start, end));
        let json = serde_json::to_string(&compact).unwrap();
        assert_eq!(json, r#""8:1-28:2""#);
    }

    #[test]
    fn compact_range_single_line() {
        let start = Location::new(PathBuf::from("test.ts"), 5, 3);
        let end = Location::new(PathBuf::from("test.ts"), 5, 15);
        let compact = CompactRange::from(Range::new(start, end));
        let json = serde_json::to_string(&compact).unwrap();
        assert_eq!(json, r#""5:3-5:15""#);
    }

    #[test]
    fn compact_range_deserializes_from_string() {
        let json = r#""8:1-28:2""#;
        let compact: CompactRange = serde_json::from_str(json).unwrap();
        let range = compact.into_range(PathBuf::from("test.ts"));
        assert_eq!(range.start().line(), 8);
        assert_eq!(range.start().column(), 1);
        assert_eq!(range.end().line(), 28);
        assert_eq!(range.end().column(), 2);
    }

    #[test]
    fn compact_range_roundtrip() {
        let start = Location::new(PathBuf::from("test.ts"), 1, 1);
        let end = Location::new(PathBuf::from("test.ts"), 100, 42);
        let original = Range::new(start, end);
        let compact = CompactRange::from(original);
        let json = serde_json::to_string(&compact).unwrap();
        let deserialized: CompactRange = serde_json::from_str(&json).unwrap();
        assert_eq!(compact, deserialized);
    }

    #[test]
    fn compact_range_from_ref() {
        let start = Location::new(PathBuf::from("test.ts"), 3, 7);
        let end = Location::new(PathBuf::from("test.ts"), 10, 1);
        let range = Range::new(start, end);
        let compact = CompactRange::from(&range);
        let json = serde_json::to_string(&compact).unwrap();
        assert_eq!(json, r#""3:7-10:1""#);
    }

    #[test]
    fn compact_range_display() {
        let compact = CompactRange::new(5, 3, 5, 15);
        assert_eq!(format!("{compact}"), "5:3-5:15");
    }

    #[test]
    fn compact_range_deserialize_invalid_format() {
        let result: Result<CompactRange, _> = serde_json::from_str(r#""bad""#);
        assert!(result.is_err());
    }

    #[test]
    fn compact_range_deserialize_missing_parts() {
        let result: Result<CompactRange, _> = serde_json::from_str(r#""1:2""#);
        assert!(result.is_err());
    }
}
