//! The analyzers on the added programming languages, on the command line and
//! over MCP with equal results. Every expected value is written by hand from
//! the source and the fixed thresholds (complexity 10, exceeded when strictly
//! greater).

mod langs_added;

use langs_added::{Mcp, cli_json, fixture};
use serde_json::{Value, json};
use std::path::Path;

const ANALYZERS: [&str; 3] = ["patterns", "hotspots", "clones"];

/// One R function with ten `if`s: complexity 11.
fn r_branchy() -> String {
    let mut source = String::from("f <- function(x) {\n");
    for _ in 0..10 {
        source.push_str("  if (x > 0) { x <- 0 }\n");
    }
    source.push_str("  x\n}\n");
    source
}

/// A `@testset` holding one `@test` and one Julia function with ten `if`s:
/// complexity 11.
fn julia_testset() -> String {
    let mut source = String::from("@testset \"t\" begin\n    @test 1 == 1\n    function f(x)\n");
    for _ in 0..10 {
        source.push_str("        if x > 0; x = 0; end\n");
    }
    source.push_str("        x\n    end\nend\n");
    source
}

/// One `fn` and one `test "t"` block with ten `if`s: complexity 11.
fn zig_with_a_test() -> String {
    let mut source =
        String::from("fn f(a: u8) u8 {\n    return a;\n}\ntest \"t\" {\n    const x: u8 = 1;\n");
    for _ in 0..10 {
        source.push_str("    if (x > 0) {}\n");
    }
    source.push_str("}\n");
    source
}

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    dir
}

/// `analyze <analyzer> <args>` on the command line and `analyze_<analyzer>`
/// with `params` over MCP; asserts both give the same result and returns it.
fn both(ws: &Path, mcp: &mut Mcp, analyzer: &str, args: &[&str], params: Value) -> Value {
    let mut argv = vec!["analyze", analyzer];
    argv.extend_from_slice(args);
    let cli = cli_json(ws, &argv);
    let result = mcp
        .call(&format!("analyze_{analyzer}"), params)
        .unwrap_or_else(|e| panic!("analyze_{analyzer}: {e}"));
    let text = result["content"][0]["text"].as_str().unwrap();
    let over_mcp: Value = serde_json::from_str(text).unwrap();
    assert_eq!(over_mcp, cli, "analyze {analyzer}: MCP and CLI differ");
    cli
}

#[test]
fn zig_test_blocks_are_excluded_by_default() {
    let ws = workspace(&[("a.zig", &zig_with_a_test())]);
    let mut mcp = Mcp::start(ws.path());
    for analyzer in ANALYZERS {
        let out = both(ws.path(), &mut mcp, analyzer, &[], json!({}));
        assert_eq!(
            out["excluded"]["tests"],
            json!([{"path": "a.zig", "range": "4:1-16:2"}]),
            "{analyzer}: {out}"
        );
        assert_eq!(out["complete"], true, "{analyzer}: {out}");
        match analyzer {
            "hotspots" => {
                assert_eq!(out["findings"][0]["functions"], 1, "{out}");
                assert_eq!(out["findings"][0]["top_functions"][0]["function"], "f");
            }
            _ => assert_eq!(out["findings"], json!([]), "{analyzer}: {out}"),
        }
    }
}

#[test]
fn zig_test_block_is_an_unnamed_function_with_include_tests() {
    let ws = workspace(&[("a.zig", &zig_with_a_test())]);
    let mut mcp = Mcp::start(ws.path());
    let out = both(
        ws.path(),
        &mut mcp,
        "patterns",
        &["--include-tests"],
        json!({"include_tests": true}),
    );
    assert_eq!(out["excluded"]["tests"], json!([]), "{out}");
    assert_eq!(
        out["findings"],
        json!([{"file": "a.zig", "range": "4:1-16:2", "rule": "excessive_branching",
                "function": null, "metric": "complexity", "value": 11, "threshold": 10,
                "metrics": {"lines": 13, "nesting": 1, "complexity": 11, "parameters": 0}}]),
        "{out}"
    );
}

#[test]
fn empty_zig_container_is_a_parse_failure() {
    let ws = workspace(&[("s.zig", "const S = struct {};\n")]);
    let mut mcp = Mcp::start(ws.path());
    for analyzer in ANALYZERS {
        let out = both(ws.path(), &mut mcp, analyzer, &[], json!({}));
        assert_eq!(
            out["incomplete"]["parse_failures"],
            json!([{"file": "s.zig", "range": "1:19-1:19"}]),
            "{analyzer}: {out}"
        );
        assert_eq!(out["complete"], false, "{analyzer}: {out}");
    }
}

