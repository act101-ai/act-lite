//! Acceptance tests of the `clones` analyzer, run through the built binary.
//! Every workspace is synthetic and built in a temporary directory; expected
//! values come from the fixture sources and the documented normalization
//! rule, not from the analyzer's output.

use assert_cmd::Command;
use regex::Regex;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// Runs `act-lite --workspace <root> analyze clones` with an empty
/// configuration directory and returns the parsed envelope, or the message of
/// the error frame. Every result is checked for words that claim safety (no
/// key or string value other than `file`, `range` and `function` values holds
/// `safe`, `extract` or `refactor`).
fn clones(
    root: &Path,
    paths: &[&str],
    focus: &[&str],
    include_tests: bool,
) -> Result<Value, String> {
    let config = tempfile::tempdir().unwrap();
    let mut command = Command::cargo_bin("act-lite").expect("the act-lite binary must be built");
    command
        .env("ACT_CONFIG_DIR", config.path())
        .env_remove("ACT_LITE_DISABLE_LSP")
        .arg("--workspace")
        .arg(root)
        .args(["analyze", "clones", "--limit", "100"])
        .args(paths);
    for path in focus {
        command.args(["--focus", path]);
    }
    if include_tests {
        command.arg("--include-tests");
    }
    let out = command.output().unwrap();
    let result = if out.status.success() {
        Ok(serde_json::from_slice(&out.stdout).expect("stdout is one JSON value"))
    } else {
        assert!(
            out.stdout.is_empty(),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        let frame: Value = serde_json::from_slice(&out.stderr).expect("stderr is one JSON value");
        Err(frame["error"]["message"].as_str().unwrap().to_string())
    };
    if let Ok(value) = &result {
        assert_no_claim(value);
    }
    result
}

fn analyze(root: &Path) -> Value {
    clones(root, &[], &[], false).unwrap()
}

fn assert_no_claim(value: &Value) {
    const WORDS: [&str; 3] = ["safe", "extract", "refactor"];
    let check = |text: &str| {
        let lower = text.to_lowercase();
        assert!(
            !WORDS.iter().any(|w| lower.contains(w)),
            "result claims safety: {text}"
        );
    };
    match value {
        Value::Object(map) => {
            for (key, item) in map {
                check(key);
                if !matches!(key.as_str(), "file" | "range" | "function") {
                    assert_no_claim(item);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(assert_no_claim),
        Value::String(text) => check(text),
        _ => {}
    }
}

/// The base function: 77 tokens by the normalization rule (the two strings
/// and the four numbers count 1 each).
const BASE: &str = r#"def summarize(orders, rate):
    total = 0
    count = 0
    for order in orders:
        if order.status == "paid" and order.amount > 0:
            total = total + order.amount * rate
            count = count + 1
        elif order.status == "refunded":
            total = total - order.amount
    if count == 0:
        return None
    return {"total": total, "count": count}
"#;

const BASE_TOKENS: u64 = 77;

/// The base with comments, blank lines and the dictionary over two lines.
fn reformatted() -> String {
    BASE.replace("    total = 0\n", "    # running sum\n    total = 0\n\n")
        .replace(
            "        elif",
            "        # refunds reduce the sum\n        elif",
        )
        .replace(
            r#"{"total": total, "count": count}"#,
            "{\"total\": total,\n            \"count\": count}",
        )
}

/// The base with every name renamed and three literals changed.
fn renamed() -> String {
    BASE.replace("summarize", "tally")
        .replace("orders", "rows")
        .replace("rate", "factor")
        .replace("count = 0", "n = 1")
        .replace("total", "acc")
        .replace("count", "n")
        .replace("order", "row")
        .replace("paid", "settled")
        .replace("refunded", "returned")
}

/// The range of a node that starts at the start of line `start` and ends on
/// line `end` at column `end_col` (all 1-based).
fn range(start: u32, end: u32, end_col: u32) -> String {
    format!("{start}:1-{end}:{end_col}")
}

/// The range of the base function at the top of a file.
fn base_range() -> String {
    range(1, 12, 44)
}

fn c2(root: &Path) {
    write(root, "a.py", BASE);
    write(root, "b.py", &reformatted());
    write(root, "c.py", &renamed());
}

#[test]
fn c1_exact_ignores_formatting_and_comments() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", BASE);
    write(dir.path(), "b.py", &reformatted());
    let out = analyze(dir.path());
    assert_eq!(out["status"], "findings");
    assert_eq!(
        out["findings"],
        json!([{"file": "a.py", "range": base_range(), "kind": "exact", "language": "Python",
                "tokens": BASE_TOKENS,
                "members": [{"file": "a.py", "range": base_range(), "variant": 1},
                            {"file": "b.py", "range": range(1, 16, 28), "variant": 1}],
                "members_total": 2, "variants": 1}])
    );
}

#[test]
fn c2_renamed_consistently_is_one_outermost_class() {
    let dir = tempfile::tempdir().unwrap();
    c2(dir.path());
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1);
    let finding = &out["findings"][0];
    assert_eq!(finding["kind"], "renamed");
    assert_eq!(finding["tokens"], BASE_TOKENS);
    assert_eq!(
        finding["members"],
        json!([{"file": "a.py", "range": base_range(), "variant": 1},
               {"file": "b.py", "range": range(1, 16, 28), "variant": 1},
               {"file": "c.py", "range": range(1, 12, 32), "variant": 2}])
    );
    assert_eq!(finding["members_total"], 3);
    assert_eq!(finding["variants"], 2);
}

#[test]
fn c3_inserted_statement_is_not_a_clone() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", BASE);
    let inserted = BASE.replace(
        "    for order in orders:\n",
        "    for order in orders:\n        log(order)\n",
    );
    write(dir.path(), "d.py", &inserted);
    let out = analyze(dir.path());
    assert_eq!(out["status"], "no_findings");
}

#[test]
fn c4_inconsistent_renaming_is_not_a_clone() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", BASE);
    let changed = BASE.replace(
        "total = total + order.amount * rate",
        "total = count + order.amount * rate",
    );
    assert_ne!(changed, BASE);
    write(dir.path(), "e.py", &changed);
    let out = analyze(dir.path());
    assert_eq!(out["status"], "no_findings");
}

