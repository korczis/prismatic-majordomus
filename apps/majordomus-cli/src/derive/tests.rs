//! The decision, driven in process over a disposable repository with a stand-in stage B.
//!
//! The stand-in is a real script the module asks for its outputs and the tests run as the
//! stage: it reads one file it would list as an input and one it would not, which is the
//! shape of the real generator's defect. What is asserted is the module's: when the stage is
//! left alone, when it is not and why, and what is and is not recorded.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::key::{declared_outputs, Ambient, Measured, ScenarioExecutable, MEMBERSHIP_ONLY};
use super::record::{Loaded, Store};
use super::*;

/// The stand-in stage B. `--outputs` declares a tree and a file; a run writes both from a
/// listed input, an unlisted library and the tracked list, and records a use-case run the
/// way the real scenarios do.
const GENERATOR: &str = r#"#!/usr/bin/env bash
set -eu
cd "$(dirname "$0")/.."
if [ "${1:-}" = --outputs ]; then printf 'out/site/\ndocs/GENERATED.md\n'; exit 0; fi
rm -rf out/site.new; mkdir -p out/site.new/pages
printf 'page(%s|%s)\n' "$(cat src/listed.txt)" "$(cat lib/unlisted.sh)" > out/site.new/pages/index.txt
git ls-files | LC_ALL=C sort > out/site.new/tracked.txt
rm -rf out/site; mv out/site.new out/site
printf 'generated(%s)\n' "$(cat src/listed.txt)" > docs/GENERATED.md
if [ -f .ai/manifest.yaml ]; then
  mkdir -p .ai/local/evidence/use-cases
  printf '{"use_case":"ran","result":"pass"}\n' > .ai/local/evidence/use-cases/ran.json
fi
"#;

const MANIFEST: &str = "schema: ai-repository/v1

repo:
  path: repo

local:
  path: local
  tracked: false
  implicit_context: false

sections:
  policy: repo/policy.yaml

context:
  documents: [README.md]
";

/// A repository on disk with the stand-in stage and one file of every class the key
/// distinguishes.
struct Tree {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Tree {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let tree = Tree { _tmp: tmp, root };
        tree.git(&["init", "-q"]);
        tree.write("scripts/generate-site-data", GENERATOR);
        tree.executable("scripts/generate-site-data");
        tree.write("src/listed.txt", "one\n");
        tree.write("lib/unlisted.sh", "echo a\n");
        tree.write("apps/majordomus-cli/src/lib.rs", "pub fn f() {}\n");
        tree.write("apps/majordomus-cli/tests/t.rs", "#[test] fn t() {}\n");
        tree.write(".gitignore", "*.swp\n/scratch/\n/.ai/local/\n*.stray\n");
        tree.write("docs/GENERATED.md", "");
        tree.write("out/site/pages/index.txt", "");
        tree.write("out/site/tracked.txt", "");
        tree.git(&["add", "-A"]);
        tree
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write(&self, rel: &str, content: &str) {
        let path = self.root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn executable(&self, rel: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.root.join(rel);
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root.join(rel)).unwrap()
    }

