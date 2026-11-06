//! Vue, Svelte and Astro component files: the three grammars load, their
//! query files compile, and component files are routed by their extension
//! only.

use act_lite_parser::{Grammar, ParserError, SourceFile};
use std::path::Path;

const COMPONENTS: [(Grammar, &str, &str); 3] = [
    (Grammar::Vue, "Vue", "vue"),
    (Grammar::Svelte, "Svelte", "svelte"),
    (Grammar::Astro, "Astro", "astro"),
];

#[test]
fn the_three_component_grammars_are_registered_and_load() {
    let all = Grammar::all();
    for (grammar, name, dir) in COMPONENTS {
        assert!(all.contains(&grammar), "{grammar:?}");
        assert_eq!(grammar.name(), name);
        assert_eq!(grammar.query_dir(), dir);
        let support = grammar.language_support();
        assert_eq!(support.grammar(), grammar);
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&support.ts_language())
            .unwrap_or_else(|e| panic!("{grammar:?}: {e}"));
    }
}

#[test]
fn exactly_the_three_are_component_entries() {
    let components: Vec<Grammar> = Grammar::all()
        .into_iter()
        .filter(Grammar::is_component)
        .collect();
    assert_eq!(components, [Grammar::Astro, Grammar::Svelte, Grammar::Vue]);
}

#[test]
fn the_component_query_files_compile() {
    for (grammar, _, _) in COMPONENTS {
        let support = grammar.language_support();
        let language = support.ts_language();
        for (name, source) in [
            ("skeleton", support.skeleton_query()),
            ("symbols", support.symbols_query()),
            ("references", support.references_query()),
        ] {
            tree_sitter::Query::new(&language, source)
                .unwrap_or_else(|e| panic!("{grammar:?}/{name}: {e}"));
        }
    }
}

#[test]
fn component_files_route_by_their_extension() {
    for (path, grammar) in [
        ("a.vue", Grammar::Vue),
        ("a.svelte", Grammar::Svelte),
        ("a.astro", Grammar::Astro),
        ("src/routes/+page.svelte", Grammar::Svelte),
        ("src/routes/+layout.svelte", Grammar::Svelte),
        ("src/pages/index.astro", Grammar::Astro),
    ] {
        assert_eq!(Grammar::from_path(Path::new(path)), Some(grammar), "{path}");
    }
}

#[test]
fn svelte_modules_keep_their_script_extension() {
    assert_eq!(
        Grammar::from_path(Path::new("x.svelte.ts")),
        Some(Grammar::TypeScript)
    );
    assert_eq!(
        Grammar::from_path(Path::new("x.svelte.js")),
        Some(Grammar::JavaScript)
    );
}

#[test]
fn near_miss_names_and_mdx_are_unrecognized() {
    for path in ["Counter.vue.bak", "x.vuex", "a.mdx", "a.VUE"] {
        assert_eq!(Grammar::from_path(Path::new(path)), None, "{path}");
    }
}

#[test]
fn near_miss_names_are_refused_with_the_unsupported_extension_error() {
    let dir = tempfile::tempdir().unwrap();
    for (name, ext) in [("Counter.vue.bak", "bak"), ("x.vuex", "vuex")] {
        let path = dir.path().join(name);
        std::fs::write(&path, "<template><p/></template>\n").unwrap();
        match SourceFile::load(&path).err() {
            Some(ParserError::UnsupportedLanguage { extension }) => {
                assert_eq!(extension, ext, "{name}")
            }
            other => panic!("{name}: {other:?}"),
        }
    }
}

#[test]
fn component_files_load_and_parse_with_their_grammar() {
    let dir = tempfile::tempdir().unwrap();
    for (name, content, grammar) in [
        (
            "a.vue",
            "<template><p>{{ a }}</p></template>\n<script setup lang=\"ts\">\nconst a = 1;\n</script>\n",
            Grammar::Vue,
        ),
        (
            "a.svelte",
            "<script lang=\"ts\">\nlet a = 1;\n</script>\n{#if a}<p>{a}</p>{/if}\n",
            Grammar::Svelte,
        ),
        (
            "a.astro",
            "---\nconst a = 1;\n---\n<p>{a}</p>\n",
            Grammar::Astro,
        ),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap();
        let file = SourceFile::load(&path).unwrap();
        assert_eq!(file.grammar(), grammar, "{name}");
        assert!(!file.tree().root_node().has_error(), "{name}");
    }
}
