//! The version obligation (ADR 0106) against real repositories with a trunk: what it reads
//! from git, how the command line renders and exits on it, and what the one writer does
//! with it — once, and never twice.

mod common;

use common::{run_in, Fixture};
use majordomus_cli::release::compat::Impact;
use majordomus_cli::release::obligation::{self, Carries, ObligationState, ReleasePolicy};
use serde_json::Value;

const MANIFEST: &str = "apps/majordomus-cli/Cargo.toml";
const LOCK: &str = "apps/majordomus-cli/Cargo.lock";

/// A fixture at 1.4.0 whose trunk is its own first commit, with a minor cadence declared.
fn fixture() -> Fixture {
    let f = Fixture::new();
    f.write(
        MANIFEST,
        "[package]\nname = \"majordomus-cli\"\nversion = \"1.4.0\"\n",
    );
    f.write(
        LOCK,
        "[[package]]\nname = \"majordomus-cli\"\nversion = \"1.4.0\"\n",
    );
    f.write(".gitattributes", "docs/generated/** merge=derived\n");
    f.write("docs/generated/x.json", "{}\n");
    let policy = std::fs::read_to_string(f.path(".ai/repo/policy.yaml")).unwrap();
    f.write(
        ".ai/repo/policy.yaml",
        &format!("{policy}\nrelease:\n  cadence: minor\n"),
    );
    f.commit("base");
    f.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
    f
}

fn declared(f: &Fixture) -> String {
    let text = std::fs::read_to_string(f.path(MANIFEST)).unwrap();
    majordomus_cli::release::version::declared_in(&text).unwrap()
}

fn json(f: &Fixture, extra: &[&str]) -> (i32, Value) {
    let mut args = vec!["release", "obligation", "--format", "json"];
    args.extend_from_slice(extra);
    let (code, out, err) = run_in(&f.root(), &args, "");
    let v = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out} {err}"));
    (code, v)
}

#[test]
fn work_owes_the_cadence_over_the_trunk_and_the_command_says_why() {
    let f = fixture();
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho changed\n");
    let (code, out, _) = run_in(&f.root(), &["release", "obligation"], "");
    assert_eq!(code, 10, "{out}");
    for line in [
        "state       owed",
        "minimum     1.5.0",
        "effective   minor",
        "carries     work (1 authored",
        "cadence is minor",
        "lib/a.sh",
        "remedy      majordomus release advance",
    ] {
        assert!(out.contains(line), "missing {line:?} in:\n{out}");
    }
    let (code, v) = json(&f, &[]);
    assert_eq!(code, 10);
    assert_eq!(v["schema"], "majordomus/version-obligation/v1");
    assert_eq!(v["state"], "owed");
    assert_eq!(v["carries"], "work");
    assert_eq!(v["cadence"]["required"], "minor");
    assert_eq!(v["trunk"]["version"], "1.4.0");
    assert_eq!(v["trunk"]["contained"], true);
    assert!(
        v["contract"]["unmeasured"].is_string(),
        "nothing is published here"
    );
}