#[test]
fn r_testthat_files_are_tests_and_package_sources_are_not() {
    let source = r_branchy();
    let ws = workspace(&[
        ("tests/testthat/test-scale.R", &source),
        ("R/test_x.R", &source),
    ]);
    let mut mcp = Mcp::start(ws.path());
    for analyzer in ANALYZERS {
        let out = both(ws.path(), &mut mcp, analyzer, &[], json!({}));
        assert_eq!(
            out["excluded"]["tests"],
            json!([{"path": "tests/testthat/test-scale.R"}]),
            "{analyzer}: {out}"
        );
        assert_eq!(out["coverage"]["analyzed_files"], 1, "{analyzer}: {out}");
        match analyzer {
            "patterns" => assert_eq!(
                out["findings"],
                json!([{"file": "R/test_x.R", "range": "1:6-13:2", "rule": "excessive_branching",
                        "function": "f", "metric": "complexity", "value": 11, "threshold": 10,
                        "metrics": {"lines": 13, "nesting": 1, "complexity": 11, "parameters": 1}}]),
                "{out}"
            ),
            "hotspots" => {
                assert_eq!(out["findings"][0]["file"], "R/test_x.R", "{out}");
                assert_eq!(out["findings"][0]["top_functions"][0]["function"], "f");
            }
            _ => assert_eq!(out["findings"], json!([]), "{out}"),
        }
    }
}

#[test]
fn julia_testset_in_a_source_file_is_not_a_test() {
    let ws = workspace(&[("src/a.jl", &julia_testset())]);
    let mut mcp = Mcp::start(ws.path());
    for analyzer in ANALYZERS {
        let out = both(ws.path(), &mut mcp, analyzer, &[], json!({}));
        assert_eq!(out["excluded"]["tests"], json!([]), "{analyzer}: {out}");
        assert_eq!(out["coverage"]["analyzed_files"], 1, "{analyzer}: {out}");
        match analyzer {
            "patterns" => assert_eq!(
                out["findings"],
                json!([{"file": "src/a.jl", "range": "3:5-15:8", "rule": "excessive_branching",
                        "function": "f", "metric": "complexity", "value": 11, "threshold": 10,
                        "metrics": {"lines": 13, "nesting": 1, "complexity": 11, "parameters": 1}}]),
                "{out}"
            ),
            "hotspots" => {
                assert_eq!(out["findings"][0]["file"], "src/a.jl", "{out}");
                assert_eq!(out["findings"][0]["top_functions"][0]["function"], "f");
            }
            _ => assert_eq!(out["findings"], json!([]), "{out}"),
        }
    }
}

/// The formats that no analyzer supports, each with its fixture.
const UNSUPPORTED_FORMATS: [(&str, &str); 7] = [
    ("JSONC", "jsonc/settings.jsonc"),
    ("XML", "xml/pom.xml"),
    ("GraphQL", "graphql/schema.graphql"),
    ("Protocol Buffers", "protobuf/shop.proto"),
    ("Make", "make/Makefile"),
    ("CMake", "cmake/CMakeLists.txt"),
    ("Nix", "nix/default.nix"),
];

fn with_fixture(rel: &str) -> tempfile::TempDir {
    let source = fixture(rel);
    let name = source.file_name().unwrap().to_str().unwrap().to_string();
    let content = std::fs::read_to_string(&source).unwrap();
    workspace(&[(&name, &content)])
}

#[test]
fn a_format_alone_is_unsupported_for_every_analyzer() {
    for (language, rel) in UNSUPPORTED_FORMATS {
        let ws = with_fixture(rel);
        let mut mcp = Mcp::start(ws.path());
        for analyzer in ANALYZERS {
            let out = both(ws.path(), &mut mcp, analyzer, &[], json!({}));
            assert_eq!(out["status"], "unsupported", "{language} {analyzer}: {out}");
            assert_eq!(
                out["coverage"]["languages"],
                json!([{"language": language, "files": 1, "supported": false}]),
                "{language} {analyzer}: {out}"
            );
            assert_eq!(out["findings"], json!([]), "{language} {analyzer}: {out}");
        }
    }
}

#[test]
fn a_format_beside_a_zig_file_is_listed_unsupported_in_a_mixed_scope() {
    for (language, rel) in UNSUPPORTED_FORMATS {
        let source = fixture(rel);
        let name = source.file_name().unwrap().to_str().unwrap().to_string();
        let content = std::fs::read_to_string(&source).unwrap();
        let ws = workspace(&[(&name, &content), ("ok.zig", "const x: u8 = 1;\n")]);
        let mut mcp = Mcp::start(ws.path());
        for analyzer in ANALYZERS {
            let out = both(ws.path(), &mut mcp, analyzer, &[], json!({}));
            assert_eq!(out["status"], "no_findings", "{language} {analyzer}: {out}");
            assert_eq!(
                out["coverage"]["languages"],
                json!([
                    {"language": language, "files": 1, "supported": false},
                    {"language": "Zig", "files": 1, "supported": true}
                ]),
                "{language} {analyzer}: {out}"
            );
        }
    }
}
