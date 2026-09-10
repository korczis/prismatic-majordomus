//! The worktree subsystem against real git.
//!
//! Nothing here mocks git. Every case builds a disposable repository, runs real
//! `git worktree add`, `move`, `remove` and `prune`, and drives the built executable the way
//! a person does. The repository is nested one level inside its temporary directory, so the
//! container — which is by definition its *sibling* — is still inside the temporary
//! directory and goes away with it.

mod common;

use std::path::{Path, PathBuf};

use common::{run_in, Fixture};
use serde_json::Value;

/// Run in a directory and require success, with git's own words on failure.
fn ok(cwd: &Path, args: &[&str]) -> String {
    let (code, out, err) = run_in(cwd, args, "");
    assert_eq!(code, 0, "`majordomus {}` failed:\n{err}", args.join(" "));
    out
}

/// Run in a directory and require this exit code.
fn code(cwd: &Path, args: &[&str], want: i32) -> String {
    let (got, out, err) = run_in(cwd, args, "");
    assert_eq!(
        got,
        want,
        "`majordomus {}` exited {got}, expected {want}\nstdout:\n{out}\nstderr:\n{err}",
        args.join(" ")
    );
    format!("{out}{err}")
}

fn json(cwd: &Path, args: &[&str]) -> Value {
    json_exit(cwd, args, 0)
}

/// The JSON form of a command that answers with a non-zero code. `worktree list` exits 10
/// when the layout rule fails, which is the whole point of it, so a test about violations
/// asks for that code rather than treating it as a failure.
fn json_exit(cwd: &Path, args: &[&str], want: i32) -> Value {
    let mut argv = args.to_vec();
    argv.extend_from_slice(&["--format", "json"]);
    let (got, out, err) = run_in(cwd, &argv, "");
    assert_eq!(
        got,
        want,
        "`majordomus {}` exited {got}, expected {want}\nstderr:\n{err}",
        argv.join(" ")
    );
    serde_json::from_str(&out).expect("one JSON document")
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

/// The container of a fixture: its sibling, by the default policy.
fn container(f: &Fixture) -> PathBuf {
    let root = f.root();
    root.parent().unwrap().join(format!(
        "{}-wt",
        root.file_name().unwrap().to_string_lossy()
    ))
}

// ---------------------------------------------------------------- the invariant

#[test]
fn create_derives_the_destination_and_never_puts_it_inside_the_repository() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature-x"]);

    let expected = container(&f).join("feature-x");
    assert!(expected.is_dir(), "{} was not created", expected.display());
    assert!(
        !root.join("feature-x").exists(),
        "a directory was created inside the repository"
    );
    let listed = git(&root, &["worktree", "list", "--porcelain"]);
    assert!(
        listed.contains(&format!("worktree {}", expected.display())),
        "git did not register the worktree:\n{listed}"
    );
}

#[test]
fn the_container_is_the_repositorys_from_every_directory_of_every_worktree() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature-x"]);
    let wt = container(&f).join("feature-x");
    let deep = wt.join("apps/foo/src");
    std::fs::create_dir_all(&deep).unwrap();

    let expected = container(&f).canonicalize().unwrap();
    for cwd in [root.clone(), root.join(".ai"), wt.clone(), deep.clone()] {
        let answered = PathBuf::from(ok(&cwd, &["worktree", "root"]).trim());
        assert_eq!(
            answered.canonicalize().unwrap(),
            expected,
            "from {} the container came out as {}",
            cwd.display(),
            answered.display()
        );
        // The bug this whole abstraction exists to prevent: deriving the container from the
        // current work tree rather than from the primary checkout.
        assert!(
            !answered.to_string_lossy().contains("feature-x-wt"),
            "the container was derived from the linked worktree: {}",
            answered.display()
        );
    }

    // and the primary checkout is the same from everywhere too
    let primary = json(&deep, &["worktree", "status"])["repository"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        PathBuf::from(primary).canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
}

