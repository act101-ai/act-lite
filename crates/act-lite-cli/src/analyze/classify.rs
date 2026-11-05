//! Classification of vendored, generated and test files and directories.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

/// Directory names that mark vendored dependencies.
const VENDORED_DIRS: [&str; 7] = [
    "vendor",
    "third_party",
    "third-party",
    "node_modules",
    "bower_components",
    "Pods",
    "Carthage",
];

/// Whether `dir` is a vendored directory: a known name, or a Python virtual
/// environment (a `pyvenv.cfg` file inside).
pub fn is_vendored_dir(dir: &Path) -> bool {
    let named = dir
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| VENDORED_DIRS.contains(&n));
    named || is_regular_file(&dir.join("pyvenv.cfg"))
}

/// Whether `path` is a regular file itself: not a symlink, FIFO, socket or
/// device. Marker and ignore files are read only when this is true.
pub(crate) fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file())
}

const CACHEDIR_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";

/// Whether `dir` holds a `CACHEDIR.TAG` file that starts with the cache
/// directory signature.
pub fn is_generated_dir(dir: &Path) -> bool {
    let tag = dir.join("CACHEDIR.TAG");
    if !is_regular_file(&tag) {
        return false;
    }
    let mut head = Vec::new();
    File::open(tag)
        .and_then(|f| {
            f.take(CACHEDIR_SIGNATURE.len() as u64)
                .read_to_end(&mut head)
        })
        .is_ok()
        && head == CACHEDIR_SIGNATURE
}

const TEST_DIRS: [&str; 5] = ["test", "tests", "__tests__", "spec", "testdata"];

/// Whether `name` matches a test file-name pattern of a language that has one.
fn is_test_name(name: &str) -> bool {
    let Some((stem, ext)) = name.rsplit_once('.') else {
        return false;
    };
    let suffixed = |ends: &[&str]| ends.iter().any(|e| stem.ends_with(e));
    match ext {
        "go" => stem.ends_with("_test"),
        "py" => stem.starts_with("test_") || stem.ends_with("_test") || name == "conftest.py",
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" => {
            stem.ends_with(".test") || stem.ends_with(".spec")
        }
        "java" | "kt" | "kts" | "scala" | "cs" | "swift" | "php" => {
            suffixed(&["Test", "Tests", "Spec"])
        }
        "dart" | "exs" | "sh" if stem.ends_with("_test") => true,
        "sh" => stem.starts_with("test_"),
        "rb" => suffixed(&["_spec", "_test"]) || stem.starts_with("test_"),
        "c" | "cc" | "cpp" | "cxx" => {
            suffixed(&["_test", "_unittest"]) || stem.starts_with("test_")
        }
        "lua" => suffixed(&["_spec", "_test"]),
        "hcl" => stem.ends_with(".tftest"),
        _ => false,
    }
}

/// Whether the relative path `rel_path` (`/` separators, a trailing `/` for a
/// directory) names a test file or lies below a test directory.
pub fn is_test_file(rel_path: &str) -> bool {
    let mut parts = rel_path.split('/');
    let name = parts.next_back().unwrap_or_default();
    parts.any(|d| TEST_DIRS.contains(&d)) || is_test_name(name)
}

/// Whether `file_name` is a generated file by name.
pub fn is_generated_name(file_name: &str) -> bool {
    const SUFFIXES: [&str; 9] = [
        ".min.js",
        ".min.css",
        ".pb.go",
        "_pb2.py",
        "_pb2_grpc.py",
        ".pb.cc",
        ".pb.h",
        ".g.dart",
        ".freezed.dart",
    ];
    SUFFIXES.iter().any(|s| file_name.ends_with(s))
        || file_name.to_ascii_lowercase().ends_with(".designer.cs")
        || file_name.ends_with(".g.cs")
        || file_name.contains(".generated.")
        || file_name == "package-lock.json"
        || file_name == "pnpm-lock.yaml"
}

const HEADER_BYTES: u64 = 4096;
const HEADER_LINES: usize = 20;
const COMMENT_OPENERS: [&str; 7] = ["//", "/*", "*", "#", "--", "<!--", ";"];

