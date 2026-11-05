use std::fs;

use assert_cmd::Command;

#[test]
fn malformed_line_error_prints_no_line_content() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join(".git")).unwrap();
    fs::create_dir(tmp.path().join("home")).unwrap();
    fs::write(
        tmp.path().join(".env"),
        "OK=1\n  \"private_key\": \"-----BEGIN PRIVATE KEY-----\\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC=\\n-----END PRIVATE KEY-----\\n\",\n",
    )
    .unwrap();

    let out = Command::cargo_bin("act-lite")
        .unwrap()
        .current_dir(tmp.path())
        .env("HOME", tmp.path().join("home"))
        .env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"))
        .args(["env", "import", ".env"])
        .output()
        .unwrap();

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("at line 2"), "stderr: {stderr}");
    assert!(stderr.contains("invalid key"), "stderr: {stderr}");
    assert!(
        !stderr.contains("MIIEvQIBADANBgkqhkiG9w0BAQEFAASC") && !stderr.contains("private_key"),
        "stderr echoes line content: {stderr}"
    );
}
