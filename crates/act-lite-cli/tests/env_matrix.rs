//! Every `env` subcommand, flag and store rule in the README.
//! Expected values come from the README, not from observed output.

use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

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

/// `act-lite` with no config-dir override: store resolution runs from HOME/XDG alone.
fn act_bare(tmp: &tempfile::TempDir, cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("act-lite").unwrap();
    cmd.current_dir(cwd);
    cmd.env("HOME", tmp.path().join("home"));
    cmd.env_remove("ACT_CONFIG_DIR");
    cmd.env_remove("XDG_CONFIG_HOME");
    cmd
}

fn reveal(tmp: &tempfile::TempDir, name: &str) -> String {
    let out = act(tmp)
        .args(["env", "get", name, "--reveal"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

fn store_files(config_dir: &Path) -> Vec<PathBuf> {
    match fs::read_dir(config_dir.join("secrets")) {
        Ok(rd) => rd.map(|e| e.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}

fn comment_lines(text: &str) -> usize {
    text.lines()
        .filter(|l| l.trim_start().starts_with('#'))
        .count()
}

#[test]
fn import_keeps_other_lines_and_adds_the_header_once() {
    let tmp = project();
    fs::write(
        tmp.path().join(".env"),
        "# my app\nA_TOKEN=firstsecret1\n\nB_TOKEN=secondsecret2\n",
    )
    .unwrap();
    act(&tmp).args(["env", "import", ".env"]).assert().success();
    let first = fs::read_to_string(tmp.path().join(".env")).unwrap();
    assert!(first.contains("# my app\n"));
    assert!(first.contains("A_TOKEN=#act:sequestered\n"));
    assert!(first.contains("B_TOKEN=#act:sequestered\n"));
    assert!(first.contains("\n\n"), "blank line kept: {first}");
    assert!(comment_lines(&first) >= 2, "header added: {first}");

    // A second import with one new plaintext line adds no second header.
    let mut more = first.clone();
    more.push_str("C_TOKEN=thirdsecret3\n");
    fs::write(tmp.path().join(".env"), &more).unwrap();
    act(&tmp).args(["env", "import", ".env"]).assert().success();
    let second = fs::read_to_string(tmp.path().join(".env")).unwrap();
    assert!(second.contains("C_TOKEN=#act:sequestered\n"));
    assert_eq!(comment_lines(&second), comment_lines(&first), "{second}");
}

#[test]
fn import_force_overwrites_a_differing_stored_value_only_with_the_flag() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "K", "--stdin"])
        .write_stdin("storedvalue1\n")
        .assert()
        .success();

    fs::write(tmp.path().join(".env"), "K=filevalue11\n").unwrap();
    let _ = act(&tmp).args(["env", "import", ".env"]).assert();
    assert_eq!(reveal(&tmp, "K").trim(), "storedvalue1");

    fs::write(tmp.path().join(".env"), "K=filevalue11\n").unwrap();
    act(&tmp)
        .args(["env", "import", ".env", "--force"])
        .assert()
        .success();
    assert_eq!(reveal(&tmp, "K").trim(), "filevalue11");
}

#[test]
fn mv_force_proceeds_through_a_value_collision_only_with_the_flag() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "K", "--stdin"])
        .write_stdin("storedvalue1\n")
        .assert()
        .success();

    fs::write(tmp.path().join(".env"), "K=filevalue11\n").unwrap();
    let _ = act(&tmp).args(["env", "mv", "K"]).assert();
    assert_eq!(reveal(&tmp, "K").trim(), "storedvalue1");

    fs::write(tmp.path().join(".env"), "K=filevalue11\n").unwrap();
    act(&tmp)
        .args(["env", "mv", "K", "--force"])
        .assert()
        .success();
    assert_eq!(reveal(&tmp, "K").trim(), "filevalue11");
    assert!(
        fs::read_to_string(tmp.path().join(".env"))
            .unwrap()
            .contains("K=#act:sequestered")
    );
}

#[test]
fn add_reads_a_hidden_prompt_and_updates_an_existing_key() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("skipped: python3 not available for a pty");
        return;
    }
    let tmp = project();
    // Drive `act-lite env add` through a pty: no --stdin, so it must prompt.
    let script = r#"
import os, pty, select, sys, time
pid, fd = pty.fork()
if pid == 0:
    os.execv(sys.argv[1], [sys.argv[1], "env", "add", "PTY_KEY"])
out = b""
def drain(t):
    global out
    while True:
        r, _, _ = select.select([fd], [], [], t)
        if not r:
            return
        try:
            d = os.read(fd, 4096)
        except OSError:
            return
        if not d:
            return
        out += d
