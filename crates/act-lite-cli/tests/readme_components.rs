//! The README's documentation of Vue, Svelte and Astro component files: the
//! capability rows, the support table with each framework's level, one
//! statement for each rule and limit, and the section's examples, which run
//! on the built binary over the committed component fixtures. Every expected
//! value is written here from the component rules, not read back from the
//! README.

#[allow(dead_code)] // the MCP helpers of the shared module are not used here
mod langs_added;

use langs_added::{cli, cli_json};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const FRAMEWORKS: [&str; 3] = ["Vue", "Svelte", "Astro"];

const HEADING: &str = "### Vue, Svelte and Astro components";

fn readme_source() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md"))
        .unwrap()
        .replace("\r\n", "\n")
}

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The component section: from its heading to the next `##` or `###`
/// heading, as written.
fn section_source() -> String {
    let readme = readme_source();
    let mut lines = readme.lines().skip_while(|l| *l != HEADING);
    assert!(lines.next().is_some(), "README has no `{HEADING}` section");
    lines
        .take_while(|l| !l.starts_with("## ") && !l.starts_with("### "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The component section with each run of whitespace collapsed to one space.
fn section() -> String {
    flat(&section_source())
}

fn must_state(statements: &[&str]) {
    let section = section();
    for statement in statements {
        assert!(
            section.contains(statement),
            "the component section lacks: {statement}"
        );
    }
}

/// The rows of the first table in `text` whose header line starts with
/// `header`, each as its trimmed cells.
fn table(text: &str, header: &str) -> Vec<Vec<String>> {
    let mut lines = text
        .lines()
        .map(str::trim)
        .skip_while(|l| !l.starts_with(header));
    assert!(lines.next().is_some(), "no table `{header}`");
    lines
        .skip(1)
        .take_while(|l| l.starts_with('|'))
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect()
        })
        .collect()
}

fn components_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/components")
        .join(name)
}

#[test]
fn the_capability_table_rows_of_the_component_entries() {
    let rows = table(
        &readme_source(),
        "| Language | `skeleton`, `symbols` | `definition` | `references` |",
    );
    for name in FRAMEWORKS {
        let row = rows
            .iter()
            .find(|r| r[0] == name)
            .unwrap_or_else(|| panic!("no capability row for {name}"));
        assert_eq!(
            row[1..],
            [
                "yes (structure and regions)",
                "name-level (regions; HTML rules in templates)",
                "name-level",
                "needs a language server",
                "yes (script regions)",
                "yes (script regions)",
                "yes (script regions)",
            ],
            "{name}"
        );
    }
}

#[test]
fn the_support_table_gives_each_framework_its_level() {
    let rows = table(&section_source(), "| | Vue | Svelte | Astro |");
    let row = |label: &str| -> Vec<String> {
        let found = rows
            .iter()
            .find(|r| r[0] == label)
            .unwrap_or_else(|| panic!("support table has no `{label}` row"));
        assert_eq!(found.len(), 4, "{found:?}");
        found[1..].to_vec()
    };
    assert_eq!(row("Level"), ["`script analysis`"; 3]);
    assert_eq!(
        row("Dependency analyzers"),
        ["not supported (`supported: false`)"; 3]
    );
    assert_eq!(row("Files"), ["`.vue`", "`.svelte`", "`.astro`"]);
    assert_eq!(
        row("Framework grammar"),
        [
            "`tree-sitter-vue-next` 0.1.0",
            "`tree-sitter-svelte-ng` 1.0.2",
            "`tree-sitter-astro-next` 0.1.1",
        ]
    );
    assert_eq!(row("`references`, `definition`"), ["name-level"; 3]);
    let analyzers = row("Code-health analyzers");
    for cell in &analyzers {
        assert!(cell.starts_with("yes ("), "{cell}");
        assert!(cell.ends_with("script regions)"), "{cell}");
    }
    assert!(analyzers[2].contains("frontmatter"), "{analyzers:?}");
    let scripts = row("Script regions");
    assert!(scripts[0].contains("`<script setup>`"), "{scripts:?}");
    assert!(scripts[0].contains("TSX"), "{scripts:?}");
    assert!(scripts[1].contains("module script"), "{scripts:?}");
    assert!(
        scripts[2].contains("frontmatter (TypeScript)") && scripts[2].contains("data blocks"),
        "{scripts:?}"
    );
    let styles = row("Style regions");
    assert!(styles[0].contains("top-level") && styles[2].contains("any depth"));
    let unread = row("Not read as code");
    assert!(
        unread[0].contains("custom blocks") && unread[0].contains("`<template lang=\"pug\">`"),
        "{unread:?}"
    );
    assert!(unread[1].contains("`<script>` inside markup"), "{unread:?}");
    assert!(unread[2].contains("attribute expressions"), "{unread:?}");
    must_state(&[
        "The level `script analysis` means: the structure, the declarations of the script and style regions, name-level `references` and `definition`, and the three code-health analyzers on the script regions.",
        "A framework at `structural only` would give the structure without region declarations or analyzer support; none of the three is at that level.",
    ]);
}