/// Whether `line` starts with a comment opener and, on the same line, holds
/// `@generated` or `Code generated` followed by `DO NOT EDIT`.
fn is_generated_marker_line(line: &str) -> bool {
    let line = line.trim_start();
    COMMENT_OPENERS.iter().any(|o| line.starts_with(o))
        && (line.contains("@generated")
            || line
                .split_once("Code generated")
                .is_some_and(|(_, rest)| rest.contains("DO NOT EDIT")))
}

/// Whether the first lines of the file at `path` carry a generated-file
/// marker in a comment. Only a regular file is read.
pub fn has_generated_header(path: &Path) -> io::Result<bool> {
    if !fs::symlink_metadata(path)?.file_type().is_file() {
        return Ok(false);
    }
    let mut head = Vec::new();
    File::open(path)?
        .take(HEADER_BYTES)
        .read_to_end(&mut head)?;
    let head = String::from_utf8_lossy(&head);
    Ok(head
        .lines()
        .take(HEADER_LINES)
        .any(is_generated_marker_line))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const SIGNATURE: &str = "Signature: 8a477f597d28d172789f06886806bc55";

    #[test]
    fn vendored_names_are_vendored() {
        let tmp = tempfile::tempdir().unwrap();
        for name in VENDORED_NAMES {
            let dir = tmp.path().join(name);
            fs::create_dir(&dir).unwrap();
            assert!(is_vendored_dir(&dir), "{name}");
        }
    }

    const VENDORED_NAMES: [&str; 7] = [
        "vendor",
        "third_party",
        "third-party",
        "node_modules",
        "bower_components",
        "Pods",
        "Carthage",
    ];

    #[test]
    fn vendored_nested_name_uses_last_component() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("a").join("node_modules");
        fs::create_dir_all(&dir).unwrap();
        assert!(is_vendored_dir(&dir));
    }

    #[test]
    fn pyvenv_cfg_marks_vendored() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("env");
        fs::create_dir(&dir).unwrap();
        assert!(!is_vendored_dir(&dir));
        fs::write(dir.join("pyvenv.cfg"), "home = /x\n").unwrap();
        assert!(is_vendored_dir(&dir));
    }

    #[test]
    fn pyvenv_cfg_directory_does_not_mark_vendored() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("env");
        fs::create_dir_all(dir.join("pyvenv.cfg")).unwrap();
        assert!(!is_vendored_dir(&dir));
    }

    #[test]
    fn near_vendored_names_are_not_vendored() {
        let tmp = tempfile::tempdir().unwrap();
        for name in [
            "vendors",
            "Vendor",
            "pods",
            "carthage",
            "node_module",
            "src",
        ] {
            let dir = tmp.path().join(name);
            fs::create_dir(&dir).unwrap();
            assert!(!is_vendored_dir(&dir), "{name}");
        }
    }

    #[test]
    fn cachedir_tag_with_signature_is_generated() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("CACHEDIR.TAG"),
            format!("{SIGNATURE}\n# comment\n"),
        )
        .unwrap();
        assert!(is_generated_dir(tmp.path()));
    }

    #[test]
    fn cachedir_tag_with_exact_signature_only_is_generated() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("CACHEDIR.TAG"), SIGNATURE).unwrap();
        assert!(is_generated_dir(tmp.path()));
    }

    #[test]
    fn cachedir_tag_with_other_bytes_is_not_generated() {
        let tmp = tempfile::tempdir().unwrap();
        let tag = tmp.path().join("CACHEDIR.TAG");
        for body in [
            "Signature: 8a477f597d28d172789f06886806bc54".to_string(),
            "signature: 8a477f597d28d172789f06886806bc55".to_string(),
            format!(" {SIGNATURE}"),
            SIGNATURE[..42].to_string(),
            String::new(),
        ] {
            fs::write(&tag, &body).unwrap();
            assert!(!is_generated_dir(tmp.path()), "{body:?}");
        }
    }

    #[test]
    fn missing_cachedir_tag_is_not_generated() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_generated_dir(tmp.path()));
        assert!(!is_generated_dir(&tmp.path().join("absent")));
    }

    #[test]
    fn generated_names_are_generated() {
        for name in [
            "a.min.js",
            "a.min.css",
            "a.pb.go",
            "a_pb2.py",
            "a_pb2_grpc.py",
            "a.pb.cc",
            "a.pb.h",
            "a.g.dart",
            "a.freezed.dart",
            "a.designer.cs",
            "Form1.Designer.cs",
            "Form1.DESIGNER.CS",
            "a.g.cs",
            "a.generated.rs",
            "a.generated.ts",
            "a.generated.",
            ".generated.rs",
            "package-lock.json",
            "pnpm-lock.yaml",
        ] {
            assert!(is_generated_name(name), "{name}");
        }
    }

    #[test]
    fn other_names_are_not_generated() {
        for name in [
            "min.js",
            "a.pb.rs",
            "a.min.JS",
            "a.G.cs",
            "a.PB.go",
            "a_pb2.rs",
            "generated.rs",
            "a.generated",
            "Package-lock.json",
            "yarn.lock",
            "xpackage-lock.json",
            "a.designer.cs.bak",
            "src.rs",
        ] {
            assert!(!is_generated_name(name), "{name}");
        }
    }

    fn header_file(body: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("f.src");
        fs::write(&path, body).unwrap();
        (tmp, path)
    }

    fn header(body: &[u8]) -> bool {
        let (_tmp, path) = header_file(body);
        has_generated_header(&path).unwrap()
    }

    #[test]
    fn generated_header_markers_match() {
        for body in [
            "// Code generated by synth. DO NOT EDIT.\nfn a() {}\n",
            "/* Code generated by synth. DO NOT EDIT. */\n",
            "# Code generated by synth, DO NOT EDIT\n",
            "// @generated\n",
            "/* @generated */\n",
            "# @generated\n",
            "-- @generated\n",
            "; @generated\n",
            "<!-- @generated -->\n",
            "  * @generated\n",
            "\t// @generated\n",
            "/**\n * @generated\n */\n",
            "// a Code generated b DO NOT EDIT c\n",
        ] {
            assert!(header(body.as_bytes()), "{body:?}");
        }
    }

    #[test]
    fn generated_header_on_line_20_matches() {
        let mut body = "// plain\n".repeat(19);
        body.push_str("# @generated\n");
        assert!(header(body.as_bytes()));
    }

    #[test]
    fn generated_header_on_line_21_does_not_match() {
        let mut body = "// plain\n".repeat(20);
        body.push_str("# @generated\n");
        assert!(!header(body.as_bytes()));
    }

    #[test]
    fn generated_header_needs_comment_opener() {
        for body in [
            "let s = \"@generated\";\n",
            "// DO NOT EDIT. Code generated\n",
            "// Code generated by synth\n",
            "// DO NOT EDIT\n",
            "x = 1 // @generated\n",
            "@generated\n",
            "Code generated DO NOT EDIT\n",
            "\n\n",
            "",
        ] {
            assert!(!header(body.as_bytes()), "{body:?}");
        }
    }

    #[test]
    fn generated_header_after_4096_bytes_does_not_match() {
        let mut body = format!("// {}\n", "x".repeat(4000));
        body.push_str(&" ".repeat(4096 - body.len() - 1));
        body.push_str("\n// @generated\n");
        assert!(body.len() > 4096);
        assert!(!header(body.as_bytes()));
    }

    #[test]
    fn generated_header_ending_at_4096_bytes_matches() {
        let marker = "// @generated";
        let mut body = " ".repeat(4096 - marker.len() - 1);
        body.push('\n');
        body.push_str(marker);
        assert_eq!(body.len(), 4096);
        body.push('\n');
        assert!(header(body.as_bytes()));
    }

    #[test]
    fn cut_multibyte_character_at_4096_is_not_an_error() {
        let mut body = "// @generated\n".to_string();
        let pad = 4095 - body.len();
        body.push_str(&"a".repeat(pad));
        body.push('é');
        let (_tmp, path) = header_file(body.as_bytes());
        assert!(has_generated_header(&path).unwrap());
        let (_tmp, path) = header_file(&[b'x', 0xff, 0xfe, b'\n']);
        assert!(!has_generated_header(&path).unwrap());
    }

    #[test]
    fn header_read_error_is_returned() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(has_generated_header(&tmp.path().join("absent")).is_err());
    }

    const TEST_NAMES: &[&str] = &[
        "a_test.go",
        "test_a.py",
        "a_test.py",
        "conftest.py",
        "a.test.js",
        "a.test.mjs",
        "a.test.cjs",
        "a.test.jsx",
        "a.test.ts",
        "a.test.tsx",
        "a.spec.js",
        "a.spec.mjs",
        "a.spec.cjs",
        "a.spec.jsx",
        "a.spec.ts",
        "a.spec.tsx",
        "FooTest.java",
        "FooTests.java",
        "FooSpec.java",
        "FooTest.kt",
        "FooTests.kt",
        "FooSpec.kt",
        "FooTest.kts",
        "FooTests.kts",
        "FooSpec.kts",
        "FooTest.scala",
        "FooTests.scala",
        "FooSpec.scala",
        "FooTest.cs",
        "FooTests.cs",
        "FooSpec.cs",
        "FooTest.swift",
        "FooTests.swift",
        "FooSpec.swift",
        "FooTest.php",
        "FooTests.php",
        "FooSpec.php",
        "a_test.dart",
        "foo_spec.rb",
        "foo_test.rb",
        "test_foo.rb",
        "foo_test.exs",
        "foo_test.c",
        "foo_unittest.c",
        "test_foo.c",
        "foo_test.cc",
        "foo_unittest.cc",
        "test_foo.cc",
        "foo_test.cpp",
        "foo_unittest.cpp",
        "test_foo.cpp",
        "foo_test.cxx",
        "foo_unittest.cxx",
        "test_foo.cxx",
        "a_spec.lua",
        "a_test.lua",
        "a_test.sh",
        "test_a.sh",
        "main.tftest.hcl",
        "src/deep/a_test.go",
        "src/deep/FooTest.java",
    ];

    #[test]
    fn every_test_file_name_pattern_is_a_test_file() {
        for name in TEST_NAMES {
            assert!(is_test_file(name), "{name}");
        }
    }

    #[test]
    fn test_directory_components_make_test_files() {
        for dir in ["test", "tests", "__tests__", "spec", "testdata"] {
            for path in [
                format!("{dir}/a.rs"),
                format!("src/{dir}/a.rs"),
                format!("src/{dir}/deep/a.go"),
                format!("{dir}/"),
                format!("src/{dir}/"),
            ] {
                assert!(is_test_file(&path), "{path}");
            }
        }
        for path in [
            "spec/a.rb",
            "__tests__/a.js",
            "testdata/a.go",
            "test/a.rs",
            "tests/cli.rs",
        ] {
            assert!(is_test_file(path), "{path}");
        }
    }

    #[test]
    fn non_test_paths_are_not_test_files() {
        for path in [
            "contest.py",
            "latest.go",
            "testing.py",
            "attest.rb",
            "src/lib.rs",
            "src/testing/a.rs",
            "a_test.rs",
            "a_test.css",
            "a.test.rs",
            "a.test.json",
            "a_test.md",
            "src/tests.rs",
            "src/test.rs",
            "Tests/a.rs",
            "mytest/a.rs",
            "specs/a.rs",
            "footest.java",
            "FooTest.rs",
            "FooTest.js",
            "a.TEST.js",
            "test_a.rs",
            "test_a.js",
            "a_test.exs.bak",
            ".",
            "src/",
            "",
        ] {
            assert!(!is_test_file(path), "{path}");
        }
    }

    #[test]
    fn test_name_in_directory_path_is_not_a_name_match() {
        assert!(!is_test_file("a_test.go/"));
    }
}
