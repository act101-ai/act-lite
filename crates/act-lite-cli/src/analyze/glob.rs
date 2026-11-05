//! Glob patterns matched against workspace-relative paths.
enum Segment {
    AnyDirs,
    Name(Vec<char>),
}

/// A compiled include or exclude pattern.
///
/// `*` and `?` stay inside one path component. A component `**` matches zero
/// or more components. Every other character matches itself. A pattern
/// without `/` matches the file name at any depth.
pub struct Glob {
    segments: Option<Vec<Segment>>,
}

impl Glob {
    pub fn new(pattern: &str) -> Glob {
        if pattern.ends_with('/') {
            return Glob { segments: None };
        }
        let mut segments = Vec::new();
        if !pattern.contains('/') {
            segments.push(Segment::AnyDirs);
        }
        segments.extend(pattern.split('/').map(|part| {
            if part == "**" {
                Segment::AnyDirs
            } else {
                Segment::Name(part.chars().collect())
            }
        }));
        Glob {
            segments: Some(segments),
        }
    }

    pub fn is_match(&self, path: &str) -> bool {
        let Some(segments) = &self.segments else {
            return false;
        };
        let parts: Vec<&str> = path.split('/').collect();
        // reach[j]: the segments consumed so far match the first j components.
        let mut reach = vec![false; parts.len() + 1];
        reach[0] = true;
        for segment in segments {
            let mut next = vec![false; parts.len() + 1];
            match segment {
                Segment::AnyDirs => {
                    let mut open = false;
                    for j in 0..=parts.len() {
                        open |= reach[j];
                        next[j] = open;
                    }
                }
                Segment::Name(name) => {
                    for j in 0..parts.len() {
                        next[j + 1] = reach[j] && name_matches(name, parts[j]);
                    }
                }
            }
            reach = next;
        }
        reach[parts.len()]
    }
}

fn name_matches(pattern: &[char], name: &str) -> bool {
    let name: Vec<char> = name.chars().collect();
    let (mut p, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, n));
                p += 1;
            }
            Some('?') => {
                p += 1;
                n += 1;
            }
            Some(&c) if c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => match star {
                Some((sp, sn)) => {
                    p = sp + 1;
                    n = sn + 1;
                    star = Some((sp, sn + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
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
