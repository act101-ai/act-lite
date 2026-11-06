//! XML: skeleton, symbols, references and the stated limits, through the
//! built `act-lite` and `act-lite mcp serve`. Expected values come from the
//! fixture source.

mod langs_added;

use langs_added::{Mcp, cli, cli_json, fixture};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

/// Serializes the tests that start an MCP server, because one of them sets
/// `ACT_LITE_DISABLE_LSP` for the server it starts.
static MCP: Mutex<()> = Mutex::new(());

/// A workspace holding a copy of `pom.xml`.
fn pom_workspace() -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    fs::copy(fixture("xml/pom.xml"), ws.path().join("pom.xml")).unwrap();
    ws
}

/// A workspace holding one file.
fn workspace_with(name: &str, content: &[u8]) -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    fs::write(ws.path().join(name), content).unwrap();
    ws
}

/// `"L:C-L:C"` of a CLI range object.
fn range(value: &Value) -> String {
    format!(
        "{}:{}-{}:{}",
        value["start"]["line"],
        value["start"]["column"],
        value["end"]["line"],
        value["end"]["column"]
    )
}

fn skeleton(ws: &Path, file: &str) -> BTreeSet<(String, String, String, String)> {
    let result = cli_json(ws, &["query", "skeleton", file]);
    result["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["kind"].as_str().unwrap().to_string(),
                d["name"].as_str().unwrap().to_string(),
                range(&d["range"]),
                range(&d["name_range"]),
            )
        })
        .collect()
}

fn symbols(ws: &Path, file: &str) -> (Value, BTreeSet<(String, String, String)>) {
    let result = cli_json(ws, &["query", "symbols", file]);
    let set = result["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["kind"].as_str().unwrap().to_string(),
                s["name"].as_str().unwrap().to_string(),
                range(&s["range"]),
            )
        })
        .collect();
    (result, set)
}

fn set<const N: usize, T: Ord + Clone>(items: [T; N]) -> BTreeSet<T> {
    items.into_iter().collect()
}

fn s4(kind: &str, name: &str, range: &str, name_range: &str) -> (String, String, String, String) {
    (kind.into(), name.into(), range.into(), name_range.into())
}

fn s3(kind: &str, name: &str, range: &str) -> (String, String, String) {
    (kind.into(), name.into(), range.into())
}

fn pom_skeleton() -> BTreeSet<(String, String, String, String)> {
    set([
        s4("element", "project", "2:1-8:11", "2:2-2:9"),
        s4("element", "dependency", "4:3-6:16", "4:4-4:14"),
        s4("element", "name", "5:5-5:22", "5:6-5:10"),
        s4("element", "empty", "7:3-7:11", "7:4-7:9"),
    ])
}

fn pom_symbols() -> BTreeSet<(String, String, String)> {
    set([
        s3("class", "project", "2:2-2:9"),
        s3("field", "xmlns", "2:10-2:15"),
        s3("class", "dependency", "4:4-4:14"),
        s3("field", "id", "4:15-4:17"),
        s3("field", "scope", "4:25-4:30"),
        s3("class", "name", "5:6-5:10"),
        s3("class", "empty", "7:4-7:9"),
    ])
}

/// The JSON of the first content block of a tool result.
fn first_json(result: &Value) -> Value {
    let text = result["content"][0]["text"].as_str().unwrap();
    serde_json::Deserializer::from_str(text)
        .into_iter::<Value>()
        .next()
        .and_then(Result::ok)
        .unwrap_or_else(|| panic!("not JSON: {text}"))
}

fn names_and_kinds(list: &Value) -> BTreeSet<(String, String)> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["name"].as_str().unwrap().to_string(),
                d["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn skeleton_of_pom_is_the_element_tree() {
    let ws = pom_workspace();
    assert_eq!(skeleton(ws.path(), "pom.xml"), pom_skeleton());
}

