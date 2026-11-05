use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where a crate's dependency line says a grammar comes from. A bare
/// version string and a `{ version = "..." }` table are both registry
/// specs and compare equal on the version; a `path` spec compares on
/// the normalized absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DepSource {
    Registry(String),
    Path(PathBuf),
}

fn dep_source(name: &str, value: &toml::Value, crate_dir: &Path) -> DepSource {
    match value {
        toml::Value::String(v) => DepSource::Registry(v.clone()),
        toml::Value::Table(t) => {
            if let Some(p) = t.get("path").and_then(|v| v.as_str()) {
                // Normalize so `vendor/x` from act-lite-parser and
                // `../act-lite-parser/vendor/x` from a sibling crate compare equal.
                let mut abs = crate_dir.to_path_buf();
                for part in p.split('/') {
                    match part {
                        "." => {}
                        ".." => {
                            abs.pop();
                        }
                        other => abs.push(other),
                    }
                }
                DepSource::Path(abs)
            } else if let Some(v) = t.get("version").and_then(|v| v.as_str()) {
                DepSource::Registry(v.to_string())
            } else if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                panic!(
                    "grammar dep {name} uses workspace inheritance; teach this \
                     guard to resolve [workspace.dependencies] before relying on it"
                );
            } else {
                panic!("unrecognized dep spec for {name}: {t:?}")
            }
        }
        other => panic!("unrecognized dep spec for {name}: {other:?}"),
    }
}

fn grammar_deps_by_manifest(root: &Path) -> BTreeMap<String, Vec<(DepSource, String)>> {
    let mut out: BTreeMap<String, Vec<(DepSource, String)>> = BTreeMap::new();
    let root_manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
    let members: Vec<String> = root_manifest
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .expect("root [workspace].members array")
        .iter()
        .map(|m| m.as_str().expect("member path string").to_string())
        .collect();
    let workspace_deps = root_manifest
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
        .cloned();
    let mut crate_dirs: Vec<PathBuf> = Vec::new();
    let mut queue: Vec<PathBuf> = members
        .iter()
        .map(|m| {
            assert!(
                !m.contains('*'),
                "glob workspace member {m}: teach this guard glob resolution before adding one"
            );
            root.join(m)
        })
        .collect();
    // Transitive closure over PATH dependencies: each is a workspace
    // member by cargo's rules, declared or not.
    while let Some(crate_dir) = queue.pop() {
        if !crate_dir.join("Cargo.toml").exists() || crate_dirs.contains(&crate_dir) {
            continue;
        }
        crate_dirs.push(crate_dir.clone());
        let manifest: toml::Value =
            toml::from_str(&std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap())
                .unwrap();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(deps) = manifest.get(section).and_then(|d| d.as_table()) else {
                continue;
            };
            for value in deps.values() {
                if let toml::Value::Table(t) = value
                    && let Some(p) = t.get("path").and_then(|v| v.as_str())
                {
                    let mut abs = crate_dir.clone();
                    for part in p.split('/') {
                        match part {
                            "." => {}
                            ".." => {
                                abs.pop();
                            }
                            other => abs.push(other),
                        }
                    }
                    queue.push(abs);
                }
            }
        }
    }
    crate_dirs.sort();
    for crate_dir in &crate_dirs {
        let manifest: toml::Value =
            toml::from_str(&std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap())
                .unwrap();
        let crate_name = manifest
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("<unnamed>")
            .to_string();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(deps) = manifest.get(section).and_then(|d| d.as_table()) else {
                continue;
            };
            collect_grammar_deps(
                deps,
                crate_dir,
                &crate_name,
                workspace_deps.as_ref(),
                &mut out,
            );
        }
        if let Some(targets) = manifest.get("target").and_then(|t| t.as_table()) {
            for target in targets.values() {
                for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    let Some(deps) = target.get(section).and_then(|d| d.as_table()) else {
                        continue;
                    };
                    collect_grammar_deps(
                        deps,
                        crate_dir,
                        &crate_name,
                        workspace_deps.as_ref(),
                        &mut out,
                    );
                }
            }
        }
    }
    out
}

fn collect_grammar_deps(
    deps: &toml::map::Map<String, toml::Value>,
    crate_dir: &Path,
    crate_name: &str,
    workspace_deps: Option<&toml::map::Map<String, toml::Value>>,
    out: &mut BTreeMap<String, Vec<(DepSource, String)>>,
) {
    for (alias, value) in deps {
        let value = if let toml::Value::Table(t) = value {
            if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                let Some(inherited) = workspace_deps.and_then(|w| w.get(alias)) else {
                    panic!(
                        "dependency {alias} of {crate_name} sets workspace = true but the \
                         root [workspace.dependencies] has no such entry"
                    );
                };
                inherited.clone()
            } else {
                value.clone()
            }
        } else {
            value.clone()
        };
        let name = match &value {
            toml::Value::Table(t) => t.get("package").and_then(|p| p.as_str()).unwrap_or(alias),
            _ => alias,
        };
        if !name.starts_with("tree-sitter-") {
            continue;
        }
        out.entry(name.to_string())
            .or_default()
            .push((dep_source(name, &value, crate_dir), crate_name.to_string()));
    }
}

#[test]
fn shared_grammar_deps_resolve_to_one_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root (two levels above crates/act-lite-parser)")
        .to_path_buf();
    for (name, decls) in grammar_deps_by_manifest(&root) {
        if decls.len() < 2 {
            continue;
        }
        // Registry-vs-registry requirement STRINGS may differ while
        // cargo still unifies them to one locked version ("0.1" and
        // "0.1.0" both resolve to the same crate) — the lock test
        // adjudicates those. What the manifests alone prove wrong:
        // two different PATHS (different bytes), or a path/registry
        // MIX (a vendored swap that left another crate behind).
        let paths: Vec<&PathBuf> = decls
            .iter()
            .filter_map(|(s, _)| match s {
                DepSource::Path(p) => Some(p),
                DepSource::Registry(_) => None,
            })
            .collect();
        let has_registry = decls
            .iter()
            .any(|(s, _)| matches!(s, DepSource::Registry(_)));
        let who: Vec<String> = decls.iter().map(|(s, c)| format!("{c} → {s:?}")).collect();
        assert!(
            !has_registry || paths.is_empty(),
            "grammar {name} is declared by {} crates with a PATH/REGISTRY MIX — one \
             binary would parse the language with two different grammars (a vendored \
             swap left a crate behind):\n  {}",
            decls.len(),
            who.join("\n  ")
        );
        assert!(
            paths.windows(2).all(|w| w[0] == w[1]),
            "grammar {name} is declared by {} crates with DIFFERENT vendored paths:\n  {}",
            decls.len(),
            who.join("\n  ")
        );
    }
}

#[test]
fn cargo_lock_holds_one_entry_per_grammar() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf();
    let lock: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join("Cargo.lock")).unwrap()).unwrap();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for package in lock
        .get("package")
        .and_then(|p| p.as_array())
        .expect("Cargo.lock [[package]] array")
    {
        let name = package
            .get("name")
            .and_then(|n| n.as_str())
            .expect("package name");
        if name.starts_with("tree-sitter-") {
            *seen.entry(name.to_string()).or_default() += 1;
        }
    }
    let split: Vec<String> = seen
        .iter()
        .filter(|&(_, n)| *n > 1)
        .map(|(name, n)| format!("{name} ×{n}"))
        .collect();
    assert!(
        split.is_empty(),
        "Cargo.lock holds multiple entries for grammar crates — a vendored swap \
         left a stale registry entry (or two crates pull different sources):\n  {}",
        split.join(", ")
    );
}
