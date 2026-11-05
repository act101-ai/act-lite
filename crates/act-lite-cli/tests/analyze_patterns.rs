//! The `patterns` analyzer on synthetic workspaces, run through the built
//! binary. Every expected value is
//! derived by hand from the fixture source and the fixed thresholds
//! (lines 50, nesting 4, complexity 10, parameters 5, each exceeded when
//! strictly greater).

use assert_cmd::Command;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// Runs `act-lite --workspace <root> analyze patterns <args>` with an empty
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
        .args(["analyze", "patterns"])
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

/// Runs `patterns` on the whole workspace at `root`: the envelope as JSON, or
/// the error message.
fn analyze(root: &Path) -> Result<Value, String> {
    binary(root, &["--limit", "1000"])
}

fn write(root: &Path, rel: &str, content: &[u8]) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        write(dir.path(), rel, content.as_bytes());
    }
    dir
}

fn findings(out: &Value) -> &Vec<Value> {
    out["findings"].as_array().unwrap()
}

fn thresholds() -> Value {
    json!({"oversized_function":50,"deep_nesting":4,"excessive_branching":10,"long_parameter_list":5})
}

/// A function with `lets` statement lines, after 6 comment lines and 4 blank
/// lines: `lets` + 2 code lines over `lets` + 12 source lines.
fn long_function(signature: &str, lets: usize) -> String {
    let mut out = format!("{signature} {{\n");
    out.push_str(&"    // note\n".repeat(6));
    out.push_str(&"\n".repeat(4));
    for i in 0..lets {
        out.push_str(&format!("    let v{i} = {i};\n"));
    }
    out.push_str("}\n");
    out
}

/// `src/t.rs`: one function at and one over each threshold.
///
/// Lines: `lines_at` 1–60, `lines_over` 62–122, `nest_at` 124–134,
/// `nest_over` 136–148, `cc_at` 150–160, `cc_over` 162–173, `p_at` 177,
/// `p_over` 180.
fn thresholds_rs() -> String {
    let mut src = long_function("fn lines_at()", 48);
    src.push('\n');
    src.push_str(&long_function("fn lines_over()", 49));
    src.push('\n');
    src.push_str(
        "fn nest_at(x: u8) {
    if x > 0 {
        for _i in 0..x {
            while x > 1 {
                match x {
                    _ => {}
                }
            }
        }
    }
}

fn nest_over(x: u8) {
    if x > 0 {
        for _i in 0..x {
            while x > 1 {
                match x {
                    _ => loop {
                        break;
                    },
                }
            }
        }
    }
}

",
    );
    src.push_str("fn cc_at(x: u8) {\n");
    src.push_str(&"    if x == 1 {}\n".repeat(9));
    src.push_str("}\n\nfn cc_over(x: u8) {\n");
    src.push_str(&"    if x == 1 {}\n".repeat(10));
    src.push_str(
        "}

struct S;
impl S {
    fn p_at(&self, a: u8, b: u8, c: u8, d: u8, e: u8) {}
}

fn p_over(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) {}
",
    );
    src
}

fn a1() -> Value {
    let dir = workspace(&[("src/t.rs", &thresholds_rs())]);
    analyze(dir.path()).unwrap()
}

#[test]
fn a1_one_finding_per_threshold_exceeded() {
    let out = a1();
    assert_eq!(out["analysis"], "patterns");
    assert_eq!(out["status"], "findings");
    assert_eq!(out["total"], 4);
    assert_eq!(
        out["findings"],
        json!([
            {"file":"src/t.rs","range":"62:1-122:2","rule":"oversized_function","function":"lines_over",
             "metric":"lines","value":51,"threshold":50,
             "metrics":{"lines":51,"nesting":0,"complexity":1,"parameters":0}},
            {"file":"src/t.rs","range":"136:1-148:2","rule":"deep_nesting","function":"nest_over",
             "metric":"nesting","value":5,"threshold":4,
             "metrics":{"lines":13,"nesting":5,"complexity":5,"parameters":1}},
            {"file":"src/t.rs","range":"162:1-173:2","rule":"excessive_branching","function":"cc_over",
             "metric":"complexity","value":11,"threshold":10,
             "metrics":{"lines":12,"nesting":1,"complexity":11,"parameters":1}},
            {"file":"src/t.rs","range":"180:1-180:55","rule":"long_parameter_list","function":"p_over",
             "metric":"parameters","value":6,"threshold":5,
             "metrics":{"lines":1,"nesting":0,"complexity":1,"parameters":6}}
        ])
    );
}

