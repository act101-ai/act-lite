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

/// Finds the test code of a Rust or Zig file; every other grammar has none.
pub fn test_regions(source: &SourceFile) -> TestRegions {
    let mut found = TestRegions::default();
    if source.grammar() == Grammar::Zig {
        collect_zig(source.tree().root_node(), &mut found.regions);
        return found;
    }
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

fn collect(root: Node, content: &str, regions: &mut Vec<TestRegion>) {
    // One entry per level being walked: its first attribute, and whether the
    // attributes since then mark a test function or a `cfg(test)` item.
    let mut levels: Vec<(Option<Node>, bool, bool)> = vec![(None, false, false)];
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return;
    }
    loop {
        let node = cursor.node();
        let (first_attribute, is_test, is_cfg_test_item) =
            levels.last_mut().expect("a level is open while walking");
        match node.kind() {
            "attribute_item" => {
                first_attribute.get_or_insert(node);
                let attribute = attribute(node);
                *is_cfg_test_item |= is_cfg_test(attribute, content);
                *is_test |= is_test_attribute(attribute, content);
            }
            kind if kind.contains("comment") => {}
            kind => {
                let first = first_attribute.take();
                let is_region = *is_cfg_test_item || (*is_test && kind == "function_item");
                *is_test = false;
                *is_cfg_test_item = false;
                if let Some(first) = first
                    && is_region
                {
                    regions.push(region(first.start_position(), first.start_byte(), node));
                } else if cursor.goto_first_child() {
                    levels.push((None, false, false));
                    continue;
                }
            }
        }
        while !cursor.goto_next_sibling() {
            levels.pop();
            if levels.is_empty() || !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// One region per Zig `test_declaration`, from `test` to its closing `}`.
fn collect_zig(root: Node, regions: &mut Vec<TestRegion>) {
    let mut cursor = root.walk();
    let mut stack: Vec<Node> = root.named_children(&mut cursor).collect();
    stack.reverse();
    while let Some(node) = stack.pop() {
        if node.kind() == "test_declaration" {
            regions.push(region(node.start_position(), node.start_byte(), node));
        } else {
            let start = stack.len();
            stack.extend(node.named_children(&mut cursor));
            stack[start..].reverse();
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

/// Whether `list`, the arguments of `all`, holds `test` directly or inside a
/// nested `all(…)`. The nested lists are read from a heap stack, so the
/// nesting depth does not grow the thread stack.
fn list_has_test(list: Node, content: &str) -> bool {
    let mut lists = vec![list];
    while let Some(list) = lists.pop() {
        let mut cursor = list.walk();
        let items: Vec<Node> = list
            .named_children(&mut cursor)
            .filter(|n| !is_comment(n.kind()))
            .collect();
        for (i, item) in items.iter().enumerate() {
            match item.kind() {
                "identifier"
                    if text(*item, content) == "test"
                        && items
                            .get(i + 1)
                            .is_none_or(|next| next.kind() != "token_tree") =>
                {
                    return true;
                }
                "token_tree"
                    if i > 0
                        && items[i - 1].kind() == "identifier"
                        && text(items[i - 1], content) == "all" =>
                {
                    lists.push(*item);
                }
                _ => {}
            }
        }
    }
    false
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
    fn zig_test_declarations_are_one_region_each() {
        let content = "const std = @import(\"std\");\n\
            fn add(a: u8, b: u8) u8 {\n\
            \x20   return a + b;\n\
            }\n\
            test \"n\" {\n\
            \x20   try std.testing.expect(add(1, 2) == 3);\n\
            }\n\
            test add {\n\
            \x20   _ = add(0, 0);\n\
            }\n\
            const S = struct {\n\
            \x20   x: u8,\n\
            \x20   test {}\n\
            };\n";
        let found = test_regions(&parse(content, Grammar::Zig));
        assert!(!found.whole_file);
        assert_eq!(ranges(&found), ["5:1-7:2", "8:1-10:2", "13:5-13:12"]);
        let starts: Vec<&str> = found
            .regions
            .iter()
            .map(|r| &content[r.bytes.clone()])
            .collect();
        assert!(
            starts
                .iter()
                .all(|t| t.starts_with("test") && t.ends_with('}'))
        );
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
