#[test]
fn generic_references_terminate_on_dockerfile_run_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Dockerfile");
    std::fs::write(
        &path,
        "FROM alpine\nRUN git clone --branch ${V} https://example.invalid/t.git\n",
    )
    .unwrap();
    let mut ws = act_lite_parser::Workspace::new(dir.path()).unwrap();
    ws.open_file(&path).unwrap();
    let sf = ws.get_file(&path).unwrap();
    // No name: the generic query. Must terminate (and not panic).
    let result = act_lite_parser::execute_references(sf);
    assert!(result.is_ok());
}
