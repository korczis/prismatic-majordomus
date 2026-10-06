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
    // What the help says about what this executable changes. It used to assert the word
    // "read-only", which was true while no capability wrote anything and stopped being true
    // with `plan.transition` (ADR 0040). The assertion is kept and made to hold the accurate
    // statement instead of the obsolete one: the help must still tell a reader that the
    // registry is overwhelmingly queries and that the exceptions name themselves.
    assert!(out.contains("query"), "{out}");
    assert!(out.contains("commands"), "{out}");
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
        "output       ResourceView",
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
    assert_eq!(v["title"], "ResourceView");
    // the two shapes objects.get answers, tagged by `source`
    let variants = v["oneOf"].as_array().expect("a tagged union");
    assert_eq!(variants.len(), 2, "{v}");
    assert!(
        v.to_string().contains("\"ObjectView\"") && v.to_string().contains("\"AnswerView\""),
        "{v}"
    );
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

/// Copy a directory tree into the fixture, keeping the relative layout.
fn copy_tree(from: &std::path::Path, to: &str, f: &Fixture) {
    for entry in std::fs::read_dir(from).unwrap() {
        let p = entry.unwrap().path();
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let target = format!("{to}/{name}");
        if p.is_dir() {
            copy_tree(&p, &target, f);
        } else {
            f.write(&target, &std::fs::read_to_string(&p).unwrap());
        }
    }
}

