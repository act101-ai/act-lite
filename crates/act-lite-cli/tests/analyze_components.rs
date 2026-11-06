//! `patterns` and `hotspots` on the script regions of Vue, Svelte and Astro
//! component files, through the built binary: the functions measured, the
//! regions not analyzed, and the status of a file whose script code is all
//! in a language that is not parsed.

use act_lite_parser::regions::{RegionKind, RegionLanguage, RegionState, view};
use act_lite_parser::{Grammar, SourceFile};
use assert_cmd::Command;
use serde_json::{Value, json};
use std::fs;
use std::path::Path;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/components");

const CHECK: [&str; 5] = [
    "function check(a: number, b: number, c: number, d: number, e: number, f: number): number {",
    "  const s = a * b - c * d",
    "  const t = s * 2",
    "  return t + e + f",
    "}",
];
const LEGACY: &str = "<script lang=\"coffee\">\nx = 1\n</script>\n";

/// Runs `act-lite --workspace <root> analyze <analyzer> <args>`: the parsed
/// stdout, or the message of the error frame on stderr.
fn run(root: &Path, analyzer: &str, args: &[&str]) -> Result<Value, String> {
    let config = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("act-lite")
        .expect("the act-lite binary must be built")
        .env("ACT_CONFIG_DIR", config.path())
        .arg("--workspace")
        .arg(root)
        .args(["analyze", analyzer, "--limit", "1000"])
        .args(args)
        .output()
        .unwrap();
    if out.status.success() {
        return Ok(serde_json::from_slice(&out.stdout).expect("stdout is one JSON value"));
    }
    let frame: Value = serde_json::from_slice(&out.stderr).expect("stderr is one JSON value");
    Err(frame["error"]["message"].as_str().unwrap().to_string())
}

fn patterns(root: &Path, args: &[&str]) -> Value {
    run(root, "patterns", args).unwrap()
}

fn hotspots(root: &Path, args: &[&str]) -> Value {
    run(root, "hotspots", args).unwrap()
}