#[test]
fn a8_details_of_a1() {
    let out = a1();
    assert_eq!(
        out["details"],
        json!({
            "thresholds": thresholds(),
            "counts_by_rule":{"deep_nesting":1,"excessive_branching":1,"long_parameter_list":1,"oversized_function":1},
            "functions":8,
            "not_applicable":[],"not_applicable_total":0
        })
    );
}

/// Every key, and every string value except those of `file`, `range` and
/// `function`.
fn written_strings(value: &Value, key: Option<&str>, out: &mut Vec<String>) {
    match value {
        Value::String(s) if !matches!(key, Some("file" | "range" | "function")) => {
            out.push(s.clone())
        }
        Value::Array(items) => items.iter().for_each(|v| written_strings(v, key, out)),
        Value::Object(map) => {
            for (k, v) in map {
                out.push(k.clone());
                written_strings(v, Some(k), out);
            }
        }
        _ => {}
    }
}

#[test]
fn e6_no_result_string_claims_safety() {
    let mut strings = Vec::new();
    written_strings(&a1(), None, &mut strings);
    for s in strings {
        for word in ["safe", "extract", "refactor"] {
            assert!(!s.to_lowercase().contains(word), "{s:?} contains {word:?}");
        }
    }
}

#[test]
fn a6_two_rules_on_one_function_sort_by_rule() {
    let src = long_function("fn both(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8)", 49);
    let dir = workspace(&[("src/both.rs", &src)]);
    let out = analyze(dir.path()).unwrap();
    let metrics = json!({"lines":51,"nesting":0,"complexity":1,"parameters":6});
    assert_eq!(
        out["findings"],
        json!([
            {"file":"src/both.rs","range":"1:1-61:2","rule":"long_parameter_list","function":"both",
             "metric":"parameters","value":6,"threshold":5,"metrics":metrics},
            {"file":"src/both.rs","range":"1:1-61:2","rule":"oversized_function","function":"both",
             "metric":"lines","value":51,"threshold":50,"metrics":metrics}
        ])
    );
}

const FIXTURE_LANGUAGES: [(&str, &str); 21] = [
    ("bash", "sh"),
    ("c", "c"),
    ("cpp", "cpp"),
    ("csharp", "cs"),
    ("dart", "dart"),
    ("elixir", "ex"),
    ("go", "go"),
    ("java", "java"),
    ("javascript", "js"),
    ("jsx", "jsx"),
    ("kotlin", "kt"),
    ("lua", "lua"),
    ("php", "php"),
    ("python", "py"),
    ("ruby", "rb"),
    ("rust", "rs"),
    ("scala", "scala"),
    ("swift", "swift"),
    ("tsx", "tsx"),
    ("typescript", "ts"),
    ("zig", "zig"),
];

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../act-lite-parser/tests/fixtures/function_model")
}

