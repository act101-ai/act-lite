//! `analyze_hotspots`: files ranked by the sum of complexity × lines over
//! their functions, each with its top functions and their metrics. Every case
//! runs the built binary.

use assert_cmd::Command;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Runs `act-lite --workspace <root> analyze hotspots <args>` with an empty
/// configuration directory: the parsed stdout, or the message of the error
/// frame on stderr.
fn binary(root: &Path, args: &[&str]) -> Result<Value, String> {
    let config = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("act-lite")
        .expect("the act-lite binary must be built")
        .env("ACT_CONFIG_DIR", config.path())
        .env_remove("ACT_LITE_DISABLE_LSP")
        .arg("--workspace")
        .arg(root)
        .args(["analyze", "hotspots"])
        .args(args)
        .output()
        .unwrap();
    if out.status.success() {
        return Ok(serde_json::from_slice(&out.stdout).expect("stdout is one JSON value"));
    }
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let frame: Value = serde_json::from_slice(&out.stderr).expect("stderr is one JSON value");
    Err(frame["error"]["message"].as_str().unwrap().to_string())
}

/// Runs the analyzer on `workspace` and returns the envelope as JSON, or the
/// request error's message.
fn hotspots(workspace: &Path, limit: u32) -> Result<Value, String> {
    binary(workspace, &["--limit", &limit.to_string()])
}

/// A temporary workspace holding `files` (path, content).
fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temporary directory");
    for (path, content) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    dir
}

/// The `top_functions` of the only finding of a one-file workspace.
fn top_functions(path: &str, content: &str) -> Vec<Value> {
    let ws = workspace(&[(path, content)]);
    let result = hotspots(ws.path(), 1000).unwrap();
    let findings = result["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{result:#}");
    findings[0]["top_functions"].as_array().unwrap().clone()
}

/// The top function named `name`.
fn named<'a>(functions: &'a [Value], name: &str) -> &'a Value {
    functions
        .iter()
        .find(|f| f["function"] == name)
        .unwrap_or_else(|| panic!("no function {name} in {functions:#?}"))
}

const A_RS: &str = "fn f(x: u8) -> u8 {
    if x > 1 {
        return 1;
    }
    if x > 2 {
        return 2;
    }
    0
    + 0
}
fn g() {
    let _ = 1;
}
";

/// One function `h` of 20 code lines and complexity 5.
fn b_rs() -> String {
    let mut source = String::from("fn h(x: u8) -> u8 {\n");
    for i in 1..=4 {
        source.push_str(&format!("    if x > {i} {{ return {i}; }}\n"));
    }
    for i in 0..13 {
        source.push_str(&format!("    let _v{i} = {i};\n"));
    }
    source.push_str("    0\n}\n");
    source
}

fn h1() -> Value {
    let b = b_rs();
    let ws = workspace(&[("src/a.rs", A_RS), ("src/b.rs", &b)]);
    hotspots(ws.path(), 100).unwrap()
}

#[test]
fn ranking_by_score_with_top_functions() {
    let result = h1();
    assert_eq!(result["status"], "findings");
    assert_eq!(
        result["findings"],
        json!([
            {"file": "src/b.rs", "score": 100, "functions": 1, "complexity": 5, "lines": 20,
             "top_functions": [
                {"function": "h", "range": "1:1-20:2", "score": 100, "complexity": 5,
                 "lines": 20, "nesting": 1, "parameters": 1}
             ],
             "top_functions_total": 1},
            {"file": "src/a.rs", "score": 33, "functions": 2, "complexity": 4, "lines": 13,
             "top_functions": [
                {"function": "f", "range": "1:1-10:2", "score": 30, "complexity": 3,
                 "lines": 10, "nesting": 1, "parameters": 1},
                {"function": "g", "range": "11:1-13:2", "score": 3, "complexity": 1,
                 "lines": 3, "nesting": 0, "parameters": 0}
             ],
             "top_functions_total": 2}
        ])
    );
    for finding in result["findings"].as_array().unwrap() {
        assert!(finding.get("range").is_none(), "{finding}");
    }
}

#[test]
fn details_explain_the_formula() {
    let result = h1();
    assert_eq!(
        result["details"],
        json!({
            "formula": "score(file) = sum over functions f in file of complexity(f) * lines(f)",
            "complexity": "cyclomatic: 1 + decision points in the function's own body",
            "lines": "lines with a non-comment token of the function's own body, nested functions and Python docstrings excluded",
            "dependency_information": false,
            "functions": 3,
            "regions_not_analyzed": [],
            "regions_not_analyzed_total": 0,
            "files_without_analyzed_code": 0
        })
    );
}

