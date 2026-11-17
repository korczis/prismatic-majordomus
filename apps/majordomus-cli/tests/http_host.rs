//! The interface a local server binds: flag, then `MAJORDOMUS_HTTP_HOST`, then loopback;
//! a linked worktree does not take the variable.
//!
//! One test, in a binary of its own, because it writes this process's environment: a
//! second test beside it would race it for the variable, and a test in a shared binary
//! would hand a non-loopback host to whichever neighbour started a server at that moment.
//! `test/cases/992_the_local_bind_is_the_machines_to_name.sh` holds the same resolution
//! from outside, through the three commands that start a server.

use majordomus_cli::cli::{
    checkout_http_host, local_http_host, resolve_http_host, HostOrigin, HTTP_HOST_ENV,
    LOOPBACK_HOST,
};

#[test]
fn the_host_is_the_flag_then_the_variable_then_loopback() {
    // the pure resolution, every arm
    assert_eq!(
        resolve_http_host(Some("10.0.0.1"), Some("0.0.0.0")),
        ("10.0.0.1".to_string(), HostOrigin::Flag)
    );
    assert_eq!(
        resolve_http_host(None, Some(" 0.0.0.0 ")),
        ("0.0.0.0".to_string(), HostOrigin::Environment)
    );
    assert_eq!(
        resolve_http_host(None, Some("   ")),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default)
    );
    assert_eq!(
        resolve_http_host(None, None),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default)
    );

    // and against the environment of this process, which is what a server reads
    std::env::remove_var(HTTP_HOST_ENV);
    assert_eq!(
        local_http_host(None),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default),
        "nothing names an interface, so the bind is loopback"
    );

    std::env::set_var(HTTP_HOST_ENV, "0.0.0.0");
    assert_eq!(
        local_http_host(None),
        ("0.0.0.0".to_string(), HostOrigin::Environment),
        "the variable names the interface when no flag does"
    );
    assert_eq!(
        local_http_host(Some("127.0.0.1")),
        ("127.0.0.1".to_string(), HostOrigin::Flag),
        "the command line wins over the variable"
    );

    std::env::set_var(HTTP_HOST_ENV, "");
    assert_eq!(
        local_http_host(None),
        (LOOPBACK_HOST.to_string(), HostOrigin::Default),
        "a variable that is set and blank is unset"
    );

    // per checkout: the primary takes the variable, a linked worktree of it does not
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    };
    std::fs::create_dir_all(&root).unwrap();
    git(&["init", "-q"]);
    git(&[
        "-c",
        "user.email=t@example.com",
        "-c",
        "user.name=t",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "init",
    ]);
    let wt = dir.path().join("repo-wt");
    git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "feature/x",
        wt.to_str().unwrap(),
    ]);
    std::env::set_var(HTTP_HOST_ENV, "0.0.0.0");
    assert_eq!(
        checkout_http_host(None, &root),
        ("0.0.0.0".to_string(), HostOrigin::Environment),
        "the primary checkout binds what the machine names"
    );
    assert_eq!(
        checkout_http_host(None, &wt),
        (LOOPBACK_HOST.to_string(), HostOrigin::LinkedWorktree),
        "a linked worktree stays on loopback"
    );
    assert_eq!(
        checkout_http_host(Some("0.0.0.0"), &wt),
        ("0.0.0.0".to_string(), HostOrigin::Flag),
        "a flag binds where it says, in a worktree too"
    );
    std::env::remove_var(HTTP_HOST_ENV);
}
