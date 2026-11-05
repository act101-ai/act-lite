/// True when `ACT_LITE_REQUIRE_LSP` names `binary`, so the test must run
/// against the real server. Otherwise prints the skip line and returns false.
/// Panics if the variable names an unknown binary or `binary` is not on PATH.
fn require_server(binary: &str) -> bool {
    let required: Vec<String> = std::env::var("ACT_LITE_REQUIRE_LSP")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let unknown: Vec<&String> = required
        .iter()
        .filter(|b| {
            !act_lite_lsp::LspServer::ALL
                .iter()
                .any(|s| s.binary_name() == b.as_str())
        })
        .collect();
    assert!(
        unknown.is_empty(),
        "ACT_LITE_REQUIRE_LSP names {unknown:?}, which are not server binaries in servers.rs"
    );
    if !required.iter().any(|b| b == binary) {
        eprintln!("skipped: ACT_LITE_REQUIRE_LSP does not name {binary}");
        return false;
    }
    assert!(
        std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path)
            .any(|dir| dir.join(binary).is_file())),
        "ACT_LITE_REQUIRE_LSP names {binary}, which is not on PATH"
    );
    true
}