#[test]
fn symbols_of_pom_are_elements_and_attributes() {
    let ws = pom_workspace();
    let (result, found) = symbols(ws.path(), "pom.xml");
    assert_eq!(found, pom_symbols());
    assert!(result.get("parse_error").is_none(), "{result}");
    for symbol in result["symbols"].as_array().unwrap() {
        let name = symbol["name"].as_str().unwrap();
        assert!(!name.is_empty(), "{symbol}");
        assert_ne!(
            range(&symbol["range"]).split('-').next(),
            range(&symbol["range"]).split('-').nth(1)
        );
    }

    let symbol = cli_json(
        ws.path(),
        &["query", "symbol", "dependency", "--file", "pom.xml"],
    );
    assert_eq!(range(&symbol["symbol"]["range"]), "4:4-4:14");

    let batch = cli_json(ws.path(), &["query", "symbols-batch", "--files", "pom.xml"]);
    let want: BTreeSet<(String, String)> = pom_skeleton()
        .into_iter()
        .map(|(kind, name, ..)| (name, kind))
        .collect();
    assert_eq!(names_and_kinds(&batch["results"][0]["symbols"]), want);
}

#[test]
fn mcp_skeleton_symbols_and_batch_name_the_same() {
    let _guard = MCP.lock().unwrap_or_else(|e| e.into_inner());
    let ws = pom_workspace();
    let mut mcp = Mcp::start(ws.path());
    let skeleton = first_json(&mcp.call("skeleton", json!({"file": "pom.xml"})).unwrap());
    let want: BTreeSet<(String, String)> = pom_skeleton()
        .into_iter()
        .map(|(kind, name, ..)| (name, kind))
        .collect();
    assert_eq!(names_and_kinds(&skeleton["declarations"]), want);

    let symbols = first_json(&mcp.call("symbols", json!({"file": "pom.xml"})).unwrap());
    let names: BTreeSet<String> = symbols["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    let want_names: BTreeSet<String> = pom_symbols().into_iter().map(|(_, name, _)| name).collect();
    assert_eq!(names, want_names);

    let batch = first_json(
        &mcp.call("symbols_batch", json!({"files": ["pom.xml"]}))
            .unwrap(),
    );
    assert_eq!(names_and_kinds(&batch["results"][0]["symbols"]), want);
}

#[test]
fn prolog_doctype_and_comments_are_not_reported() {
    let ws = workspace_with(
        "d.xml",
        b"<?xml version=\"1.0\"?>\n<!DOCTYPE r [<!ELEMENT r ANY>]>\n<!-- c -->\n<r/>\n",
    );
    assert_eq!(
        skeleton(ws.path(), "d.xml"),
        set([s4("element", "r", "4:1-4:5", "4:2-4:3")])
    );
    let (_, found) = symbols(ws.path(), "d.xml");
    assert_eq!(found, set([s3("class", "r", "4:2-4:3")]));
}

#[test]
fn prefixed_names_are_written_as_in_the_source() {
    let ws = workspace_with(
        "s.xsd",
        b"<xs:schema xmlns:xs=\"u\">\n  <xs:element name=\"a\"/>\n</xs:schema>\n",
    );
    assert_eq!(
        skeleton(ws.path(), "s.xsd"),
        set([
            s4("element", "xs:schema", "1:1-3:13", "1:2-1:11"),
            s4("element", "xs:element", "2:3-2:25", "2:4-2:14"),
        ])
    );
    let (_, found) = symbols(ws.path(), "s.xsd");
    assert_eq!(
        found,
        set([
            s3("class", "xs:schema", "1:2-1:11"),
            s3("field", "xmlns:xs", "1:12-1:20"),
            s3("class", "xs:element", "2:4-2:14"),
            s3("field", "name", "2:15-2:19"),
        ])
    );
}

#[test]
fn a_binary_plist_is_refused_and_outlined_without_symbols() {
    let ws = workspace_with("Info.plist", b"bplist00\xd1\x01\x02\x00\xff\xfe");
    let output = cli(ws.path(), &["query", "skeleton", "Info.plist"]);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("UTF-8"), "{stderr}");
    let outline = cli_json(ws.path(), &["query", "repo-outline", "--symbols"]);
    let file = outline["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| {
            f["path"]
                .as_str()
                .is_some_and(|p| p.ends_with("Info.plist"))
        })
        .unwrap_or_else(|| panic!("Info.plist not listed: {outline}"));
    assert!(
        file.get("symbols")
            .is_none_or(|s| s.as_array().is_some_and(|l| l.is_empty())),
        "{file}"
    );
}

