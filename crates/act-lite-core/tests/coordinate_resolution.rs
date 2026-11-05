use act_lite_core::{ChangeSet, Edit, Location, Range};
use std::path::PathBuf;

fn byte_loc(offset: usize) -> Location {
    Location::with_byte_offset(PathBuf::from("test.txt"), offset)
}

fn lc_loc(line: u32, column: u32) -> Location {
    Location::new(PathBuf::from("test.txt"), line, column)
}

// --- byte → line/column (fills the 0:0 placeholder) ---

#[test]
fn byte_to_line_col_ascii() {
    // Lines start at bytes 0, 4, 9; content length 11, no trailing newline.
    let content = "abc\ndefg\nhi";
    let cases: [(usize, u32, u32); 8] = [
        (0, 1, 1),
        (1, 1, 2),
        (3, 1, 4), // at the first '\n'
        (4, 2, 1),
        (5, 2, 2),
        (8, 2, 5), // at the second '\n'
        (9, 3, 1),
        (11, 3, 3), // at EOF
    ];
    for (offset, line, column) in cases {
        let mut loc = byte_loc(offset);
        loc.resolve_coordinates(content);
        assert_eq!(
            (loc.line(), loc.column()),
            (line, column),
            "offset {offset}"
        );
        assert_eq!(
            loc.byte_offset(),
            Some(offset),
            "offset {offset}: byte_offset is canonical and never overwritten"
        );
    }
}

#[test]
fn byte_to_line_col_trailing_newline_eof() {
    let content = "abc\n"; // EOF sits one past the final newline: line 2, column 1.
    let mut loc = byte_loc(4);
    loc.resolve_coordinates(content);
    assert_eq!((loc.line(), loc.column()), (2, 1));
}

#[test]
fn byte_to_line_col_past_eof_clamps_to_content_end() {
    let content = "ab\ncd\n"; // content end = line 3, column 1
    let mut loc = byte_loc(1000);
    loc.resolve_coordinates(content);
    assert_eq!((loc.line(), loc.column()), (3, 1));
}

#[test]
fn byte_to_line_col_empty_content() {
    let mut loc = byte_loc(0);
    loc.resolve_coordinates("");
    assert_eq!((loc.line(), loc.column()), (1, 1));
}

#[test]
fn byte_to_line_col_columns_are_byte_units() {
    // 'é' and 'ö' are 2 bytes each: "héllo" = 6 bytes ('\n' at 6), line 2
    // ("wörld") starts at byte 7.
    let content = "héllo\nwörld";
    let mut loc = byte_loc(3); // after h(1) + é(2) bytes of line 1
    loc.resolve_coordinates(content);
    assert_eq!((loc.line(), loc.column()), (1, 4));
    let mut loc = byte_loc(8); // after w(1) byte of line 2: 'ö' starts here
    loc.resolve_coordinates(content);
    assert_eq!((loc.line(), loc.column()), (2, 2));
}

#[test]
fn byte_to_line_col_mid_char_offset_walks_back_to_char_boundary() {
    // Offset 2 falls inside 'é' (bytes 1..3): the derivation walks back to
    // the nearest char boundary (1) instead of panicking on a slice.
    let content = "héllo";
    let mut loc = byte_loc(2);
    loc.resolve_coordinates(content);
    assert_eq!((loc.line(), loc.column()), (1, 2));
    assert_eq!(loc.byte_offset(), Some(2), "authored offset kept as-is");
}

// --- line/column → byte (fills the absent byte_offset) ---

#[test]
fn line_col_to_byte_ascii() {
    let content = "abc\ndefg\nhi";
    let cases: [((u32, u32), usize); 7] = [
        ((1, 1), 0),
        ((1, 3), 2),
        ((1, 4), 3), // at the first '\n'
        ((2, 1), 4),
        ((2, 5), 8), // line end
        ((3, 2), 10),
        ((3, 3), 11), // content end
    ];
    for ((line, column), offset) in cases {
        let mut loc = lc_loc(line, column);
        loc.resolve_coordinates(content);
        assert_eq!(
            loc.byte_offset(),
            Some(offset),
            "line {line} column {column}"
        );
        assert_eq!(
            (loc.line(), loc.column()),
            (line, column),
            "authored line/column never overwritten"
        );
    }
}

#[test]
fn line_col_to_byte_column_past_line_end_clamps_to_line_end() {
    let content = "abc\ndefg\nhi";
    let mut loc = lc_loc(2, 99);
    loc.resolve_coordinates(content);
    assert_eq!(loc.byte_offset(), Some(8));
}

#[test]
fn line_col_to_byte_line_past_eof_clamps_to_content_end() {
    let content = "abc\ndefg\nhi"; // only 2 newlines: line 9 does not exist
    let mut loc = lc_loc(9, 1);
    loc.resolve_coordinates(content);
    assert_eq!(loc.byte_offset(), Some(11));
}