/// The findings the functions of an `.expected.json` give in `file`, in
/// location order and then rule order.
fn expected_findings(file: &str, functions: &Value) -> Vec<Value> {
    // In byte order of the rule name.
    let rules = [
        ("deep_nesting", "nesting", 4),
        ("excessive_branching", "complexity", 10),
        ("long_parameter_list", "parameters", 5),
        ("oversized_function", "lines", 50),
    ];
    let mut out = Vec::new();
    for f in functions.as_array().unwrap() {
        let metrics = json!({"lines":f["lines"],"nesting":f["nesting"],
                             "complexity":f["complexity"],"parameters":f["parameters"]});
        for (rule, metric, threshold) in rules {
            if f[metric].as_u64().is_some_and(|v| v > threshold) {
                out.push(
                    json!({"file":file,"range":f["range"],"rule":rule,"function":f["name"],
                                "metric":metric,"value":f[metric],"threshold":threshold,
                                "metrics":metrics}),
                );
            }
        }
    }
    out
}

#[test]
fn a7_every_language_over_and_under() {
    let mut failures = Vec::new();
    for (lang, ext) in FIXTURE_LANGUAGES {
        let read = |stem: &str| {
            fs::read_to_string(fixture_dir().join(lang).join(format!("{stem}.{ext}"))).unwrap()
        };
        let expected = |stem: &str| -> Value {
            let path = fixture_dir()
                .join(lang)
                .join(format!("{stem}.expected.json"));
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
        };
        let over_rel = format!("src/over.{ext}");
        let under_rel = format!("src/under.{ext}");
        let dir = workspace(&[(&over_rel, &read("over")), (&under_rel, &read("under"))]);
        let out = analyze(dir.path()).unwrap();

        let want = expected_findings(&over_rel, &expected("over"));
        let applicable = if lang == "bash" { 3 } else { 4 };
        assert_eq!(
            want.len(),
            applicable,
            "{lang}: the over fixture's main function"
        );
        assert!(
            expected_findings(&under_rel, &expected("under")).is_empty(),
            "{lang}: the under fixture"
        );
        if findings(&out) != &want {
            failures.push(format!(
                "{lang}:\n expected {}\n actual   {}",
                Value::Array(want),
                out["findings"]
            ));
        }
        let not_applicable = if lang == "bash" {
            json!([{"language":"Bash","rules":["long_parameter_list"],"rules_total":1}])
        } else {
            json!([])
        };
        assert_eq!(out["details"]["not_applicable"], not_applicable, "{lang}");
        if lang == "bash" {
            assert!(
                findings(&out)
                    .iter()
                    .all(|f| f["metrics"]["parameters"].is_null()),
                "{}",
                out["findings"]
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The findings of a workspace holding only `rel` with `source`.
fn findings_of(rel: &str, source: &str) -> Vec<Value> {
    let dir = workspace(&[(rel, source)]);
    findings(&analyze(dir.path()).unwrap()).clone()
}

#[test]
fn a2_else_if_chain_gives_no_finding() {
    let src = "function chain() {\n    if (a) {} else if (b) {} else if (c) {} else if (d) {} else if (e) {} else if (f) {}\n}\n";
    assert_eq!(findings_of("src/chain.ts", src), Vec::<Value>::new());
}

#[test]
fn a3_nested_function_is_measured_alone() {
    let src = "function outer() { const f = () => { if (a) { if (b) { if (c) { if (d) { if (e) {} } } } } }; return f; }\n";
    let found = findings_of("src/outer.js", src);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["rule"], "deep_nesting");
    assert_eq!(found[0]["function"], Value::Null);
    assert_eq!(found[0]["value"], 5);
    assert_eq!(found[0]["range"], "1:30-1:93");
}

#[test]
fn a4_python_complexity_gives_no_finding() {
    let src = r#"def classify(n, items):
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
"#;
    assert_eq!(findings_of("src/classify.py", src), Vec::<Value>::new());
}

#[test]
fn dart_accessor_with_a_comment_before_its_body_is_measured() {
    let src = format!(
        "int get top /* note */ {{\n{}  return 0;\n}}\n",
        "  if (a) return 1;\n".repeat(10)
    );
    assert_eq!(
        reported(&findings_of("src/top.dart", &src)),
        [(json!("top"), json!("excessive_branching"), json!(11))]
    );
}

/// The `(function, rule, value)` of each finding.
fn reported(found: &[Value]) -> Vec<(Value, Value, Value)> {
    found
        .iter()
        .map(|f| (f["function"].clone(), f["rule"].clone(), f["value"].clone()))
        .collect()
}

#[test]
fn a5_parameters_by_family() {
    let go = "package p\n\ntype R struct{}\n\nfunc (r *R) f(a, b, c int, d string, e ...int) {}\n\nfunc g(a, b, c, d, e, f int) {}\n";
    assert_eq!(
        reported(&findings_of("src/p.go", go)),
        [(json!("g"), json!("long_parameter_list"), json!(6))]
    );
    let py = "class C:\n    def m(self, a, b, c, d, e):\n        pass\n\n\ndef h(self, a, b, c, d, e):\n    pass\n\n\ndef k(a, /, b, *, c, **kw):\n    pass\n";
    assert_eq!(
        reported(&findings_of("src/p.py", py)),
        [(json!("h"), json!("long_parameter_list"), json!(6))]
    );
    let ts = "function t(this: X, a, b, c, d, e) {}\n";
    assert_eq!(findings_of("src/p.ts", ts), Vec::<Value>::new());
}

#[test]
fn elixir_guarded_anonymous_function_counts_all_its_parameters() {
    let src = "f = fn a, b, c, d, e, f when a > 0 -> a end\n";
    let rules: Vec<_> = reported(&findings_of("lib/g.ex", src))
        .into_iter()
        .map(|(_, rule, value)| (rule, value))
        .collect();
    assert_eq!(rules, [(json!("long_parameter_list"), json!(6))]);
}

#[test]
fn no_function_gives_details_with_zero_counts() {
    let dir = workspace(&[("src/lib.rs", "const X: u8 = 1;\n")]);
    let out = analyze(dir.path()).unwrap();
    assert_eq!(out["status"], "no_findings");
    assert_eq!(
        out["details"],
        json!({
            "thresholds": thresholds(),
            "counts_by_rule":{"deep_nesting":0,"excessive_branching":0,"long_parameter_list":0,"oversized_function":0},
            "functions":0,
            "not_applicable":[],"not_applicable_total":0
        })
    );
}

#[test]
fn skipped_bash_file_is_listed_as_not_applicable() {
    let dir = workspace(&[("src/ok.rs", "fn ok() {\n    let x = 1;\n}")]);
    let mut big = b"#!/bin/bash\n".to_vec();
    big.resize(8 * 1024 * 1024, b'#');
    big.push(b'\n');
    write(dir.path(), "scripts/big.sh", &big);
    let out = analyze(dir.path()).unwrap();
    assert_eq!(
        out["incomplete"]["skipped_files"][0]["file"],
        "scripts/big.sh"
    );
    assert_eq!(out["incomplete"]["skipped_files"][0]["kind"], "size_limit");
    assert_eq!(out["details"]["functions"], 1);
    assert_eq!(
        out["details"]["not_applicable"],
        json!([{"language":"Bash","rules":["long_parameter_list"],"rules_total":1}])
    );
    assert_eq!(out["details"]["not_applicable_total"], 1);
}

#[test]
fn file_with_a_parse_error_is_measured() {
    let broken = "fn b(p1: u8, p2: u8, p3: u8,\n     p4: u8, p5: u8, p6: u8) -> u8 {\n    let a = p1;\n    let c = ;\n    a\n}\n";
    let dir = workspace(&[("src/broken.rs", broken)]);
    let out = analyze(dir.path()).unwrap();
    assert_eq!(out["status"], "findings");
    assert_eq!(
        out["findings"],
        json!([{"file":"src/broken.rs","range":"1:1-6:2","rule":"long_parameter_list","function":"b",
                "metric":"parameters","value":6,"threshold":5,
                "metrics":{"lines":6,"nesting":0,"complexity":1,"parameters":6}}])
    );
    assert_eq!(
        out["incomplete"]["parse_failures"][0]["file"],
        "src/broken.rs"
    );
}
