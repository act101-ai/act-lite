use std::fs;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

/// Build an isolated HOME + git project so the store lands under a temp dir.
fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join(".git")).unwrap();
    fs::create_dir(tmp.path().join("home")).unwrap();
    tmp
}

fn act(tmp: &tempfile::TempDir) -> Command {
    let mut cmd = Command::cargo_bin("act-lite").unwrap();
    cmd.current_dir(tmp.path());
    cmd.env("HOME", tmp.path().join("home"));
    cmd.env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"));
    cmd
}

#[test]
fn import_then_run_redacts_and_get_reveals() {
    let tmp = project();
    fs::write(tmp.path().join(".env"), "GH_TOKEN=supersecretvalue\n").unwrap();

    // import: plaintext leaves the working tree, silhouette stays.
    act(&tmp).args(["env", "import", ".env"]).assert().success();
    let silhouette = fs::read_to_string(tmp.path().join(".env")).unwrap();
    assert!(silhouette.contains("GH_TOKEN=#act:sequestered"));
    assert!(!silhouette.contains("supersecretvalue"));

    // list: names only.
    act(&tmp)
        .args(["env", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("GH_TOKEN"));

    // run: child echoes the token, output comes back redacted.
    act(&tmp)
        .args(["env", "run", "--", "sh", "-c", "echo $GH_TOKEN"])
        .assert()
        .success()
        .stdout(predicates::str::contains("***"))
        .stdout(predicates::str::contains("supersecretvalue").not());

    // get without --reveal: redacted.
    act(&tmp)
        .args(["env", "get", "GH_TOKEN"])
        .assert()
        .success()
        .stdout(predicates::str::contains("supersecretvalue").not());

    // get --reveal: prints the value.
    act(&tmp)
        .args(["env", "get", "GH_TOKEN", "--reveal"])
        .assert()
        .success()
        .stdout(predicates::str::contains("supersecretvalue"));
}

#[test]
fn add_via_stdin_then_rm() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "API_KEY", "--stdin"])
        .write_stdin("anothersecretvalue\n")
        .assert()
        .success();
    act(&tmp)
        .args(["env", "get", "API_KEY", "--reveal"])
        .assert()
        .success()
        .stdout(predicates::str::contains("anothersecretvalue"));
    act(&tmp).args(["env", "rm", "API_KEY"]).assert().success();
    act(&tmp).args(["env", "get", "API_KEY"]).assert().failure();
}

#[test]
fn run_forwards_stdin_to_child() {
    let tmp = project();
    // `cat` only produces output if stdin is forwarded to the child.
    act(&tmp)
        .args(["env", "run", "--", "sh", "-c", "cat"])
        .write_stdin("hello-from-stdin")
        .assert()
        .success()
        .stdout(predicates::str::contains("hello-from-stdin"));
}

#[test]
fn add_and_rm_never_touch_the_repo_env() {
    let tmp = project();
    let mixed = "REAL=plaintextsecret\nOTHER=#act:sequestered\n";
    fs::write(tmp.path().join(".env"), mixed).unwrap();

    act(&tmp)
        .args(["env", "add", "NEW", "--stdin"])
        .write_stdin("anothersecret\n")
        .assert()
        .success();
    act(&tmp).args(["env", "rm", "NEW"]).assert().success();

    assert_eq!(fs::read_to_string(tmp.path().join(".env")).unwrap(), mixed);
}

#[test]
fn mv_sequester_keeps_mixed_file() {
    let tmp = project();
    fs::write(
        tmp.path().join(".env"),
        "DATABASE_URL=postgres://localhost/app\nCLOUDFLARE_API_KEY=cfkey_abc123\n",
    )
    .unwrap();

    // Sequester just CLOUDFLARE_API_KEY.
    act(&tmp)
        .args(["env", "mv", "CLOUDFLARE_API_KEY"])
        .assert()
        .success();

    let env_after = fs::read_to_string(tmp.path().join(".env")).unwrap();
    // App line untouched.
    assert!(env_after.contains("DATABASE_URL=postgres://localhost/app"));
    // Marker present.
    assert!(env_after.contains("CLOUDFLARE_API_KEY=#act:sequestered"));
    // Plaintext value gone.
    assert!(!env_after.contains("cfkey_abc123"));

    // Value reached the store.
    act(&tmp)
        .args(["env", "get", "CLOUDFLARE_API_KEY", "--reveal"])
        .assert()
        .success()
        .stdout(predicates::str::contains("cfkey_abc123"));
}

#[test]
fn mv_restore_round_trips() {
    let tmp = project();
    fs::write(tmp.path().join(".env"), "CLOUDFLARE_API_KEY=cfkey_abc123\n").unwrap();

    // Sequester first.
    act(&tmp)
        .args(["env", "mv", "CLOUDFLARE_API_KEY"])
        .assert()
        .success();

    // Restore.
    act(&tmp)
        .args(["env", "mv", "CLOUDFLARE_API_KEY", "--restore"])
        .assert()
        .success();

    let env_after = fs::read_to_string(tmp.path().join(".env")).unwrap();
    // Plaintext value back.
    assert!(env_after.contains("CLOUDFLARE_API_KEY=cfkey_abc123"));
    // Marker gone.
    assert!(!env_after.contains("#act:sequestered"));

    // Dropped from store.
    act(&tmp)
        .args(["env", "get", "CLOUDFLARE_API_KEY"])
        .assert()
        .failure();
}