#[test]
fn a_worktree_created_from_a_linked_worktree_lands_in_the_same_container() {
    let f = Fixture::new();
    ok(&f.root(), &["worktree", "create", "first"]);
    let first = container(&f).join("first");
    ok(&first, &["worktree", "create", "second"]);
    assert!(
        container(&f).join("second").is_dir(),
        "the second worktree did not land beside the first"
    );
    assert!(
        !first.join("second").exists() && !first.parent().unwrap().join("first-wt").exists(),
        "a nested container was created"
    );
}

#[test]
fn creating_the_same_worktree_twice_names_the_existing_one_and_creates_nothing() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature-x"]);
    let before = git(&root, &["worktree", "list", "--porcelain"]);

    let out = code(&root, &["worktree", "create", "feature-x"], 10);
    assert!(
        out.contains("already registered"),
        "unhelpful message: {out}"
    );
    assert_eq!(
        git(&root, &["worktree", "list", "--porcelain"]),
        before,
        "a second worktree was created for a name that already existed"
    );
}

#[test]
fn the_suffix_is_the_only_thing_that_moves_the_container() {
    let f = Fixture::new();
    let root = f.root();
    let policy = std::fs::read_to_string(root.join(".ai/repo/policy.yaml")).unwrap();
    std::fs::write(
        root.join(".ai/repo/policy.yaml"),
        format!("{policy}\nworktree:\n  root:\n    suffix: \"-trees\"\n"),
    )
    .unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "policy"]);

    let answered = ok(&root, &["worktree", "root"]);
    assert!(
        answered.trim().ends_with("-trees"),
        "the container did not follow the policy: {answered}"
    );
    assert!(
        !answered.contains("-wt"),
        "the old suffix survived a policy change, so something holds a copy of it: {answered}"
    );
}

// ---------------------------------------------------------------- violations

#[test]
fn a_worktree_outside_the_container_is_reported_and_nothing_repairs_it_on_its_own() {
    let f = Fixture::new();
    let root = f.root();
    let outside = f.parent().join("elsewhere");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "legacy",
            outside.to_str().unwrap(),
        ],
    );

    let report = json_exit(&root, &["worktree", "list"], 10);
    let violations = report["violations"].as_array().unwrap();
    assert_eq!(
        violations.len(),
        1,
        "expected exactly one violation: {report:#}"
    );
    assert_eq!(violations[0]["code"], "OutsideCanonicalRoot");
    assert_eq!(violations[0]["branch"], "legacy");
    assert!(violations[0]["proposed_path"]
        .as_str()
        .unwrap()
        .ends_with("-wt/elsewhere"));

    // the primary checkout is exempt and is never one of them
    let primary = report["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["kind"] == "primary")
        .expect("a primary worktree");
    assert_eq!(primary["policy"], "exempt");

    // list says so through its exit code, without anything parsing its output
    code(&root, &["worktree", "list"], 10);

    // planning changes nothing
    let before = git(&root, &["worktree", "list", "--porcelain"]);
    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    assert_eq!(plan["applied"], false);
    assert_eq!(plan["safe"], 1);
    assert_eq!(
        git(&root, &["worktree", "list", "--porcelain"]),
        before,
        "--plan moved something"
    );
    assert!(outside.is_dir(), "--plan removed the worktree");
    assert!(
        !container(&f).join("elsewhere").exists(),
        "--plan created the destination"
    );

    // applying moves it, and then the topology is clean
    let applied = json(&root, &["worktree", "migrate", "--apply"]);
    assert_eq!(applied["applied"], true);
    assert!(
        container(&f).join("elsewhere").is_dir(),
        "--apply did not move it"
    );
    assert!(!outside.exists(), "the old path survived the move");
    ok(&root, &["worktree", "list"]);
}

#[test]
fn a_worktree_nested_below_the_container_is_not_one_of_its_worktrees() {
    let f = Fixture::new();
    let root = f.root();
    let nested = container(&f).join("group/deep");
    std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "nested",
            nested.to_str().unwrap(),
        ],
    );

    let report = json_exit(&root, &["worktree", "list"], 10);
    let v = report["violations"].as_array().unwrap();
    assert_eq!(v.len(), 1, "a nested worktree is not compliant: {report:#}");
    assert!(
        v[0]["reason"].as_str().unwrap().contains("nested"),
        "the reason should say it is nested: {}",
        v[0]["reason"]
    );
}