#[test]
fn the_advance_is_written_once_and_a_retry_writes_nothing() {
    let f = fixture();
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho changed\n");

    let (code, out, _) = run_in(&f.root(), &["release", "advance", "--dry-run"], "");
    assert_eq!(code, 0, "{out}");
    assert!(
        out.contains("1.4.0 -> 1.5.0") && out.contains("(unwritten)"),
        "{out}"
    );
    assert_eq!(declared(&f), "1.4.0", "a dry run wrote");

    let (code, out, _) = run_in(&f.root(), &["release", "advance"], "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(declared(&f), "1.5.0");
    let lock = std::fs::read_to_string(f.path(LOCK)).unwrap();
    assert!(lock.contains("version = \"1.5.0\""), "{lock}");

    for _ in 0..2 {
        let (code, out, _) = run_in(&f.root(), &["release", "advance"], "");
        assert_eq!(code, 0, "{out}");
        assert!(
            out.contains("holds (satisfied)") && out.contains("nothing written"),
            "{out}"
        );
    }
    assert_eq!(declared(&f), "1.5.0", "a retry advanced again");
    let (code, v) = json(&f, &[]);
    assert_eq!((code, v["state"].as_str()), (0, Some("satisfied")));
}

#[test]
fn an_override_below_the_obligation_is_refused_and_one_above_is_written() {
    let f = fixture();
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho changed\n");
    let (code, out, _) = run_in(&f.root(), &["release", "bump", "--level", "patch"], "");
    assert_eq!(code, 10, "{out}");
    assert!(
        out.contains("below 1.5.0, the minimum the version obligation"),
        "{out}"
    );
    assert!(
        out.contains("cadence is minor"),
        "the refusal explains its inputs: {out}"
    );
    assert_eq!(declared(&f), "1.4.0", "a refused bump wrote");

    let (code, out, _) = run_in(&f.root(), &["release", "bump", "--exact", "1.9.0"], "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(declared(&f), "1.9.0");
}

#[test]
fn the_second_of_two_branches_from_one_trunk_advances_from_the_first() {
    let f = fixture();
    let trunk = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    // A advances and lands.
    f.git(&["checkout", "-q", "-b", "feature/a"]);
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho a\n");
    assert_eq!(run_in(&f.root(), &["release", "advance"], "").0, 0);
    f.commit("feat: a");
    let a = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    // B advanced from the same trunk before A landed.
    f.git(&["checkout", "-q", "-b", "feature/b", &trunk]);
    f.write("lib/b.sh", "#!/usr/bin/env bash\necho b\n");
    assert_eq!(run_in(&f.root(), &["release", "advance"], "").0, 0);
    assert_eq!(declared(&f), "1.5.0");
    f.commit("feat: b");
    f.git(&["update-ref", "refs/remotes/origin/master", &a]);
    f.git(&["merge", "-q", "--no-edit", "refs/remotes/origin/master"]);
    assert_eq!(declared(&f), "1.5.0", "the two advances merge cleanly");

    let (code, v) = json(&f, &[]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["state"], "owed", "B may not reuse A's version");
    assert_eq!(v["minimum"], "1.6.0");
    assert_eq!(
        v["id"], "feature/b@1.5.0",
        "a moved trunk is a new obligation"
    );
    assert_eq!(run_in(&f.root(), &["release", "advance"], "").0, 0);
    assert_eq!(declared(&f), "1.6.0");
    f.commit("chore(release): b over a");

    // the merge gate's question, asked of the merge commit against its first parent
    f.git(&["checkout", "-q", "-b", "trunk", &a]);
    f.git(&["merge", "-q", "--no-ff", "--no-edit", "feature/b"]);
    let (code, v) = json(&f, &["--base", "HEAD^1"]);
    assert_eq!((code, v["state"].as_str()), (0, Some("satisfied")), "{v}");

    // the same merge without B's second advance is what the gate refuses
    f.git(&["checkout", "-q", "-b", "trunk-unadvanced", &a]);
    f.git(&["merge", "-q", "--no-ff", "--no-edit", "feature/b~1"]);
    let (code, v) = json(&f, &["--base", "HEAD^1"]);
    assert_eq!((code, v["state"].as_str()), (10, Some("owed")), "{v}");
}

#[test]
fn a_tree_behind_its_trunk_is_refused_and_nothing_is_written() {
    let f = fixture();
    let base = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.git(&["checkout", "-q", "-b", "ahead"]);
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho a\n");
    assert_eq!(run_in(&f.root(), &["release", "advance"], "").0, 0);
    f.commit("feat: ahead");
    f.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
    f.git(&["checkout", "-q", "-b", "behind", &base]);
    f.write("lib/c.sh", "#!/usr/bin/env bash\necho c\n");

    let (code, v) = json(&f, &[]);
    assert_eq!((code, v["state"].as_str()), (10, Some("behind")), "{v}");
    assert_eq!(v["trunk"]["contained"], false);
    assert!(v["remedy"]
        .as_str()
        .unwrap()
        .starts_with("merge origin/master"));
    let (code, out, _) = run_in(&f.root(), &["release", "advance"], "");
    assert_eq!(code, 10, "{out}");
    assert!(
        out.contains("REFUSED") && out.contains("not contained"),
        "{out}"
    );
    assert_eq!(declared(&f), "1.4.0");
}

#[test]
fn projections_release_records_and_an_advance_alone_owe_nothing() {
    let f = fixture();
    f.write("docs/generated/x.json", "{\"x\": 1}\n");
    let (code, v) = json(&f, &[]);
    assert_eq!(
        (code, v["carries"].as_str(), v["state"].as_str()),
        (0, Some("generated-sync"), Some("not-owed"))
    );

    f.write(".ai/repo/releases/v1.4.0.yaml", "version: 1.4.0\n");
    let (code, v) = json(&f, &[]);
    assert_eq!((code, v["carries"].as_str()), (0, Some("release-evidence")));
    assert_eq!(v["paths"]["release_evidence"], 1);

    f.git(&["checkout", "-q", "--", "."]);
    f.git(&["clean", "-qfd", ".ai/repo/releases"]);
    f.write(
        MANIFEST,
        "[package]\nname = \"majordomus-cli\"\nversion = \"1.5.0\"\n",
    );
    f.write(
        LOCK,
        "[[package]]\nname = \"majordomus-cli\"\nversion = \"1.5.0\"\n",
    );
    let (code, v) = json(&f, &[]);
    assert_eq!(
        (code, v["carries"].as_str()),
        (0, Some("version-advance")),
        "{v}"
    );
    assert_eq!(v["paths"]["version_advance"], 2);

    // a dependency beside the version is a change to the build, and so work
    f.write(
        MANIFEST,
        "[package]\nname = \"majordomus-cli\"\nversion = \"1.5.0\"\n\n[dependencies]\nx = \"1\"\n",
    );
    let (_, v) = json(&f, &[]);
    assert_eq!(v["carries"], "work");
}

#[test]
fn a_trunk_nobody_can_read_is_unverified_and_never_a_pass() {
    let f = fixture();
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho changed\n");
    let (code, v) = json(&f, &["--base", "refs/remotes/origin/nowhere"]);
    assert_eq!((code, v["state"].as_str()), (12, Some("unverified")), "{v}");
    assert!(v["remedy"].as_str().unwrap().contains("git fetch"));
    let (code, out, _) = run_in(
        &f.root(),
        &[
            "release",
            "advance",
            "--base",
            "refs/remotes/origin/nowhere",
        ],
        "",
    );
    assert_eq!(code, 12, "{out}");
    assert!(out.contains("trunk       unreadable"), "{out}");

    // a trunk whose manifest states no version is unreadable too
    f.git(&["checkout", "-q", "-b", "bare"]);
    std::fs::remove_file(f.path(MANIFEST)).unwrap();
    f.commit("no manifest");
    f.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
    let (code, v) = json(&f, &[]);
    assert_eq!((code, v["state"].as_str()), (12, Some("unverified")), "{v}");
}

#[test]
fn the_library_reads_the_policy_and_answers_the_same_value() {
    let f = fixture();
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho changed\n");
    let policy = obligation::policy_of(&f.root());
    assert_eq!(policy.cadence, Impact::Minor);
    assert_eq!(policy.trunk(), "origin/master");

    let app = common::load_app(&f);
    let o = obligation::obligation(
        &f.root(),
        &app.context.registry,
        &app.index().objects,
        &policy,
        None,
    );
    assert_eq!((o.state, o.carries), (ObligationState::Owed, Carries::Work));
    assert_eq!(o.minimum, "1.5.0");

    // without a cadence only the contract binds, and it cannot be measured here
    let quiet = obligation::obligation(
        &f.root(),
        &app.context.registry,
        &app.index().objects,
        &ReleasePolicy::default(),
        None,
    );
    assert_eq!(quiet.state, ObligationState::NotOwed);
    assert_eq!(quiet.cadence.required, Impact::None);

    // an unreadable policy is no cadence, never a guess
    let plain = Fixture::plain_dir();
    assert_eq!(
        obligation::policy_of(&plain.root()),
        ReleasePolicy::default()
    );

    // a detached tree is named by its commit
    f.git(&["checkout", "-q", "--detach"]);
    let detached = obligation::observe(&f.root(), "origin/master");
    assert!(
        detached.subject.starts_with("HEAD@"),
        "{}",
        detached.subject
    );
}

#[test]
fn a_merge_of_the_trunk_in_progress_contains_it_and_measures_from_it() {
    let f = fixture();
    let base = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.git(&["checkout", "-q", "-b", "ahead"]);
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho a\n");
    assert_eq!(run_in(&f.root(), &["release", "advance"], "").0, 0);
    f.commit("feat: ahead");
    f.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
    f.git(&["checkout", "-q", "-b", "refreshing", &base]);
    f.write("lib/b.sh", "#!/usr/bin/env bash\necho b\n");
    f.commit("feat: b");
    f.git(&[
        "merge",
        "--no-commit",
        "--no-ff",
        "refs/remotes/origin/master",
    ]);

    let (code, v) = json(&f, &[]);
    assert_eq!(v["trunk"]["contained"], true, "{v}");
    assert_eq!((code, v["state"].as_str()), (10, Some("owed")), "{v}");
    assert_eq!(
        v["paths"]["work"], 1,
        "only the branch's own path is its work: {v}"
    );
    assert_eq!(v["minimum"], "1.6.0");
}

/// An obligation nobody could read sets no floor of its own: the bump falls back to the
/// contract's, and an explicit version is written. The gate and finish refuse an unverified
/// obligation; the writer does not invent a minimum for it.
#[test]
fn an_unreadable_obligation_sets_no_floor_on_the_bump() {
    let f = fixture();
    f.git(&["update-ref", "-d", "refs/remotes/origin/master"]);
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho changed\n");
    let (code, out, _) = run_in(&f.root(), &["release", "bump", "--exact", "1.4.1"], "");
    assert_eq!(code, 0, "{out}");
    assert_eq!(declared(&f), "1.4.1");
}

/// The capability every machine surface serves answers with the value the command line
/// renders: one decision, reached through the one execution path.
#[test]
fn the_capability_answers_with_the_command_lines_value() {
    let f = fixture();
    f.write("lib/a.sh", "#!/usr/bin/env bash\necho changed\n");
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "run",
            "release.obligation",
            "--input",
            "{}",
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    let answer = &v["output"];
    let (_, cli) = json(&f, &[]);
    assert_eq!(answer, &cli, "the capability and the command line disagree");
    let (code, out, _) = run_in(
        &f.root(),
        &[
            "run",
            "release.obligation",
            "--input",
            r#"{"base":"refs/remotes/origin/nowhere"}"#,
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["output"]["state"], "unverified");
}

#[test]
fn outside_a_repository_neither_command_answers() {
    let plain = Fixture::plain_dir();
    for args in [&["release", "obligation"][..], &["release", "advance"][..]] {
        let (code, _, err) = run_in(&plain.root(), args, "");
        assert_ne!(code, 0, "{args:?} answered outside a repository: {err}");
    }
}

/// A trunk this tree shares no history with is not contained, and the tree's change set is
/// measured from the trunk itself.
#[test]
fn a_trunk_with_no_common_history_is_not_contained() {
    let f = fixture();
    let home = f.git(&["symbolic-ref", "--short", "HEAD"]).trim().to_string();
    f.git(&["checkout", "-q", "--orphan", "elsewhere"]);
    f.commit("an unrelated root");
    f.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
    f.git(&["checkout", "-q", &home]);
    let (_, v) = json(&f, &[]);
    assert_eq!(v["trunk"]["contained"], false, "{v}");
}
