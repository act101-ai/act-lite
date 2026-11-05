//! Test regions embedded in source files.

use act_lite_core::CompactRange;
use act_lite_parser::{Grammar, SourceFile};
use tree_sitter::{Node, Point};

/// A test item: its range from the first attribute to the item's end, and
/// the same span as byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRegion {
    pub range: CompactRange,
    pub bytes: std::ops::Range<usize>,
}

/// The test code of one file: the whole file, or the listed regions.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct TestRegions {
    pub whole_file: bool,
    pub regions: Vec<TestRegion>,
}

/// Finds the test code of a Rust file; every other grammar has none.
pub fn test_regions(source: &SourceFile) -> TestRegions {
    let mut found = TestRegions::default();
    if source.grammar() != Grammar::Rust {
        return found;
    }
    let content = source.content();
    let root = source.tree().root_node();
    let mut cursor = root.walk();
    found.whole_file = root
        .children(&mut cursor)
        .any(|node| node.kind() == "inner_attribute_item" && is_cfg_test(attribute(node), content));
    if !found.whole_file {
        collect(root, content, &mut found.regions);
    }
    found
}

/// Overwrites each byte inside the regions with a space, keeping newlines.
pub fn blank(content: &str, regions: &[TestRegion]) -> String {
    let mut bytes = content.as_bytes().to_vec();
    for region in regions {
        for byte in &mut bytes[region.bytes.clone()] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    String::from_utf8(bytes).expect("blanking a UTF-8 span keeps the text UTF-8")
}

fn collect(parent: Node, content: &str, regions: &mut Vec<TestRegion>) {
    let mut cursor = parent.walk();
    let mut first_attribute: Option<Node> = None;
    let mut is_test = false;
    let mut is_cfg_test_item = false;
    for node in parent.children(&mut cursor) {
        match node.kind() {
            "attribute_item" => {
                first_attribute.get_or_insert(node);
                let attribute = attribute(node);
                is_cfg_test_item |= is_cfg_test(attribute, content);
                is_test |= is_test_attribute(attribute, content);
            }
            kind if kind.contains("comment") => {}
            kind => {
                if let Some(first) = first_attribute.take()
                    && (is_cfg_test_item || (is_test && kind == "function_item"))
                {
                    regions.push(region(first.start_position(), first.start_byte(), node));
                    is_test = false;
                    is_cfg_test_item = false;
                    continue;
                }
                is_test = false;
                is_cfg_test_item = false;
                collect(node, content, regions);
            }
        }
    }
}

fn region(start: Point, start_byte: usize, item: Node) -> TestRegion {
    let end = item.end_position();
    TestRegion {
        range: CompactRange::new(
            start.row as u32 + 1,
            start.column as u32 + 1,
            end.row as u32 + 1,
            end.column as u32 + 1,
        ),
        bytes: start_byte..item.end_byte(),
    }
}

fn attribute(item: Node) -> Option<Node> {
    let mut cursor = item.walk();
    item.children(&mut cursor).find(|c| c.kind() == "attribute")
}

fn is_test_attribute(attribute: Option<Node>, content: &str) -> bool {
    attribute
        .and_then(|a| a.named_child(0))
        .is_some_and(|path| text(path, content).rsplit("::").next() == Some("test"))
}

fn is_cfg_test(attribute: Option<Node>, content: &str) -> bool {
    let Some(attribute) = attribute else {
        return false;
    };
    let Some(path) = attribute.named_child(0) else {
        return false;
    };
    let Some(arguments) = attribute.child_by_field_name("arguments") else {
        return false;
    };
    text(path, content) == "cfg" && is_test_predicate(arguments, content)
}

/// Whether a parenthesized predicate is `(test)` or `(all(…, test, …))`.
fn is_test_predicate(arguments: Node, content: &str) -> bool {
    let mut cursor = arguments.walk();
    let items: Vec<Node> = arguments
        .named_children(&mut cursor)
        .filter(|n| !is_comment(n.kind()))
        .collect();
    match items.as_slice() {
        [only] if only.kind() == "identifier" => text(*only, content) == "test",
        [name, list] if name.kind() == "identifier" && list.kind() == "token_tree" => {
            text(*name, content) == "all" && list_has_test(*list, content)
        }
        _ => false,
    }
}

fn is_comment(kind: &str) -> bool {
    kind == "line_comment" || kind == "block_comment"
}

fn list_has_test(list: Node, content: &str) -> bool {
    let mut cursor = list.walk();
    let items: Vec<Node> = list
        .named_children(&mut cursor)
        .filter(|n| !is_comment(n.kind()))
        .collect();
    items.iter().enumerate().any(|(i, item)| match item.kind() {
        "identifier" => {
            text(*item, content) == "test"
                && items
                    .get(i + 1)
                    .is_none_or(|next| next.kind() != "token_tree")
        }
        "token_tree" => {
            i > 0
                && items[i - 1].kind() == "identifier"
                && text(items[i - 1], content) == "all"
                && list_has_test(*item, content)
        }
        _ => false,
    })
}

fn text<'a>(node: Node, content: &'a str) -> &'a str {
    &content[node.byte_range()]
}

#[cfg(test)]
mod tests {
    #[test]
    fn comments_inside_the_cfg_predicate_are_skipped() {
        let found = regions("#[cfg(/* c */ test)]\nmod t {}\n");
        assert_eq!(found.regions.len(), 1, "a comment before `test` is skipped");
        let found = regions("#[cfg(all(/* c */ test, unix))]\nmod t {}\n");
        assert_eq!(found.regions.len(), 1, "a comment inside all(…) is skipped");
        let found = regions("#[cfg(any(/* c */ test, unix))]\nmod t {}\n");
        assert_eq!(found.regions.len(), 0);
    }