#[test]
fn line_col_to_byte_no_trailing_newline_append_position() {
    // Content ends without '\n': a position on the line AFTER the last real
    // line clamps to the content end — where the line/col applier's cut
    // lands for an append (it fabricates no phantom blank line).
    let content = "ab\ncd";
    let mut loc = lc_loc(3, 1);
    loc.resolve_coordinates(content);
    assert_eq!(loc.byte_offset(), Some(5));
}

#[test]
fn line_col_to_byte_empty_content() {
    let mut loc = lc_loc(1, 1);
    loc.resolve_coordinates("");
    assert_eq!(loc.byte_offset(), Some(0));
}

#[test]
fn line_col_to_byte_columns_count_chars() {
    let content = "héllo\nwörld";
    let mut loc = lc_loc(1, 2); // after 'h'
    loc.resolve_coordinates(content);
    assert_eq!(loc.byte_offset(), Some(1));
    let mut loc = lc_loc(1, 3); // after 'h' + 'é' (2 bytes)
    loc.resolve_coordinates(content);
    assert_eq!(loc.byte_offset(), Some(3));
    let mut loc = lc_loc(2, 3); // after 'w' + 'ö' on line 2 (starts at 7): 7 + 1 + 2
    loc.resolve_coordinates(content);
    assert_eq!(loc.byte_offset(), Some(10));
}

// --- fill discipline: idempotency, no reconciliation of authored coordinates ---

#[test]
fn resolve_preserves_complete_locations_unchanged() {
    let content = "ab\ncd";
    // Authored (1,1) disagrees with byte 5 (EOF): the fill does NOT
    // reconcile — it only fills the missing half.
    let mut loc = Location::full(PathBuf::from("test.txt"), 1, 1, 5);
    loc.resolve_coordinates(content);
    assert_eq!((loc.line(), loc.column()), (1, 1));
    assert_eq!(loc.byte_offset(), Some(5));
}

#[test]
fn resolve_is_idempotent() {
    let content = "ab\ncd";
    let mut once = lc_loc(2, 2);
    once.resolve_coordinates(content);
    let mut twice = lc_loc(2, 2);
    twice.resolve_coordinates(content);
    twice.resolve_coordinates(content);
    assert_eq!(once, twice);
}

// --- creation shapes (changeset_creates_file must keep reading them) ---

#[test]
fn byte_zero_anchor_resolves_to_line_one_column_one() {
    let mut range = Range::from_byte_offsets(PathBuf::from("test.txt"), 0, 5);
    range.resolve_coordinates("hello");
    assert_eq!((range.start().line(), range.start().column()), (1, 1));
    assert_eq!((range.end().line(), range.end().column()), (1, 6));
}

#[test]
fn whole_file_creation_range_fills_anchored() {
    // The (1,1)→(MAX,MAX) creation shape against EMPTY content: the start
    // stays byte-0 anchored (creation detection reads Some(0)), the end
    // clamps to the content end (0).
    let mut range = Range::new(lc_loc(1, 1), lc_loc(u32::MAX, u32::MAX));
    range.resolve_coordinates("");
    assert_eq!(range.start().byte_offset(), Some(0));
    assert_eq!(range.end().byte_offset(), Some(0));
    assert_eq!((range.start().line(), range.start().column()), (1, 1));

    // Against non-empty content the end clamps to the content end.
    let mut range = Range::new(lc_loc(1, 1), lc_loc(u32::MAX, u32::MAX));
    range.resolve_coordinates("abc\ndef");
    assert_eq!(range.start().byte_offset(), Some(0));
    assert_eq!(range.end().byte_offset(), Some(7));
}

// --- ChangeSet-level fill with a per-file content lookup ---

#[test]
fn changeset_fill_is_best_effort_per_file() {
    let mut cs = ChangeSet::new("mixed conventions".to_string());
    cs.add_edit(Edit::new(
        Range::from_byte_offsets(PathBuf::from("test.txt"), 1, 2),
        "x".to_string(),
    ))
    .unwrap();
    cs.add_edit(Edit::insert(
        Location::new(PathBuf::from("other.txt"), 1, 1),
        "y".to_string(),
    ))
    .unwrap();

    let contents = [(PathBuf::from("test.txt"), "abc".to_string())];
    cs.resolve_coordinates(|file| {
        contents
            .iter()
            .find(|(path, _)| path == file)
            .map(|(_, content)| content.clone())
    });

    // test.txt resolved against its content: byte → line/column filled.
    for edit in cs.edits_for_file(&PathBuf::from("test.txt")) {
        assert_eq!(
            (edit.range().start().line(), edit.range().start().column()),
            (1, 2)
        );
        assert_eq!(edit.range().end().line(), 1);
        assert_eq!(edit.range().end().column(), 3);
    }
    // other.txt had no content to resolve against: left as authored — no
    // byte_offset is fabricated from nothing.
    for edit in cs.edits_for_file(&PathBuf::from("other.txt")) {
        assert_eq!(edit.range().start().byte_offset(), None);
    }
}