#[test]
fn the_region_elements_and_language_rules_are_stated() {
    must_state(&[
        "A Vue `<script>` or `<style>` below the top level is not a region (Vue does not process it), and neither is a custom block (`<i18n>`, `<docs>`, or any other top-level element that is not `<template>`, `<script>` or `<style>`).",
        "A Svelte `<script>` or `<style>` inside markup, for example in `<svelte:head>`, is not a region: Svelte passes it to the page.",
        "An Astro frontmatter runs from the opening `---` to the next `---` line.",
        "Each region is a separate unit, with its own parse and its own scope: two script regions of one file are never parsed together.",
        "a file that breaks them is analyzed region by region, without an error.",
        "Any other `lang` value (`coffee`, `less`, `sass`, `stylus`, `pug`, …) is a language act-lite does not read: the element is reported as a block and its content is not parsed.",
        "Attribute values are compared byte for byte and case-sensitively, except Astro's `type`, which follows HTML: surrounding whitespace is removed and the comparison ignores ASCII case. When an element has the attribute twice, the first one counts.",
    ]);
    let rows = table(&section_source(), "| Region | Attribute | Language |");
    let expected: [(&str, &str, &str); 8] = [
        (
            "Vue `<script>`",
            "no `lang`, or `lang=\"js\"` / `\"jsx\"` / `\"ts\"` / `\"tsx\"`",
            "JavaScript / JSX / TypeScript / TSX",
        ),
        (
            "Svelte `<script>`",
            "no `lang`, or `lang=\"js\"` / `\"ts\"`",
            "JavaScript / TypeScript",
        ),
        ("Astro frontmatter", "none", "TypeScript"),
        (
            "Astro `<script>`",
            "no attribute, or only `src` (Astro processes the script)",
            "TypeScript",
        ),
        (
            "Astro `<script>`",
            "any other attribute, such as `is:inline`, `type=\"module\"` or `define:vars` (Astro sends the script unchanged)",
            "JavaScript",
        ),
        (
            "Astro `<script>`",
            "a `type` that is not empty, not `module` and not a JavaScript MIME type, such as `application/ld+json`",
            "a data block: reported as a block, not code",
        ),
        (
            "`<style>`",
            "no `lang`, or `lang=\"css\"` / `\"scss\"`",
            "CSS / SCSS",
        ),
        (
            "Vue `<template>`",
            "no `lang`, or `lang=\"html\"`",
            "its elements are structure",
        ),
    ];
    let found: Vec<(&str, &str, &str)> = rows
        .iter()
        .map(|r| (r[0].as_str(), r[1].as_str(), r[2].as_str()))
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn empty_and_malformed_regions_are_stated() {
    must_state(&[
        "An element with no content (`<style></style>`, `<script src=\"x.js\"></script>`, Astro's `<script src=\"x.js\"/>`, an empty frontmatter) is reported as a block and is not parsed.",
        "A region is malformed when its start tag, its content or its end tag is missing (an unclosed `<script>`, a frontmatter without its closing `---`), or when it lies inside a syntax error of the framework grammar's tree.",
        "A malformed region is not parsed, because its content can run to the end of the file and hold the template.",
        "Navigation reports nothing for it and nothing inside that syntax error, the file carries `parse_error: true` and a parse failure, and the analyzers list the region in `details.regions_not_analyzed`.",
        "the analyzers list it with `parse timeout`, and navigation reports its block with no declarations and `parse_error: true`.",
    ]);
}

#[test]
fn the_structure_conventions_are_stated() {
    must_state(&[
        "`skeleton`, `symbols`, `symbols-batch` and `repo-outline` report, in source order (an enclosing block before its content):",
        "Each region element, each Vue top-level `<template>` and custom block, and each Astro data block is a `module`, named by its start tag as written, cut to its first line (`<script setup lang=\"ts\">`; a frontmatter is `---`). In `skeleton` its range is the element; in `symbols` it is the start tag.",
        "Each element of a Vue `<template>`, of Svelte markup and of an Astro template is an `element`, named by its tag as written (`div`, `Layout`, `svelte:head`), and each literal `id` value and each token of a literal `class` value is a `field`. In `symbols`, an element is a `class`, as in HTML.",
        "A dynamic attribute (`:class`, `v-bind:id`, `class={x}`, `class:list={…}`) is not reported, nor is any token of a value that holds an expression (`class=\"a {x}\"`).",
        "Each `{#snippet name(…)}` is a `function`. Other template blocks (`{#if}`, `{#each}`, `{#await}`, `{#key}`, `v-if`, `v-for`, slots) are not declarations.",
        "Each well-formed script and style region of a supported language gives the declarations and symbols that a `.js`, `.jsx`, `.ts`, `.tsx`, `.css` or `.scss` file with the same code gives, at their places in the component file.",
        "`parse_error: true` is set when the framework grammar's tree or any region's tree holds a syntax error.",
        "`repo-outline` lists the file under `Vue`, `Svelte` or `Astro`.",
        "`callers`, `callees` and `implementations` fail with `query type '<q>' needs a language server, and act-lite has none configured for Vue files` (or Svelte, or Astro files).",
    ]);
}

#[test]
fn the_reference_rules_are_stated() {
    must_state(&["Every answer is a name match (`\"source\": \"tree-sitter\"`)."]);
    let rows = table(
        &section_source(),
        "| Position | `references` | `definition` |",
    );
    let positions: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    assert_eq!(
        positions,
        [
            "In a script or style region of language L",
            "In the template, outside an expression",
            "In a template expression, or in a `<script>` or `<style>` that is not parsed",
        ]
    );
    assert!(
        rows[0][1].contains("every region of L in the files of the same framework")
            && rows[0][1].contains("every plain file of L in the workspace"),
        "{:?}",
        rows[0]
    );
    assert!(rows[0][2].contains("nearest declaration"), "{:?}", rows[0]);
    assert!(
        rows[1][1].contains("HTML's rules")
            && rows[1][1].contains("Script regions are not searched."),
        "{:?}",
        rows[1]
    );
    assert!(
        rows[1][2].contains("An `id` value is its own definition"),
        "{:?}",
        rows[1]
    );
    assert!(
        rows[2][1].contains("`symbol not found: <name> in <file>`"),
        "{:?}",
        rows[2]
    );
    assert!(
        rows[2][2].contains("`No symbol found at <file>:<line>:<column>`"),
        "{:?}",
        rows[2]
    );
    must_state(&[
        "From a plain file (`.ts`, `.js`), `references` and `definition` do not read component files, so the uses of a TypeScript export inside components are found only from a component file.",
        "MCP `references` takes `symbol` and `file`: in a component file it uses the first region symbol with that name, then the first template symbol, else the first whole-word match outside template expressions.",
    ]);
}

#[test]
fn the_analyzers_on_script_regions_are_stated() {
    must_state(&[
        "`analyze_patterns`, `analyze_hotspots` and `analyze_clones` analyze the well-formed script and frontmatter regions in JavaScript, JSX, TypeScript or TSX. They never analyze style regions or the template.",
        "A component file counts once in `coverage.languages`, under `Vue`, `Svelte` or `Astro`.",
        "the top-level code of a region (the body of `<script setup>`, of a Svelte instance script or of an Astro frontmatter) is not a function, and only the functions written in it are measured.",
        "A hotspot item covers all the analyzed regions of its file.",
        "a TypeScript function duplicated between a `.ts` file and a TypeScript region of a component file is one class with `language` `TypeScript`, and a JavaScript region never matches a TypeScript one.",
        "The `details` of the three analyzers always hold `regions_not_analyzed`, `regions_not_analyzed_total` and `files_without_analyzed_code`:",
        "`regions_not_analyzed` has one entry per script or frontmatter region of an in-scope component file that was not analyzed: `range` is the region's start tag, and `reason` is `unsupported script language: <value as written>`, `malformed region` or `parse timeout`. It holds at most 30 entries in location order; `regions_not_analyzed_total` gives the full count.",
        "Style regions and Astro data blocks are never listed.",
        "A malformed region also gives the file a parse failure, so the result is incomplete. A region in an unsupported language does not make the result incomplete",
        "it is not counted in `coverage.analyzed_files`, it is counted in `files_without_analyzed_code`, and for the status rules it is a file in an unsupported language, so a scope of only such files is `unsupported`.",
    ]);
}

/// The README's `details` example is the output of `analyze patterns` on a
/// workspace with the two files it names.
#[test]
fn the_details_example_is_the_analyzer_output() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(
        ws.path().join("Legacy.vue"),
        "<script lang=\"coffee\">\nx = 1\n</script>\n",
    )
    .unwrap();
    std::fs::write(
        ws.path().join("Unclosed.vue"),
        "<script lang=\"ts\">\nfunction a() {}\n<template><p/></template>\n",
    )
    .unwrap();
    let result = cli_json(ws.path(), &["analyze", "patterns", "."]);
    let details = &result["details"];
    let source = section_source();
    let start = source
        .find("```json\n\"regions_not_analyzed\"")
        .expect("the section has the `details` example");
    let body = &source[start + "```json\n".len()..];
    let example: Value =
        serde_json::from_str(&format!("{{{}}}", &body[..body.find("```").unwrap()])).unwrap();
    assert_eq!(
        example,
        json!({
            "regions_not_analyzed": details["regions_not_analyzed"],
            "regions_not_analyzed_total": details["regions_not_analyzed_total"],
            "files_without_analyzed_code": details["files_without_analyzed_code"],
        }),
        "{result}"
    );
    assert_eq!(result["complete"], false, "{result}");
}

