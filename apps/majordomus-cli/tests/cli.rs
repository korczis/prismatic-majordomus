//! The public command line, black-box: exit codes, stdout, stderr.

mod common;

use common::{run_in, Fixture, BIN};
use std::process::Command;

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN).args(args).output().expect("spawn");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn help_lists_the_one_command_and_exits_zero() {
    let (code, out, err) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("Usage: majordomus <COMMAND>"), "{out}");
    assert!(out.contains("mcp"), "{out}");
    assert!(out.contains("read-only"), "{out}");
    let commands = out
        .split("Commands:")
        .nth(1)
        .unwrap()
        .split("Options:")
        .next()
        .unwrap();
    assert!(
        !commands.contains("doctor"),
        "help advertises a command that does not exist:\n{out}"
    );
    assert!(err.is_empty(), "help wrote to stderr: {err}");
}

#[test]
fn version_is_the_crate_version() {
    let (code, out, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert_eq!(
        out.trim(),
        format!("majordomus {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn mcp_help_names_transport_discovery_and_inspect() {
    let (code, out, err) = run(&["mcp", "--help"]);
    assert_eq!(code, 0);
    for needle in [
        "Usage: majordomus mcp",
        "--discovery",
        "--strict",
        "--inspect",
        "--format",
        "--transport",
        "stdio",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in:\n{out}");
    }
    assert!(err.is_empty());
}

#[test]
fn no_arguments_is_a_usage_error() {
    let (code, out, err) = run(&[]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(err.contains("Usage"), "{err}");
}

#[test]
fn unknown_command_is_a_usage_error_on_stderr() {
    let (code, out, err) = run(&["nonsense"]);
    assert_eq!(code, 2);
    assert!(out.is_empty(), "usage error leaked to stdout: {out}");
    assert!(err.contains("unrecognized subcommand 'nonsense'"), "{err}");
}

#[test]
fn unknown_option_is_a_usage_error() {
    let (code, out, err) = run(&["mcp", "--no-such-option"]);
    assert_eq!(code, 2);
    assert!(out.is_empty());
    assert!(
        err.contains("unexpected argument '--no-such-option'"),
        "{err}"
    );
}

#[test]
fn invalid_explicit_repo_is_refused_not_replaced() {
    let f = Fixture::new();
    let missing = f.path("does-not-exist");
    let (code, out, err) = run_in(
        &f.root(),
        &["mcp", "--repo", missing.to_str().unwrap(), "--inspect"],
        "",
    );
    assert_eq!(code, 13, "{err}");
    assert!(out.is_empty());
    assert!(err.contains("does-not-exist"), "{err}");
}

#[test]
fn explicit_repo_overrides_the_working_directory() {
    let f = Fixture::new();
    let elsewhere = Fixture::plain_dir();
    let (code, out, _) = run_in(
        &elsewhere.root(),
        &["mcp", "--repo", f.root().to_str().unwrap(), "--inspect"],
        "",
    );
    assert_eq!(code, 0);
    assert!(
        out.contains(&format!("repository  {}", f.root().display())),
        "{out}"
    );
}
