//! The half of `project.envrc-is-an-adapter` that lives in this crate.
//!
//! The rule's other half — that the shell entry point runs no program which inspects the
//! repository, carries no control flow and stays within a budget — is
//! `test/cases/100_environment.sh`, which reads the file as written. What cannot be checked
//! from there is what the entry point *delegates to*, and that is the three properties
//! below:
//!
//! - the fast resolution builds no index, because entering a directory must not pay for one;
//! - a shell evaluates the exported script back to the values it was given, whatever the
//!   repository's path contains, because standard output is the environment direnv applies;
//! - the banner is silent when nothing is watching, because a script, a hook and a
//!   continuous-integration run all read standard error.

use std::path::{Path, PathBuf};
use std::process::Command;

use majordomus_cli::environment::render::{banner, BannerMode, Presentation};
use majordomus_cli::environment::shell::{export, Dialect};
use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs, RepositoryEnvironment};
use majordomus_cli::perf::COUNTERS;
use majordomus_cli::Repository;

/// A repository at `name` under a temporary directory, with the layer's manifest and
/// nothing else. The name is a parameter because the second property below is about what a
/// path may contain.
fn repository(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path().join(name);
    std::fs::create_dir_all(root.join(".ai/repo")).expect("the layer");
    std::fs::write(
        root.join(".ai/manifest.yaml"),
        "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n",
    )
    .expect("the manifest");
    std::fs::write(
        root.join(".ai/repo/policy.yaml"),
        "version: 1\ncontext:\n  always_loaded_budget_lines: 150\n",
    )
    .expect("the policy");
    (dir, root)
}

/// The snapshot a shell prompt asks for: sealed, so nothing outside the process is read or
/// written and the test says the same thing on every machine.
fn fast(root: &Path) -> RepositoryEnvironment {
    let repository = Repository::open(root).expect("the repository opens");
    resolve(
        &Inputs {
            repository: &repository,
            share: None,
            index: None,
            registry: None,
        },
        &EnvironmentQuery::fast().sealed(),
    )
}

#[test]
fn the_fast_resolution_builds_no_index() {
    let (_dir, root) = repository("plain");
    let before = COUNTERS.snapshot().index_builds;
    let environment = fast(&root);
    let after = COUNTERS.snapshot().index_builds;
    assert_eq!(
        before, after,
        "the resolution a shell prompt asks for built an index"
    );
    // and it still answers: an absent value is absent, never a default
    assert!(!environment.digest().is_empty());
}

#[test]
fn a_shell_evaluates_the_export_back_to_the_values_it_was_given() {
    // Every character a repository's path may legally contain and a shell would otherwise
    // interpret. If the quoting is wrong, the assignment below comes back wrong or the
    // shell refuses the script outright.
    let hostile = "a dir with spaces $HOME `id` \"quoted\" 'single' $(id) ;&|";
    let (_dir, root) = repository(hostile);
    let environment = fast(&root);
    let script = export(&environment, Some("/share/dir with spaces"), Dialect::Posix);

    // nothing but comments and assignments reaches standard output
    for line in script.lines() {
        assert!(
            line.is_empty() || line.starts_with('#') || line.starts_with("export "),
            "the export carries something that is not an assignment: {line}"
        );
    }

    // a real shell reads it and gives the values back unaltered
    let program = format!(
        "set -eu\n. /dev/stdin <<'SCRIPT'\n{script}\nSCRIPT\nprintf '%s' \"${{MAJORDOMUS_ROOT:-}}\"\n"
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg(&program)
        .output()
        .expect("a shell runs");
    assert!(
        out.status.success(),
        "a shell refused the export: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let seen = String::from_utf8_lossy(&out.stdout).to_string();
    if !seen.is_empty() {
        assert_eq!(
            seen,
            root.display().to_string(),
            "the repository root did not survive the round trip through a shell"
        );
    }
}

#[test]
fn the_banner_is_silent_when_nothing_is_watching() {
    let (_dir, root) = repository("quiet");
    let environment = fast(&root);
    let unwatched = Presentation {
        interactive: false,
        ..Presentation::default()
    };
    assert!(
        banner(&environment, BannerMode::Auto, &unwatched, None).is_none(),
        "the banner drew itself where no terminal was reading"
    );
    // `off` is silent even at a terminal, and an explicit mode draws even away from one:
    // the mode is the person's decision and `auto` is the only one that asks.
    let watching = Presentation {
        interactive: true,
        ..Presentation::default()
    };
    assert!(banner(&environment, BannerMode::Off, &watching, None).is_none());
    assert!(banner(&environment, BannerMode::Full, &unwatched, None).is_some());
}

#[test]
fn an_unchanged_repository_gets_the_shorter_form_on_the_way_back_in() {
    // What keeps entering a directory from becoming visual punishment: `auto` shows the box
    // on a first look and the two-line form when the snapshot is the one already seen.
    let (_dir, root) = repository("again");
    let environment = fast(&root);
    let watching = Presentation {
        interactive: true,
        ..Presentation::default()
    };
    let first = banner(&environment, BannerMode::Auto, &watching, None).expect("a first look");
    let again = banner(
        &environment,
        BannerMode::Auto,
        &watching,
        Some(&environment.digest()),
    )
    .expect("a second look");
    assert!(
        again.lines().count() < first.lines().count(),
        "re-entering an unchanged repository drew as much as the first entry did"
    );
}