#[test]
fn each_limit_is_stated() {
    must_state(&[
        // Template expressions.
        "Code in templates is not parsed as JavaScript or TypeScript: Vue `{{ … }}` and directive values (`v-if`, `v-for`, `:prop`, `@event`), Svelte `{…}`, block tags and event handlers (`on:click={…}`, `onclick={…}`), and Astro `{…}` in the template and in attributes.",
        "It is not in `skeleton` or `symbols`, it is not measured by `patterns` or `hotspots` (an arrow function in `on:click={() => …}` is not a function), and it is not a clone candidate.",
        // Template-to-script references.
        "A name used in a template is not linked to its declaration in a script region: `references` from a script region does not list template uses, and a template position never resolves into a script region.",
        "A component tag (`<Layout>`, `<Comp>`) is an element named by its tag, not resolved to the imported component.",
        // Generated bindings.
        "act-lite reports the code as written, not what a framework compiler generates:",
        // Cross-component dependencies.
        "`graph`, `analyze_cycles`, `analyze_coupling` and `analyze_impact` do not support Vue, Svelte or Astro: they list the component files in `coverage.languages` with `supported: false`.",
        "Imports inside component scripts are not relationships, so a file that only components import has no dependent in `analyze_impact`.",
        "An import of a component file from a JavaScript or TypeScript file with its extension (`import Card from './Card.vue'`) is an edge with `to_analyzed: false`; without the extension (`'./Card'`) it is unresolved (`no file matches ./Card`).",
        "a TypeScript region of a `.vue` file does not find uses in `.svelte` or `.astro` files.",
    ]);
    let rows = table(
        &section_source(),
        "| Framework | Construct | What act-lite reports |",
    );
    let constructs: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r[0].as_str(), r[1].as_str()))
        .collect();
    assert_eq!(
        constructs,
        [
            (
                "Vue",
                "`defineProps`, `defineEmits`, `defineModel`, `defineExpose`, `withDefaults`"
            ),
            (
                "Vue",
                "top-level bindings of `<script setup>` exposed to the template; the shared scope of `<script>` and `<script setup>`"
            ),
            ("Vue", "Options API members (`data`, `methods`, `computed`)"),
            (
                "Svelte",
                "`$:` reactive statements that assign an undeclared name"
            ),
            (
                "Svelte",
                "`export let` props; the runes `$state`, `$derived`, `$props`, `$effect`"
            ),
            ("Svelte", "`$store` auto-subscriptions"),
            (
                "Astro",
                "`Astro.props`, `Astro.request` and the other `Astro` globals; `interface Props`"
            ),
            (
                "all",
                "the function that a framework compiles a `<script setup>` body, a Svelte instance script or an Astro frontmatter into"
            ),
            (
                "Astro",
                "a frontmatter that the Astro grammar ends early (for example at a regular expression that holds `\"`)"
            ),
            ("Svelte", "text with `&` that the Svelte grammar rejects"),
        ]
    );
    let reports: Vec<&str> = rows.iter().map(|r| r[2].as_str()).collect();
    assert!(reports[0].contains("`variable` `props`"), "{reports:?}");
    assert!(reports[1].contains("its own scope"), "{reports:?}");
    assert!(reports[3].contains("not a declaration"), "{reports:?}");
    assert!(reports[5].contains("not linked to `store`"), "{reports:?}");
    assert_eq!(reports[7], "not a function");
    assert!(reports[8].contains("a malformed region"), "{reports:?}");
    assert!(
        reports[9].contains("the regions outside the error are still analyzed"),
        "{reports:?}"
    );
}

