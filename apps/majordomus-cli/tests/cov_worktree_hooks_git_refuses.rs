//! `carry_hooks_path` when git cannot be run, or refuses one particular read or write that a
//! real repository will not refuse on its own: every such outcome is a reported `Failed`,
//! never a panic, and never a worktree half-changed without saying so (I2718).
//!
//! The process's `PATH` is replaced to put a stand-in `git` in front of the real one, so
//! this file holds one test: nothing else in its process may run git meanwhile.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use majordomus_cli::worktree::{carry_hooks_path, HooksPath};

/// The real git, found on the `PATH` this test started with.
fn real_git() -> PathBuf {
    let path = std::env::var_os("PATH").expect("PATH");
    std::env::split_paths(&path)
        .map(|d| d.join("git"))
        .find(|g| g.is_file())
        .expect("git on PATH")
}

fn git(real: &Path, cwd: &Path, args: &[&str]) -> String {
    let out = Command::new(real)
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A primary checkout with a tracked `.githooks/` named by its absolute path, and one
/// linked worktree `wt` that therefore runs the primary's hooks.
fn fixture(real: &Path, root: &Path, name: &str) -> (PathBuf, PathBuf) {
    let primary = root.join(name);
    std::fs::create_dir_all(primary.join(".githooks")).unwrap();
    std::fs::write(primary.join(".githooks/pre-commit"), "#!/bin/sh\n").unwrap();
    git(
        real,
        root,
        &["init", "-q", "-b", "master", primary.to_str().unwrap()],
    );
    git(real, &primary, &["add", "."]);
    git(
        real,
        &primary,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "hooks",
        ],
    );
    let hooks = primary.join(".githooks");
    git(
        real,
        &primary,
        &["config", "core.hooksPath", hooks.to_str().unwrap()],
    );
    let wt = root.join(format!("{name}-wt")).join("topic");
    git(
        real,
        &primary,
        &["worktree", "add", "-q", "-b", "topic", wt.to_str().unwrap()],
    );
    (primary, wt)
}

/// The stand-in. A run whose arguments contain `$STUB_ARGS` is answered by the stand-in —
/// `$STUB_TEXT` on standard output when `$STUB_DO` is `print`, else a refusal (exit 128,
/// with words) — once armed: at once when `$STUB_ARM` is empty, else only after a run whose
/// arguments contain `$STUB_ARM` has been handed to the real git. Every other run is the
/// real git's.
fn stand_in(dir: &Path, real: &Path) {
    let real = real.display();
    let script = format!(
        r#"#!/bin/sh
case " $* " in
  *" $STUB_ARGS "*)
    if [ -z "$STUB_ARM" ] || [ -e "$STUB_MARK" ]; then
      if [ "$STUB_DO" = print ]; then printf '%s\n' "$STUB_TEXT"; exit 0; fi
      echo "fatal: refused by the test" >&2
      exit 128
    fi ;;
esac
if [ -n "$STUB_ARM" ]; then
  case " $* " in
    *" $STUB_ARM "*) '{real}' "$@"; rc=$?; : > "$STUB_MARK"; exit $rc ;;
  esac
fi
exec '{real}' "$@"
"#
    );
    let path = dir.join("git");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn failed(outcome: HooksPath) -> String {
    match outcome {
        HooksPath::Failed { message } => message,
        other => panic!("expected a failure, got {other:?}"),
    }
}

#[test]
fn every_git_run_carry_hooks_path_makes_can_fail_and_is_reported() {
    let real = real_git();
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let bin = root.join("bin");
    let empty = root.join("empty");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&empty).unwrap();
    stand_in(&bin, &real);
    let original = std::env::var_os("PATH").unwrap();

    // No git at all: it cannot even be started.
    let (primary, wt) = fixture(&real, &root, "none");
    std::env::set_var("PATH", &empty);
    let message = failed(carry_hooks_path(&primary, &wt));
    std::env::set_var("PATH", &original);
    assert!(message.contains("running git in"), "{message}");

    // Each read or write refused in turn; `fixture` uses the real git directly.
    let mut path = vec![bin.clone()];
    path.extend(std::env::split_paths(&original));
    std::env::set_var("PATH", std::env::join_paths(path).unwrap());
    struct Case {
        name: &'static str,
        args: &'static str,
        print: Option<&'static str>,
        arm: &'static str,
        version_one: bool,
        says: &'static str,
    }
    let refuse = |name, args| Case {
        name,
        args,
        print: None,
        arm: "",
        version_one: false,
        says: "refused by the test",
    };
    for case in [
        // whether the extension is on
        refuse("bool", "--type=bool --get extensions.worktreeConfig"),
        // whether core.worktree is in the shared file
        refuse("core-worktree", "--get core.worktree"),
        // ... and it is
        Case {
            print: Some("/somewhere"),
            says: "core.worktree is set in the shared configuration (/somewhere)",
            ..refuse("core-worktree-set", "--get core.worktree")
        },
        // the repository's format version
        refuse("version", "--get core.repositoryformatversion"),
        // writing the format version
        refuse("write-version", "core.repositoryformatversion 1"),
        // writing the extension, in a repository already at version 1
        Case {
            version_one: true,
            ..refuse("write-extension", "extensions.worktreeConfig true")
        },
        // the hooks directory asked again, after the value was set, to verify it
        Case {
            arm: "config --worktree core.hooksPath",
            ..refuse("verify", "--git-path hooks")
        },
    ] {
        let (primary, wt) = fixture(&real, &root, case.name);
        if case.version_one {
            git(
                &real,
                &primary,
                &["config", "core.repositoryformatversion", "1"],
            );
        }
        std::env::set_var("STUB_ARGS", case.args);
        std::env::set_var("STUB_DO", if case.print.is_some() { "print" } else { "" });
        std::env::set_var("STUB_TEXT", case.print.unwrap_or(""));
        std::env::set_var("STUB_ARM", case.arm);
        std::env::set_var("STUB_MARK", root.join(format!("{}.armed", case.name)));
        let message = failed(carry_hooks_path(&primary, &wt));
        assert!(message.contains(case.says), "{}: {message}", case.name);
        let bound = Command::new(&real)
            .arg("-C")
            .arg(&wt)
            .args(["config", "--worktree", "--get", "core.hooksPath"])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap()
            .stdout;
        if case.arm.is_empty() {
            // refused before anything was written: the worktree has no value of its own
            assert!(bound.is_empty(), "{}", case.name);
        } else {
            // written, and then the verification could not be made: reported, not hidden
            assert!(!bound.is_empty(), "{}", case.name);
            assert!(root.join(format!("{}.armed", case.name)).exists());
        }
    }
    for key in ["STUB_ARGS", "STUB_DO", "STUB_TEXT", "STUB_ARM", "STUB_MARK"] {
        std::env::remove_var(key);
    }
    std::env::set_var("PATH", &original);
}