/// Five statements of 13 tokens each.
const RUN: &str = "    x1 = a * 2 + b * 3 - a / 4
    x2 = a * 5 + b * 6 - a / 7
    x3 = a * 8 + b * 9 - a / 10
    x4 = a * 11 + b * 12 - a / 13
    x5 = a * 14 + b * 15 - a / 16
";

#[test]
fn c5_statement_run_is_not_a_clone() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "p.py",
        &format!("def p(a, b):\n{RUN}    return x1\n"),
    );
    write(
        dir.path(),
        "q.py",
        &format!("def q(a, b):\n    y = 0\n{RUN}    return y\n"),
    );
    let out = analyze(dir.path());
    assert_eq!(out["status"], "no_findings");
}

/// A function of 8 + 39 + `tail` tokens.
fn sized(tail: &str) -> String {
    format!(
        "def f(a, b):\n    x = a * 2 + b * 3 - a / 4\n    x = x * 5 + b * 6 - a / 7\n    x = x * 8 + b * 9 - a / 10\n    {tail}\n"
    )
}

#[test]
fn c6_minimum_size_is_50_tokens() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", &sized("return x"));
    write(dir.path(), "b.py", &sized("return x"));
    assert_eq!(analyze(dir.path())["status"], "no_findings");

    write(dir.path(), "a.py", &sized("return -x"));
    write(dir.path(), "b.py", &sized("return -x"));
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1);
    assert_eq!(out["findings"][0]["kind"], "exact");
    assert_eq!(out["findings"][0]["tokens"], 50);
}

#[test]
fn c7_clone_inside_a_larger_function() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", BASE);
    write(dir.path(), "b.py", &reformatted());
    let inner: String = BASE.lines().map(|l| format!("    {l}\n")).collect();
    write(
        dir.path(),
        "f.py",
        &format!(
            "def wrapper(orders):\n    print(orders)\n{inner}    return summarize(orders, 2)\n"
        ),
    );
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1);
    let finding = &out["findings"][0];
    assert_eq!(finding["kind"], "exact");
    assert_eq!(
        finding["members"],
        json!([{"file": "a.py", "range": base_range(), "variant": 1},
               {"file": "b.py", "range": range(1, 16, 28), "variant": 1},
               {"file": "f.py", "range": "3:5-14:48", "variant": 1}])
    );
}

fn c8(root: &Path) {
    for n in 1..=25 {
        write(root, &format!("m{n:02}.py"), BASE);
    }
}

fn member_files(finding: &Value) -> Vec<String> {
    finding["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn c8_members_are_capped_at_20() {
    let dir = tempfile::tempdir().unwrap();
    c8(dir.path());
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1);
    let expected: Vec<String> = (1..=20).map(|n| format!("m{n:02}.py")).collect();
    assert_eq!(member_files(&out["findings"][0]), expected);
    assert_eq!(out["findings"][0]["members_total"], 25);
}

