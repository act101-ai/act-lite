//! The plugin and marketplace manifests carry the crate's version, so a
//! release tag names one version everywhere.

use std::path::Path;

fn manifest_version(path: &Path) -> String {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let json: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()));
    let version = match json.get("plugins").and_then(|p| p.as_array()) {
        Some(plugins) => plugins
            .iter()
            .find(|p| p.get("name").and_then(|n| n.as_str()) == Some("act-lite"))
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str()),
        None => json.get("version").and_then(|v| v.as_str()),
    };
    version
        .unwrap_or_else(|| panic!("{} has no act-lite version", path.display()))
        .to_string()
}

#[test]
fn plugin_manifests_carry_the_crate_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crate_version = env!("CARGO_PKG_VERSION");
    for rel in [
        "plugin/.claude-plugin/plugin.json",
        ".claude-plugin/marketplace.json",
    ] {
        let path = root.join(rel);
        assert_eq!(
            manifest_version(&path),
            crate_version,
            "{rel} must carry the crate version {crate_version}"
        );
    }
}
