//! Make: skeleton, symbols, target and assignment forms, the
//! no-execution limitation, the name-level reference evidence and the
//! language-server errors.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

const MAKEFILE: &str = "CC ?= cc\nOBJS := a.o b.o\nexport PATH\ndefine HELP\ntext\nendef\n.PHONY: all clean\nall: app\napp: $(OBJS)\n\t$(CC) -o $@ $^\n%.o: %.c\n\t$(CC) -c $<\nclean:\n\trm -f app\ninclude common.mk\nifeq ($(OS),x)\nX = 1\nendif\n";

const NAME_LEVEL_HINT: &str =
    "Name-level baseline — a same-name symbol in an unrelated scope can appear.";

const SAME_NAME: &str = "x = 1\nifeq ($(A),1)\nx = 2\nendif\n";

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    for (file, content) in files {
        std::fs::write(ws.path().join(file), content).unwrap();
    }
    ws
}

/// `line:column-line:column` of a CLI range object.
fn span(range: &Value) -> String {
    format!(
        "{}:{}-{}:{}",
        range["start"]["line"],
        range["start"]["column"],
        range["end"]["line"],
        range["end"]["column"]
    )
}

fn mcp_payload(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    let json = text.split("\nNote:").next().unwrap_or_default();
    serde_json::from_str(json).unwrap_or_else(|e| panic!("not JSON ({e}): {result}"))
}

fn mcp_text(result: &Value) -> String {
    result["content"]
        .as_array()
        .unwrap_or_else(|| panic!("no content in {result}"))
        .iter()
        .filter_map(|block| block["text"].as_str())
        .collect()
}

fn items<'a>(answer: &'a Value, key: &str) -> &'a Vec<Value> {
    answer[key]
        .as_array()
        .unwrap_or_else(|| panic!("no {key} in {answer}"))
}

/// The `(kind, name)` pairs of `answer[key]`.
fn kind_names(answer: &Value, key: &str) -> BTreeSet<(String, String)> {
    items(answer, key)
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(list: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    list.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

const SKELETON_NAMES: [(&str, &str); 9] = [
    ("variable", "CC"),
    ("variable", "OBJS"),
    ("variable", "HELP"),
    ("function", "all"),
    ("function", "app"),
    ("function", "%.o"),
    ("function", "clean"),
    ("include", "common.mk"),
    ("variable", "X"),
];

fn skeleton_names() -> BTreeSet<(String, String)> {
    pairs(&SKELETON_NAMES)
}

#[test]
fn fixture_holds_the_design_bytes() {
    assert_eq!(
        std::fs::read_to_string(fixture("make/Makefile")).unwrap(),
        MAKEFILE
    );
}

#[test]
fn skeleton_is_the_table_set() {
    let skeleton = cli_json(&fixture("make"), &["query", "skeleton", "Makefile"]);
    let actual: BTreeSet<(String, String, String, String)> = items(&skeleton, "declarations")
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                span(&d["range"]),
                span(&d["name_range"]),
            )
        })
        .collect();
    let expected: BTreeSet<(String, String, String, String)> = [
        ("variable", "CC", "1:1-2:1", "1:1-1:3"),
        ("variable", "OBJS", "2:1-3:1", "2:1-2:5"),
        ("variable", "HELP", "4:1-7:1", "4:8-4:12"),
        ("function", "all", "8:1-9:1", "8:1-8:4"),
        ("function", "app", "9:1-11:1", "9:1-9:4"),
        ("function", "%.o", "11:1-13:1", "11:1-11:4"),
        ("function", "clean", "13:1-15:1", "13:1-13:6"),
        ("include", "common.mk", "15:1-16:1", "15:9-15:18"),
        ("variable", "X", "17:1-18:1", "17:1-17:2"),
    ]
    .into_iter()
    .map(|(k, n, r, nr)| (k.into(), n.into(), r.into(), nr.into()))
    .collect();
    assert_eq!(actual, expected);
    let names: BTreeSet<&str> = items(&skeleton, "declarations")
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(".PHONY") && !names.contains("PATH"));
}

#[test]
fn symbols_are_the_table_set() {
    let symbols = cli_json(&fixture("make"), &["query", "symbols", "Makefile"]);
    let actual: BTreeSet<(String, String, String)> = items(&symbols, "symbols")
        .iter()
        .map(|s| {
            let name = s["name"].as_str().unwrap();
            assert!(!name.is_empty(), "empty name in {s}");
            assert_ne!(s["range"]["start"], s["range"]["end"], "zero width in {s}");
            (
                s["kind"].as_str().unwrap().to_string(),
                name.to_string(),
                span(&s["range"]),
            )
        })
        .collect();
    let expected: BTreeSet<(String, String, String)> = [
        ("variable", "CC", "1:1-1:3"),
        ("variable", "OBJS", "2:1-2:5"),
        ("variable", "HELP", "4:8-4:12"),
        ("function", "all", "8:1-8:4"),
        ("function", "app", "9:1-9:4"),
        ("function", "%.o", "11:1-11:4"),
        ("function", "clean", "13:1-13:6"),
        ("variable", "X", "17:1-17:2"),
    ]
    .into_iter()
    .map(|(k, n, r)| (k.into(), n.into(), r.into()))
    .collect();
    assert_eq!(actual, expected);
    assert!(symbols.get("parse_error").is_none(), "{symbols}");
}