#[test]
fn c8_with_focus_lists_the_focus_member_first() {
    let dir = tempfile::tempdir().unwrap();
    c8(dir.path());
    let out = clones(dir.path(), &[], &["m25.py"], false).unwrap();
    let finding = &out["findings"][0];
    let mut expected = vec!["m25.py".to_string()];
    expected.extend((1..=19).map(|n| format!("m{n:02}.py")));
    assert_eq!(member_files(finding), expected);
    assert_eq!(finding["file"], "m25.py");
    assert_eq!(finding["members_total"], 25);
}

/// A second function, unlike the base, of more than 50 tokens.
const OTHER: &str = "def other(a, b):\n    x = a * 2 + b * 3 - a / 4\n    y = x * 5 + b * 6 - a / 7\n    z = y * 8 + x * 9 - a / 10\n    w = z * 8 + y * 9 - x / 10\n    return [x, y, z, w]\n";

#[test]
fn c9_focus_reports_only_the_classes_it_touches() {
    let dir = tempfile::tempdir().unwrap();
    c2(dir.path());
    write(dir.path(), "z.py", OTHER);
    write(dir.path(), "y.py", OTHER);
    let all = analyze(dir.path());
    assert_eq!(all["total"], 2);

    let out = clones(dir.path(), &[], &["c.py"], false).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["findings"][0]["file"], "c.py");
    assert_eq!(member_files(&out["findings"][0]), ["c.py", "a.py", "b.py"]);
    assert_eq!(out["details"]["focus"], json!(["c.py"]));
    assert_eq!(out["details"]["focus_total"], 1);
    assert_eq!(out["details"]["focus_not_analyzed"], json!([]));
    assert_eq!(out["coverage"]["analyzed_files"], 5);

    let out = clones(dir.path(), &[], &["z.py"], false).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(member_files(&out["findings"][0]), ["z.py", "y.py"]);
}

#[test]
fn c9_focus_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("ws");
    c2(&root);
    write(&root, "z.py", OTHER);
    write(&root, "y.py", OTHER);
    write(tmp.path(), "x.py", OTHER);
    write(&root, "sub/s.py", OTHER);

    assert_eq!(
        clones(&root, &[], &["nope.py"], false).unwrap_err(),
        "The path 'nope.py' does not exist.\n\nNearest root: \n\na.py\nb.py\nc.py\nsub\ny.py\nz.py"
    );
    assert_eq!(
        clones(&root, &[], &["../x.py"], false).unwrap_err(),
        "path must be inside the workspace root: ../x.py"
    );
    assert_eq!(
        clones(&root, &["sub"], &["a.py"], false).unwrap_err(),
        "focus path is not inside the requested paths: a.py"
    );
}

#[test]
fn focus_errors_follow_the_paths_and_limit_checks_in_focus_order() {
    let dir = tempfile::tempdir().unwrap();
    c2(dir.path());
    write(dir.path(), "sub/s.py", OTHER);
    assert!(
        clones(dir.path(), &["nope"], &["a.py"], false)
            .unwrap_err()
            .starts_with("The path 'nope' does not exist.")
    );
    assert_eq!(
        clones(dir.path(), &["sub"], &["a.py", "nope.py"], false).unwrap_err(),
        "focus path is not inside the requested paths: a.py"
    );
    let out = clones(dir.path(), &["sub"], &["sub/s.py", "sub"], false).unwrap();
    assert_eq!(out["details"]["focus"], json!(["sub/", "sub/s.py"]));
}

#[test]
fn c10_parse_error_subtrees_are_not_candidates() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", BASE);
    write(
        dir.path(),
        "g.py",
        &BASE.replace("count = count + 1", "count = count + "),
    );
    let out = analyze(dir.path());
    assert_eq!(out["status"], "incomplete");
    assert_eq!(out["total"], 0);
    assert_eq!(out["incomplete"]["parse_failures"][0]["file"], "g.py");
}

/// A JavaScript function of more than 50 tokens.
const JS: &str = "function f(a, b) {\n  let x = a * 2 + b * 3 - a / 4;\n  let y = x * 5 + b * 6 - a / 7;\n  let z = y * 8 + x * 9 - a / 10;\n  return [x, y, z];\n}\n";

#[test]
fn c11_languages_do_not_mix() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "x.js", JS);
    write(dir.path(), "x.ts", JS);
    assert_eq!(analyze(dir.path())["status"], "no_findings");
}

#[test]
fn javascript_and_jsx_do_not_mix() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "x.js", JS);
    write(dir.path(), "y.jsx", JS);
    assert_eq!(analyze(dir.path())["status"], "no_findings");
    write(dir.path(), "z.jsx", JS);
    assert_eq!(analyze(dir.path())["total"], 1);
}