drain(1.5)
os.write(fd, sys.argv[2].encode() + b"\n")
drain(1.5)
_, status = os.waitpid(pid, 0)
sys.stdout.buffer.write(out)
sys.exit(os.waitstatus_to_exitcode(status))
"#;
    let bin = assert_cmd::cargo::cargo_bin("act-lite");
    let run = |value: &str| {
        Command::new("python3")
            .args(["-I", "-c", script, bin.to_str().unwrap(), value])
            .current_dir(tmp.path())
            .env("HOME", tmp.path().join("home"))
            .env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"))
            .assert()
            .success()
            .stdout(predicates::str::contains(value).not())
    };
    run("promptvalue-one");
    assert_eq!(reveal(&tmp, "PTY_KEY").trim(), "promptvalue-one");
    run("promptvalue-two");
    assert_eq!(reveal(&tmp, "PTY_KEY").trim(), "promptvalue-two");
}

#[test]
fn rm_removes_the_key_from_list_and_get() {
    let tmp = project();
    for (k, v) in [("KEEP_ME", "keepvalue123\n"), ("DROP_ME", "dropvalue123\n")] {
        act(&tmp)
            .args(["env", "add", k, "--stdin"])
            .write_stdin(v)
            .assert()
            .success();
    }
    act(&tmp).args(["env", "rm", "DROP_ME"]).assert().success();
    act(&tmp)
        .args(["env", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("KEEP_ME"))
        .stdout(predicates::str::contains("DROP_ME").not());
    act(&tmp).args(["env", "get", "DROP_ME"]).assert().failure();
}

#[test]
fn run_passes_the_child_exit_code_through() {
    let tmp = project();
    for code in [0, 3, 42] {
        act(&tmp)
            .args(["env", "run", "--", "sh", "-c", &format!("exit {code}")])
            .assert()
            .code(code);
    }
}

#[test]
fn run_redacts_a_value_split_across_output_chunks() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "SPLIT", "--stdin"])
        .write_stdin("abcdefghij\n")
        .assert()
        .success();
    act(&tmp)
        .args([
            "env",
            "run",
            "--",
            "sh",
            "-c",
            "printf abcde; sleep 1; printf 'fghij\\n'",
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("abcdefghij").not())
        .stdout(predicates::str::contains("***"));
}

#[test]
fn run_redacts_a_value_split_by_a_pause() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "TOKEN", "--stdin"])
        .write_stdin("supersecretvalue")
        .assert()
        .success();
    act(&tmp)
        .args([
            "env",
            "run",
            "--",
            "sh",
            "-c",
            "printf super; sleep 0.3; printf \"secretvalue\\n\"",
        ])
        .assert()
        .success()
        .stdout("***\n");
}

#[test]
fn run_keeps_a_utf8_character_split_by_a_pause() {
    let tmp = project();
    act(&tmp)
        .args([
            "env",
            "run",
            "--",
            "sh",
            "-c",
            "printf 'caf\\303'; sleep 0.3; printf '\\251\\n'",
        ])
        .assert()
        .success()
        .stdout("café\n");
}

#[test]
fn run_redacts_a_value_split_across_stdout_and_stderr() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "TOKEN", "--stdin"])
        .write_stdin("supersecretvalue")
        .assert()
        .success();
    let bin = assert_cmd::cargo::cargo_bin("act-lite");
    // Both streams of `env run` go to one pipe, as with `2>&1`.
    let merged = |script: &str| {
        Command::new("sh")
            .args(["-c", "\"$0\" env run -- sh -c \"$1\" 2>&1"])
            .arg(&bin)
            .arg(script)
            .current_dir(tmp.path())
            .env("HOME", tmp.path().join("home"))
            .env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"))
            .assert()
            .success()
    };
    // The value is contiguous in the order the child writes: neither part shows.
    for script in [
        "printf super; sleep 0.3; printf 'secretvalue\\n' >&2",
        "printf super; exec 1>&-; sleep 0.3; printf 'secretvalue\\n' >&2",
    ] {
        merged(script).stdout(
            predicates::str::contains("super")
                .not()
                .and(predicates::str::contains("secretvalue").not())
                .and(predicates::str::contains("***")),
        );
    }
    // stdout holds `s`; stderr passes it, so the written order is `sup`,
    // `ersecretvalue\n`, `s` and the value is contiguous there: both parts are masked.
    let script = "printf sups; sleep 0.3; printf 'ersecretvalue\\n' >&2";
    merged(script).stdout("******\ns");
    act(&tmp)
        .args(["env", "run", "--", "sh", "-c", script])
        .assert()
        .success()
        .stdout("***s")
        .stderr("***\n");
}