    use super::*;

    fn parse(content: &str, grammar: Grammar) -> SourceFile {
        SourceFile::from_content("src/lib.rs", content.to_string(), grammar).unwrap()
    }

    fn regions(content: &str) -> TestRegions {
        test_regions(&parse(content, Grammar::Rust))
    }

    fn ranges(found: &TestRegions) -> Vec<String> {
        found.regions.iter().map(|r| r.range.to_string()).collect()
    }

    const FIXTURE: &str = "fn add(a: i32, b: i32) -> i32 {\n    let sum = a + b;\n    if sum > 100 {\n        return 100;\n    }\n    sum\n}\n\
        \n\
        \n\
        #[cfg(test)]\n\
        mod tests {\n\
        \x20   #[test]\n\
        \x20   fn t() {}\n\
        }\n";

    #[test]
    fn cfg_test_module_is_one_region() {
        let found = regions(FIXTURE);
        assert!(!found.whole_file);
        assert_eq!(ranges(&found), ["10:1-14:2"]);
    }

    #[test]
    fn attribute_with_last_segment_test_outside_a_test_item() {
        let found = regions("fn a() {}\n\n#[tokio::test]\nasync fn t() {}\n");
        assert_eq!(ranges(&found), ["3:1-4:16"]);
    }

    #[test]
    fn test_function_inside_a_test_module_adds_no_region() {
        let found = regions("#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n");
        assert_eq!(ranges(&found), ["1:1-5:2"]);
    }

    #[test]
    fn several_attributes_start_at_the_first() {
        let found = regions("#[test]\n#[ignore]\nfn t() {}\n");
        assert_eq!(ranges(&found), ["1:1-3:10"]);
    }

    #[test]
    fn test_attribute_on_a_non_function_is_not_a_region() {
        assert!(regions("#[test]\nstruct S;\n").regions.is_empty());
    }

    #[test]
    fn cfg_all_with_test_is_a_region() {
        let found = regions("#[cfg(all(test, unix))]\nmod t {}\n");
        assert_eq!(ranges(&found), ["1:1-2:9"]);
    }

    #[test]
    fn cfg_all_without_a_direct_test_is_not_a_region() {
        assert!(
            regions("#[cfg(all(not(test), unix))]\nmod t {}\n")
                .regions
                .is_empty()
        );
    }

    #[test]
    fn cfg_any_and_cfg_attr_are_not_regions() {
        assert!(
            regions("#[cfg(any(test, unix))]\nmod t {}\n")
                .regions
                .is_empty()
        );
        assert!(
            regions("#[cfg_attr(test, derive(Debug))]\nstruct S;\n")
                .regions
                .is_empty()
        );
    }

    #[test]
    fn inner_cfg_test_marks_the_whole_file() {
        let found = regions("#![cfg(test)]\nfn t() {}\n");
        assert!(found.whole_file);
    }

    #[test]
    fn inner_cfg_test_inside_a_module_is_not_the_whole_file() {
        assert!(!regions("mod m {\n    #![cfg(test)]\n}\n").whole_file);
    }

    #[test]
    fn out_of_line_test_module_declaration_is_a_region() {
        let found = regions("fn a() {}\n#[cfg(test)]\nmod tests;\n");
        assert_eq!(ranges(&found), ["2:1-3:11"]);
    }

    #[test]
    fn other_grammars_have_no_region() {
        let go = parse("package a\n\nfunc TestX() {}\n", Grammar::Go);
        let py = parse("def test_x():\n    pass\n", Grammar::Python);
        let ts = parse("test('x', () => {});\n", Grammar::TypeScript);
        for file in [go, py, ts] {
            assert_eq!(test_regions(&file), TestRegions::default());
        }
    }

    #[test]
    fn blank_keeps_length_and_newlines() {
        let found = regions(FIXTURE);
        let out = blank(FIXTURE, &found.regions);
        assert_eq!(out.len(), FIXTURE.len());
        assert_eq!(
            out.match_indices('\n').collect::<Vec<_>>(),
            FIXTURE.match_indices('\n').collect::<Vec<_>>()
        );
        assert!(out[..FIXTURE.find("#[cfg").unwrap()] == FIXTURE[..FIXTURE.find("#[cfg").unwrap()]);
        assert!(
            out[FIXTURE.find("#[cfg").unwrap()..]
                .chars()
                .all(|c| c == ' ' || c == '\n')
        );
    }

    #[test]
    fn blank_leaves_later_locations_and_removes_a_syntax_error_in_a_region() {
        let content =
            "fn a() {\n    1\n}\n#[cfg(test)]\nmod tests {\n    fn broken( {\n}\nfn b() {}\n";
        let before = parse(content, Grammar::Rust);
        assert!(before.tree().root_node().has_error());
        let found = test_regions(&before);
        assert_eq!(found.regions.len(), 1);
        let blanked = blank(content, &found.regions);
        let after = parse(&blanked, Grammar::Rust);
        assert!(!after.tree().root_node().has_error());
        let first = after.tree().root_node().child(0).unwrap();
        assert_eq!(first.kind(), "function_item");
        assert_eq!(first.start_position().row, 0);
        assert_eq!(first.end_position().row, 2);
    }
}