#[test]
fn c12_larger_class_first() {
    let dir = tempfile::tempdir().unwrap();
    let statements: Vec<String> = (0..5)
        .map(|i| format!("    x = a * {i} + b * 3 - a / 4\n"))
        .collect();
    let small = format!("def s(a, b):\n{}", statements[..4].concat());
    let large = format!(
        "def l(a, b):\n{}    return -a * b + 1\n",
        statements.concat()
    );
    write(dir.path(), "a.py", &small);
    write(dir.path(), "b.py", &small);
    write(dir.path(), "c.py", &large);
    write(dir.path(), "d.py", &large);
    let out = analyze(dir.path());
    assert_eq!(out["total"], 2);
    assert_eq!(out["findings"][0]["tokens"], 80);
    assert_eq!(member_files(&out["findings"][0]), ["c.py", "d.py"]);
    assert_eq!(out["findings"][1]["tokens"], 60);
    assert_eq!(member_files(&out["findings"][1]), ["a.py", "b.py"]);
}

#[test]
fn c13_details() {
    let dir = tempfile::tempdir().unwrap();
    c2(dir.path());
    let details = analyze(dir.path())["details"].clone();
    assert_eq!(details["min_tokens"], 50);
    assert_eq!(
        details["exact"],
        "same syntax tree; formatting and comments ignored"
    );
    assert_eq!(
        details["renamed"],
        "identifiers renamed consistently by first appearance; string, character, numeric and symbol literals abstracted; booleans and null kept"
    );
    assert!(details["candidates"].as_u64().unwrap() >= 3);
    assert_eq!(details["focus"], json!([]));
    assert_eq!(details["focus_total"], 0);
    assert_eq!(details["focus_not_analyzed"], json!([]));
    assert_eq!(details["focus_not_analyzed_total"], 0);
}

/// One C14 fixture: `r1` holds `@T@` where `r3` uses another name.
struct Lang {
    ext: &'static str,
    language: &'static str,
    prefix: &'static str,
    r1: &'static str,
    suffix: &'static str,
    /// Line of `r1` (and `r2`) in its file.
    line: u64,
    /// Name pairs for `r2` and their replacements, whole words.
    names: &'static [(&'static str, &'static str)],
}

/// The literal changes of every `r2`.
const LITERALS: [(&str, &str); 8] = [
    ("paid", "settled"),
    ("done", "closed"),
    ("0", "7"),
    ("1", "9"),
    ("2", "6"),
    ("3", "8"),
    ("4", "11"),
    ("5", "12"),
];

/// The names of every `r2` besides each fixture's own.
const NAMES: [(&str, &str); 6] = [
    ("r1", "r2"),
    ("orders", "rows"),
    ("rate", "factor"),
    ("total", "acc"),
    ("count", "n"),
    ("label", "tag"),
];

fn replace_words(text: &str, pairs: &[(&str, &str)]) -> String {
    let alternatives: Vec<String> = pairs.iter().map(|(from, _)| regex::escape(from)).collect();
    let pattern = Regex::new(&format!(r"\b({})\b", alternatives.join("|"))).unwrap();
    pattern
        .replace_all(text, |caps: &regex::Captures| {
            let word = &caps[1];
            pairs.iter().find(|(from, _)| *from == word).unwrap().1
        })
        .into_owned()
}

const C_LIKE: &str = "int r1(int orders, int rate) {
    int total = 0;
    int count = 1;
    const char *label = \"paid\";
    total = @T@ + orders * rate;
    count = count + total * 2;
    label = \"done\";
    rate = rate * 3 - count;
    orders = orders - rate / 4;
    total = total * rate + 5;
    return total + count;
}
";

const JS_LIKE: &str = "function r1(orders, rate) {
  let total = 0;
  let count = 1;
  let label = \"paid\";
  total = @T@ + orders * rate;
  count = count + total * 2;
  label = label + \"done\";
  rate = rate * 3 - count;
  orders = orders - rate / 4;
  total = total * rate + 5;
  return total + count;
}
";

const TS_LIKE: &str = "function r1(orders: number, rate: number): number {
  let total = 0;
  let count = 1;
  let label = \"paid\";
  total = @T@ + orders * rate;
  count = count + total * 2;
  label = label + \"done\";
  rate = rate * 3 - count;
  orders = orders - rate / 4;
  total = total * rate + 5;
  return total + count;
}
";