#[test]
fn run_does_not_hold_stderr_behind_an_ordinary_partial_stdout_line() {
    use std::io::BufRead;
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "TOKEN", "--stdin"])
        .write_stdin("supersecretvalue")
        .assert()
        .success();
    // stdout rewrites a partial progress line every 50 ms for 2 s, faster than
    // the idle flush; `step N` cannot start the value.
    let started = std::time::Instant::now();
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
        .args([
            "env",
            "run",
            "--",
            "sh",
            "-c",
            "i=0; while [ $i -lt 40 ]; do printf '\\rstep %d' $i; i=$((i+1)); sleep 0.05; done & \
                 sleep 0.1; echo warn >&2; wait; echo",
        ])
        .current_dir(tmp.path())
        .env("HOME", tmp.path().join("home"))
        .env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let took = started.elapsed();
    assert!(child.wait().unwrap().success());
    assert_eq!(line, "warn\n");
    assert!(
        took < std::time::Duration::from_secs(1),
        "stderr line arrived after {took:?}"
    );
}

#[test]
fn run_masks_a_value_alternating_between_streams_in_arrival_order() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "TOKEN", "--stdin"])
        .write_stdin("supersecretvalue")
        .assert()
        .success();
    let bin = assert_cmd::cargo::cargo_bin("act-lite");
    // Both streams of `env run` go to one pipe, as with `2>&1`. The value is
    // contiguous in the order the child writes, so each stream shows only
    // `***` and no byte leaves that order.
    let merged = |script: &str| {
        Command::new("sh")
            .args(["-c", "\"$0\" env run -- sh -c \"$1\" 2>&1"])
            .arg(&bin)
            .arg(script)
            .current_dir(tmp.path())
            .env("HOME", tmp.path().join("home"))
            .env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"))
            .assert()
            .success()
            .stdout("******\n");
    };
    merged(
        "printf su; sleep 0.2; printf pe >&2; sleep 0.2; printf rs; sleep 0.2; \
             printf ec >&2; sleep 0.2; printf 'retvalue\\n'",
    );
    // One byte at a time, stdout first, then stderr, alternating.
    for pause in ["0.05", "0.2"] {
        let script: Vec<String> = "supersecretvalue"
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let to = if i % 2 == 0 { "" } else { " >&2" };
                format!("printf {c}{to}; sleep {pause}")
            })
            .chain(["echo".to_string()])
            .collect();
        merged(&script.join("; "));
    }
}

#[test]
fn run_releases_the_other_stream_behind_a_held_tail() {
    use std::io::Read;
    use std::time::{Duration, Instant};
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "TOKEN", "--stdin"])
        .write_stdin("abcdef123456")
        .assert()
        .success();
    // stdout's `a` could start the value and stays held until stdout goes on;
    // `aContinue? ` cannot start it, so stderr does not wait for stdout.
    let started = Instant::now();
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("act-lite"))
        .args([
            "env",
            "run",
            "--",
            "sh",
            "-c",
            "printf 'progress a'; sleep 0.2; printf 'Continue? ' >&2; sleep 3; echo; echo done",
        ])
        .current_dir(tmp.path())
        .env("HOME", tmp.path().join("home"))
        .env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        stdout.read_to_string(&mut text).unwrap();
        text
    });
    let mut stderr = child.stderr.take().unwrap();
    let (mut err, mut buf) = (Vec::new(), [0u8; 64]);
    let mut prompt_at = None;
    loop {
        let n = stderr.read(&mut buf).unwrap();
        if n == 0 {
            break;
        }
        err.extend_from_slice(&buf[..n]);
        if prompt_at.is_none() && err == b"Continue? " {
            prompt_at = Some(started.elapsed());
        }
    }
    assert!(child.wait().unwrap().success());
    assert_eq!(out.join().unwrap(), "progress a\ndone\n");
    assert_eq!(String::from_utf8(err).unwrap(), "Continue? ");
    let at = prompt_at.unwrap();
    assert!(
        at < Duration::from_secs(1),
        "stderr prompt arrived after {at:?}"
    );
}