// ---------------------------------------------------------------- safety

#[test]
fn a_dirty_worktree_is_never_removed_or_moved_by_default() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "dirty-one"]);
    let wt = container(&f).join("dirty-one");

    // a modified tracked file
    std::fs::write(wt.join("README.md"), "changed\n").unwrap();
    let out = code(&root, &["worktree", "remove", "dirty-one"], 10);
    assert!(out.contains("dirty"), "unhelpful message: {out}");
    assert!(wt.is_dir(), "a dirty worktree was removed");

    // and an untracked file alone is enough
    git(&wt, &["checkout", "--", "README.md"]);
    std::fs::write(wt.join("scratch.txt"), "notes\n").unwrap();
    let out = code(&root, &["worktree", "remove", "dirty-one"], 10);
    assert!(
        out.contains("dirty"),
        "an untracked file is uncommitted work too: {out}"
    );
    assert!(wt.is_dir());

    // --force is the explicit intent, and only then
    ok(&root, &["worktree", "remove", "dirty-one", "--force"]);
    assert!(!wt.exists());
}

#[test]
fn a_dirty_worktree_blocks_its_own_migration_and_not_the_others() {
    let f = Fixture::new();
    let root = f.root();
    let a = f.parent().join("stale-a");
    let b = f.parent().join("stale-b");
    git(
        &root,
        &["worktree", "add", "-q", "-b", "aa", a.to_str().unwrap()],
    );
    git(
        &root,
        &["worktree", "add", "-q", "-b", "bb", b.to_str().unwrap()],
    );
    std::fs::write(a.join("scratch.txt"), "work\n").unwrap();

    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    let steps = plan["steps"].as_array().unwrap();
    let dirty = steps
        .iter()
        .find(|s| s["from"].as_str().unwrap().ends_with("stale-a"))
        .unwrap();
    let clean = steps
        .iter()
        .find(|s| s["from"].as_str().unwrap().ends_with("stale-b"))
        .unwrap();
    assert_eq!(dirty["safe"], false);
    assert!(dirty["blocked_by"]
        .as_str()
        .unwrap()
        .contains("uncommitted"));
    assert_eq!(clean["safe"], true);

    // applying moves the safe one and leaves the dirty one exactly where it was
    code(&root, &["worktree", "migrate", "--apply"], 10);
    assert!(a.is_dir(), "the dirty worktree was moved");
    assert!(a.join("scratch.txt").is_file(), "uncommitted work was lost");
    assert!(
        container(&f).join("stale-b").is_dir(),
        "the safe move was not made"
    );
}

#[test]
fn the_primary_checkout_can_never_be_removed_through_a_worktree_command() {
    let f = Fixture::new();
    let root = f.root();
    for selector in [root.to_str().unwrap(), "master", "main"] {
        let (_, out, err) = run_in(&root, &["worktree", "remove", selector], "");
        let text = format!("{out}{err}");
        assert!(
            text.contains("primary checkout") || text.contains("no worktree"),
            "removing the primary checkout by `{selector}` gave: {text}"
        );
    }
    assert!(
        root.join(".ai/manifest.yaml").is_file(),
        "the primary checkout was damaged"
    );
}

#[test]
fn removing_a_worktree_leaves_its_branch_alone() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "keep-branch"]);
    ok(&root, &["worktree", "remove", "keep-branch"]);
    assert!(
        !container(&f).join("keep-branch").exists(),
        "the directory survived remove"
    );
    let branches = git(&root, &["branch", "--list", "keep-branch"]);
    assert!(
        branches.contains("keep-branch"),
        "remove deleted the branch; the two lifecycles are separate"
    );
}

#[test]
fn a_locked_worktree_is_refused_until_it_is_unlocked() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "locked-one"]);
    let wt = container(&f).join("locked-one");
    git(
        &root,
        &[
            "worktree",
            "lock",
            "--reason",
            "in use",
            wt.to_str().unwrap(),
        ],
    );

    let out = code(&root, &["worktree", "remove", "locked-one"], 10);
    assert!(out.contains("locked"), "unhelpful message: {out}");
    assert!(wt.is_dir());

    git(&root, &["worktree", "unlock", wt.to_str().unwrap()]);
    ok(&root, &["worktree", "remove", "locked-one"]);
}