const LANGS: [Lang; 20] = [
    Lang {
        ext: "sh",
        language: "Bash",
        prefix: "",
        r1: "r1() {
  local total=$(( 0 ))
  local count=$(( 1 ))
  local label='paid'
  total=$(( $@T@ + $orders * $rate ))
  count=$(( $count + $total * 2 ))
  label='done'
  rate=$(( $rate * 3 - $count ))
  orders=$(( $orders - $rate / 4 ))
  total=$(( $total * $rate + 5 ))
  echo $total $count $label
}
",
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "c",
        language: "C",
        prefix: "",
        r1: C_LIKE,
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "cpp",
        language: "C++",
        prefix: "",
        r1: C_LIKE,
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "cs",
        language: "C#",
        prefix: "class Shop {\n",
        r1: "int r1(int orders, int rate) {
    int total = 0;
    int count = 1;
    string label = \"paid\";
    total = @T@ + orders * rate;
    count = count + total * 2;
    label = label + \"done\";
    rate = rate * 3 - count;
    orders = orders - rate / 4;
    total = total * rate + 5;
    return total + count;
}
",
        suffix: "}\n",
        line: 2,
        names: &[],
    },
    Lang {
        ext: "dart",
        language: "Dart",
        prefix: "",
        r1: "Amount r1(Amount orders, Amount rate) {
  var total = 0;
  var count = 1;
  var label = \"paid\";
  total = @T@ + orders * rate;
  count = count + total * 2;
  label = label + \"done\";
  rate = rate * 3 - count;
  orders = orders - rate ~/ 4;
  total = total * rate + 5;
  return total + count;
}
",
        suffix: "",
        line: 1,
        names: &[("Amount", "Value")],
    },
    Lang {
        ext: "ex",
        language: "Elixir",
        prefix: "defmodule Shop do\n",
        r1: "def r1(orders, rate) do
  total = 0
  count = 1
  label = \"paid\"
  total = @T@ + orders * rate
  count = if count > 0, do: count + total * 2, else: total
  label = Meter.limit(label, \"done\")
  rate = rate * 3 - count
  orders = orders - rate / 4
  total = total * rate + 5
  total + count
end
",
        suffix: "end\n",
        line: 2,
        names: &[("Meter", "Gauge"), ("limit", "cap")],
    },
    Lang {
        ext: "go",
        language: "Go",
        prefix: "package shop\n\n",
        r1: "func r1(orders Amount, rate Amount) Amount {
\ttotal := 0
\tcount := 1
\tlabel := \"paid\"
\ttotal = @T@ + orders*rate
\tcount = count + total*2
\tlabel = label + \"done\"
\trate = rate*3 - count
\torders = orders - rate/4
\ttotal = total*rate + 5
\treturn total + count
}
",
        suffix: "",
        line: 3,
        names: &[("Amount", "Value")],
    },
    Lang {
        ext: "java",
        language: "Java",
        prefix: "class Shop {\n",
        r1: "int r1(int orders, int rate) {
    int total = 0;
    int count = 1;
    String label = \"paid\";
    total = @T@ + orders * rate;
    count = count + total * 2;
    label = label + \"done\";
    rate = rate * 3 - count;
    orders = orders - rate / 4;
    total = total * rate + 5;
    return total + count;
}
",
        suffix: "}\n",
        line: 2,
        names: &[("String", "Text")],
    },
    Lang {
        ext: "js",
        language: "JavaScript",
        prefix: "",
        r1: JS_LIKE,
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "jsx",
        language: "JSX",
        prefix: "",
        r1: JS_LIKE,
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "kt",
        language: "Kotlin",
        prefix: "",
        r1: "fun r1(orders: Int, rate: Int): Int {
    var total = 0
    var count = 1
    var label = \"paid\"
    total = @T@ + orders * rate
    count = count + total * 2
    label = label + \"done\"
    rate = rate * 3 - count
    orders = orders - rate / 4
    total = total * rate + 5
    return total + count
}
",
        suffix: "",
        line: 1,
        names: &[("Int", "Num")],
    },
    Lang {
        ext: "lua",
        language: "Lua",
        prefix: "",
        r1: "function r1(orders, rate)
  local total = 0
  local count = 1
  local label = \"paid\"
  total = @T@ + orders * rate
  count = count + total * 2
  label = label .. \"done\"
  rate = rate * 3 - count
  orders = orders - rate / 4
  total = total * rate + 5
  return total + count
end
",
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "php",
        language: "PHP",
        prefix: "<?php\n",
        r1: "function r1($orders, $rate) {
    $total = 0;
    $count = 1;
    $label = \"paid\";
    $total = $@T@ + $orders * $rate;
    $count = $count + $total * 2;
    $label = strlen($label) . 'done';
    $rate = $rate * 3 - $count;
    $orders = $orders - $rate / 4;
    $total = $total * $rate + 5;
    return $total + $count;
}
",
        suffix: "",
        line: 2,
        names: &[("strlen", "size")],
    },
    Lang {
        ext: "py",
        language: "Python",
        prefix: "",
        r1: "def r1(orders, rate):
    total = 0
    count = 1
    label = \"paid\"
    total = @T@ + orders * rate
    count = count + total * 2
    label = label + \"done\"
    rate = rate * 3 - count
    orders = orders - rate / 4
    total = total * rate + 5
    return total + count
",
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "rb",
        language: "Ruby",
        prefix: "",
        r1: "def r1(orders, rate)
  total = 0
  count = 1
  @label = \"paid\"
  total = @T@ + orders * rate
  count = count + total * LIMIT
  @label = @label + \"done\"
  rate = rate * 3 - count
  orders = orders - rate / 4
  total = total * rate + 5
  total + count
end
",
        suffix: "",
        line: 1,
        names: &[("LIMIT", "CAP")],
    },
    Lang {
        ext: "rs",
        language: "Rust",
        prefix: "",
        r1: "fn r1(orders: u32, rate: u32) -> u32 {
    let mut total = 0;
    let mut count = 1;
    let mut label = \"paid\";
    total = @T@ + orders * rate;
    count = count + total * 2;
    label = \"done\";
    let rate = rate * 3 - count;
    let orders = orders - rate / 4;
    total = total * rate + 5;
    total + count
}
",
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "scala",
        language: "Scala",
        prefix: "object Shop {\n",
        r1: "def r1(orders: Int, rate: Int): Int = {
  var total = 0
  var count = 1
  var label = \"paid\"
  total = @T@ + orders * rate
  count = count + total * 2
  label = label + \"done\"
  rate = rate * 3 - count
  orders = orders - rate / 4
  total = total * rate + 5
  total + count
}
",
        suffix: "}\n",
        line: 2,
        names: &[("Int", "Num")],
    },
    Lang {
        ext: "swift",
        language: "Swift",
        prefix: "",
        r1: "func r1(orders: Int, rate: Int) -> Int {
    var total = 0
    var count = 1
    var label = \"paid\"
    total = @T@ + orders * rate
    count = count + total * 2
    label = label + \"done\"
    rate = rate * 3 - count
    orders = orders - rate / 4
    total = total * rate + 5
    return total + count
}
",
        suffix: "",
        line: 1,
        names: &[("Int", "Num")],
    },
    Lang {
        ext: "tsx",
        language: "TSX",
        prefix: "",
        r1: TS_LIKE,
        suffix: "",
        line: 1,
        names: &[],
    },
    Lang {
        ext: "ts",
        language: "TypeScript",
        prefix: "",
        r1: TS_LIKE,
        suffix: "",
        line: 1,
        names: &[],
    },
];