#[test]
fn run_masks_a_value_split_within_one_stream_while_the_other_stream_flows() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "TOKEN", "--stdin"])
        .write_stdin("supersecretvalue")
        .assert()
        .success();
    // More stderr bytes than the value's length pass while stdout holds `super`.
    act(&tmp)
        .args([
            "env",
            "run",
            "--",
            "sh",
            "-c",
            "printf super; sleep 0.2; printf 'xxxxxxxxxxxxxxxxxxxx\\n' >&2; sleep 0.2; \
                 printf 'secretvalue\\n'",
        ])
        .assert()
        .success()
        .stdout("***\n")
        .stderr("xxxxxxxxxxxxxxxxxxxx\n");
    act(&tmp)
        .args(["env", "add", "SHORTER", "--stdin"])
        .write_stdin("abcdef123456")
        .assert()
        .success();
    // Each stream holds part of a different value while the other one flows.
    act(&tmp)
        .args([
            "env",
            "run",
            "--",
            "sh",
            "-c",
            "printf abc; sleep 0.2; printf 'supersecretv' >&2; sleep 0.2; \
                 printf 'def123456\\n'; sleep 0.2; printf 'alue\\n' >&2",
        ])
        .assert()
        .success()
        .stdout("***\n")
        .stderr("***\n");
}

#[test]
fn run_redacts_long_output_in_linear_time() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "TOKEN", "--stdin"])
        .write_stdin("supersecretvalue")
        .assert()
        .success();
    for (script, expected) in [
        (
            "yes supersecretvalue | head -n 200000",
            "***\n".repeat(200_000),
        ),
        (
            "yes supersecretvalue | head -n 20000 | tr -d '\\n'",
            "***".repeat(20_000),
        ),
    ] {
        let started = std::time::Instant::now();
        let out = act(&tmp)
            .args(["env", "run", "--", "sh", "-c", script])
            .output()
            .unwrap();
        let took = started.elapsed();
        assert!(out.status.success(), "{script}");
        assert!(
            out.stdout == expected.as_bytes(),
            "{script}: output differs"
        );
        assert!(
            took < std::time::Duration::from_secs(10),
            "{script}: took {took:?}"
        );
    }
}

#[test]
fn run_redacts_stderr() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "ERR_SECRET", "--stdin"])
        .write_stdin("stderrsecret99\n")
        .assert()
        .success();
    act(&tmp)
        .args(["env", "run", "--", "sh", "-c", "echo $ERR_SECRET >&2"])
        .assert()
        .success()
        .stderr(predicates::str::contains("stderrsecret99").not())
        .stderr(predicates::str::contains("***"));
}

#[test]
fn list_and_get_never_print_values_without_reveal() {
    let tmp = project();
    act(&tmp)
        .args(["env", "add", "LISTED", "--stdin"])
        .write_stdin("listedvalue77\n")
        .assert()
        .success();
    act(&tmp)
        .args(["env", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("LISTED"))
        .stdout(predicates::str::contains("listedvalue77").not());
    let out = act(&tmp)
        .args(["env", "get", "LISTED"])
        .assert()
        .success()
        .stdout(predicates::str::contains("listedvalue77").not())
        .get_output()
        .stdout
        .clone();
    let line = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = line.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), 1, "one redacted line expected: {line:?}");
    act(&tmp)
        .args(["env", "get", "NO_SUCH_KEY"])
        .assert()
        .failure();
}

fn add_one(cmd: &mut Command) {
    cmd.args(["env", "add", "STORE_KEY", "--stdin"])
        .write_stdin("storevalue123\n")
        .assert()
        .success();
}

#[test]
fn store_defaults_to_home_config_act_secrets_with_mode_0600() {
    let tmp = project();
    add_one(&mut act_bare(&tmp, tmp.path()));
    let files = store_files(&tmp.path().join("home/.config/act"));
    assert_eq!(files.len(), 1, "files: {files:?}");
    assert_eq!(files[0].extension().unwrap(), "env");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&files[0]).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn store_uses_xdg_config_home_act_when_set() {
    let tmp = project();
    let xdg = tmp.path().join("xdg");
    add_one(act_bare(&tmp, tmp.path()).env("XDG_CONFIG_HOME", &xdg));
    assert_eq!(store_files(&xdg.join("act")).len(), 1);
    assert!(store_files(&tmp.path().join("home/.config/act")).is_empty());
}

#[test]
fn act_config_dir_replaces_both_xdg_and_default() {
    let tmp = project();
    let xdg = tmp.path().join("xdg");
    let cfg = tmp.path().join("cfg");
    add_one(
        act_bare(&tmp, tmp.path())
            .env("XDG_CONFIG_HOME", &xdg)
            .env("ACT_CONFIG_DIR", &cfg),
    );
    assert_eq!(store_files(&cfg).len(), 1);
    assert!(store_files(&xdg.join("act")).is_empty());
    assert!(store_files(&tmp.path().join("home/.config/act")).is_empty());
}

