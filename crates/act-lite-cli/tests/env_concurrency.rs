use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

/// Build an isolated HOME + git project so the store lands under a temp dir.
fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join(".git")).unwrap();
    fs::create_dir(tmp.path().join("home")).unwrap();
    tmp
}

fn act(tmp: &tempfile::TempDir) -> Command {
    let mut cmd = Command::new(assert_cmd::cargo::cargo_bin("act-lite"));
    cmd.current_dir(tmp.path());
    cmd.env("HOME", tmp.path().join("home"));
    cmd.env("ACT_CONFIG_DIR", tmp.path().join("home").join("act"));
    cmd
}

#[test]
fn concurrent_adds_keep_every_value() {
    const N: usize = 20;
    let tmp = project();

    // Start every add before waiting on any, so their store writes overlap.
    let children: Vec<_> = (1..=N)
        .map(|i| {
            let mut child = act(&tmp)
                .args(["env", "add", &format!("K{i}"), "--stdin"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("value-number-{i}").as_bytes())
                .unwrap();
            child
        })
        .collect();
    for (i, child) in (1..=N).zip(children) {
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success(), "add K{i}: {out:?}");
    }

    let list = act(&tmp).args(["env", "list"]).output().unwrap();
    assert!(list.status.success(), "list: {list:?}");
    let mut keys: Vec<String> = String::from_utf8(list.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect();
    keys.sort();
    let mut expected: Vec<String> = (1..=N).map(|i| format!("K{i}")).collect();
    expected.sort();
    assert_eq!(keys, expected);

    for i in 1..=N {
        let get = act(&tmp)
            .args(["env", "get", &format!("K{i}"), "--reveal"])
            .output()
            .unwrap();
        assert!(get.status.success(), "get K{i}: {get:?}");
        assert_eq!(
            String::from_utf8(get.stdout).unwrap(),
            format!("value-number-{i}\n")
        );
    }
}

#[test]
fn concurrent_mv_never_loses_a_value() {
    const N: usize = 8;
    const TRIALS: usize = 5;
    for trial in 1..=TRIALS {
        let tmp = project();
        let env: String = (1..=N)
            .map(|i| format!("K{i}=value-number-{i}\n"))
            .collect();
        fs::write(tmp.path().join(".env"), &env).unwrap();

        // Start every mv before waiting on any, so their writes overlap.
        let children: Vec<_> = (1..=N)
            .map(|i| {
                act(&tmp)
                    .args(["env", "mv", &format!("K{i}")])
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        for (i, child) in (1..=N).zip(children) {
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "trial {trial}: mv K{i}: {out:?}");
        }

        let expected_env: String = (1..=N)
            .map(|i| format!("K{i}=#act:sequestered\n"))
            .collect();
        assert_eq!(
            fs::read_to_string(tmp.path().join(".env")).unwrap(),
            expected_env,
            "trial {trial}"
        );
        for i in 1..=N {
            let get = act(&tmp)
                .args(["env", "get", &format!("K{i}"), "--reveal"])
                .output()
                .unwrap();
            assert!(get.status.success(), "trial {trial}: get K{i}: {get:?}");
            assert_eq!(
                String::from_utf8(get.stdout).unwrap(),
                format!("value-number-{i}\n"),
                "trial {trial}"
            );
        }
    }
}

#[test]
fn concurrent_import_and_rm_with_adds() {
    const N: usize = 20;
    const TRIALS: usize = 5;
    for trial in 1..=TRIALS {
        let tmp = project();
        let mut add_r = act(&tmp)
            .args(["env", "add", "R", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        add_r.stdin.take().unwrap().write_all(b"old").unwrap();
        let out = add_r.wait_with_output().unwrap();
        assert!(out.status.success(), "trial {trial}: add R: {out:?}");
        let imp = tmp.path().join("imp.env");
        fs::write(&imp, "I1=a\nI2=b\nI3=c\n").unwrap();

        // Start every command before waiting on any, so their store writes overlap.
        let mut children = Vec::new();
        for i in 1..=N {
            let mut child = act(&tmp)
                .args(["env", "add", &format!("K{i}"), "--stdin"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(format!("value-number-{i}").as_bytes())
                .unwrap();
            children.push((format!("add K{i}"), child));
            if i == N / 2 {
                for (label, args) in [
                    ("rm R", vec!["env", "rm", "R"]),
                    ("import", vec!["env", "import", imp.to_str().unwrap()]),
                ] {
                    let child = act(&tmp)
                        .args(args)
                        .stdout(Stdio::null())
                        .stderr(Stdio::piped())
                        .spawn()
                        .unwrap();
                    children.push((label.to_string(), child));
                }
            }
        }
        for (label, child) in children {
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success(), "trial {trial}: {label}: {out:?}");
        }

        let list = act(&tmp).args(["env", "list"]).output().unwrap();
        assert!(list.status.success(), "trial {trial}: list: {list:?}");
        let mut keys: Vec<String> = String::from_utf8(list.stdout)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        keys.sort();
        let mut expected: Vec<String> = ["I1", "I2", "I3"].map(String::from).to_vec();
        expected.extend((1..=N).map(|i| format!("K{i}")));
        expected.sort();
        assert_eq!(keys, expected, "trial {trial}");

        let values = (1..=N)
            .map(|i| (format!("K{i}"), format!("value-number-{i}")))
            .chain([("I1", "a"), ("I2", "b"), ("I3", "c")].map(|(k, v)| (k.into(), v.into())));
        for (key, value) in values {
            let get = act(&tmp)
                .args(["env", "get", &key, "--reveal"])
                .output()
                .unwrap();
            assert!(get.status.success(), "trial {trial}: get {key}: {get:?}");
            assert_eq!(
                String::from_utf8(get.stdout).unwrap(),
                format!("{value}\n"),
                "trial {trial}"
            );
        }

        let marked: Vec<String> = fs::read_to_string(&imp)
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        assert_eq!(
            marked,
            [
                "I1=#act:sequestered",
                "I2=#act:sequestered",
                "I3=#act:sequestered"
            ],
            "trial {trial}"
        );
    }
}
