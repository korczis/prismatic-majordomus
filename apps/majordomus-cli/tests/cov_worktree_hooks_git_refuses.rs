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

/// The stand-in: refuses (exit 128, with words) any run whose arguments contain
/// `$REFUSE_ARGS`, and hands every other run to the real git.
fn stand_in(dir: &Path, real: &Path) {
    let script = format!(
        "#!/bin/sh\ncase \" $* \" in\n  *\" $REFUSE_ARGS \"*)\n    \
         echo \"fatal: refused by the test\" >&2\n    exit 128 ;;\nesac\nexec '{}' \"$@\"\n",
        real.display()
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
    for (name, refuse, version_one) in [
        // whether the extension is on
        ("bool", "--type=bool --get extensions.worktreeConfig", false),
        // the repository's format version
        ("version", "--get core.repositoryformatversion", false),
        // writing the format version
        ("write-version", "core.repositoryformatversion 1", false),
        // writing the extension, in a repository already at version 1
        ("write-extension", "extensions.worktreeConfig true", true),
    ] {
        let (primary, wt) = fixture(&real, &root, name);
        if version_one {
            git(
                &real,
                &primary,
                &["config", "core.repositoryformatversion", "1"],
            );
        }
        std::env::set_var("REFUSE_ARGS", refuse);
        let message = failed(carry_hooks_path(&primary, &wt));
        assert!(message.contains("refused by the test"), "{name}: {message}");
        // nothing was bound: the worktree has no value of its own
        assert!(
            Command::new(&real)
                .arg("-C")
                .arg(&wt)
                .args(["config", "--worktree", "--get", "core.hooksPath"])
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .output()
                .unwrap()
                .stdout
                .is_empty(),
            "{name}"
        );
    }
    std::env::remove_var("REFUSE_ARGS");
    std::env::set_var("PATH", &original);
}