#[test]
fn linked_worktrees_share_one_store() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("home")).unwrap();
    let main = tmp.path().join("main");
    let linked = tmp.path().join("linked");
    fs::create_dir(&main).unwrap();
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(&main)
            .env("HOME", tmp.path().join("home"))
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    };
    git(&["init", "-q"]);
    git(&["commit", "-q", "--allow-empty", "-m", "init"]);
    git(&["worktree", "add", "-q", linked.to_str().unwrap()]);

    let bare = |cwd: &Path| {
        let mut c = act_bare(&tmp, cwd);
        c.env("ACT_CONFIG_DIR", tmp.path().join("cfg"));
        c
    };
    bare(&main)
        .args(["env", "add", "SHARED", "--stdin"])
        .write_stdin("sharedvalue12\n")
        .assert()
        .success();
    bare(&linked)
        .args(["env", "get", "SHARED", "--reveal"])
        .assert()
        .success()
        .stdout(predicates::str::contains("sharedvalue12"));
    assert_eq!(store_files(&tmp.path().join("cfg")).len(), 1);
}

#[test]
fn workspace_flag_does_not_search_upward_for_git() {
    let tmp = project();
    let sub = tmp.path().join("sub");
    fs::create_dir(&sub).unwrap();
    let sub_arg = sub.to_str().unwrap().to_string();
    // Same cwd inside the repo; only --workspace differs, so the stores differ.
    act(&tmp)
        .args(["env", "add", "ONLY_SUB", "--stdin", "--workspace", &sub_arg])
        .write_stdin("subvalue1234\n")
        .assert()
        .success();
    act(&tmp)
        .args(["env", "get", "ONLY_SUB"])
        .assert()
        .failure();
    // Run from inside the sub-directory with no flag: nearest .git ancestor is the repo root.
    act(&tmp)
        .current_dir(&sub)
        .args(["env", "get", "ONLY_SUB"])
        .assert()
        .failure();
    assert_eq!(store_files(&tmp.path().join("home/act")).len(), 1);
}

fn have_act() -> bool {
    std::process::Command::new("act")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn upstream_act(tmp: &tempfile::TempDir) -> Command {
    let mut cmd = Command::new("act");
    cmd.current_dir(tmp.path());
    cmd.env("HOME", tmp.path().join("home"));
    cmd.env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"));
    cmd.env_remove("XDG_CONFIG_HOME");
    cmd
}

#[test]
fn interop_act_sequesters_and_act_lite_reads() {
    if !have_act() {
        eprintln!("skipped: act not on PATH");
        return;
    }
    let tmp = project();
    fs::write(tmp.path().join(".env"), "INTEROP_A=actvalue12345\n").unwrap();
    upstream_act(&tmp)
        .args(["env", "import", ".env"])
        .assert()
        .success();
    act(&tmp)
        .args(["env", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("INTEROP_A"));
    assert_eq!(reveal(&tmp, "INTEROP_A").trim(), "actvalue12345");
    act(&tmp)
        .args(["env", "run", "--", "sh", "-c", "echo $INTEROP_A"])
        .assert()
        .success()
        .stdout(predicates::str::contains("actvalue12345").not());
}

#[test]
fn interop_act_lite_sequesters_and_act_reads() {
    if !have_act() {
        eprintln!("skipped: act not on PATH");
        return;
    }
    let tmp = project();
    fs::write(tmp.path().join(".env"), "INTEROP_B=litevalue12345\n").unwrap();
    act(&tmp).args(["env", "import", ".env"]).assert().success();
    upstream_act(&tmp)
        .args(["env", "list"])
        .assert()
        .success()
        .stdout(predicates::str::contains("INTEROP_B"));
    upstream_act(&tmp)
        .args(["env", "get", "INTEROP_B", "--reveal"])
        .assert()
        .success()
        .stdout(predicates::str::contains("litevalue12345"));
}

#[test]
fn mv_restore_writes_the_stored_value_back_into_env() {
    let tmp = project();
    fs::write(tmp.path().join(".env"), "RESTORE_ME=restorevalue1\n").unwrap();
    act(&tmp)
        .args(["env", "mv", "RESTORE_ME"])
        .assert()
        .success();
    assert!(
        fs::read_to_string(tmp.path().join(".env"))
            .unwrap()
            .contains("RESTORE_ME=#act:sequestered")
    );
    act(&tmp)
        .args(["env", "mv", "RESTORE_ME", "--restore"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(tmp.path().join(".env")).unwrap(),
        "RESTORE_ME=restorevalue1\n"
    );
}

#[test]
fn run_forwards_piped_stdin_to_the_child() {
    let tmp = project();
    act(&tmp)
        .args(["env", "run", "--", "sh", "-c", "read line; echo got:$line"])
        .write_stdin("piped-data\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("got:piped-data"));
}