#[test]
fn a_path_this_repository_does_not_own_is_not_a_worktree_of_it() {
    let other = Fixture::new();
    let f = Fixture::new();
    ok(&other.root(), &["worktree", "create", "theirs"]);
    let theirs = other
        .root()
        .parent()
        .unwrap()
        .join(format!(
            "{}-wt",
            other.root().file_name().unwrap().to_string_lossy()
        ))
        .join("theirs");

    let out = code(
        &f.root(),
        &["worktree", "remove", theirs.to_str().unwrap()],
        12,
    );
    assert!(out.contains("no worktree"), "unhelpful message: {out}");
    assert!(theirs.is_dir(), "another repository's worktree was removed");
}

#[test]
fn a_symlink_cannot_make_a_path_outside_the_container_look_like_one_inside_it() {
    let f = Fixture::new();
    let root = f.root();
    let outside = f.parent().join("really-outside");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "sneaky",
            outside.to_str().unwrap(),
        ],
    );

    // a link inside the container pointing at the worktree that is outside it
    std::fs::create_dir_all(container(&f)).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, container(&f).join("really-outside")).unwrap();

    let report = json_exit(&root, &["worktree", "list"], 10);
    assert_eq!(
        report["violations"].as_array().unwrap().len(),
        1,
        "the link made an out-of-place worktree look compliant: {report:#}"
    );
}

#[test]
fn a_selector_is_exact_and_an_ambiguous_one_is_refused_rather_than_guessed() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "alpha"]);
    let wt = container(&f).join("alpha");

    // all three exact forms resolve to the same worktree
    for selector in [
        wt.to_str().unwrap(), // the path
        "alpha",              // the directory name, and here also the branch
    ] {
        let answered = ok(&root, &["worktree", "path", selector]);
        assert_eq!(
            PathBuf::from(answered.trim()).canonicalize().unwrap(),
            wt.canonicalize().unwrap()
        );
    }

    // nothing is matched by prefix or similarity
    let out = code(&root, &["worktree", "path", "alph"], 12);
    assert!(out.contains("no worktree"), "a prefix matched: {out}");
    let out = code(&root, &["worktree", "path", "ALPHA"], 12);
    assert!(
        out.contains("no worktree"),
        "case-insensitive matching: {out}"
    );
}

// ---------------------------------------------------------------- concurrency

#[test]
fn two_processes_creating_at_once_produce_one_worktree_and_one_clear_refusal() {
    let f = Fixture::new();
    let root = f.root();

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || run_in(&root, &["worktree", "create", "contended"], ""))
        })
        .collect();
    let results: Vec<(i32, String, String)> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();

    let succeeded = results.iter().filter(|(c, _, _)| *c == 0).count();
    assert_eq!(
        succeeded, 1,
        "expected exactly one success, got {succeeded}: {results:#?}"
    );
    let refused = results.iter().find(|(c, _, _)| *c != 0).unwrap();
    assert_eq!(
        refused.0, 10,
        "the loser should refuse cleanly: {refused:#?}"
    );

    // and git's own metadata is intact: exactly one worktree of that name
    // Count registered work trees, not occurrences of the name: `branch refs/heads/contended`
    // carries it too, and counting that would pass whatever happened.
    let listed = git(&root, &["worktree", "list", "--porcelain"]);
    let registered = listed
        .lines()
        .filter(|l| l.starts_with("worktree ") && l.ends_with("/contended"))
        .count();
    assert_eq!(
        registered, 1,
        "the topology is not clean after a contended create:\n{listed}"
    );
    ok(&root, &["worktree", "list"]);
}

// ---------------------------------------------------------------- shapes