/// The member's start line.
fn start_line(member: &Value) -> u64 {
    let range = member["range"].as_str().unwrap();
    range.split(':').next().unwrap().parse().unwrap()
}

#[test]
fn c14_every_language_renamed_class_without_r3() {
    let mut failures = Vec::new();
    for lang in &LANGS {
        let dir = tempfile::tempdir().unwrap();
        let r1 = lang.r1.replace("@T@", "total");
        let r3 = lang.r1.replace("@T@", "count");
        let mut names: Vec<(&str, &str)> = NAMES.to_vec();
        if lang.ext == "sh" {
            // A Bash function name is a `word`, which is not renamed.
            names.retain(|(from, _)| *from != "r1");
        }
        names.extend(lang.names);
        names.extend(LITERALS);
        let r2 = replace_words(&r1, &names);
        let r3 = replace_words(&r3, &[("r1", "r3")]);
        assert_ne!(r2, r1);
        let one = format!("one.{}", lang.ext);
        let two = format!("two.{}", lang.ext);
        write(
            dir.path(),
            &one,
            &format!("{}{r1}{}", lang.prefix, lang.suffix),
        );
        write(
            dir.path(),
            &two,
            &format!("{}{r2}\n{r3}{}", lang.prefix, lang.suffix),
        );
        let out = analyze(dir.path());
        let ok = out["status"] == "findings"
            && out["complete"] == true
            && out["total"] == 1
            && out["findings"][0]["kind"] == "renamed"
            && out["findings"][0]["language"] == lang.language
            && out["findings"][0]["members_total"] == 2
            && out["findings"][0]["variants"] == 2
            && member_files(&out["findings"][0]) == [one.clone(), two.clone()]
            && out["findings"][0]["members"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .all(|(i, m)| m["variant"] == i as u64 + 1 && start_line(m) == lang.line)
            && out["findings"][0]["tokens"].as_u64().unwrap() >= 50;
        if !ok {
            failures.push(format!(
                "{}: {}",
                lang.language,
                serde_json::to_string(&out["findings"]).unwrap()
                    + " "
                    + &serde_json::to_string(&out["incomplete"]["parse_failures"]).unwrap()
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn rust_copy_in_a_test_module_counts_only_with_include_tests() {
    let dir = tempfile::tempdir().unwrap();
    let function = LANGS[15].r1.replace("@T@", "total");
    write(dir.path(), "src/a.rs", &function);
    let indented: String = function.lines().map(|l| format!("    {l}\n")).collect();
    write(
        dir.path(),
        "src/b.rs",
        &format!("fn main() {{}}\n\n#[cfg(test)]\nmod tests {{\n{indented}}}\n"),
    );
    let out = clones(dir.path(), &[], &[], false).unwrap();
    assert_eq!(out["status"], "no_findings");

    let out = clones(dir.path(), &[], &[], true).unwrap();
    assert_eq!(out["total"], 1);
    assert_eq!(out["findings"][0]["kind"], "exact");
    assert_eq!(member_files(&out["findings"][0]), ["src/a.rs", "src/b.rs"]);
}

#[test]
fn focus_on_a_test_file_or_a_data_file_is_not_analyzed() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", BASE);
    write(dir.path(), "b.py", BASE);
    write(dir.path(), "tests/copy.py", BASE);
    write(dir.path(), "data.json", "{\"a\": 1}\n");
    for focus in ["tests/copy.py", "data.json"] {
        let out = clones(dir.path(), &[], &[focus], false).unwrap();
        assert_eq!(out["status"], "no_findings", "{focus}");
        assert_eq!(out["details"]["focus_not_analyzed"], json!([focus]));
        assert_eq!(out["details"]["focus_not_analyzed_total"], 1);
    }
    let out = clones(dir.path(), &[], &["tests/copy.py", "data.json"], false).unwrap();
    assert_eq!(
        out["details"]["focus_not_analyzed"],
        json!(["data.json", "tests/copy.py"])
    );
}

/// A TypeScript function of 52 tokens whose parameters and result have type
/// `ty`; its body has 35.
fn typed_ts(ty: &str) -> String {
    format!(
        "function f(a: {ty}, b: {ty}, c: {ty}): {ty} {{\n  let x = a * 2 + b * 3 - c / 4;\n  let y = x * 5 + b * 6 - c / 7;\n  return y;\n}}\n"
    )
}

#[test]
fn type_keywords_are_not_literals() {
    for (one, other) in [("string", "number"), ("string", "boolean")] {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.ts", &typed_ts(one));
        write(dir.path(), "b.ts", &typed_ts(other));
        assert_eq!(
            analyze(dir.path())["status"],
            "no_findings",
            "{one} {other}"
        );
    }
    let dir = tempfile::tempdir().unwrap();
    // 52 tokens; the body has 31.
    let php = |ty: &str| {
        format!(
            "<?php\nfunction f({ty} $a, {ty} $b, {ty} $c, {ty} $d): {ty} {{\n    $x = $a * 2 + $b;\n    $z = $x * 8 - $b + $c;\n    return $z;\n}}\n"
        )
    };
    write(dir.path(), "a.php", &php("string"));
    write(dir.path(), "b.php", &php("float"));
    assert_eq!(analyze(dir.path())["status"], "no_findings");
    write(dir.path(), "c.php", &php("float"));
    assert_eq!(analyze(dir.path())["total"], 1);
}

#[test]
fn a_clone_inside_a_replaced_literal_is_not_reported_again() {
    let dir = tempfile::tempdir().unwrap();
    let sum: Vec<String> = (1..=30).map(|n| n.to_string()).collect();
    let function = format!(
        "def f(a, b):\n    x = a * 2 + b * 3 - a / 4\n    x = x * 5 + b * 6 - a / 7\n    x = x * 8 + b * 9 - a / 10\n    y = x * 2 + b\n    return f\"{{{}}}\"\n",
        sum.join(" + ")
    );
    write(dir.path(), "a.py", &function);
    write(dir.path(), "b.py", &function);
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1, "{}", out["findings"]);
    assert_eq!(out["findings"][0]["tokens"], 56);
    assert_eq!(out["findings"][0]["range"], "1:1-6:155");
}

/// A Kotlin function of more than 50 tokens that holds `label` once.
fn kotlin_label(label: &str) -> String {
    format!(
        "fun report(x: Int, y: Int): String {{\n    val a = x + y * 2\n    val b = a - x / 3\n    val c = b * a + y\n    val label = \"{label}\"\n    if (c > a && b < y) {{\n        return label + a + b + c\n    }}\n    return label\n}}\n"
    )
}

/// A Dart function of more than 50 tokens that holds `label` once.
fn dart_label(label: &str) -> String {
    format!(
        "String report(int x, int y) {{\n  var a = x + y * 2;\n  var b = a - x * 3;\n  var c = b * a + y;\n  var label = '{label}';\n  if (c > a && b < y) {{\n    return label + c.toString();\n  }}\n  return label;\n}}\n"
    )
}

#[test]
fn string_text_the_grammar_keeps_out_of_the_tree_is_part_of_the_form() {
    // Kotlin `$x` is an identifier: the literal is not replaced, so its
    // text stays in the renamed form as well.
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.kt", &kotlin_label("paid $x"));
    write(dir.path(), "b.kt", &kotlin_label("paid $x"));
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1, "{}", out["findings"]);
    assert_eq!(out["findings"][0]["kind"], "exact");
    write(dir.path(), "b.kt", &kotlin_label("failed $x"));
    let out = analyze(dir.path());
    assert_eq!(out["status"], "no_findings", "{}", out["findings"]);

    // Dart `$x` is not an identifier token, so the literal is replaced in
    // the renamed form; the exact forms differ.
    for (one, other) in [("paid $x", "failed $x"), ("paid", "failed")] {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.dart", &dart_label(one));
        write(dir.path(), "b.dart", &dart_label(one));
        let out = analyze(dir.path());
        assert_eq!(out["total"], 1, "{}", out["findings"]);
        assert_eq!(out["findings"][0]["kind"], "exact");
        write(dir.path(), "b.dart", &dart_label(other));
        let out = analyze(dir.path());
        assert_eq!(out["total"], 1, "{}", out["findings"]);
        let finding = &out["findings"][0];
        assert_eq!(finding["kind"], "renamed", "{one} {other}");
        assert_eq!(finding["variants"], 2);
    }
}

#[test]
fn a_comment_between_adjacent_strings_does_not_change_the_form() {
    for (plain, commented) in [
        ("'a' 'b ${x}'", "'a' /* note */ 'b ${x}'"),
        ("'a' 'b'", "'a' /* note */ 'b'"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.dart", &dart_label_expr(plain));
        write(dir.path(), "b.dart", &dart_label_expr(commented));
        let out = analyze(dir.path());
        assert_eq!(out["total"], 1, "{plain}: {}", out["findings"]);
        assert_eq!(out["findings"][0]["kind"], "exact", "{plain}");
        assert_eq!(out["findings"][0]["variants"], 1, "{plain}");
    }
}

/// A Dart function of more than 50 tokens that assigns `label` the
/// expression `expr`.
fn dart_label_expr(expr: &str) -> String {
    format!(
        "String report(int x, int y) {{\n  var a = x + y * 2;\n  var b = a - x * 3;\n  var c = b * a + y;\n  var label = {expr};\n  if (c > a && b < y) {{\n    return label + c.toString();\n  }}\n  return label;\n}}\n"
    )
}

/// A C function of more than 50 tokens that assigns `label` the expression
/// `expr`.
fn c_label_expr(expr: &str) -> String {
    format!(
        "const char *report(int x, int y) {{\n  int a = x + y * 2;\n  int b = a - x * 3;\n  int c = b * a + y;\n  const char *label = {expr};\n  if (c > a && b < y) {{\n    return label + c;\n  }}\n  return label;\n}}\n"
    )
}

/// A Python function of more than 50 tokens that assigns `label` the
/// expression `expr`.
fn python_label_expr(expr: &str) -> String {
    format!(
        "def report(x, y):\n    a = x + y * 2\n    b = a - x * 3\n    c = b * a + y\n    label = ({expr})\n    if c > a and b < y:\n        return label + str(c)\n    return label + str(a) + str(b)\n"
    )
}

#[test]
fn comments_between_quoted_parts_of_a_c_concatenation_do_not_change_the_form() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.c", &c_label_expr("\"a\" \"b\""));
    write(dir.path(), "b.c", &c_label_expr("\"a\" /* one */ \"b\""));
    write(
        dir.path(),
        "c.c",
        &c_label_expr("\"a\"  /* one */\n   /* two */  \"b\""),
    );
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1, "{}", out["findings"]);
    assert_eq!(out["findings"][0]["kind"], "exact");
    assert_eq!(out["findings"][0]["variants"], 1);
    assert_eq!(member_files(&out["findings"][0]).len(), 3);
}

#[test]
fn comments_between_parts_of_a_python_interpolated_concatenation_do_not_change_the_form() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.py", &python_label_expr("f\"a{x}\" \"b\""));
    write(
        dir.path(),
        "b.py",
        &python_label_expr("f\"a{x}\"  # one\n        # two\n        \"b\""),
    );
    let out = analyze(dir.path());
    assert_eq!(out["total"], 1, "{}", out["findings"]);
    assert_eq!(out["findings"][0]["kind"], "exact");
    assert_eq!(out["findings"][0]["variants"], 1);
}