/// Collects every key and every string value outside `file`, `range` and
/// `function` values.
fn written_text(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                out.push(key.clone());
                if !matches!(key.as_str(), "file" | "range" | "function") {
                    written_text(value, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| written_text(item, out)),
        Value::String(text) => out.push(text.clone()),
        _ => {}
    }
}

#[test]
fn no_result_claims_safety() {
    let mut text = Vec::new();
    written_text(&h1(), &mut text);
    for word in ["safe", "extract", "refactor"] {
        assert!(
            !text.iter().any(|t| t.to_lowercase().contains(word)),
            "{word} in {text:?}"
        );
    }
}

#[test]
fn equal_scores_are_in_path_byte_order() {
    let ws = workspace(&[("a/b.rs", "fn f() {}\n"), ("a-b/x.rs", "fn f() {}\n")]);
    let result = hotspots(ws.path(), 100).unwrap();
    let files: Vec<&str> = result["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["a-b/x.rs", "a/b.rs"]);
}

#[test]
fn a_file_without_functions_is_no_item() {
    let ws = workspace(&[("m.py", "X = 1\n")]);
    let result = hotspots(ws.path(), 100).unwrap();
    assert_eq!(result["status"], "no_findings");
    assert_eq!(result["coverage"]["analyzed_files"], 1);
    assert_eq!(result["details"]["functions"], 0);
}

#[test]
fn top_functions_are_capped_at_five() {
    let source: String = (0..7).map(|i| format!("fn f{i}() {{}}\n")).collect();
    let ws = workspace(&[("src/s.rs", &source)]);
    let result = hotspots(ws.path(), 100).unwrap();
    let finding = &result["findings"][0];
    assert_eq!(finding["functions"], 7);
    assert_eq!(finding["top_functions_total"], 7);
    let names: Vec<&str> = finding["top_functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["function"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["f0", "f1", "f2", "f3", "f4"]);
}

#[test]
fn else_if_chain_values() {
    let fs = top_functions(
        "src/g.ts",
        "function g(a, b, c, d, e, f) {\n    if (a) {} else if (b) {} else if (c) {} else if (d) {} else if (e) {} else if (f) {}\n}\n",
    );
    let g = named(&fs, "g");
    assert_eq!(
        (g["nesting"].clone(), g["complexity"].clone()),
        (json!(1), json!(7))
    );
}

#[test]
fn nested_function_values_are_its_own() {
    let fs = top_functions(
        "src/o.js",
        "function outer() { const f = () => { if (a) { if (b) { if (c) { if (d) { if (e) {} } } } } }; return f; }",
    );
    let outer = named(&fs, "outer");
    assert_eq!(
        (outer["nesting"].clone(), outer["complexity"].clone()),
        (json!(0), json!(1))
    );
    let arrow = fs.iter().find(|f| f["function"].is_null()).unwrap();
    assert_eq!(arrow["nesting"], 5);
}

#[test]
fn python_complexity_values() {
    let fs = top_functions(
        "src/c.py",
        r#"def classify(n, items):
    if n < 0 and n > -10:
        return "neg"
    elif n == 0:
        return "zero"
    for x in items:
        while x > 0:
            x -= 1
    try:
        pass
    except ValueError:
        pass
    return "a" if n else "b"
"#,
    );
    let c = named(&fs, "classify");
    assert_eq!(
        (
            c["complexity"].clone(),
            c["nesting"].clone(),
            c["parameters"].clone(),
            c["lines"].clone()
        ),
        (json!(8), json!(2), json!(2), json!(13))
    );
}

#[test]
fn parameter_values_by_family() {
    let go = top_functions(
        "src/p.go",
        "package p\nfunc (r *R) f(a, b, c int, d string, e ...int) {}\nfunc g(a, b, c, d, e, f int) {}\n",
    );
    assert_eq!(named(&go, "f")["parameters"], 5);
    assert_eq!(named(&go, "g")["parameters"], 6);
    let py = top_functions(
        "src/p.py",
        "class C:\n    def m(self, a, b, c, d, e):\n        pass\n\ndef h(self, a, b, c, d, e):\n    pass\n\ndef k(a, /, b, *, c, **kw):\n    pass\n",
    );
    assert_eq!(named(&py, "m")["parameters"], 5);
    assert_eq!(named(&py, "h")["parameters"], 6);
    assert_eq!(named(&py, "k")["parameters"], 4);
    let ts = top_functions("src/t.ts", "function t(this: X, a, b, c, d, e) {}\n");
    assert_eq!(named(&ts, "t")["parameters"], 5);
}

#[test]
fn language_specific_forms() {
    let dart = top_functions("src/top.dart", "int get top {\n  return 1;\n}\n");
    assert_eq!(named(&dart, "top")["range"], "1:1-3:2");
    let dart = top_functions(
        "src/top.dart",
        "int get top /* note */ {\n  if (a) return 1;\n  return 2;\n}\n",
    );
    assert_eq!(named(&dart, "top")["range"], "1:1-4:2");
    assert_eq!(named(&dart, "top")["complexity"], 2);

    let swift = top_functions(
        "src/s.swift",
        "struct S {\n    var v: Int {\n        return 1\n    }\n}\n",
    );
    assert_eq!(swift.len(), 1);
    assert!(swift[0]["function"].is_null());
    assert_eq!(swift[0]["range"], "2:16-4:6");

    let cs = top_functions(
        "src/c.cs",
        "class C {\n    int P => 1;\n    int this[int i] => i;\n    int Q { get; set; }\n}\n",
    );
    assert_eq!(cs.len(), 2);
    assert_eq!(named(&cs, "P")["range"], "2:5-2:16");
    assert_eq!(named(&cs, "this")["parameters"], 1);

    let py = top_functions(
        "src/s.py",
        "class C:\n    @staticmethod\n    def s(self, a):\n        pass\n\n    @staticmethod\n    def t(x, a):\n        pass\n",
    );
    assert_eq!(named(&py, "s")["parameters"], 1);
    assert_eq!(named(&py, "s")["range"], "3:5-4:13");
    assert_eq!(named(&py, "t")["parameters"], 2);

    let ruby = top_functions("src/l.rb", "l = ->(x) { x + 1 }\n");
    assert_eq!(ruby.len(), 1);
    assert!(ruby[0]["function"].is_null());
    assert_eq!(ruby[0]["range"], "1:5-1:20");

    let ex = top_functions("src/m.ex", "defmodule M do\n  def f(x), do: x\nend\n");
    assert_eq!(named(&ex, "f")["range"], "2:3-2:18");
    assert_eq!(named(&ex, "f")["parameters"], 1);

    let rs = top_functions("src/g.rs", "fn foo<T: Clone>(x: T) -> T {\n    x\n}\n");
    assert_eq!(named(&rs, "foo")["range"], "1:1-3:2");

    let go = top_functions(
        "src/m.go",
        "package p\n\nfunc Map[T any](x T) T {\n\treturn x\n}\n",
    );
    assert_eq!(named(&go, "Map")["range"], "3:1-5:2");

    let java = top_functions(
        "src/A.java",
        "class A {\n    int x;\n    {\n        x = 1;\n    }\n}\n",
    );
    assert_eq!(java.len(), 1);
    assert!(java[0]["function"].is_null());
    assert_eq!(java[0]["range"], "3:5-5:6");
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../act-lite-parser/tests/fixtures/function_model")
}

#[test]
fn every_language_over_fixture_through_top_functions() {
    let mut languages = 0;
    for dir in fs::read_dir(fixtures()).unwrap() {
        let dir = dir.unwrap().path();
        let source = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                let name = p.file_name().unwrap().to_str().unwrap();
                name.starts_with("over.") && !name.ends_with(".expected.json")
            })
            .unwrap();
        let name = format!("src/{}", source.file_name().unwrap().to_str().unwrap());
        let content = fs::read_to_string(&source).unwrap();
        let expected: Vec<Value> =
            serde_json::from_str(&fs::read_to_string(dir.join("over.expected.json")).unwrap())
                .unwrap();
        let found = top_functions(&name, &content);
        assert_eq!(found.len(), expected.len(), "{}: {found:#?}", dir.display());
        for want in &expected {
            let got = found
                .iter()
                .find(|f| f["range"] == want["range"])
                .unwrap_or_else(|| panic!("{}: no function at {}", dir.display(), want["range"]));
            for key in ["lines", "nesting", "complexity", "parameters"] {
                assert_eq!(got[key], want[key], "{}: {key} of {want}", dir.display());
            }
            assert_eq!(got["function"], want["name"], "{}", dir.display());
            let score = want["complexity"].as_u64().unwrap() * want["lines"].as_u64().unwrap();
            assert_eq!(got["score"], score, "{}", dir.display());
        }
        if dir.ends_with("bash") {
            assert!(found.iter().all(|f| f["parameters"].is_null()));
        }
        languages += 1;
    }
    assert_eq!(languages, 23);
}
