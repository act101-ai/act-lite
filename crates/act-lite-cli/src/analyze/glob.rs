//! Glob patterns matched against workspace-relative paths.

/// A compiled include or exclude pattern.
pub struct Glob;

impl Glob {
    pub fn new(_pattern: &str) -> Glob {
        Glob
    }

    pub fn is_match(&self, _path: &str) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::Glob;

    fn m(pattern: &str, path: &str) -> bool {
        Glob::new(pattern).is_match(path)
    }

    #[test]
    fn double_star_component_spans_zero_or_more_components() {
        assert!(m("src/**/x.rs", "src/x.rs"));
        assert!(m("src/**/x.rs", "src/a/b/x.rs"));
        assert!(!m("src/**/x.rs", "src/ax.rs"));
        assert!(m("**/a.rs", "a.rs"));
        assert!(m("**/a.rs", "x/y/a.rs"));
    }

    #[test]
    fn pattern_without_slash_matches_file_name_at_any_depth() {
        assert!(m("*.rs", "a.rs"));
        assert!(m("*.rs", "lib.rs"));
        assert!(m("*.rs", "src/x/a.rs"));
        assert!(!m("*.rs", "src/x/a.rs.txt"));
    }

    #[test]
    fn pattern_with_slash_matches_whole_path() {
        assert!(m("src/*.rs", "src/x.rs"));
        assert!(m("src/*.rs", "src/ax.rs"));
        assert!(!m("src/*.rs", "src/a/b/x.rs"));
        assert!(!m("src/*.rs", "lib/src/x.rs"));
    }

    #[test]
    fn question_mark_matches_one_character_but_not_slash() {
        assert!(m("?.rs", "a.rs"));
        assert!(!m("?.rs", "ab.rs"));
        assert!(!m("src?x.rs", "src/x.rs"));
        assert!(m("src/?.rs", "src/é.rs"));
    }

    #[test]
    fn brackets_are_literal() {
        assert!(m("[ab].rs", "[ab].rs"));
        assert!(!m("[ab].rs", "a.rs"));
        assert!(!m("[ab].rs", "b.rs"));
    }

    #[test]
    fn double_star_inside_component_is_single_star() {
        assert!(m("a**b.rs", "axxb.rs"));
        assert!(m("a**b.rs", "ab.rs"));
        assert!(!m("a**b.rs", "a/b.rs"));
        assert!(!m("src/a**b.rs", "src/a/x/b.rs"));
    }

    #[test]
    fn trailing_slash_matches_no_file() {
        assert!(!m("src/", "src"));
        assert!(!m("src/", "src/x.rs"));
        assert!(!m("src/**/", "src/x.rs"));
    }

    #[test]
    fn matching_is_case_sensitive() {
        assert!(!m("*.RS", "a.rs"));
        assert!(m("*.RS", "a.RS"));
    }
}