    /// Run the stand-in stage B, as `scripts/derive` would between `begin` and `record`.
    fn stage_b(&self) {
        let out = Command::new(self.root.join("scripts/generate-site-data"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// One whole incremental pass: begin, run the stage when told to, record. Answers
    /// whether the stage ran.
    fn derive(&self, ambient: &Ambient) -> bool {
        let begun = begin(&self.root, ambient, Mode::Incremental).unwrap();
        if begun.code == EXIT_REUSE {
            return false;
        }
        assert_eq!(begun.code, EXIT_RUN, "{:?}", begun.lines);
        self.stage_b();
        assert_eq!(record(&self.root, ambient).unwrap().code, 0);
        true
    }

    /// A tree whose stage B has run and been recorded, with its outputs tracked: the state
    /// a committed, derived checkout is in.
    fn derived() -> (Self, Ambient) {
        let tree = Tree::new();
        let ambient = ambient();
        assert!(tree.derive(&ambient), "a first derive runs the stage");
        // staging what it wrote moves nothing the stage reads: the paths were tracked already
        tree.git(&["add", "-A"]);
        assert!(!tree.derive(&ambient), "and the second has nothing to do");
        (tree, ambient)
    }

    fn plan(&self, ambient: &Ambient) -> Plan {
        plan(&self.root, ambient).unwrap()
    }

    fn store(&self) -> Store {
        Store::open(&self.root).unwrap()
    }
}

/// Surroundings a test controls entirely.
fn ambient() -> Ambient {
    Ambient {
        executable: "generation-1 debug aarch64-apple-darwin".into(),
        scenario_executable: ScenarioExecutable::This,
        toolchain: "bash 5\njq 1.7".into(),
        environment: vec![("MJ_UC_JOBS".into(), "8".into())],
        share: None,
        git_config: "user.name=t".into(),
    }
}

fn kinds(plan: &Plan) -> Vec<String> {
    plan.reasons
        .iter()
        .map(|r| {
            serde_json::to_value(r).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect()
}

/// The reasons of a plan that must run, after asserting that it must.
fn must_run(tree: &Tree, ambient: &Ambient, what: &str) -> Plan {
    let plan = tree.plan(ambient);
    assert_eq!(plan.state, State::Regenerate, "{what}: {:?}", plan.reasons);
    plan
}

fn reusable(tree: &Tree, ambient: &Ambient, what: &str) {
    let plan = tree.plan(ambient);
    assert_eq!(plan.state, State::Reuse, "{what}: {:?}", plan.reasons);
}

#[test]
fn a_first_run_has_nothing_to_reuse_and_a_recorded_one_is_left_alone() {
    let tree = Tree::new();
    let ambient = ambient();

    let first = tree.plan(&ambient);
    assert_eq!(first.state, State::Regenerate);
    assert_eq!(first.reasons, vec![Reason::NoRecord]);
    assert!(first.cacheable);

    let begun = begin(&tree.root, &ambient, Mode::Incremental).unwrap();
    assert_eq!(begun.code, EXIT_RUN);
    assert!(
        begun.lines[0].contains("must run: no run of it is recorded for this checkout"),
        "{:?}",
        begun.lines
    );
    assert!(tree.store().pending().is_some(), "a run is in flight");
    tree.stage_b();
    let recorded = record(&tree.root, &ambient).unwrap();
    assert_eq!(recorded.code, 0);
    assert!(
        recorded.lines[0].contains("recorded (key "),
        "{:?}",
        recorded.lines
    );
    assert!(tree.store().pending().is_none(), "and no longer is");

    let again = tree.plan(&ambient);
    assert_eq!(again.state, State::Reuse, "{:?}", again.reasons);
    assert_eq!(
        again.reasons,
        vec![Reason::KeyMatch, Reason::OutputsVerified]
    );
    assert_eq!(again.key, again.recorded_key);
    assert_eq!(
        again.sentence(),
        "its inputs are the recorded run's; its outputs are as that run left them"
    );

    let skipped = begin(&tree.root, &ambient, Mode::Incremental).unwrap();
    assert_eq!(skipped.code, EXIT_REUSE);
    assert!(
        skipped.lines[0].starts_with("==> derive: stage B: skipped (key "),
        "{:?}",
        skipped.lines
    );
    assert!(skipped.lines[0].contains("inputs and outputs unchanged since the run recorded "));
    assert!(
        tree.store().pending().is_none(),
        "a skip leaves nothing in flight"
    );
}

#[test]
fn a_plan_writes_nothing() {
    let (tree, ambient) = Tree::derived();
    let listing = |dir: &Path| -> Vec<(String, Vec<u8>)> {
        let mut found: Vec<(String, Vec<u8>)> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        found.sort();
        found
    };
    let dir = tree.store().record_path().parent().unwrap().to_path_buf();
    let before = listing(&dir);
    tree.write("src/listed.txt", "two\n");
    must_run(&tree, &ambient, "an edited input");
    assert_eq!(listing(&dir), before, "a plan is a read");
    assert!(matches!(tree.store().record(), Loaded::Found(_)));
}

#[test]
fn every_tracked_input_moves_the_key_whether_the_generator_lists_it_or_not() {
    let (tree, ambient) = Tree::derived();

    // an input the generator would list
    tree.write("src/listed.txt", "two\n");
    let plan = must_run(&tree, &ambient, "a listed input");
    assert!(plan.reasons.contains(&Reason::InputChanged {
        path: "src/listed.txt".into()
    }));
    assert_eq!(plan.moved, 1);
    assert_eq!(
        plan.sentence(),
        "1 input(s) moved since the recorded run: src/listed.txt"
    );
    // the same bytes back are the same key: content decides, never a timestamp
    tree.write("src/listed.txt", "one\n");
    reusable(&tree, &ambient, "the listed input restored");

    // one it would not: the class the generator's own hash misses
    tree.write("lib/unlisted.sh", "echo b\n");
    let plan = must_run(&tree, &ambient, "an unlisted input");
    assert_eq!(
        plan.reasons,
        vec![Reason::InputChanged {
            path: "lib/unlisted.sh".into()
        }]
    );
    tree.write("lib/unlisted.sh", "echo a\n");

    // the generator itself: the operation's version, and a tracked file like any other
    let text = tree.read("scripts/generate-site-data");
    tree.write(
        "scripts/generate-site-data",
        &format!("{text}# a comment\n"),
    );
    tree.executable("scripts/generate-site-data");
    let plan = must_run(&tree, &ambient, "the generator's own text");
    assert_eq!(
        kinds(&plan),
        vec!["operation_version_changed", "input_changed"]
    );
    assert!(plan.sentence().starts_with("the generator changed; "));
    tree.write("scripts/generate-site-data", &text);
    tree.executable("scripts/generate-site-data");
    reusable(&tree, &ambient, "the generator restored");

    // a source of the crate is read by the shell tool, so its bytes count
    tree.write("apps/majordomus-cli/src/lib.rs", "pub fn g() {}\n");
    must_run(&tree, &ambient, "a crate source");
    tree.write("apps/majordomus-cli/src/lib.rs", "pub fn f() {}\n");

    // a mode bit is content too
    tree.executable("lib/unlisted.sh");
    let plan = must_run(&tree, &ambient, "a mode change");
    assert_eq!(kinds(&plan), vec!["input_changed"]);
}

#[test]
fn the_index_and_the_work_tree_are_both_in_the_key() {
    let (tree, ambient) = Tree::derived();

    // a staged new file: membership moved
    tree.write("docs/NEW.md", "new\n");
    tree.git(&["add", "docs/NEW.md"]);
    let plan = must_run(&tree, &ambient, "a staged new file");
    assert_eq!(
        plan.reasons,
        vec![Reason::InputAdded {
            path: "docs/NEW.md".into()
        }]
    );
    assert!(tree.derive(&ambient), "and the stage runs for it");
    assert!(
        tree.read("out/site/tracked.txt").contains("docs/NEW.md"),
        "which is what a full run would have written"
    );
    reusable(&tree, &ambient, "after that run");

    // a tracked file edited and not staged: the bytes on disk are what the stage reads
    tree.write("docs/NEW.md", "edited, not staged\n");
    let plan = must_run(&tree, &ambient, "an unstaged tracked change");
    assert_eq!(kinds(&plan), vec!["input_changed"]);
    // staging it changes nothing the stage reads
    let unstaged = tree.plan(&ambient).key;
    tree.git(&["add", "docs/NEW.md"]);
    assert_eq!(tree.plan(&ambient).key, unstaged);

    // deleted from the work tree and still in the index: missing, which is not empty
    std::fs::remove_file(tree.root.join("docs/NEW.md")).unwrap();
    let missing = tree.plan(&ambient).key;
    tree.write("docs/NEW.md", "");
    assert_ne!(tree.plan(&ambient).key, missing);

    // removed from the index
    std::fs::remove_file(tree.root.join("docs/NEW.md")).unwrap();
    tree.git(&["rm", "-q", "--cached", "docs/NEW.md"]);
    let plan = must_run(&tree, &ambient, "a path that left the index");
    assert!(plan.reasons.contains(&Reason::InputRemoved {
        path: "docs/NEW.md".into()
    }));
}

#[test]
fn a_test_of_the_crate_counts_by_its_presence_and_not_its_bytes() {
    let (tree, ambient) = Tree::derived();
    assert!(MEMBERSHIP_ONLY.contains(&"apps/majordomus-cli/tests/"));

    tree.write(
        "apps/majordomus-cli/tests/t.rs",
        "#[test] fn other() { assert!(true); }\n",
    );
    reusable(&tree, &ambient, "a test's body");
    assert!(!tree.derive(&ambient), "so the stage is skipped");

    // a new test is a new tracked path, and the stage resolves links against those
    tree.write("apps/majordomus-cli/tests/u.rs", "#[test] fn u() {}\n");
    tree.git(&["add", "apps/majordomus-cli/tests/u.rs"]);
    let plan = must_run(&tree, &ambient, "a new test file");
    assert_eq!(kinds(&plan), vec!["input_added"]);
    assert!(tree.derive(&ambient));

    // and one that a claim names may not vanish unnoticed
    std::fs::remove_file(tree.root.join("apps/majordomus-cli/tests/u.rs")).unwrap();
    let plan = must_run(&tree, &ambient, "a test file removed from disk");
    assert_eq!(kinds(&plan), vec!["input_changed"]);
}

#[test]
fn what_git_ignores_is_still_read_by_a_glob_and_is_in_the_key() {
    let (tree, ambient) = Tree::derived();

    tree.write("lib/edit.swp", "a\n");
    let plan = must_run(&tree, &ambient, "an ignored file beside the inputs");
    assert_eq!(
        plan.reasons,
        vec![Reason::InputAdded {
            path: "lib/edit.swp".into()
        }]
    );
    assert!(tree.derive(&ambient));
    tree.write("lib/edit.swp", "b\n");
    let plan = must_run(&tree, &ambient, "its bytes");
    assert_eq!(kinds(&plan), vec!["input_changed"]);
    assert!(tree.derive(&ambient));

    // a wholly ignored directory is one name, and what is inside it is not read
    tree.write("scratch/a.txt", "a\n");
    let plan = must_run(&tree, &ambient, "an ignored directory appearing");
    assert_eq!(
        plan.reasons,
        vec![Reason::InputAdded {
            path: "scratch/".into()
        }]
    );
    assert!(tree.derive(&ambient));
    tree.write("scratch/a.txt", "b\n");
    reusable(&tree, &ambient, "a file inside it");
}

#[test]
fn an_output_that_is_missing_changed_or_not_generated_forces_the_run() {
    let (tree, ambient) = Tree::derived();
    let page = "out/site/pages/index.txt";
    let kept = tree.read(page);

    std::fs::remove_file(tree.root.join(page)).unwrap();
    let plan = must_run(&tree, &ambient, "a deleted output");
    assert_eq!(
        plan.reasons,
        vec![Reason::OutputMissing { path: page.into() }]
    );
    assert_eq!(
        plan.sentence(),
        format!("1 output(s) are not what the recorded run left: {page} (missing)")
    );
    assert!(tree.derive(&ambient), "the stage runs");
    assert_eq!(tree.read(page), kept, "and puts it back");

    tree.write(page, "tampered\n");
    let plan = must_run(&tree, &ambient, "a tampered output");
    assert_eq!(
        plan.reasons,
        vec![Reason::OutputTampered { path: page.into() }]
    );
    assert!(tree.derive(&ambient));
    assert_eq!(tree.read(page), kept);

    // the single-file output is held to the same
    tree.write("docs/GENERATED.md", "edited by hand\n");
    let plan = must_run(&tree, &ambient, "a tampered document");
    assert_eq!(
        plan.reasons,
        vec![Reason::OutputTampered {
            path: "docs/GENERATED.md".into()
        }]
    );
    assert!(tree.derive(&ambient));

    // a file under an owned tree that the run did not leave (ignored, so not the preflight's)
    tree.write("out/site/pages/left.stray", "x\n");
    let plan = must_run(&tree, &ambient, "a stray file in an owned tree");
    assert_eq!(
        plan.reasons,
        vec![Reason::OutputUnexpected {
            path: "out/site/pages/left.stray".into()
        }]
    );
    assert!(tree.derive(&ambient));
    assert!(
        !tree.root.join("out/site/pages/left.stray").exists(),
        "the run replaced the tree"
    );
    reusable(&tree, &ambient, "after the run");
}

#[test]
fn a_tree_with_no_key_is_never_skipped_and_never_recorded() {
    let (tree, ambient) = Tree::derived();

    // an untracked file that is not ignored: the stage's globs read it, the index does not
    tree.write("docs/untracked.md", "x\n");
    let plan = must_run(&tree, &ambient, "an untracked file");
    assert!(!plan.cacheable);
    assert_eq!(kinds(&plan), vec!["uncacheable"]);
    assert!(
        plan.sentence()
            .contains("1 untracked file(s) are not ignored (the first: docs/untracked.md)"),
        "{}",
        plan.sentence()
    );
    let begun = begin(&tree.root, &ambient, Mode::Incremental).unwrap();
    assert_eq!(begun.code, EXIT_RUN);
    assert!(
        tree.store().pending().is_none(),
        "nothing is in flight to record"
    );
    tree.stage_b();
    let recorded = record(&tree.root, &ambient).unwrap();
    assert_eq!(recorded.code, 0);
    assert!(
        recorded.lines[0].contains("not recorded"),
        "{:?}",
        recorded.lines
    );
    assert_eq!(tree.store().record(), Loaded::Absent);
    std::fs::remove_file(tree.root.join("docs/untracked.md")).unwrap();

    // scenarios that would run another executable than the one deciding
    let mut other = ambient.clone();
    other.scenario_executable = ScenarioExecutable::Other("MAJORDOMUS_BIN is unset".into());
    assert_eq!(
        kinds(&must_run(&tree, &other, "another executable")),
        vec!["uncacheable"]
    );

    // a share directory outside the tree is content the key does not hold
    let elsewhere = tempfile::tempdir().unwrap();
    let mut outside = ambient.clone();
    outside.share = Some(elsewhere.path().to_path_buf());
    let plan = must_run(&tree, &outside, "a share outside the tree");
    assert!(
        plan.sentence().contains("outside this tree"),
        "{}",
        plan.sentence()
    );

    // a tracked symlink
    std::os::unix::fs::symlink("listed.txt", tree.root.join("src/link.txt")).unwrap();
    tree.git(&["add", "src/link.txt"]);
    let plan = must_run(&tree, &ambient, "a tracked symlink");
    assert!(
        plan.sentence()
            .contains("src/link.txt is tracked with mode 120000"),
        "{}",
        plan.sentence()
    );
    tree.git(&["rm", "-q", "-f", "--cached", "src/link.txt"]);
    std::fs::remove_file(tree.root.join("src/link.txt")).unwrap();

    // a generator that does not say what it owns
    tree.write("scripts/generate-site-data", "#!/bin/sh\nexit 3\n");
    tree.executable("scripts/generate-site-data");
    let plan = must_run(&tree, &ambient, "a generator with no declaration");
    assert!(
        plan.sentence().contains("--outputs exited 3"),
        "{}",
        plan.sentence()
    );
    assert!(declared_outputs(&tree.root).is_err());
    tree.write(
        "scripts/generate-site-data",
        "#!/bin/sh\necho ../elsewhere/\n",
    );
    assert!(declared_outputs(&tree.root)
        .unwrap_err()
        .contains("not a path inside the tree"));
    tree.write("scripts/generate-site-data", "#!/bin/sh\nexit 0\n");
    assert!(declared_outputs(&tree.root)
        .unwrap_err()
        .contains("names no output"));
}

#[test]
fn a_run_that_did_not_finish_leaves_nothing_to_reuse() {
    let (tree, ambient) = Tree::derived();
    tree.write("src/listed.txt", "two\n");

    // the stage begins, and is killed: nobody records
    assert_eq!(
        begin(&tree.root, &ambient, Mode::Incremental).unwrap().code,
        EXIT_RUN
    );
    assert_eq!(
        tree.store().record(),
        Loaded::Absent,
        "the record went when the run began"
    );
    assert!(tree.store().pending().is_some());

    // even with the input back as it was recorded, nothing vouches for the outputs
    tree.write("src/listed.txt", "one\n");
    let plan = must_run(&tree, &ambient, "after an unfinished run");
    assert_eq!(plan.reasons, vec![Reason::InterruptedRun, Reason::NoRecord]);
    assert_eq!(
        plan.sentence(),
        "the run before this one did not finish; no run of it is recorded for this checkout"
    );
    assert!(tree.derive(&ambient), "so the stage runs");
    reusable(&tree, &ambient, "after a finished one");
}

#[test]
fn a_result_is_recorded_only_for_a_run_that_was_begun_and_not_disturbed() {
    let (tree, ambient) = Tree::derived();

    // a record asked for with no run begun: nothing changes
    let before = tree.store().record();
    let unasked = record(&tree.root, &ambient).unwrap();
    assert_eq!(unasked.code, 0);
    assert!(
        unasked.lines[0].contains("no run was begun"),
        "{:?}",
        unasked.lines
    );
    assert_eq!(tree.store().record(), before);

    // an input edited while the stage runs: what it read is not known, so nothing is kept
    tree.write("src/listed.txt", "two\n");
    assert_eq!(
        begin(&tree.root, &ambient, Mode::Incremental).unwrap().code,
        EXIT_RUN
    );
    tree.stage_b();
    tree.write("lib/unlisted.sh", "echo edited meanwhile\n");
    let disturbed = record(&tree.root, &ambient).unwrap();
    assert_eq!(disturbed.code, 0);
    assert!(
        disturbed.lines[0]
            .contains("not recorded: something it could read moved while it ran (tree)"),
        "{:?}",
        disturbed.lines
    );
    assert_eq!(tree.store().record(), Loaded::Absent);
    assert!(tree.store().pending().is_none());
    assert!(
        tree.derive(&ambient),
        "the next derive runs the stage again"
    );

    // a marker nobody can read is a run nobody can vouch for
    assert_eq!(
        begin(&tree.root, &ambient, Mode::Shadow).unwrap().code,
        EXIT_REUSE
    );
    std::fs::write(tree.store().pending_path(), "{ not json").unwrap();
    let unreadable = record(&tree.root, &ambient).unwrap();
    assert!(unreadable.lines[0].contains("the marker of the run is unreadable"));
    assert_eq!(tree.store().record(), Loaded::Absent);
}

#[test]
fn the_surroundings_are_in_the_key() {
    let (tree, ambient) = Tree::derived();
    let moved = |change: &dyn Fn(&mut Ambient)| {
        let mut other = ambient.clone();
        change(&mut other);
        kinds(&must_run(&tree, &other, "a changed surrounding"))
    };
    assert_eq!(
        moved(&|a| a.executable = "generation-2 debug aarch64-apple-darwin".into()),
        vec!["executable_changed"]
    );
    assert_eq!(
        moved(&|a| a.toolchain = "bash 5\njq 1.8".into()),
        vec!["toolchain_changed"]
    );
    assert_eq!(
        moved(&|a| a.environment = vec![("MJ_UC_JOBS".into(), "4".into())]),
        vec!["environment_changed"]
    );
    assert_eq!(
        moved(&|a| a.git_config = "user.name=u".into()),
        vec!["git_config_changed"]
    );
    // the share directory, named inside the tree, is the same wherever the tree is
    std::fs::create_dir_all(tree.root.join("share")).unwrap();
    assert_eq!(
        moved(&|a| a.share = Some(tree.root.join("share"))),
        vec!["environment_changed"]
    );
    reusable(&tree, &ambient, "the surroundings as recorded");
}

#[test]
fn the_local_evidence_is_in_the_key_and_the_stages_own_writing_of_it_is_not_a_disturbance() {
    let tree = Tree::new();
    tree.write(".ai/manifest.yaml", MANIFEST);
    // a checkout somebody has worked in: its local half exists before any derive
    tree.write(".ai/local/state/ledger.jsonl", "");
    tree.git(&["add", "-A"]);
    let ambient = ambient();

    // the first run writes its own evidence between begin and record, and is recorded
    assert!(tree.derive(&ambient));
    assert!(tree
        .root
        .join(".ai/local/evidence/use-cases/ran.json")
        .is_file());
    assert!(matches!(tree.store().record(), Loaded::Found(_)));
    reusable(&tree, &ambient, "with the evidence the run itself left");

    // a stale file for a use case this run does not execute reaches the committed output
    tree.write(
        ".ai/local/evidence/use-cases/stale.json",
        "{\"use_case\":\"stale\",\"result\":\"pass\"}\n",
    );
    let plan = must_run(&tree, &ambient, "a stale local evidence file");
    assert_eq!(plan.reasons, vec![Reason::LocalStateChanged]);
    assert_eq!(plan.sentence(), "the local use-case evidence changed");
    assert!(tree.derive(&ambient));

    // whether it says pass is what is read, not its other bytes
    tree.write(
        ".ai/local/evidence/use-cases/stale.json",
        "{\"use_case\":\"stale\",\"timing\":7,\"result\":\"pass\"}\n",
    );
    reusable(&tree, &ambient, "the same verdict with other bytes");
    tree.write(
        ".ai/local/evidence/use-cases/stale.json",
        "{\"use_case\":\"stale\",\"result\":\"fail\"}\n",
    );
    must_run(&tree, &ambient, "another verdict");
}

#[test]
fn a_record_this_executable_does_not_read_is_never_used() {
    let (tree, ambient) = Tree::derived();
    let path = tree.store().record_path();
    let good = std::fs::read_to_string(&path).unwrap();

    std::fs::write(&path, "{ truncated").unwrap();
    let plan = must_run(&tree, &ambient, "a truncated record");
    assert_eq!(kinds(&plan), vec!["record_invalid"]);
    assert!(plan
        .sentence()
        .starts_with("the record is not usable: it does not parse"));

    // a field nothing reads: a record from some other executable
    let mut value: serde_json::Value = serde_json::from_str(&good).unwrap();
    value["outcome"] = "failure".into();
    std::fs::write(&path, value.to_string()).unwrap();
    assert_eq!(
        kinds(&must_run(&tree, &ambient, "an unknown field")),
        vec!["record_invalid"]
    );

    // another key schema
    let mut value: serde_json::Value = serde_json::from_str(&good).unwrap();
    value["key_schema"] = 0.into();
    std::fs::write(&path, value.to_string()).unwrap();
    let plan = must_run(&tree, &ambient, "another key schema");
    assert!(
        plan.sentence().contains("built under schema 0"),
        "{}",
        plan.sentence()
    );

    // another record schema
    let mut value: serde_json::Value = serde_json::from_str(&good).unwrap();
    value["schema"] = "majordomus/derive-record/v0".into();
    std::fs::write(&path, value.to_string()).unwrap();
    assert_eq!(
        kinds(&must_run(&tree, &ambient, "another schema")),
        vec!["record_invalid"]
    );

    // a key whose components agree and whose digest does not is still not this tree's
    let mut value: serde_json::Value = serde_json::from_str(&good).unwrap();
    value["key"] = "0".repeat(64).into();
    std::fs::write(&path, value.to_string()).unwrap();
    assert_eq!(
        kinds(&must_run(&tree, &ambient, "a forged key")),
        vec!["record_invalid"]
    );

    std::fs::write(&path, &good).unwrap();
    reusable(&tree, &ambient, "the record as written");

    let forgotten = forget(&tree.root).unwrap();
    assert_eq!(forgotten.code, 0);
    assert_eq!(tree.store().record(), Loaded::Absent);
    assert_eq!(
        must_run(&tree, &ambient, "after forget").reasons,
        vec![Reason::NoRecord]
    );
}

#[test]
fn shadow_runs_the_stage_anyway_and_refuses_a_run_that_changed_what_the_skip_would_keep() {
    let (tree, ambient) = Tree::derived();

    // the decision is to reuse; the stage runs regardless and leaves the same bytes
    let begun = begin(&tree.root, &ambient, Mode::Shadow).unwrap();
    assert_eq!(begun.code, EXIT_REUSE);
    assert!(
        begun.lines[0].contains("shadow: the incremental mode would skip it"),
        "{:?}",
        begun.lines
    );
    assert!(
        matches!(tree.store().record(), Loaded::Found(_)),
        "the record is what it is compared with"
    );
    tree.stage_b();
    let agreed = record(&tree.root, &ambient).unwrap();
    assert_eq!(agreed.code, 0, "{:?}", agreed.lines);
    assert!(agreed.lines[0].contains("shadow: the run left every output as the skip would have"));
    assert!(agreed.lines[1].contains("recorded (key "));

    // a run that changes an output under an unchanged key: an input the key does not hold,
    // or a stage that is not deterministic. Either way the skip would have been wrong.
    assert_eq!(
        begin(&tree.root, &ambient, Mode::Shadow).unwrap().code,
        EXIT_REUSE
    );
    tree.stage_b();
    tree.write(
        "out/site/pages/index.txt",
        "what a run with an unkeyed input wrote\n",
    );
    tree.write("out/site/pages/extra.stray", "and a page it added\n");
    std::fs::remove_file(tree.root.join("docs/GENERATED.md")).unwrap();
    let diverged = record(&tree.root, &ambient).unwrap();
    assert_eq!(diverged.code, EXIT_DIVERGED);
    assert!(
        diverged.lines[0].contains("SHADOW DIVERGENCE")
            && diverged.lines[0].contains("3 output(s)"),
        "{:?}",
        diverged.lines
    );
    assert_eq!(
        tree.store().record(),
        Loaded::Absent,
        "the record that allowed the skip is dropped"
    );
    assert!(tree.store().pending().is_none());
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tree.store().divergence_path()).unwrap())
            .unwrap();
    assert_eq!(report["schema"], "majordomus/derive-shadow-divergence/v1");
    let differing = report["differing"].as_array().unwrap();
    assert_eq!(differing.len(), 3);
    assert_eq!(differing[0]["path"], "docs/GENERATED.md");
    assert!(differing[0]["written"].is_null(), "removed by the run");
    assert_eq!(differing[1]["path"], "out/site/pages/extra.stray");
    assert!(differing[1]["kept"].is_null(), "added by the run");
    assert_eq!(differing[2]["path"], "out/site/pages/index.txt");
    assert_ne!(differing[2]["kept"], differing[2]["written"]);

    // a shadow run whose decision was to run has nothing to compare, and records
    std::fs::remove_file(tree.root.join("out/site/pages/extra.stray")).unwrap();
    assert_eq!(
        begin(&tree.root, &ambient, Mode::Shadow).unwrap().code,
        EXIT_RUN
    );
    tree.stage_b();
    let ran = record(&tree.root, &ambient).unwrap();
    assert_eq!(ran.code, 0);
    assert!(
        ran.lines[0].contains("would have run it too"),
        "{:?}",
        ran.lines
    );
    reusable(&tree, &ambient, "after a recorded shadow run");
}

#[test]
fn the_key_is_a_function_of_the_tree_and_its_surroundings_alone() {
    let (tree, ambient) = Tree::derived();
    let one = super::key::measure(&tree.root, &ambient);
    let two = super::key::measure(&tree.root, &ambient);
    assert_eq!(one, two);
    let Measured::Keyed(f) = one else {
        panic!("the tree has a key")
    };
    assert_eq!(f.key.len(), 64);
    assert_ne!(
        f.key, f.settled,
        "the settled digest leaves the local state out"
    );
    // the stage's own outputs are in the key by name only; their bytes are verified apart
    assert!(f
        .inputs
        .contains(&("out/site/pages/index.txt".into(), "tracked".into())));
    assert!(f
        .outputs
        .iter()
        .any(|(p, _)| p == "out/site/pages/index.txt"));
    assert!(f
        .inputs
        .contains(&("apps/majordomus-cli/tests/t.rs".into(), "present".into())));
    assert!(
        f.inputs.windows(2).all(|w| w[0].0 < w[1].0),
        "sorted, each path once"
    );
    for name in [
        "operation",
        "tree",
        "executable",
        "toolchain",
        "environment",
        "git_config",
        "local_state",
    ] {
        assert!(f.components.contains_key(name), "{name}");
    }
}

#[test]
fn what_this_process_is_surrounded_by_can_be_read() {
    let ambient = Ambient::capture();
    assert!(ambient.executable.starts_with(crate::GENERATION));
    assert!(ambient.toolchain.contains("git\t"), "{}", ambient.toolchain);
    assert!(ambient
        .toolchain
        .lines()
        .last()
        .unwrap()
        .starts_with("os\t"));
    assert!(ambient.git_config.contains("exit "));
    assert!(
        ambient
            .environment
            .iter()
            .all(|(k, _)| k != "MAJORDOMUS_BIN" && !k.starts_with("MAJORDOMUS_DERIVE_")),
        "the derive's own knobs are not inputs of the stage"
    );
    // a test binary is not what MAJORDOMUS_BIN names, whatever it names
    assert!(matches!(
        ambient.scenario_executable,
        ScenarioExecutable::Other(_)
    ));
}

#[test]
fn a_directory_that_is_not_a_work_tree_has_no_store() {
    let plain = tempfile::tempdir().unwrap();
    let error = plan(plain.path(), &ambient()).unwrap_err();
    assert!(error.contains("is not a git work tree"), "{error}");
    assert!(begin(plain.path(), &ambient(), Mode::Incremental).is_err());
    assert!(record(plain.path(), &ambient()).is_err());
    assert!(forget(plain.path()).is_err());
}