#[test]
fn mv_on_absent_key_errors() {
    let tmp = project();
    fs::write(tmp.path().join(".env"), "OTHER_KEY=somevalue\n").unwrap();

    act(&tmp)
        .args(["env", "mv", "NOPE"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not found in"));
}

#[test]
fn mv_restore_not_in_store_errors() {
    let tmp = project();
    // No keys in store — restore must fail.
    act(&tmp)
        .args(["env", "mv", "GHOST", "--restore"])
        .assert()
        .failure();
}

#[test]
fn run_with_a_missing_program_names_it() {
    let tmp = project();
    act(&tmp)
        .args(["env", "run", "--", "definitely-not-a-real-binary-xyz"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "failed to start `definitely-not-a-real-binary-xyz`",
        ));
}

#[test]
fn workspace_flag_selects_the_project_root_as_given() {
    let tmp = project();
    let pkg = tmp.path().join("packages").join("api");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join(".env"), "API_KEY=pkgvalue123\n").unwrap();
    fs::write(tmp.path().join(".env"), "ROOT=rootvalue1\n").unwrap();
    let pkg_arg = pkg.to_str().unwrap().to_string();

    // mv from the repo root, targeting the package: only the package .env changes.
    act(&tmp)
        .args(["env", "mv", "API_KEY", "--workspace", &pkg_arg])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(tmp.path().join(".env")).unwrap(),
        "ROOT=rootvalue1\n"
    );
    assert!(
        fs::read_to_string(pkg.join(".env"))
            .unwrap()
            .contains("API_KEY=#act:sequestered")
    );

    // The package has its own store; the root store is empty.
    act(&tmp)
        .args(["env", "list", "--workspace", &pkg_arg])
        .assert()
        .success()
        .stdout(predicates::str::contains("API_KEY"));
    act(&tmp)
        .args(["env", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("API_KEY").not());
}

#[test]
fn workspace_flag_with_a_missing_dir_errors() {
    let tmp = project();
    let missing = tmp.path().join("nope").to_str().unwrap().to_string();
    let out = act(&tmp)
        .args(["env", "list", "--workspace", &missing])
        .assert()
        .failure();
    // stderr is JSON, which escapes the `\` of a Windows path; compare decoded.
    let err: serde_json::Value = serde_json::from_slice(&out.get_output().stderr).unwrap();
    assert_eq!(
        err["error"]["chain"][0],
        format!(
            "--workspace {missing} does not exist or is not readable; pass an existing project \
             directory"
        )
    );
}

#[test]
fn add_warns_when_a_value_is_too_short_to_redact() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "PORT", "--stdin"])
        .write_stdin("8080\n")
        .assert()
        .success()
        .stderr(predicates::str::contains("will not be redacted"));
}

#[test]
fn import_warns_for_each_sequestered_value_too_short_to_redact() {
    let tmp = project();
    fs::write(
        tmp.path().join(".env"),
        "PORT=8080\nGH_TOKEN=supersecretvalue\n",
    )
    .unwrap();
    act(&tmp)
        .args(["env", "import", ".env"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "warning: PORT is shorter than 6 characters and will not be redacted",
        ))
        .stderr(predicates::str::contains("warning: GH_TOKEN").not())
        .stderr(predicates::str::contains(
            "Sequestered 2 secret(s) from .env; 0 key(s) were already sequestered.",
        ));
}

#[test]
fn mv_warns_when_the_sequestered_value_is_too_short_to_redact() {
    let tmp = project();
    fs::write(tmp.path().join(".env"), "PORT=8080\n").unwrap();
    act(&tmp)
        .args(["env", "mv", "PORT"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "warning: PORT is shorter than 6 characters and will not be redacted",
        ));
}

#[test]
fn import_warns_about_a_marked_key_the_store_lacks() {
    let tmp = project();
    fs::write(
        tmp.path().join(".env"),
        "GONE=#act:sequestered\nNEW=newvalue1\n",
    )
    .unwrap();
    act(&tmp)
        .args(["env", "import", ".env"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "warning: GONE is marked #act:sequestered in .env but the store has no value for \
             it; use `act-lite env add GONE` to enter one",
        ));
}

#[test]
fn format_flag_does_not_change_env_output() {
    let tmp = project();
    fs::write(
        tmp.path().join(".env"),
        "GH_TOKEN=supersecretvalue\nDB_URL=postgres://u:p@h/db\n",
    )
    .unwrap();
    act(&tmp).args(["env", "import", ".env"]).assert().success();

    let json = act(&tmp)
        .args(["--format", "json", "env", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = act(&tmp)
        .args(["--format", "text", "env", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(json, text);
    let listed = String::from_utf8(json).unwrap();
    assert!(listed.contains("GH_TOKEN\n"), "list output: {listed}");
    assert!(listed.contains("DB_URL\n"), "list output: {listed}");
}
