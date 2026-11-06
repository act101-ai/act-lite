//! Ignore rules from `.gitignore` files and `.git/info/exclude`.

use super::classify::open_regular;
use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The ignore rules that apply to the entries of one directory: git's
/// `gitignore` pattern rules, read from the `.gitignore` files from the
/// work-tree top down to that directory and from `.git/info/exclude` at the
/// top. The global excludes file is not read and `git` is not run.
#[derive(Debug, Clone)]
pub struct IgnoreRules {
    root: PathBuf,
    /// Whether the directory of these rules is inside a work tree.
    in_work_tree: bool,
    /// Each pattern set with the directory its patterns are relative to,
    /// from the lowest precedence to the highest.
    layers: Vec<(PathBuf, Arc<Gitignore>)>,
}

impl IgnoreRules {
    /// The rules for the entries of the workspace root.
    ///
    /// The work-tree top is the nearest of `root` and its ancestors that
    /// contains a `.git` entry; outside a work tree no pattern applies.
    pub fn for_root(root: &Path) -> IgnoreRules {
        let mut rules = IgnoreRules {
            root: root.to_path_buf(),
            in_work_tree: false,
            layers: Vec::new(),
        };
        if let Some(top) = root.ancestors().find(|dir| has_git(dir)) {
            rules.start_work_tree(top);
            for dir in root
                .ancestors()
                .take_while(|dir| *dir != top)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                rules.add(dir, &dir.join(".gitignore"));
            }
        }
        rules
    }

    /// The rules for the entries of `dir`, a directory below the directory
    /// of these rules. A `dir` that contains a `.git` entry is a new
    /// work-tree top, and the outer patterns do not apply in it.
    pub fn child(&self, dir: &Path) -> IgnoreRules {
        let mut rules = self.clone();
        if has_git(dir) {
            rules.layers.clear();
            rules.start_work_tree(dir);
        } else if rules.in_work_tree {
            rules.add(dir, &dir.join(".gitignore"));
        }
        rules
    }

    /// Whether `path` or one of its directories below the workspace root is
    /// ignored. Never true for the workspace root or its ancestors.
    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        if self.root.starts_with(path) {
            return false;
        }
        path.ancestors()
            .take_while(|p| *p != self.root && p.starts_with(&self.root))
            .enumerate()
            .any(|(i, p)| self.matches(p, is_dir || i > 0))
    }

    /// The decision of the pattern with the highest precedence that matches
    /// `path`; a later pattern and a deeper `.gitignore` win.
    fn matches(&self, path: &Path, is_dir: bool) -> bool {
        for (dir, set) in self.layers.iter().rev() {
            let Ok(rel) = path.strip_prefix(dir) else {
                continue;
            };
            if rel.as_os_str().is_empty() {
                continue;
            }
            match set.matched(rel, is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }

    /// Adds `.git/info/exclude` (when `.git` and `info` are directories
    /// themselves, not symlinks) and the `.gitignore` of the work-tree top
    /// `top`.
    fn start_work_tree(&mut self, top: &Path) {
        self.in_work_tree = true;
        let git = top.join(".git");
        let info = git.join("info");
        if is_real_dir(&git) && is_real_dir(&info) {
            self.add(top, &info.join("exclude"));
        }
        self.add(top, &top.join(".gitignore"));
    }

    /// Adds the patterns of `file`, relative to `dir`. A file that is not a
    /// regular file (a symlink among others) or that cannot be read adds no
    /// pattern; a pattern that is not valid is skipped.
    fn add(&mut self, dir: &Path, file: &Path) {
        let Ok(handle) = open_regular(file) else {
            return;
        };
        let mut builder = GitignoreBuilder::new(dir);
        for (i, line) in BufReader::new(handle).lines().enumerate() {
            let Ok(line) = line else {
                break;
            };
            let line = if i == 0 {
                line.trim_start_matches('\u{feff}')
            } else {
                &line
            };
            let _ = builder.add_line(Some(file.to_path_buf()), line);
        }
        if let Ok(set) = builder.build()
            && !set.is_empty()
        {
            self.layers.push((dir.to_path_buf(), Arc::new(set)));
        }
    }
}

/// Whether `dir` contains a `.git` entry (a directory, the file of a linked
/// worktree, or a symlink to an existing target), the same test MCP uses to
/// find a root. A `dir` with a `.git` entry is a work-tree top.
fn has_git(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Whether `path` is a directory itself, not a symlink to one.
fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A temporary directory that no ancestor work tree covers.
    fn outside_work_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            dir.path().ancestors().all(|d| !d.join(".git").exists()),
            "the temporary directory is inside a git work tree"
        );
        dir
    }

    /// The `conv` tree: a `.gitignore` with `build-out/` and `*.log`.
    fn conv(top: &Path) {
        write(&top.join(".gitignore"), "build-out/\n*.log\n");
        write(&top.join("build-out/x.rs"), "");
        write(&top.join("debug.log"), "");
        write(&top.join("src/lib.rs"), "");
    }

    #[test]
    fn work_tree_patterns_apply() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        conv(top);
        let rules = IgnoreRules::for_root(top);
        assert!(rules.is_ignored(&top.join("build-out"), true));
        assert!(rules.is_ignored(&top.join("debug.log"), false));
        assert!(!rules.is_ignored(&top.join("src"), true));
        assert!(!rules.is_ignored(top, true));
        let src = rules.child(&top.join("src"));
        assert!(!src.is_ignored(&top.join("src/lib.rs"), false));
        assert!(rules.is_ignored(&top.join("build-out/x.rs"), false));
    }

    #[test]
    fn outside_a_work_tree_nothing_is_ignored() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        conv(top);
        let rules = IgnoreRules::for_root(top);
        assert!(!rules.is_ignored(&top.join("build-out"), true));
        assert!(!rules.is_ignored(&top.join("debug.log"), false));
        assert!(!rules.is_ignored(&top.join("build-out/x.rs"), false));
    }

    #[test]
    fn git_file_marks_the_top() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        write(&top.join(".git"), "gitdir: /nowhere/.git/worktrees/x\n");
        write(&top.join(".gitignore"), "*.log\n");
        let root = top.join("ws");
        fs::create_dir(&root).unwrap();
        let rules = IgnoreRules::for_root(&root);
        assert!(rules.is_ignored(&root.join("debug.log"), false));
        assert!(!rules.is_ignored(&root.join("a.tmp"), false));
    }

    #[test]
    fn info_exclude_applies() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        write(&top.join(".git/info/exclude"), "*.tmp\n");
        let rules = IgnoreRules::for_root(top);
        assert!(rules.is_ignored(&top.join("a.tmp"), false));
        assert!(!rules.is_ignored(&top.join("a.rs"), false));
    }

    #[test]
    fn ancestor_gitignore_applies_below_the_root() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        write(&top.join(".gitignore"), "*.log\n");
        write(&top.join("a/.gitignore"), "*.bak\n");
        let root = top.join("a/ws");
        fs::create_dir(&root).unwrap();
        let rules = IgnoreRules::for_root(&root);
        assert!(rules.is_ignored(&root.join("x.bak"), false));
        assert!(rules.is_ignored(&root.join("x.log"), false));
        assert!(!rules.is_ignored(&root.join("x.rs"), false));
        let sub = rules.child(&root.join("sub"));
        assert!(sub.is_ignored(&root.join("sub/y.bak"), false));
    }

    #[test]
    fn root_under_an_ignored_directory_is_not_ignored() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        write(&top.join(".gitignore"), "target/\n*.log\n");
        let root = top.join("target/ws");
        fs::create_dir_all(&root).unwrap();
        let rules = IgnoreRules::for_root(&root);
        assert!(!rules.is_ignored(&root, true));
        assert!(!rules.is_ignored(&top.join("target"), true));
        assert!(!rules.is_ignored(&root.join("x.rs"), false));
        assert!(!rules.is_ignored(&root.join("src"), true));
        assert!(rules.is_ignored(&root.join("x.log"), false));
        assert!(rules.is_ignored(&root.join("target"), true));
    }

    #[test]
    fn negation_keeps_a_path() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        write(&top.join(".gitignore"), "*.log\n!keep.log\n");
        let rules = IgnoreRules::for_root(top);
        assert!(rules.is_ignored(&top.join("debug.log"), false));
        assert!(!rules.is_ignored(&top.join("keep.log"), false));
    }

    #[test]
    fn nested_repository_uses_its_own_patterns() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        write(&top.join(".gitignore"), "*.log\n");
        let inner = top.join("inner");
        fs::create_dir_all(inner.join(".git")).unwrap();
        write(&inner.join(".gitignore"), "*.bak\n");
        let rules = IgnoreRules::for_root(top).child(&inner);
        assert!(!rules.is_ignored(&inner.join("x.log"), false));
        assert!(rules.is_ignored(&inner.join("x.bak"), false));
    }

    #[test]
    fn nested_repository_outside_a_work_tree() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        write(&top.join(".gitignore"), "*.log\n");
        let inner = top.join("inner");
        write(&inner.join(".git/info/exclude"), "*.tmp\n");
        write(&inner.join(".gitignore"), "*.bak\n");
        let rules = IgnoreRules::for_root(top);
        assert!(!rules.is_ignored(&top.join("x.log"), false));
        let inner_rules = rules.child(&inner);
        assert!(inner_rules.is_ignored(&inner.join("x.bak"), false));
        assert!(inner_rules.is_ignored(&inner.join("x.tmp"), false));
        assert!(!inner_rules.is_ignored(&inner.join("x.log"), false));
    }

    #[test]
    fn patterns_are_relative_to_their_directory() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        write(&top.join("sub/.gitignore"), "/a.rs\n");
        let rules = IgnoreRules::for_root(top);
        let sub = rules.child(&top.join("sub"));
        assert!(sub.is_ignored(&top.join("sub/a.rs"), false));
        assert!(!rules.is_ignored(&top.join("a.rs"), false));
        assert!(!sub.is_ignored(&top.join("a.rs"), false));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_git_is_a_work_tree_top() {
        let outside = outside_work_tree();
        write(&outside.path().join("info/exclude"), "*.tmp\n");
        let tmp = outside_work_tree();
        let top = tmp.path();
        std::os::unix::fs::symlink(outside.path(), top.join(".git")).unwrap();
        write(&top.join(".gitignore"), "*.log\n");
        let rules = IgnoreRules::for_root(top);
        assert!(!rules.is_ignored(&top.join("a.tmp"), false));
        assert!(rules.is_ignored(&top.join("a.log"), false));
    }

    #[cfg(unix)]
    #[test]
    fn nested_symlinked_git_drops_the_outer_patterns() {
        let outside = outside_work_tree();
        write(&outside.path().join("info/exclude"), "*.tmp\n");
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        write(&top.join(".gitignore"), "*.log\n");
        let inner = top.join("inner");
        fs::create_dir(&inner).unwrap();
        std::os::unix::fs::symlink(outside.path(), inner.join(".git")).unwrap();
        let rules = IgnoreRules::for_root(top).child(&inner);
        assert!(!rules.is_ignored(&inner.join("a.tmp"), false));
        assert!(!rules.is_ignored(&inner.join("a.log"), false));
    }

    #[cfg(unix)]
    #[test]
    fn info_symlinked_outside_adds_no_pattern() {
        let outside = outside_work_tree();
        write(&outside.path().join("exclude"), "*.tmp\n");
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        std::os::unix::fs::symlink(outside.path(), top.join(".git/info")).unwrap();
        let rules = IgnoreRules::for_root(top);
        assert!(!rules.is_ignored(&top.join("a.tmp"), false));
    }

    #[test]
    fn unreadable_gitignore_adds_no_pattern() {
        let tmp = outside_work_tree();
        let top = tmp.path();
        fs::create_dir(top.join(".git")).unwrap();
        write(&top.join(".gitignore"), "*.log\n");
        fs::create_dir_all(top.join("sub/.gitignore")).unwrap();
        let sub = IgnoreRules::for_root(top).child(&top.join("sub"));
        assert!(sub.is_ignored(&top.join("sub/x.log"), false));
        assert!(!sub.is_ignored(&top.join("sub/x.rs"), false));
    }
}