#[test]
fn symbols_batch_lists_the_skeleton_set_on_cli_and_mcp() {
    let expected: BTreeSet<(String, String)> =
        skeleton_names().into_iter().map(|(k, n)| (n, k)).collect();
    let by_name_kind = |batch: &Value| -> BTreeSet<(String, String)> {
        items(&batch["results"][0], "symbols")
            .iter()
            .map(|s| {
                (
                    s["name"].as_str().unwrap().to_string(),
                    s["kind"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    };
    let ws = fixture("make");
    let batch = cli_json(&ws, &["query", "symbols-batch", "--files", "Makefile"]);
    assert_eq!(by_name_kind(&batch), expected, "{batch}");
    let result = Mcp::start(&ws)
        .call("symbols_batch", json!({ "files": ["Makefile"] }))
        .unwrap();
    assert_eq!(by_name_kind(&mcp_payload(&result)), expected, "{result}");
}

#[test]
fn query_symbol_returns_the_name_range() {
    let symbol = cli_json(
        &fixture("make"),
        &["query", "symbol", "app", "--file", "Makefile"],
    );
    assert_eq!(span(&symbol["symbol"]["range"]), "9:1-9:4", "{symbol}");
}

#[test]
fn mcp_skeleton_and_symbols_give_the_table_names_and_kinds() {
    let mut mcp = Mcp::start(&fixture("make"));
    let skeleton = mcp_payload(&mcp.call("skeleton", json!({ "file": "Makefile" })).unwrap());
    assert_eq!(kind_names(&skeleton, "declarations"), skeleton_names());

    let symbols = mcp_payload(&mcp.call("symbols", json!({ "file": "Makefile" })).unwrap());
    let expected = pairs(&[
        ("Variable", "CC"),
        ("Variable", "OBJS"),
        ("Variable", "HELP"),
        ("Function", "all"),
        ("Function", "app"),
        ("Function", "%.o"),
        ("Function", "clean"),
        ("Variable", "X"),
    ]);
    assert_eq!(kind_names(&symbols, "symbols"), expected);
}

fn symbols_of(source: &str) -> Value {
    let ws = workspace(&[("Makefile", source)]);
    cli_json(ws.path(), &["query", "symbols", "Makefile"])
}

#[test]
fn a_rule_with_two_targets_gives_two_functions_with_the_rule_range() {
    let ws = workspace(&[("Makefile", "a b: c\n\ttouch $@\n")]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "Makefile"]);
    let found: BTreeSet<(String, String, String)> = items(&skeleton, "declarations")
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                span(&d["range"]),
            )
        })
        .collect();
    let expected = [("function", "a"), ("function", "b")]
        .into_iter()
        .map(|(k, n)| (k.to_string(), n.to_string(), "1:1-3:1".to_string()))
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn assignment_forms_are_variables() {
    let symbols = symbols_of(
        "a ::= 1\nb += 2\nc != echo 3\noverride d = 4\nexport e = 5\nf ?= 6\ng := 7\nh = 8\n",
    );
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[
            ("variable", "a"),
            ("variable", "b"),
            ("variable", "c"),
            ("variable", "d"),
            ("variable", "e"),
            ("variable", "f"),
            ("variable", "g"),
            ("variable", "h"),
        ])
    );
}

#[test]
fn include_forms_are_skeleton_entries_only() {
    let ws = workspace(&[(
        "Makefile",
        "include a.mk\n-include b.mk\nsinclude c.mk\ninclude d.mk e.mk\n",
    )]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "Makefile"]);
    assert_eq!(
        kind_names(&skeleton, "declarations"),
        pairs(&[
            ("include", "a.mk"),
            ("include", "b.mk"),
            ("include", "c.mk"),
            ("include", "d.mk e.mk"),
        ])
    );
    let symbols = cli_json(ws.path(), &["query", "symbols", "Makefile"]);
    assert!(items(&symbols, "symbols").is_empty(), "{symbols}");
}

#[test]
fn special_targets_and_target_specific_assignments_are_not_reported() {
    let symbols =
        symbols_of(".PHONY: app\n.DEFAULT_GOAL := app\napp: CFLAGS += -O2\napp:\n\ttrue\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[("function", "app"), ("variable", ".DEFAULT_GOAL")])
    );
}

#[test]
fn a_target_made_by_eval_is_not_reported() {
    let symbols = symbols_of("$(eval gen: dep)\nreal:\n\ttrue\n");
    assert_eq!(
        kind_names(&symbols, "symbols"),
        pairs(&[("function", "real")])
    );
}