#[test]
fn the_machine_forms_are_stable_and_carry_what_the_documentation_says() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "shape"]);

    let r = json(&root, &["worktree", "root"]);
    for key in [
        "repository_root",
        "worktree_root",
        "suffix",
        "strategy",
        "exists",
        "current_worktree",
        "current_kind",
        "git_common_dir",
    ] {
        assert!(
            r.get(key).is_some(),
            "worktree root --format json lacks {key}"
        );
    }
    assert_eq!(r["suffix"], "-wt");
    assert_eq!(r["strategy"], "sibling");

    let s = json(&root, &["worktree", "status"]);
    for key in [
        "repository",
        "current",
        "kind",
        "canonical_root",
        "policy",
        "dirty",
        "changes",
        "repository_violations",
    ] {
        assert!(
            s.get(key).is_some(),
            "worktree status --format json lacks {key}"
        );
    }
    assert_eq!(s["kind"], "primary");
    assert_eq!(s["policy"], "exempt");

    // the tab separated form: one line per worktree, eight fields, `-` where a field is absent
    let tsv = ok(&root, &["worktree", "list", "--tsv"]);
    let lines: Vec<&str> = tsv.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2, "one line per worktree:\n{tsv}");
    for line in &lines {
        let fields: Vec<&str> = line.split('\t').collect();
        assert_eq!(fields.len(), 8, "eight fields per line: {line}");
        assert!(
            fields.iter().all(|f| !f.is_empty()),
            "an empty field would shift every column after it: {line}"
        );
    }
    assert!(lines[0].starts_with("primary\texempt\t"));
    assert!(lines[1].starts_with("linked\tpass\t"));

    // dirtiness costs a subprocess per worktree and is absent until it is asked for
    let without = json(&root, &["worktree", "list"]);
    assert!(without["worktrees"][0].get("dirty").is_none());
    let with = json(&root, &["worktree", "list", "--status"]);
    assert!(with["worktrees"][0].get("dirty").is_some());
}

#[test]
fn every_read_only_answer_is_the_same_through_the_capability_registry() {
    // The command line executes the capability the registry exposes at its path; this proves
    // there is no second implementation behind the human rendering.
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "shared"]);

    let via_cli = json(&root, &["worktree", "list"]);
    let described = ok(
        &root,
        &[
            "capabilities",
            "describe",
            "worktree.list",
            "--format",
            "json",
        ],
    );
    assert!(
        described.contains("worktree.list"),
        "the capability is not in the registry"
    );
    assert_eq!(via_cli["root"]["suffix"], "-wt");
}

// ---------------------------------------------------------------- refusals

#[test]
fn a_bare_repository_is_refused_by_name_rather_than_treated_as_a_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let bare = dir.path().join("bare.git");
    std::process::Command::new("git")
        .args(["init", "-q", "--bare"])
        .arg(&bare)
        .status()
        .expect("git init --bare");

    let e = majordomus_cli::worktree::WorktreeService::open(
        &bare,
        majordomus_cli::worktree::WorktreePolicy::default(),
        ".ai/repo/policy.yaml",
    )
    .unwrap_err();
    assert_eq!(e.code(), "BareRepositoryUnsupported");
    assert!(
        e.to_string().contains("non-bare primary checkout"),
        "the message should say what is required: {e}"
    );
}

#[test]
fn a_directory_that_is_not_a_git_repository_is_refused_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let e = majordomus_cli::worktree::WorktreeService::open(
        dir.path(),
        majordomus_cli::worktree::WorktreePolicy::default(),
        ".ai/repo/policy.yaml",
    )
    .unwrap_err();
    assert_eq!(e.code(), "NotInGitRepository");
}

#[test]
fn a_branch_already_checked_out_elsewhere_is_named_rather_than_stolen() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "taken"]);
    let out = code(
        &root,
        &["worktree", "create", "other-name", "--branch", "taken"],
        10,
    );
    assert!(
        out.contains("already checked out"),
        "unhelpful message: {out}"
    );
    assert!(
        !container(&f).join("other-name").exists(),
        "a worktree was created for a branch that could not be checked out"
    );
}

#[test]
fn a_base_that_does_not_resolve_locally_is_refused_and_nothing_is_fetched() {
    let f = Fixture::new();
    let root = f.root();
    let out = code(
        &root,
        &[
            "worktree",
            "create",
            "from-nowhere",
            "--base",
            "origin/does-not-exist",
        ],
        12,
    );
    assert!(out.contains("does not resolve"), "unhelpful message: {out}");
    assert!(!container(&f).join("from-nowhere").exists());
}
