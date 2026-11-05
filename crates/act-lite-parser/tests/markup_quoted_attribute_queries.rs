use act_lite_parser::{
    Grammar, QueryResult, SourceFile, execute_references_for_symbol, execute_symbols,
};

fn symbol_names_and_kinds(source: &SourceFile) -> Vec<(String, String)> {
    match execute_symbols(source).expect("symbols must not error") {
        QueryResult::Symbols { symbols } => symbols
            .into_iter()
            .map(|s| (s.name().to_string(), format!("{:?}", s.kind())))
            .collect(),
        other => panic!("expected symbols result, got {other:?}"),
    }
}

fn reference_positions(source: &SourceFile, name: &str, line: u32, column: u32) -> Vec<(u32, u32)> {
    match execute_references_for_symbol(source, name, line, column)
        .expect("references must resolve")
    {
        QueryResult::References { references, .. } => references
            .into_iter()
            .map(|r| (r.line(), r.column()))
            .collect(),
        other => panic!("expected references result, got {other:?}"),
    }
}

// ── RED pins: the quoted shapes the six defect sites never matched ──────────

/// The row's own carrier document: a quoted `id`, a quoted `class` and a
/// quoted `href` in one tag. The id value must be a symbol (field).
#[test]
fn html_quoted_id_document_reports_the_id_value_as_a_field_symbol() {
    let source = SourceFile::from_content(
        "test.html",
        "<div id=\"main\" class=\"hero\" href=\"#main\">x</div>\n".to_string(),
        Grammar::Html,
    )
    .unwrap();

    let rows = symbol_names_and_kinds(&source);
    assert!(
        rows.iter().any(|(n, k)| n == "main" && k == "Field"),
        "html symbols: quoted id value must be a Field symbol, got {rows:?}"
    );
}

/// Same document, references for the id value anchored at its own coordinate
/// (line 1, column 10 is the `main` inside `id="main"`). Before the fix the
/// query matched no quoted shape, so this came back empty. After the fix the
/// id site answers; the `href="#main"` site does NOT join it (its node text is
/// `#main` — see the module note), so exactly one reference is expected.
#[test]
fn html_quoted_id_site_resolves_references_for_the_id_name() {
    let source = SourceFile::from_content(
        "test.html",
        "<div id=\"main\" class=\"hero\" href=\"#main\">x</div>\n".to_string(),
        Grammar::Html,
    )
    .unwrap();

    assert_eq!(
        reference_positions(&source, "main", 1, 10),
        [(1, 10)],
        "references for the quoted id value must include the id site itself"
    );
}

/// The href twin's own surface: anchored at the `#main` inside
/// `href="#main"` (line 1, column 35), references for the `#`-prefixed node
/// text must resolve to the href site. Before the fix the quoted href value
/// matched nothing and this came back empty.
#[test]
fn html_quoted_href_site_resolves_references_for_the_fragment_name() {
    let source = SourceFile::from_content(
        "test.html",
        "<div id=\"main\" class=\"hero\" href=\"#main\">x</div>\n".to_string(),
        Grammar::Html,
    )
    .unwrap();

    assert_eq!(
        reference_positions(&source, "#main", 1, 35),
        [(1, 35)],
        "references for '#main' must include the quoted href site"
    );
}

/// Single-quoted values produce the identical
/// `(quoted_attribute_value (attribute_value))` shape (measured against the
/// pinned html grammar), so one twin covers both quote characters.
#[test]
fn html_single_quoted_id_behaves_as_quoted() {
    let source = SourceFile::from_content(
        "test.html",
        "<div id='main'>x</div>\n".to_string(),
        Grammar::Html,
    )
    .unwrap();

    let rows = symbol_names_and_kinds(&source);
    assert!(
        rows.iter().any(|(n, k)| n == "main" && k == "Field"),
        "html symbols: single-quoted id value must be a Field symbol, got {rows:?}"
    );
    assert_eq!(
        reference_positions(&source, "main", 1, 10),
        [(1, 10)],
        "references for the single-quoted id value must resolve to its site"
    );
}

// ── Controls: shapes the fix must not disturb ───────────────────────────────

/// The bare patterns stay: unquoted values are legal HTML and were the one
/// shape that already worked, for both the symbol and the reference surface.
#[test]
fn html_unquoted_id_still_reports_the_symbol_and_its_reference() {
    let source = SourceFile::from_content(
        "test.html",
        "<div id=main>x</div>\n".to_string(),
        Grammar::Html,
    )
    .unwrap();

    let rows = symbol_names_and_kinds(&source);
    assert!(
        rows.iter().any(|(n, k)| n == "main" && k == "Field"),
        "html symbols: unquoted id value must stay a Field symbol, got {rows:?}"
    );
    assert_eq!(
        reference_positions(&source, "main", 1, 9),
        [(1, 9)],
        "references for the unquoted id value must keep resolving"
    );
}

/// A valueless attribute (`<div id>`) produces `(attribute (attribute_name))`
/// with no value node at all: neither pattern may match, so no symbol and no
/// crash — and the tag's own symbol is unaffected.
#[test]
fn html_valueless_id_attribute_emits_no_symbol() {
    let source =
        SourceFile::from_content("test.html", "<div id>x</div>\n".to_string(), Grammar::Html)
            .unwrap();

    let rows = symbol_names_and_kinds(&source);
    assert!(
        rows.iter()
            .all(|(n, _)| !n.is_empty() && n != "id" && n != "main"),
        "html symbols: valueless id must emit no symbol, got {rows:?}"
    );
    assert!(
        rows.iter().any(|(n, _)| n == "div"),
        "html symbols: the tag symbol must be unaffected, got {rows:?}"
    );
}

/// An empty quoted value (`id=""`) produces
/// `(attribute (attribute_name) (quoted_attribute_value))` — the wrapper with
/// NO `attribute_value` child inside — so neither the bare pattern nor the
/// quoted twin matches and no zero-width name is ever emitted.
#[test]
fn html_empty_quoted_id_attribute_emits_no_symbol() {
    let source = SourceFile::from_content(
        "test.html",
        "<div id=\"\">x</div>\n".to_string(),
        Grammar::Html,
    )
    .unwrap();

    let rows = symbol_names_and_kinds(&source);
    assert!(
        rows.iter().all(|(n, _)| !n.is_empty()),
        "html symbols: empty quoted id must emit no (blank) symbol, got {rows:?}"
    );
    assert!(
        rows.iter().any(|(n, _)| n == "div"),
        "html symbols: the tag symbol must be unaffected, got {rows:?}"
    );
}