#[test]
fn the_share_directory_is_found_in_the_repository_when_no_override_is_given() {
    let f = Fixture::new();
    // a repository that carries the distribution itself, as this one does
    let dist = common::dist_share();
    // the schema root is a tree — <vendor>/<name>/<name>.v<n>.{proto,schema.json} — so it
    // is copied as one, not as a directory listing. share/schemas/generated/ comes with it:
    // those are the contracts of the *generated documents* rather than a kind's, and kind
    // discovery tells the two apart by the vendor level rather than by what was copied.
    copy_tree(&dist.join("schemas"), "share/schemas", &f);
    f.write(
        "share/kinds.yaml",
        &std::fs::read_to_string(dist.join("kinds.yaml")).unwrap(),
    );
    f.write(
        "share/skeleton/ai/repo/scope.yaml",
        &std::fs::read_to_string(dist.join("skeleton/ai/repo/scope.yaml")).unwrap(),
    );
    // the distribution model too: a repository that carries the distribution carries this,
    // and the capabilities that read a release target have nothing to be timed on without it
    f.write(
        "share/distribution.yaml",
        &std::fs::read_to_string(dist.join("distribution.yaml")).unwrap(),
    );
    f.commit("distribution");
    let out = std::process::Command::new(BIN)
        .args(["capabilities", "validate"])
        .current_dir(f.root())
        .env_remove("MAJORDOMUS_SHARE")
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    // the findings are on stdout; a failure that printed only stderr said nothing about why
    assert_eq!(
        out.status.code(),
        Some(0),
        "{text}{}",
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

// ------------------------------------------------------------------- release version, bump

/// A fixture laid out as the tool's own tree — the manifest, its lock and the projection
/// the shell tool reads, all stating `version` — with `registry` committed as the registry
/// projection and the commit tagged `v<version>`: a release git knows of.
fn released(f: &Fixture, registry: &serde_json::Value, version: &str) {
    use majordomus_cli::release::{surface, version as v};
    f.write(
        v::MANIFEST,
        &format!("[package]\nname = \"majordomus-cli\"\nversion = \"{version}\"\n"),
    );
    f.write(
        v::LOCK,
        &format!("[[package]]\nname = \"majordomus-cli\"\nversion = \"{version}\"\n"),
    );
    f.write(v::PROJECTION, &v::render_projection(version));
    f.write(surface::REGISTRY, &serde_json::to_string(registry).unwrap());
    f.commit(&format!("chore: release {version}"));
    f.git(&["tag", &format!("v{version}")]);
}

/// A fix, committed after the release.
fn fix_since(f: &Fixture) {
    f.write("notes.txt", "repaired\n");
    f.commit("fix: repair the thing");
}

fn release(f: &Fixture, args: &[&str]) -> (i32, String) {
    let mut all = vec!["release"];
    all.extend_from_slice(args);
    let (code, out, err) = run_in(&f.root(), &all, "");
    (code, format!("{out}{err}"))
}

/// The surface grew since the release — every capability is new against an empty registry
/// — and the commits say only "fix": the contract decides a minor, the commits are shown as
/// the evidence they are, and the writer raises to the report's `next`, refuses to go under
/// it, carries an explicit override's provenance, and says when there is nothing to write.
#[test]
fn release_version_and_bump_take_the_contracts_answer_over_the_commits() {
    let f = Fixture::new();
    let empty =
        serde_json::json!({"schema": "majordomus/capability-registry/v1", "capabilities": []});
    released(&f, &empty, "0.1.0");
    fix_since(&f);

    let (code, out) = release(&f, &["version"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("last release 0.1.0"), "{out}");
    assert!(out.contains("commits      1 since it"), "{out}");
    assert!(
        out.contains("next         0.2.0 (decided by the contract)"),
        "{out}"
    );
    assert!(
        out.contains("commits imply patch -> 0.1.1 (evidence; the contract decides)"),
        "{out}"
    );
    assert!(
        !out.contains("because"),
        "a measured contract gave a reason not to: {out}"
    );

    let (code, out) = release(&f, &["bump", "--dry-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "release: 0.1.0 -> 0.2.0 (minor required since v0.1.0, from the public contract)"
        ),
        "{out}"
    );
    // the commit subjects understate the window, and the writer says so
    assert!(
        out.contains("classify this window as patch and the contract moved by minor"),
        "{out}"
    );
    assert!(out.contains("(unwritten)"), "{out}");
    // the layer's record of the version is part of what the writer would write
    assert!(
        out.contains(".ai/manifest.yaml written_for (unwritten)"),
        "{out}"
    );
    assert!(!out.contains("explicit override"), "{out}");

    // an override under the contract's floor is refused, and nothing is written
    let (code, out) = release(&f, &["bump", "--level", "patch"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("REFUSED 0.1.1 is below 0.2.0"), "{out}");
    let manifest = std::fs::read_to_string(f.path("apps/majordomus-cli/Cargo.toml")).unwrap();
    assert!(manifest.contains("version = \"0.1.0\""), "{manifest}");

    // one above it is allowed, and never silent about what was measured
    let (code, out) = release(&f, &["bump", "--exact", "0.3.0", "--dry-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("from explicit --exact"), "{out}");
    assert!(
        out.contains("required 0.2.0, selected 0.3.0; source: explicit override"),
        "{out}"
    );

    // the write: the report's `next`, in the manifest and the lock
    let (code, out) = release(&f, &["bump"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("now declares 0.2.0"), "{out}");
    let manifest = std::fs::read_to_string(f.path("apps/majordomus-cli/Cargo.toml")).unwrap();
    assert!(manifest.contains("version = \"0.2.0\""), "{manifest}");
    // the layer is written for the version it now declares, stamped beside its schema
    let layer = || std::fs::read_to_string(f.path(".ai/manifest.yaml")).unwrap();
    assert!(
        layer().contains("schema: ai-repository/v1\nwritten_for: \"0.2.0\"\n"),
        "{}",
        layer()
    );
    assert!(out.contains(".ai/manifest.yaml written"), "{out}");

    // a layer whose record fell behind a version that stands is stamped again, and only it
    f.write(
        ".ai/manifest.yaml",
        &layer().replace("written_for: \"0.2.0\"", "written_for: \"0.1.0\""),
    );
    let (code, out) = release(&f, &["bump"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("the version is already 0.2.0; .ai/manifest.yaml stamped with it"),
        "{out}"
    );
    assert!(layer().contains("written_for: \"0.2.0\""), "{}", layer());

    // a dry run at the version that stands writes nothing, a stale record included
    f.write(
        ".ai/manifest.yaml",
        &layer().replace("written_for: \"0.2.0\"", "written_for: \"0.1.0\""),
    );
    let (code, out) = release(&f, &["bump", "--dry-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("nothing written"), "{out}");
    assert!(layer().contains("written_for: \"0.1.0\""), "{}", layer());
    // a layer the writer cannot write is an error, never a record left silently behind
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let path = f.path(".ai/manifest.yaml");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
        let (code, out) = release(&f, &["bump"]);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_ne!(
            code, 0,
            "an unwritable layer was reported as stamped: {out}"
        );
        assert!(layer().contains("written_for: \"0.1.0\""), "{}", layer());
    }
    let (code, out) = release(&f, &["bump"]);
    assert_eq!(code, 0, "{out}");
    assert!(layer().contains("written_for: \"0.2.0\""), "{}", layer());

    // and again: the version already covers what the contract requires
    let (code, out) = release(&f, &["bump"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "the version is already 0.2.0, which covers the minor the contract requires since v0.1.0; nothing written"
        ),
        "{out}"
    );
}

/// The surface is the release's to the byte and a fix landed since: the contract requires
/// no release, so the commits make it a patch — and both the report and the writer name the
/// two of them as the deciders.
#[test]
fn an_unchanged_contract_and_a_fix_are_a_patch_decided_by_both() {
    let f = Fixture::new();
    let surface =
        majordomus_cli::generate::registry_manifest(&common::load_app(&f).context.registry);
    released(&f, &surface, "0.1.0");
    fix_since(&f);

    let (code, out) = release(&f, &["version"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("next         0.1.1 (decided by the contract and the commits)"),
        "{out}"
    );
    assert!(
        out.contains("(evidence; the contract requires no release, so a patch carries them)"),
        "{out}"
    );

    let (code, out) = release(&f, &["bump", "--dry-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains(
            "0.1.0 -> 0.1.1 (none required since v0.1.0, from the public contract and the commits)"
        ),
        "{out}"
    );
}

/// Nothing published: the contract cannot be measured, so `release version` decides nothing
/// and says why, the commit inference is labelled as evidence only, and `release bump`
/// without a target refuses to guess rather than raising to what the commits say.
#[test]
fn with_nothing_published_the_report_decides_nothing_and_the_writer_refuses_to_guess() {
    let f = Fixture::new();
    use majordomus_cli::release::version as v;
    f.write(
        v::MANIFEST,
        "[package]\nname = \"majordomus-cli\"\nversion = \"0.1.0\"\n",
    );
    f.write(v::PROJECTION, &v::render_projection("0.1.0"));
    f.commit("feat: the first feature");

    let (code, out) = release(&f, &["version"]);
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("next         — (undecided: the contract could not be measured)"),
        "{out}"
    );
    assert!(out.contains("             because "), "{out}");
    assert!(out.contains("name it deliberately"), "{out}");
    assert!(
        out.contains("(evidence only; it does not answer in the contract's place)"),
        "{out}"
    );

    let (code, out) = release(&f, &["bump"]);
    assert_eq!(code, 12, "{out}");
    assert!(
        out.contains("the public contract cannot be measured here, so there is no bump to derive"),
        "{out}"
    );
    assert!(out.contains("name the version deliberately"), "{out}");
    let manifest = std::fs::read_to_string(f.path("apps/majordomus-cli/Cargo.toml")).unwrap();
    assert!(
        manifest.contains("version = \"0.1.0\""),
        "nothing is written: {manifest}"
    );
}

/// Runs `majordomus <args>` in the fixture with a stdout whose reader is already gone: the
/// other end of a socket pair, closed before the process starts, so its first write fails
/// with a broken pipe — every time, with no race against a reader that closes late.
#[cfg(unix)]
fn with_stdout_gone(f: &Fixture, args: &[&str]) -> (i32, String) {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;
    let (stdout, reader) = UnixStream::pair().expect("a socket pair");
    drop(reader);
    let out = Command::new(BIN)
        .args(args)
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .stdin(Stdio::null())
        .stdout(Stdio::from(OwnedFd::from(stdout)))
        .stderr(Stdio::piped())
        .output()
        .expect("spawn majordomus");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    )
}

/// A reader that has gone away is a failure the exit code states, never a success: the
/// report and the refusal are each one write, so the broken pipe meets the one write and
/// comes back as the transport failure it is — and the writer, refusing, writes nothing.
#[cfg(unix)]
#[test]
fn release_version_and_bump_fail_when_nobody_reads_what_they_print() {
    let f = Fixture::new();
    use majordomus_cli::release::version as v;
    f.write(
        v::MANIFEST,
        "[package]\nname = \"majordomus-cli\"\nversion = \"0.1.0\"\n",
    );
    f.write(v::PROJECTION, &v::render_projection("0.1.0"));
    f.commit("feat: the first feature");

    for args in [&["release", "version"][..], &["release", "bump"][..]] {
        let (code, err) = with_stdout_gone(&f, args);
        assert_eq!(code, 13, "{args:?}: {err}");
        assert!(err.contains("majordomus: transport:"), "{args:?}: {err}");
    }
    let manifest = std::fs::read_to_string(f.path("apps/majordomus-cli/Cargo.toml")).unwrap();
    assert!(
        manifest.contains("version = \"0.1.0\""),
        "nothing is written: {manifest}"
    );
}
