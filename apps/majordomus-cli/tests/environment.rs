//! The half of the shell entry point that is this executable's: the fast resolution the
//! adapter calls on every `cd`, the script it prints for `eval`, and the banner.
//!
//! `.ai/repo/rules/project/envrc-is-an-adapter.v1.md` names this file as the proof of three
//! things, and each has a test here: that the fast resolution answers without building the
//! index, that a shell evaluates the exported script back to the values it was given
//! whatever the repository's path contains, and that the banner is silent when nothing is
//! watching. `test/cases/100_environment.sh` holds the other half — the `.envrc` itself.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{run_in, Fixture};

/// A rule document the index cannot read: the front matter does not parse, so a resolution
/// that builds the index over the layer has something to say about this repository.
const UNREADABLE: &str = "---\nid: project.broken\nversion: [1\nkind: rule\n---\n\n# Broken\n";

/// Copy a tree, so a fixture can be asked the same questions from a hostile path.
fn copy_tree(from: &Path, to: &Path) {
    let status = Command::new("cp")
        .arg("-R")
        .arg(from)
        .arg(to)
        .status()
        .expect("cp -R");
    assert!(status.success(), "copying the fixture to {to:?} failed");
}

/// The assignments of an exported script, in order, as (name, raw right-hand side).
fn assignments(script: &str) -> Vec<(String, String)> {
    script
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let l = l.strip_prefix("export ")?;
            let (name, value) = l.split_once('=')?;
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

/// Entering a directory must not fail, so the fast path answers a layer that a full
/// resolution has something to complain about. The document below is committed and
/// unreadable; `env export` is the call the adapter makes on every `cd`, and it answers.
#[test]
fn entering_a_repository_answers_over_a_layer_the_index_would_reject() {
    let f = Fixture::new();
    f.write(".ai/repo/rules/project/broken.v1.md", UNREADABLE);
    f.commit("a rule document the index cannot read");

    let (code, out, err) = run_in(&f.root(), &["env", "export", "--shell", "posix"], "");
    assert_eq!(code, 0, "env export refused the layer: {err}");
    let vars = assignments(&out);
    assert!(
        vars.iter().any(|(name, _)| name == "MAJORDOMUS_ROOT"),
        "the exported script names no repository root: {out}"
    );
    assert!(
        vars.iter().all(|(name, _)| name.starts_with("MAJORDOMUS_")),
        "the exported script assigns something that is not this tool's: {out}"
    );
}

/// What the adapter prints is evaluated by whatever shell the person happens to use, in a
/// directory whose path this repository does not choose. A space, a quote and a dollar in
/// the path are the three characters that break a hand-rolled quoting, so the round trip is
/// run from a directory carrying all three.
#[test]
fn the_exported_script_survives_a_shell_and_a_hostile_path() {
    let f = Fixture::new();
    let holder = tempfile::tempdir().expect("tempdir");
    let hostile: PathBuf = holder.path().join("a dir with 'quotes' and $dollars");
    copy_tree(&f.root(), &hostile);

    let (code, script, err) = run_in(&hostile, &["env", "export", "--shell", "posix"], "");
    assert_eq!(code, 0, "env export failed from a hostile path: {err}");

    // The one thing a shell does with this text: evaluate it, and read a value back out.
    let out = Command::new("sh")
        .arg("-c")
        .arg("eval \"$1\"; printf '%s' \"$MAJORDOMUS_ROOT\"")
        .arg("sh")
        .arg(&script)
        .output()
        .expect("sh -c eval");
    assert!(
        out.status.success(),
        "a shell could not evaluate the exported script: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let seen = String::from_utf8_lossy(&out.stdout).to_string();
    let want = std::fs::canonicalize(&hostile).expect("canonical hostile path");
    assert_eq!(
        std::fs::canonicalize(&seen).expect("canonical exported root"),
        want,
        "the shell read back a different root than the one exported"
    );
}

/// The banner is for a terminal. Nothing is watching a pipe, a CI log or a shell that asked
/// for no banner, and printing there is noise in somebody's transcript — so the default
/// resolves to silence and only an explicit mode overrides it.
#[test]
fn the_banner_is_silent_when_nothing_is_watching() {
    let f = Fixture::new();

    let (code, out, err) = run_in(&f.root(), &["env", "banner"], "");
    assert_eq!(code, 0, "env banner failed: {err}");
    assert!(
        out.is_empty(),
        "the banner wrote to standard output, which direnv reads as the environment: {out}"
    );
    assert!(
        err.lines().all(|l| l.trim().is_empty() || l.contains("INFO") || l.contains("DEBUG")),
        "the banner rendered into a pipe: {err}"
    );

    // and it is not silent because it cannot render: asked for, it renders.
    let asked = Command::new(common::BIN)
        .args(["env", "banner", "--mode", "compact"])
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "error")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env("MAJORDOMUS_BANNER", "compact")
        .output()
        .expect("spawn majordomus");
    assert!(
        !String::from_utf8_lossy(&asked.stderr).trim().is_empty(),
        "the banner is silent even when it is asked for, so the test above proves nothing"
    );
}
