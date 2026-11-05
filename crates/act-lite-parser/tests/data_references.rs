use act_lite_parser::{QueryResult, Workspace, execute_references_for_symbol_across};

/// `(line, column)` of every reference to `name`, anchored at `line:column`.
fn references(file: &str, source: &str, name: &str, line: u32, column: u32) -> Vec<(u32, u32)> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(file);
    std::fs::write(&path, source).unwrap();
    let mut ws = Workspace::new(dir.path()).unwrap();
    ws.open_file(&path).unwrap();
    let sf = ws.get_file(&path).unwrap();
    match execute_references_for_symbol_across(sf, &[], name, line, column).unwrap() {
        QueryResult::References { references, .. } => references
            .into_iter()
            .map(|r| (r.line(), r.column()))
            .collect(),
        other => panic!("expected references, got {other:?}"),
    }
}

#[test]
fn yaml_key_is_a_reference_and_an_equal_value_is_not() {
    let source = "name: services\nservices:\n  web: 1\n";
    assert_eq!(references("f.yaml", source, "services", 2, 1), [(2, 1)]);
}

#[test]
fn json_key_is_a_reference_and_an_equal_value_is_not() {
    let source = "{\n  \"name\": \"name\",\n  \"other\": 1\n}\n";
    assert_eq!(references("f.json", source, "name", 2, 4), [(2, 4)]);
}

#[test]
fn markdown_heading_is_a_reference_and_an_equal_word_is_not() {
    let source = "# Run\n\nRun it.\n";
    assert_eq!(references("f.md", source, "Run", 1, 3), [(1, 3)]);
}

#[test]
fn yaml_alias_is_a_reference_to_its_anchor() {
    let source = "base: &x
  a: 1
other: *x
";
    assert_eq!(references("f.yaml", source, "x", 1, 8), [(1, 8), (3, 9)]);
    assert_eq!(references("f.yaml", source, "x", 3, 9), [(1, 8), (3, 9)]);
}

#[test]
fn dockerfile_stage_alias_and_from_flag_are_references_but_an_image_name_is_not() {
    let source = "FROM alpine AS build\nFROM build AS final\nCOPY --from=build /a /a\n";
    let expected = [(1, 16), (2, 6), (3, 13)];
    assert_eq!(references("Dockerfile", source, "build", 1, 16), expected);
    // Anchored inside the `--from=` flag.
    assert_eq!(references("Dockerfile", source, "build", 3, 14), expected);
    // `alpine` is an image, not a declared stage: only the `FROM` image name.
    assert_eq!(references("Dockerfile", source, "alpine", 1, 6), []);
}