#[test]
fn recipes_are_not_parsed_and_variables_are_not_expanded() {
    let symbols = symbols_of("T = a\n$(T): x\n\tfoo = bar\n");
    assert_eq!(kind_names(&symbols, "symbols"), pairs(&[("variable", "T")]));
}

#[test]
fn an_assignment_in_each_branch_is_reported_once_per_branch() {
    let ws = workspace(&[("Makefile", SAME_NAME)]);
    let skeleton = cli_json(ws.path(), &["query", "skeleton", "Makefile"]);
    let found: Vec<(String, String)> = items(&skeleton, "declarations")
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        found,
        vec![
            ("variable".to_string(), "x".to_string()),
            ("variable".to_string(), "x".to_string())
        ]
    );
}

/// Every path under `dir` with its modification time and, for a file, its bytes.
fn listing(dir: &Path) -> Vec<(String, std::time::SystemTime, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(&path).unwrap() {
            let entry = entry.unwrap();
            let meta = entry.metadata().unwrap();
            let rel = entry
                .path()
                .strip_prefix(dir)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let content = meta.is_file().then(|| std::fs::read(entry.path()).unwrap());
            out.push((rel, meta.modified().unwrap(), content));
            if meta.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn queries_do_not_run_recipes() {
    let ws = workspace(&[("Makefile", "clean:\n\trm -rf out\n")]);
    std::fs::create_dir(ws.path().join("out")).unwrap();
    std::fs::write(ws.path().join("out").join("keep.txt"), "kept").unwrap();
    let before = listing(ws.path());
    cli_json(ws.path(), &["query", "skeleton", "Makefile"]);
    cli_json(ws.path(), &["query", "symbols", "Makefile"]);
    cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "clean",
            "--file",
            "Makefile",
            "--line",
            "1",
            "--column",
            "1",
        ],
    );
    assert!(ws.path().join("out/keep.txt").exists());
    assert_eq!(listing(ws.path()), before);
}

#[test]
fn same_name_assignments_are_a_name_match_on_cli() {
    let ws = workspace(&[("Makefile", SAME_NAME)]);
    let answer = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "x",
            "--file",
            "Makefile",
            "--line",
            "1",
            "--column",
            "1",
        ],
    );
    assert_eq!(answer["source"], "tree-sitter", "{answer}");
    let sites: BTreeSet<(u64, u64)> = items(&answer, "references")
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(sites, [(1, 1), (3, 1)].into_iter().collect());
}

#[test]
fn targets_prerequisites_and_variable_uses_are_name_matches() {
    let ws = workspace(&[(
        "Makefile",
        "app: dep\n\ttrue\ndep:\n\ttrue\nV = 1\nW = $(V) ${V}\n",
    )]);
    let sites = |name: &str, line: &str| -> BTreeSet<(u64, u64)> {
        let answer = cli_json(
            ws.path(),
            &[
                "query",
                "references",
                name,
                "--file",
                "Makefile",
                "--line",
                line,
                "--column",
                "1",
            ],
        );
        items(&answer, "references")
            .iter()
            .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
            .collect()
    };
    assert_eq!(sites("dep", "3"), [(1, 6), (3, 1)].into_iter().collect());
    assert_eq!(
        sites("V", "5"),
        [(5, 1), (6, 7), (6, 12)].into_iter().collect()
    );
}

fn assert_mcp_name_match(mcp: &mut Mcp, reason: &str) {
    let result = mcp
        .call("references", json!({ "symbol": "x", "file": "Makefile" }))
        .unwrap();
    let answer = mcp_payload(&result);
    assert_eq!(
        answer["resolved_by"],
        format!("tree-sitter workspace baseline (name-level; {reason})"),
        "{answer}"
    );
    assert_eq!(
        answer["references"].as_array().map(Vec::len),
        Some(2),
        "{answer}"
    );
    let text = mcp_text(&result);
    assert!(text.contains(NAME_LEVEL_HINT), "{text}");
}

#[test]
fn same_name_assignments_are_a_name_match_on_mcp() {
    let ws = workspace(&[("Makefile", SAME_NAME)]);
    assert_mcp_name_match(&mut Mcp::start(ws.path()), "no language server");
}

#[test]
fn mcp_reason_reads_disabled_when_language_servers_are_disabled() {
    let ws = workspace(&[("Makefile", SAME_NAME)]);
    let mut mcp = Mcp::start_with_env(ws.path(), &[("ACT_LITE_DISABLE_LSP", "1")]);
    assert_mcp_name_match(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server_on_cli_and_mcp() {
    let ws = fixture("make");
    let output = cli(&ws, &["query", "callers", "app", "--file", "Makefile"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains("needs a language server, and act-lite has none configured for Make files"),
        "{stderr}"
    );
    assert!(!stderr.contains("hint:"), "{stderr}");
    let message = Mcp::start(&ws)
        .call("callers", json!({ "symbol": "app", "file": "Makefile" }))
        .unwrap_err();
    assert!(
        message.contains(
            "query type 'callers' needs a language server, and act-lite has none configured for Make files"
        ),
        "{message}"
    );
}