#[test]
fn the_partial_support_list_is_stated() {
    must_state(&[
        "These parts of a component file are not read as code: template expressions; Vue templates with a `lang` other than `html`, and custom blocks; script regions in an unsupported language; style regions in an unsupported language; and `<script>` elements that are not regions (Vue below the top level, Svelte inside markup such as `<svelte:head>`), whose code passes to the page unread.",
        "Those `<script>` elements are not listed in `regions_not_analyzed`, which covers component scripts only.",
    ]);
}

#[test]
fn the_section_names_no_design_section() {
    let section = section_source();
    assert!(!section.contains('§'), "the component section cites `§`");
}

/// The `(line, column)` of each reference in a `references` result.
fn sites(result: &Value) -> Vec<(u64, u64)> {
    result["references"]
        .as_array()
        .unwrap_or_else(|| panic!("no `references` in {result}"))
        .iter()
        .map(|r| (r["line"].as_u64().unwrap(), r["column"].as_u64().unwrap()))
        .collect()
}

#[test]
fn the_component_examples_run() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::copy(
        components_fixture("Counter.vue"),
        ws.path().join("Counter.vue"),
    )
    .unwrap();
    let examples: Vec<(String, String)> = section_source()
        .lines()
        .filter(|l| l.starts_with("act-lite query "))
        .map(|l| {
            let (command, comment) = l.split_once('#').expect("each example has a comment");
            (command.trim().to_string(), comment.trim().to_string())
        })
        .collect();
    assert_eq!(examples.len(), 3, "{examples:?}");
    for (example, comment) in &examples {
        let args: Vec<&str> = example.split_whitespace().skip(1).collect();
        match (args[1], args.get(6).copied()) {
            ("skeleton", _) => {
                assert_eq!(
                    comment, "blocks, template elements, region declarations",
                    "{example}"
                );
                let result = cli_json(ws.path(), &args);
                let found: Vec<(String, String)> = result["declarations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| {
                        (
                            d["kind"].as_str().unwrap().to_string(),
                            d["name"].as_str().unwrap().to_string(),
                        )
                    })
                    .collect();
                for (kind, name) in [
                    ("module", "<script setup lang=\"ts\">"),
                    ("module", "<template>"),
                    ("element", "div"),
                    ("field", "app"),
                    ("function", "increment"),
                    ("class", ".counter"),
                ] {
                    assert!(
                        found.contains(&(kind.to_string(), name.to_string())),
                        "{kind} {name}: {result}"
                    );
                }
                assert!(result.get("parse_error").is_none(), "{result}");
            }
            ("references", Some("9")) => {
                assert_eq!(comment, "script uses only: 9:7, 11:19, 12:5", "{example}");
                let result = cli_json(ws.path(), &args);
                assert_eq!(result["source"], "tree-sitter", "{result}");
                assert_eq!(sites(&result), [(9, 7), (11, 19), (12, 5)], "{result}");
            }
            ("references", Some("20")) => {
                assert_eq!(comment, "in {{ count }}: symbol not found", "{example}");
                let output = cli(ws.path(), &args);
                assert_eq!(output.status.code(), Some(1), "{example}");
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(
                    stderr.contains("symbol not found: count in "),
                    "{example}: {stderr}"
                );
            }
            _ => panic!("unexpected example: {example}"),
        }
    }
}
