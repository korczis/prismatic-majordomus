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

#[test]
fn new_commands_have_help_and_honest_exit_codes() {
    for args in [
        &["serve", "--help"][..],
        &["capabilities", "--help"],
        &["capabilities", "list", "--help"],
        &["generate", "--help"],
    ] {
        let (code, out, err) = run(args);
        assert_eq!(code, 0, "{args:?}: {err}");
        assert!(out.contains("Usage: majordomus"), "{out}");
    }
    let (_, out, _) = run(&["--help"]);
    for c in ["mcp", "serve", "capabilities", "generate"] {
        assert!(
            out.contains(&format!("\n  {c} ")),
            "top-level help lacks {c}:\n{out}"
        );
    }
    let f = Fixture::new();
    let (code, _, err) = run_in(&f.root(), &["capabilities", "describe", "nope.x"], "");
    assert_eq!(code, 12, "{err}");
    assert!(err.contains("unknown capability: nope.x"), "{err}");
    let (code, out, _) = run_in(
        &f.root(),
        &[
            "capabilities",
            "describe",
            "objects.get",
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["exposure"]["http"]["path"], "/api/v1/object");
    let (code, out, _) = run_in(&f.root(), &["capabilities", "schema", "objects.get"], "");
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["properties"]["uri"].is_object());
    let (code, out, _) = run_in(&f.root(), &["capabilities", "validate"], "");
    assert_eq!(code, 0);
    assert!(
        out.contains("validate: 0 failure(s)") && out.contains("OK   openapi"),
        "{out}"
    );
    let (code, out, _) = run_in(
        &f.root(),
        &["capabilities", "list", "--exposure", "cli"],
        "",
    );
    assert_eq!(code, 0);
    assert!(
        out.contains("capabilities.list")
            && out.contains("capabilities.describe")
            && !out.contains("objects.get"),
        "{out}"
    );
    let (code, _, err) = run_in(
        &f.root(),
        &["capabilities", "list", "--kind", "nonsense"],
        "",
    );
    assert_ne!(code, 0);
    assert!(err.contains("not query, command or resource"), "{err}");
}

#[test]
fn capabilities_text_output_names_projections_and_provenance() {
    let f = Fixture::new();
    let (code, out, _) = run_in(&f.root(), &["capabilities", "list", "--kind", "query"], "");
    assert_eq!(code, 0);
    assert!(
        out.contains("objects.get")
            && out.contains("mcp=tool:majordomus_get")
            && out.contains("http=GET /api/v1/object"),
        "{out}"
    );
    assert!(out.contains("cli=capabilities list"), "{out}");
    assert!(
        out.lines().last().unwrap().starts_with("capabilities: "),
        "{out}"
    );
    let (code, out, _) = run_in(&f.root(), &["capabilities", "describe", "objects.get"], "");
    assert_eq!(code, 0);
    for needle in [
        "id           objects.get",
        "kind         query",
        "http         GET /api/v1/object",
        "cli          none",
        "input        GetInput",
        "output       ObjectView",
        "provenance   builtin ",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in:\n{out}");
    }
    let (code, out, _) = run_in(
        &f.root(),
        &["capabilities", "describe", "rule.project.alpha@1"],
        "",
    );
    assert_eq!(code, 0);
    assert!(
        out.contains("provenance   .ai/repo/rules/project/alpha.v1.md (class rule, section rules)"),
        "{out}"
    );
    assert!(
        out.contains("mcp          {\"resource\":{\"uri\":\"majordomus://rule/project.alpha@1\""),
        "{out}"
    );
    assert!(out.contains("http         none"), "{out}");
    let (code, out, _) = run_in(
        &f.root(),
        &["capabilities", "describe", "claim.policy-parse"],
        "",
    );
    assert_eq!(code, 0);
    assert!(
        out.contains("docs/CLAIMS.yaml#claims.0") || out.contains("docs/CLAIMS.yaml (class claims"),
        "{out}"
    );
    let (code, out, _) = run_in(
        &f.root(),
        &["capabilities", "schema", "objects.get", "--side", "output"],
        "",
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["title"], "ObjectView");
    let (code, out, _) = run_in(
        &f.root(),
        &[
            "mcp",
            "--inspect",
            "--format",
            "json",
            "--discovery",
            "filesystem",
        ],
        "",
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["repository"]["repository"]["discovery"], "filesystem");
}

#[test]
fn the_share_directory_is_found_in_the_repository_when_no_override_is_given() {
    let f = Fixture::new();
    // a repository that carries the distribution itself, as this one does
    let dist = common::dist_share();
    for entry in std::fs::read_dir(dist.join("schemas")).unwrap() {
        let p = entry.unwrap().path();
        f.write(
            &format!("share/schemas/{}", p.file_name().unwrap().to_str().unwrap()),
            &std::fs::read_to_string(&p).unwrap(),
        );
    }
    f.write(
        "share/kinds.yaml",
        &std::fs::read_to_string(dist.join("kinds.yaml")).unwrap(),
    );
    f.commit("distribution");
    let out = std::process::Command::new(BIN)
        .args(["capabilities", "validate"])
        .current_dir(f.root())
        .env_remove("MAJORDOMUS_SHARE")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("(repository)"), "{text}");
    assert!(text.contains("kinds from share/kinds.yaml"), "{text}");
    // an explicit share without kinds.yaml is an error, never a fallback
    let (code, _, err) = run_in(
        &f.root(),
        &[
            "capabilities",
            "validate",
            "--share",
            f.path("docs").to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(code, 12, "{err}");
    assert!(err.contains("no share directory holds kinds.yaml"), "{err}");
}