fn workspace(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (rel, content) in files {
        let path = dir.path().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    dir
}

fn lines(lines: &[&str]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// The analyzer workspace: `check` in a TypeScript file and in a script
/// region of each framework, and a Vue file whose script is CoffeeScript.
fn analyzer_workspace() -> tempfile::TempDir {
    let util = format!("export {}", lines(&CHECK));
    let indented: Vec<String> = CHECK.iter().map(|l| format!("  {l}")).collect();
    let indented: Vec<&str> = indented.iter().map(String::as_str).collect();
    let mut vue = vec![
        "<template>",
        "  <p>{{ a && b && c && d && e && f && g && h && i && j && k && l }}</p>",
        "</template>",
        "<script setup lang=\"ts\">",
        "const a = 1",
        "if (a) {} else if (a) {} else if (a) {} else if (a) {} else if (a) {} else if (a) {}",
    ];
    vue.extend(CHECK);
    vue.push("</script>");
    let mut svelte = vec!["<script lang=\"ts\">"];
    svelte.extend(indented);
    svelte.extend(["</script>", "<p>{check(1, 2, 3, 4, 5, 6)}</p>"]);
    let mut astro = vec!["---"];
    astro.extend(CHECK);
    astro.extend(["---", "<p>{check(1, 2, 3, 4, 5, 6)}</p>"]);
    workspace(&[
        ("util.ts", &util),
        ("Big.vue", &lines(&vue)),
        ("Big.svelte", &lines(&svelte)),
        ("Big.astro", &lines(&astro)),
        ("Legacy.vue", LEGACY),
    ])
}

fn legacy_entry(file: &str) -> Value {
    json!({"file": file, "range": "1:1-1:23", "reason": "unsupported script language: coffee"})
}

/// The `(file, range)` of each item.
fn places(items: &Value) -> Vec<(String, String)> {
    items
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["file"].as_str().unwrap().to_string(),
                f["range"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn place(file: &str, range: &str) -> (String, String) {
    (file.to_string(), range.to_string())
}

#[test]
fn each_analyzer_lists_the_three_entries_as_supported() {
    let dir = analyzer_workspace();
    for analyzer in ["patterns", "hotspots", "clones"] {
        let out = run(dir.path(), analyzer, &[]).unwrap();
        assert_eq!(
            out["coverage"]["languages"],
            json!([
                {"language": "Astro", "files": 1, "supported": true},
                {"language": "Svelte", "files": 1, "supported": true},
                {"language": "TypeScript", "files": 1, "supported": true},
                {"language": "Vue", "files": 2, "supported": true}
            ]),
            "{analyzer}"
        );
    }
}

#[test]
fn patterns_measures_check_in_every_script_region_at_component_locations() {
    let dir = analyzer_workspace();
    let out = patterns(dir.path(), &[]);
    let findings = out["findings"].as_array().unwrap();
    assert_eq!(
        places(&out["findings"]),
        vec![
            place("Big.astro", "2:1-6:2"),
            place("Big.svelte", "2:3-6:4"),
            place("Big.vue", "7:1-11:2"),
            place("util.ts", "1:8-5:2"),
        ]
    );
    for f in findings {
        assert_eq!(f["rule"], "long_parameter_list", "{f}");
        assert_eq!(f["function"], "check", "{f}");
        assert_eq!(f["value"], 6, "{f}");
    }
    assert_eq!(out["status"], "findings");
    assert_eq!(out["complete"], true);
    assert_eq!(out["coverage"]["analyzed_files"], 4);
    assert_eq!(out["details"]["functions"], 4);
    assert_eq!(
        out["details"]["regions_not_analyzed"],
        json!([legacy_entry("Legacy.vue")])
    );
    assert_eq!(out["details"]["regions_not_analyzed_total"], 1);
    assert_eq!(out["details"]["files_without_analyzed_code"], 1);
}

#[test]
fn hotspots_gives_one_item_per_component_file_over_its_regions() {
    let dir = analyzer_workspace();
    let out = hotspots(dir.path(), &[]);
    let mut files: Vec<&str> = out["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["file"].as_str().unwrap())
        .collect();
    files.sort_unstable();
    assert_eq!(files, ["Big.astro", "Big.svelte", "Big.vue", "util.ts"]);
    let vue = out["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["file"] == "Big.vue")
        .unwrap();
    assert_eq!(vue["score"], 5);
    assert_eq!(vue["functions"], 1);
    assert_eq!(vue["top_functions"][0]["range"], "7:1-11:2");
    assert_eq!(out["details"]["functions"], 4);
    assert_eq!(
        out["details"]["regions_not_analyzed"],
        json!([legacy_entry("Legacy.vue")])
    );
    assert_eq!(out["details"]["regions_not_analyzed_total"], 1);
    assert_eq!(out["details"]["files_without_analyzed_code"], 1);
}

#[test]
fn clones_details_gain_the_region_keys() {
    let dir = analyzer_workspace();
    let out = run(dir.path(), "clones", &[]).unwrap();
    assert_eq!(
        out["details"]["regions_not_analyzed"],
        json!([legacy_entry("Legacy.vue")])
    );
    assert_eq!(out["details"]["regions_not_analyzed_total"], 1);
    assert_eq!(out["details"]["files_without_analyzed_code"], 1);
}

#[test]
fn template_code_and_top_level_region_code_are_not_measured() {
    let dir = analyzer_workspace();
    let out = patterns(dir.path(), &[]);
    let on_vue: Vec<&Value> = out["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["file"] == "Big.vue")
        .collect();
    assert_eq!(on_vue.len(), 1, "{on_vue:?}");
    for f in on_vue {
        let range = f["range"].as_str().unwrap();
        assert!(!range.starts_with("2:") && !range.starts_with("6:"), "{f}");
    }
}

#[test]
fn a_file_whose_script_code_is_all_unsupported_alone_is_unsupported() {
    let dir = workspace(&[("Legacy.vue", LEGACY)]);
    let out = patterns(dir.path(), &[]);
    assert_eq!(out["status"], "unsupported");
    assert_eq!(out["complete"], true);
    assert_eq!(out["coverage"]["analyzed_files"], 0);
    assert_eq!(
        out["coverage"]["languages"],
        json!([{"language": "Vue", "files": 1, "supported": true}])
    );
    assert_eq!(out["details"]["files_without_analyzed_code"], 1);
    assert_eq!(out["details"]["functions"], 0);
    assert_eq!(
        out["details"]["regions_not_analyzed"],
        json!([legacy_entry("Legacy.vue")])
    );
    assert_eq!(out["incomplete"]["parse_failures"], json!([]));
}

#[test]
fn a_file_whose_script_code_is_all_unsupported_beside_a_typescript_file_is_no_findings() {
    let dir = workspace(&[
        ("Legacy.vue", LEGACY),
        (
            "src/ok.ts",
            "export function ok(a: number): number {\n  return a\n}\n",
        ),
    ]);
    let out = patterns(dir.path(), &[]);
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["coverage"]["analyzed_files"], 1);
    assert_eq!(out["details"]["files_without_analyzed_code"], 1);
    assert_eq!(
        out["details"]["regions_not_analyzed"],
        json!([legacy_entry("Legacy.vue")])
    );
}

#[test]
fn a_file_whose_script_code_is_all_unsupported_beside_a_test_file_is_the_empty_scope_error() {
    let dir = workspace(&[
        ("Legacy.vue", LEGACY),
        (
            "src/__tests__/B.vue",
            "<script setup lang=\"ts\">\nfunction b() {}\n</script>\n",
        ),
    ]);
    for analyzer in ["patterns", "hotspots", "clones"] {
        let err = run(dir.path(), analyzer, &[]).unwrap_err();
        assert!(
            err.starts_with("no files to analyze in the requested scope: 1 excluded as tests"),
            "{analyzer}: {err}"
        );
    }
}

#[test]
fn a_typescript_file_keeps_its_findings_beside_a_component_file() {
    let ok = "export function ok(a: number, b: number, c: number, d: number, e: number, f: number) {\n  return a\n}\n";
    let component = "<script setup lang=\"ts\">\nfunction v(a: number, b: number, c: number, d: number, e: number, f: number) {\n  return a\n}\n</script>\n";
    let alone = workspace(&[("src/ok.ts", ok)]);
    let beside = workspace(&[("src/ok.ts", ok), ("src/A.vue", component)]);
    let of = |out: &Value, file: &str| -> Vec<Value> {
        out["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["file"] == file)
            .cloned()
            .collect()
    };
    let alone = patterns(alone.path(), &["src"]);
    let beside = patterns(beside.path(), &["src"]);
    assert_eq!(of(&alone, "src/ok.ts").len(), 1);
    assert_eq!(of(&alone, "src/ok.ts"), of(&beside, "src/ok.ts"));
    assert_eq!(
        places(&json!(of(&beside, "src/A.vue"))),
        vec![place("src/A.vue", "2:1-4:2")]
    );
}

#[test]
fn malformed_and_broken_regions_through_patterns() {
    let dir = workspace(&[
        (
            "Unclosed.vue",
            "<script setup lang=\"ts\">\nconst a = 1\n\n<template>\n  <p>{{ a }}</p>\n</template>\n",
        ),
        ("Unclosed.astro", "---\nconst a = 1;\n<p>{a}</p>\n"),
        (
            "BadTs.vue",
            "<template><p/></template>\n<script lang=\"ts\">\nfunction f(a: number {\n  return a\n}\nfunction g(b: number) { return b }\n</script>\n",
        ),
        (
            "Regex.astro",
            "---\nfunction e(v) { return v.replace(/\"/g, \"&quot;\"); }\n---\n<p/>\n",
        ),
    ]);
    let out = patterns(dir.path(), &[]);
    let failures = places(&out["incomplete"]["parse_failures"]);
    assert_eq!(failures.len(), 4, "{failures:?}");
    assert_eq!(failures[0], place("BadTs.vue", "3:21-3:21"));
    assert_eq!(failures[1].0, "Regex.astro");
    assert!(failures[1].1.starts_with("1:4-"), "{failures:?}");
    assert_eq!(failures[2].0, "Unclosed.astro");
    assert!(failures[2].1.starts_with("3:"), "{failures:?}");
    assert_eq!(failures[3].0, "Unclosed.vue");
    assert!(failures[3].1.starts_with("1:1-"), "{failures:?}");
    assert_eq!(out["complete"], false);
    assert_eq!(
        out["details"]["regions_not_analyzed"],
        json!([
            {"file": "Regex.astro", "range": "1:1-1:4", "reason": "malformed region"},
            {"file": "Unclosed.astro", "range": "1:1-1:4", "reason": "malformed region"},
            {"file": "Unclosed.vue", "range": "1:1-1:25", "reason": "malformed region"}
        ])
    );
    assert_eq!(out["details"]["regions_not_analyzed_total"], 3);
    assert_eq!(out["details"]["files_without_analyzed_code"], 0);
    // `BadTs.vue` is analyzed on its recovered tree.
    assert_eq!(out["details"]["functions"], 2);
}

#[test]
fn a_function_outside_an_outer_error_is_measured() {
    let dir = workspace(&[(
        "Amp.svelte",
        "<p>Set & go</p>\n<script>function f() {}</script>\n",
    )]);
    let out = hotspots(dir.path(), &[]);
    assert_eq!(
        places(&out["incomplete"]["parse_failures"]),
        vec![place("Amp.svelte", "1:8-1:12")]
    );
    assert_eq!(out["details"]["functions"], 1);
    assert_eq!(out["findings"][0]["top_functions"][0]["function"], "f");
    assert_eq!(out["findings"][0]["top_functions"][0]["range"], "2:9-2:24");
}

#[test]
fn empty_regions_are_analyzed_with_no_function() {
    let dir = workspace(&[(
        "Empty.astro",
        "---\n---\n<script src=\"/x.js\"/>\n<style></style>\n",
    )]);
    let out = patterns(dir.path(), &["Empty.astro"]);
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["coverage"]["analyzed_files"], 1);
    assert_eq!(out["details"]["regions_not_analyzed"], json!([]));
    assert_eq!(out["details"]["regions_not_analyzed_total"], 0);
    assert_eq!(out["details"]["files_without_analyzed_code"], 0);
}

#[test]
fn an_error_in_a_style_region_is_not_a_parse_failure() {
    let dir = workspace(&[(
        "Style.vue",
        "<script setup lang=\"ts\">\nfunction a() {}\n</script>\n<style>\n.a { color: red; } } {\n</style>\n",
    )]);
    let out = patterns(dir.path(), &[]);
    assert_eq!(out["incomplete"]["parse_failures"], json!([]));
    assert_eq!(out["status"], "no_findings");
    assert_eq!(out["details"]["functions"], 1);
    assert_eq!(out["details"]["regions_not_analyzed"], json!([]));
}

#[test]
fn files_out_of_scope_are_not_listed() {
    let dir = workspace(&[
        ("src/ok.ts", "export function ok() {}\n"),
        ("src/__tests__/L.vue", LEGACY),
        ("other/Legacy.vue", LEGACY),
    ]);
    let out = patterns(dir.path(), &["src"]);
    assert_eq!(out["details"]["regions_not_analyzed"], json!([]));
    assert_eq!(out["details"]["regions_not_analyzed_total"], 0);
    assert_eq!(out["details"]["files_without_analyzed_code"], 0);
    let all = patterns(dir.path(), &[]);
    assert_eq!(
        all["details"]["regions_not_analyzed"],
        json!([legacy_entry("other/Legacy.vue")])
    );
}

#[test]
fn the_list_is_cut_at_30_with_its_total() {
    let files: Vec<(String, &str)> = (0..31).map(|i| (format!("L{i:02}.vue"), LEGACY)).collect();
    let files: Vec<(&str, &str)> = files.iter().map(|(r, c)| (r.as_str(), *c)).collect();
    let mut files = files;
    files.push(("ok.ts", "export function ok() {}\n"));
    let dir = workspace(&files);
    let out = patterns(dir.path(), &[]);
    assert_eq!(
        out["details"]["regions_not_analyzed"]
            .as_array()
            .unwrap()
            .len(),
        30
    );
    assert_eq!(
        out["details"]["regions_not_analyzed"][0],
        legacy_entry("L00.vue")
    );
    assert_eq!(out["details"]["regions_not_analyzed_total"], 31);
    assert_eq!(out["details"]["files_without_analyzed_code"], 31);
}

/// The extension of a view file of a region of `grammar`.
fn extension(grammar: Grammar) -> &'static str {
    match grammar {
        Grammar::JavaScript => "js",
        Grammar::Jsx => "jsx",
        Grammar::TypeScript => "ts",
        Grammar::Tsx => "tsx",
        other => panic!("not a script language: {other:?}"),
    }
}

/// `(start line, start column, end line, end column)` of an `a:b-c:d` range.
fn bounds(range: &str) -> (u32, u32, u32, u32) {
    let n: Vec<u32> = range
        .split(['-', ':'])
        .map(|s| s.parse().unwrap())
        .collect();
    (n[0], n[1], n[2], n[3])
}

fn inside(range: &str, outer: (u32, u32, u32, u32)) -> bool {
    let r = bounds(range);
    (r.0, r.1) >= (outer.0, outer.1) && (r.2, r.3) <= (outer.2, outer.3)
}

/// The items of `items` whose `range` lies in `outer`, without `file`.
fn in_region(items: &[Value], outer: (u32, u32, u32, u32)) -> Vec<Value> {
    items
        .iter()
        .filter(|f| inside(f["range"].as_str().unwrap(), outer))
        .map(|f| {
            let mut f = f.clone();
            f.as_object_mut().unwrap().remove("file");
            f
        })
        .collect()
}

fn top_functions(out: &Value) -> Vec<Value> {
    out["findings"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f["top_functions"].as_array().unwrap().clone())
        .collect()
}

#[test]
fn region_results_equal_those_of_the_region_view() {
    let mut checked = 0;
    for name in ["Counter.vue", "Counter.svelte", "Card.astro"] {
        let text = fs::read_to_string(Path::new(FIXTURES).join(name)).unwrap();
        let grammar = Grammar::from_path(Path::new(name)).unwrap();
        let source = SourceFile::from_content(name, text.clone(), grammar).unwrap();
        let component = workspace(&[(name, &text)]);
        let c_patterns = patterns(component.path(), &[]);
        let c_hotspots = hotspots(component.path(), &[]);
        let c_findings = c_patterns["findings"].as_array().unwrap().clone();
        let c_top = top_functions(&c_hotspots);
        for region in source.regions() {
            if region.kind == RegionKind::Style || region.state != RegionState::WellFormed {
                continue;
            }
            let RegionLanguage::Supported(language) = &region.language else {
                continue;
            };
            let view_name = format!("view.{}", extension(*language));
            let view_dir = workspace(&[(&view_name, &view(&text, region))]);
            let v_patterns = patterns(view_dir.path(), &[]);
            let v_hotspots = hotspots(view_dir.path(), &[]);
            let e = &region.element;
            let outer = (e.start_line(), e.start_col(), e.end_line(), e.end_col());
            let all = (1, 1, u32::MAX, u32::MAX);
            assert_eq!(
                in_region(&c_findings, outer),
                in_region(v_patterns["findings"].as_array().unwrap(), all),
                "{name} {outer:?}"
            );
            let view_top = in_region(&top_functions(&v_hotspots), all);
            assert_eq!(in_region(&c_top, outer), view_top, "{name} {outer:?}");
            checked += view_top.len();
        }
    }
    // `increment` twice, `shout` and `hello`.
    assert_eq!(checked, 4);
}

#[test]
fn whole_details_of_a_component_workspace() {
    let dir = analyzer_workspace();
    let out = hotspots(dir.path(), &[]);
    assert_eq!(
        out["details"],
        json!({
            "formula": "score(file) = sum over functions f in file of complexity(f) * lines(f)",
            "complexity": "cyclomatic: 1 + decision points in the function's own body",
            "lines": "lines with a non-comment token of the function's own body, nested functions and Python docstrings excluded",
            "dependency_information": false,
            "functions": 4,
            "regions_not_analyzed": [legacy_entry("Legacy.vue")],
            "regions_not_analyzed_total": 1,
            "files_without_analyzed_code": 1
        })
    );
}

fn clones(root: &Path, args: &[&str]) -> Value {
    run(root, "clones", args).unwrap()
}

/// The `(file, range)` of each member of each class.
fn classes(out: &Value) -> Vec<Vec<(String, String)>> {
    out["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| places(&f["members"]))
        .collect()
}

#[test]
fn clones_finds_check_in_every_script_region_and_the_typescript_file() {
    let dir = analyzer_workspace();
    let out = clones(dir.path(), &[]);
    assert_eq!(out["total"], 1, "{out}");
    let finding = &out["findings"][0];
    assert_eq!(finding["kind"], "exact");
    assert_eq!(finding["language"], "TypeScript");
    assert_eq!(finding["tokens"], 53);
    assert_eq!(finding["members_total"], 4);
    assert_eq!(
        classes(&out),
        vec![vec![
            place("Big.astro", "2:1-6:2"),
            place("Big.svelte", "2:3-6:4"),
            place("Big.vue", "7:1-11:2"),
            place("util.ts", "1:8-5:2"),
        ]]
    );
    assert_eq!(out["complete"], true);
}

#[test]
fn clones_focus_on_a_component_file_reports_its_class_and_lists_every_region_not_analyzed() {
    let dir = analyzer_workspace();
    let out = clones(dir.path(), &["--focus", "Big.vue"]);
    assert_eq!(out["total"], 1, "{out}");
    assert_eq!(out["details"]["focus"], json!(["Big.vue"]));
    assert_eq!(out["details"]["focus_not_analyzed"], json!([]));
    assert_eq!(
        classes(&out),
        vec![vec![
            place("Big.vue", "7:1-11:2"),
            place("Big.astro", "2:1-6:2"),
            place("Big.svelte", "2:3-6:4"),
            place("util.ts", "1:8-5:2"),
        ]]
    );
    assert_eq!(
        out["details"]["regions_not_analyzed"],
        json!([legacy_entry("Legacy.vue")])
    );
    assert_eq!(out["details"]["regions_not_analyzed_total"], 1);
}

/// Template markup of more than 50 tokens.
fn markup() -> String {
    (0..10)
        .map(|i| format!("  <p class=\"row\" id=\"r{i}\">text {i}</p>\n"))
        .collect()
}

#[test]
fn template_markup_is_never_a_clone_candidate() {
    let template = format!("<template>\n<div>\n{}</div>\n</template>\n", markup());
    let svelte = format!("<div>\n{}</div>\n", markup());
    let astro = format!("---\n---\n<div>\n{}</div>\n", markup());
    let dir = workspace(&[
        ("A.vue", &template),
        ("B.vue", &template),
        ("A.svelte", &svelte),
        ("B.svelte", &svelte),
        ("A.astro", &astro),
        ("B.astro", &astro),
    ]);
    let out = clones(dir.path(), &[]);
    assert_eq!(out["status"], "no_findings", "{out}");
    assert_eq!(out["details"]["candidates"], 0);

    // The same markup beside a duplicated script region: only the region
    // function is a class.
    let script = format!("<script setup lang=\"ts\">\n{}</script>\n", lines(&CHECK));
    let with_script = format!("{template}{script}");
    let dir = workspace(&[("A.vue", &with_script), ("B.vue", &with_script)]);
    let out = clones(dir.path(), &[]);
    assert_eq!(
        classes(&out),
        vec![vec![
            place("A.vue", "16:1-20:2"),
            place("B.vue", "16:1-20:2")
        ]],
        "{out}"
    );
    assert_eq!(out["findings"][0]["language"], "TypeScript");
}

#[test]
fn region_clone_candidates_equal_those_of_the_region_view() {
    let fixture = |name: &str| fs::read_to_string(Path::new(FIXTURES).join(name)).unwrap();
    let big = analyzer_workspace();
    let analyzer = |name: &str| fs::read_to_string(big.path().join(name)).unwrap();
    let mix = format!(
        "<script>\n{}</script>\n",
        lines(&[
            "function mix(a, b, c, d, e, f) {",
            "  const s = a * b - c * d + e * f",
            "  const t = s * 2 - a + b - c + d",
            "  return t + e + f - s * 3",
            "}",
        ])
    );
    // Each component file and the ranges of the functions of 50 tokens or
    // more in its script regions.
    let files: Vec<(&str, String, Vec<&str>)> = vec![
        ("Counter.vue", fixture("Counter.vue"), vec![]),
        ("Counter.svelte", fixture("Counter.svelte"), vec![]),
        ("Card.astro", fixture("Card.astro"), vec![]),
        ("Big.vue", analyzer("Big.vue"), vec!["7:1-11:2"]),
        ("Big.svelte", analyzer("Big.svelte"), vec!["2:3-6:4"]),
        ("Big.astro", analyzer("Big.astro"), vec!["2:1-6:2"]),
        ("Mix.vue", mix, vec!["2:1-6:2"]),
    ];
    let mut checked = 0;
    let mut regions = 0;
    for (name, text, expected) in &files {
        let grammar = Grammar::from_path(Path::new(name)).unwrap();
        let source = SourceFile::from_content(*name, text.clone(), grammar).unwrap();
        let mut found = Vec::new();
        for region in source.regions() {
            if region.kind == RegionKind::Style || region.state != RegionState::WellFormed {
                continue;
            }
            let RegionLanguage::Supported(language) = &region.language else {
                continue;
            };
            regions += 1;
            let view_name = format!("view.{}", extension(*language));
            let dir = workspace(&[(name, text), (&view_name, &view(text, region))]);
            let out = clones(dir.path(), &[]);
            for class in classes(&out) {
                assert_eq!(class.len(), 2, "{name}: {class:?}");
                let (c, v) = (&class[0], &class[1]);
                assert_eq!(c.0, *name, "{class:?}");
                assert_eq!(v.0, view_name, "{class:?}");
                assert_eq!(c.1, v.1, "{class:?}");
                let e = &region.element;
                let outer = (e.start_line(), e.start_col(), e.end_line(), e.end_col());
                assert!(inside(&c.1, outer), "{name}: {class:?}");
                found.push(c.1.clone());
            }
        }
        assert_eq!(&found, expected, "{name}");
        checked += found.len();
    }
    assert_eq!(regions, 11);
    assert_eq!(checked, 4);
}

/// A Vue file with two script regions that hold the same function and
/// `n` script regions in an unsupported language.
fn clone_and_unsupported(n: usize) -> String {
    let check = "function check(a, b, c, d, e, f) {\n  const s = a + b + c + d + e + f;\n  if (s > 10) { return s * 2; }\n  return s - 1;\n}\n";
    let mut text = format!(
        "<script>\n{check}</script>\n<script>\n{}</script>\n",
        check.replace("check", "check2")
    );
    for _ in 0..n {
        text.push_str("<script lang=\"coffee\">x = 1</script>\n");
    }
    text
}

#[test]
fn a_clone_reread_lists_each_region_not_analyzed_once() {
    let ws = workspace(&[("M.vue", &clone_and_unsupported(200))]);
    let out = run(ws.path(), "clones", &[]).unwrap();
    assert_eq!(out["total"], 1, "{out}");
    assert_eq!(out["details"]["regions_not_analyzed_total"], 200, "{out}");
    assert_eq!(
        out["details"]["regions_not_analyzed"]
            .as_array()
            .unwrap()
            .len(),
        30
    );
}

#[test]
fn a_clone_reread_stays_linear_in_the_regions_not_analyzed() {
    if std::env::var("ACT_LITE_CHECK_PERF").as_deref() != Ok("1") {
        eprintln!("skipped: set ACT_LITE_CHECK_PERF=1 to time 64,000 unsupported regions");
        return;
    }
    let ws = workspace(&[("M.vue", &clone_and_unsupported(64_000))]);
    run(ws.path(), "clones", &[]).unwrap();
    let started = std::time::Instant::now();
    let out = run(ws.path(), "clones", &[]).unwrap();
    let elapsed = started.elapsed();
    assert_eq!(out["details"]["regions_not_analyzed_total"], 64_000);
    assert!(elapsed.as_secs_f64() < 2.0, "{elapsed:?}");
}