const SAME_NAME: &str = "<r><a><name/></a><b><name/></b></r>\n";

#[test]
fn references_are_name_matches() {
    let ws = workspace_with("n.xml", SAME_NAME.as_bytes());
    let result = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "name",
            "--file",
            "n.xml",
            "--line",
            "1",
            "--column",
            "8",
        ],
    );
    assert_eq!(result["source"], "tree-sitter", "{result}");
    let sites: BTreeSet<(u64, u64)> = result["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(sites, set([(1, 8), (1, 22)]));
}

#[test]
fn mcp_references_say_name_level() {
    let _guard = MCP.lock().unwrap_or_else(|e| e.into_inner());
    let ws = workspace_with("n.xml", SAME_NAME.as_bytes());
    let check = |mcp: &mut Mcp, why: &str| {
        let result = mcp
            .call("references", json!({"symbol": "name", "file": "n.xml"}))
            .unwrap();
        let answer = first_json(&result);
        assert_eq!(
            answer["resolved_by"],
            format!("tree-sitter workspace baseline (name-level; {why})"),
            "{answer}"
        );
        let lines: Vec<u64> = answer["references"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["line"].as_u64().unwrap())
            .collect();
        assert_eq!(lines, [1, 1], "{answer}");
        let hint = result["content"][1]["text"].as_str().unwrap_or_default();
        assert!(hint.contains("Name-level baseline"), "{hint}");
    };
    let mut mcp = Mcp::start(ws.path());
    check(&mut mcp, "no language server");
    drop(mcp);
    // SAFETY: every test that starts a server holds the `MCP` lock, and no
    // other thread of this test binary reads the environment meanwhile.
    unsafe { std::env::set_var("ACT_LITE_DISABLE_LSP", "1") };
    let mut mcp = Mcp::start(ws.path());
    unsafe { std::env::remove_var("ACT_LITE_DISABLE_LSP") };
    check(&mut mcp, "language servers are disabled");
}

#[test]
fn callers_needs_a_language_server() {
    let _guard = MCP.lock().unwrap_or_else(|e| e.into_inner());
    let ws = pom_workspace();
    let output = cli(
        ws.path(),
        &["query", "callers", "project", "--file", "pom.xml"],
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("needs a language server, and act-lite has none configured for XML files"),
        "{stderr}"
    );
    assert!(!stderr.lines().any(|l| l.starts_with("hint:")), "{stderr}");
    let mut mcp = Mcp::start(ws.path());
    let error = mcp
        .call("callers", json!({"symbol": "project", "file": "pom.xml"}))
        .unwrap_err();
    assert!(
        error.contains("needs a language server, and act-lite has none configured for XML files"),
        "{error}"
    );
}

#[test]
fn no_build_system_semantics() {
    let ws = pom_workspace();
    assert!(
        skeleton(ws.path(), "pom.xml")
            .iter()
            .all(|(kind, ..)| kind != "import" && kind != "include")
    );
    let props = workspace_with("p.xml", b"<p><v>${x}</v></p>\n");
    let names: BTreeSet<String> = skeleton(props.path(), "p.xml")
        .into_iter()
        .map(|(_, name, ..)| name)
        .collect();
    assert_eq!(names, set(["p".to_string(), "v".to_string()]));
    let (_, found) = symbols(props.path(), "p.xml");
    assert!(
        found.iter().all(|(_, name, _)| !name.contains('x')),
        "{found:?}"
    );
}

#[test]
fn attribute_names_are_references_too() {
    let ws = workspace_with("i.xml", b"<r><a id=\"1\"/><b id=\"2\"/></r>\n");
    let result = cli_json(
        ws.path(),
        &[
            "query",
            "references",
            "id",
            "--file",
            "i.xml",
            "--line",
            "1",
            "--column",
            "7",
        ],
    );
    let sites: BTreeSet<(u64, u64)> = result["references"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect();
    assert_eq!(sites, set([(1, 7), (1, 18)]));
}
