//! Test regions embedded in source files.

use act_lite_core::CompactRange;
use act_lite_parser::SourceFile;

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

pub fn test_regions(_source: &SourceFile) -> TestRegions {
    TestRegions::default()
}

pub fn blank(content: &str, _regions: &[TestRegion]) -> String {
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use act_lite_parser::Grammar;

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
        assert_eq!(ranges(&found), ["3:1-4:18"]);
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
        assert!(regions("#[cfg(all(not(test), unix))]\nmod t {}\n")
            .regions
            .is_empty());
    }

    #[test]
    fn cfg_any_and_cfg_attr_are_not_regions() {
        assert!(regions("#[cfg(any(test, unix))]\nmod t {}\n")
            .regions
            .is_empty());
        assert!(regions("#[cfg_attr(test, derive(Debug))]\nstruct S;\n")
            .regions
            .is_empty());
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
        assert!(out[FIXTURE.find("#[cfg").unwrap()..]
            .chars()
            .all(|c| c == ' ' || c == '\n'));
    }

    #[test]
    fn blank_leaves_later_locations_and_removes_a_syntax_error_in_a_region() {
        let content = "fn a() {\n    1\n}\n#[cfg(test)]\nmod tests {\n    fn broken( {\n}\nfn b() {}\n";
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
