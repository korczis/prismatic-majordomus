//! The worktree topology against real git.
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

/// Run in a directory and require success, with the executable's own words on failure.
fn ok(cwd: &Path, args: &[&str]) -> String {
    let (code, out, err) = run_in(cwd, args, "");
    assert_eq!(
        code,
        0,
        "`majordomus {}` failed:\n{out}\n{err}",
        args.join(" ")
    );
    out
}

/// Run in a directory and require this exit code; answers stdout and stderr together.
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

fn json(cwd: &Path, args: &[&str]) -> Value {
    json_exit(cwd, args, 0)
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

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

fn registered_paths(root: &Path) -> Vec<PathBuf> {
    git(root, &["worktree", "list", "--porcelain"])
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(|p| canonical(Path::new(p)))
        .collect()
}

// ---------------------------------------------------------------- the invariant (A, B, C)

#[test]
fn the_container_and_every_path_derive_from_git_identity_with_no_configuration() {
    let f = Fixture::new();
    let root = f.root();
    let container = f.container();
    assert_eq!(
        canonical(&PathBuf::from(ok(&root, &["worktree", "root"]).trim())),
        canonical(&container).parent().unwrap().join("repo-wt")
    );
    for (branch, rel) in [
        ("feature/improve-cli", "feature/improve-cli"),
        (
            "fix/cockpit-websocket-reconnect",
            "fix/cockpit-websocket-reconnect",
        ),
        (
            "feature/providers/openai-streaming",
            "feature/providers/openai-streaming",
        ),
        ("refactor/a/b/c", "refactor/a/b/c"),
    ] {
        let p = ok(&root, &["worktree", "path", branch]);
        assert_eq!(
            PathBuf::from(p.trim()),
            container.join(rel),
            "hierarchy must be preserved for {branch}"
        );
    }
}

#[test]
fn create_puts_the_worktree_at_the_branch_path_and_never_inside_the_repository() {
    let f = Fixture::new();
    let root = f.root();
    let out = ok(&root, &["worktree", "create", "feature/improve-cli"]);
    let expected = f.container().join("feature/improve-cli");
    assert!(out.contains("feature/improve-cli (new"), "{out}");
    assert!(expected.is_dir(), "{} was not created", expected.display());
    assert!(
        !root.join("feature").exists(),
        "a directory was created inside the repository"
    );
    assert!(registered_paths(&root).contains(&canonical(&expected)));
    assert_eq!(
        git(&expected, &["symbolic-ref", "--short", "HEAD"]),
        "feature/improve-cli"
    );
    // a new branch starts from the trunk
    assert_eq!(
        git(&expected, &["rev-parse", "HEAD"]),
        git(&root, &["rev-parse", "HEAD"])
    );
}

#[test]
fn the_identity_is_the_same_from_every_directory_of_every_worktree() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature/x/y"]);
    let wt = f.container().join("feature/x/y");
    let deep = wt.join("apps/foo/src");
    std::fs::create_dir_all(&deep).unwrap();
    let expected = canonical(&f.container());
    for cwd in [root.clone(), root.join(".ai"), wt.clone(), deep.clone()] {
        let answered = PathBuf::from(ok(&cwd, &["worktree", "root"]).trim());
        assert_eq!(canonical(&answered), expected, "from {}", cwd.display());
        assert!(
            !answered.to_string_lossy().contains("y-wt"),
            "derived from the linked worktree"
        );
        let s = json(&cwd, &["worktree", "status"]);
        assert_eq!(
            canonical(Path::new(
                s["repository"]["primary_worktree"].as_str().unwrap()
            )),
            canonical(&root)
        );
    }
    // and from inside the linked worktree, status is about that worktree
    let s = json(&deep, &["worktree", "status"]);
    assert_eq!(s["worktree"]["branch"], "feature/x/y");
    assert_eq!(s["worktree"]["standing"], "canonical");
    assert_eq!(s["canonical"], true);
    // a worktree created from inside a linked worktree lands in the same container
    ok(&deep, &["worktree", "create", "fix/from-inside"]);
    assert!(f.container().join("fix/from-inside").is_dir());
}

#[test]
fn a_new_branch_needs_no_registration_anywhere() {
    let f = Fixture::new();
    let root = f.root();
    let before = std::fs::read_to_string(root.join(".ai/repo/policy.yaml")).unwrap();
    ok(&root, &["worktree", "create", "feature/new-dashboard"]);
    let t = json(&root, &["worktree", "topology"]);
    assert!(t["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["branch"] == "feature/new-dashboard" && w["standing"] == "canonical"));
    assert_eq!(
        std::fs::read_to_string(root.join(".ai/repo/policy.yaml")).unwrap(),
        before,
        "the policy changed"
    );
    assert!(
        git(&root, &["status", "--porcelain"]).is_empty(),
        "the tree changed"
    );
}

// ---------------------------------------------------------------- detection (D)

#[test]
fn a_misplaced_worktree_is_detected_with_its_expected_path_and_nothing_repairs_it_alone() {
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
            "feature/legacy",
            outside.to_str().unwrap(),
        ],
    );

    let t = json_exit(&root, &["worktree", "topology"], 10);
    assert_eq!(t["valid"], false);
    let w = t["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["branch"] == "feature/legacy")
        .unwrap();
    assert_eq!(w["standing"], "misplaced");
    assert_eq!(
        canonical(Path::new(w["expected_path"].as_str().unwrap())),
        canonical(&f.container()).join("feature/legacy")
    );
    assert!(t["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "worktree.path_mismatch" && d["severity"] == "error"));
    code(&root, &["worktree", "validate"], 10);
    code(&root, &["worktree", "list"], 10);
    // the guard refuses from there, and passes from the primary checkout on the trunk
    let refused = code(&outside, &["worktree", "guard"], 10);
    assert!(refused.contains("worktree.path_mismatch"), "{refused}");
    ok(&root, &["worktree", "guard"]);
    // status from there says misplaced, exit 10
    let s = json_exit(&outside, &["worktree", "status"], 10);
    assert_eq!(s["canonical"], false);
    // and nothing moved
    assert!(outside.is_dir());
    assert!(!f.container().join("feature/legacy").exists());
}

#[test]
fn a_worktree_nested_inside_the_primary_checkout_is_misplaced_and_says_so() {
    let f = Fixture::new();
    let root = f.root();
    let nested = root.join("nested/inside");
    std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "fix/nested",
            nested.to_str().unwrap(),
        ],
    );
    let t = json_exit(&root, &["worktree", "topology"], 10);
    let codes: Vec<&str> = t["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"worktree.path_mismatch"));
    assert!(codes.contains(&"worktree.nested"));
}

#[test]
fn a_sessions_scratch_checkout_is_ephemeral_reported_refused_by_the_guard_and_never_migrated_unasked(
) {
    let f = Fixture::new();
    let root = f.root();
    let agent = root.join(".claude/worktrees/agent-1");
    std::fs::create_dir_all(agent.parent().unwrap()).unwrap();
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "pr74",
            agent.to_str().unwrap(),
        ],
    );
    std::fs::write(agent.join("wip.txt"), "session work\n").unwrap();

    let t = json(&root, &["worktree", "topology"]);
    let w = t["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["branch"] == "pr74")
        .unwrap();
    assert_eq!(w["standing"], "ephemeral");
    assert_eq!(
        t["valid"], true,
        "an ephemeral checkout is a warning, not an error: {t:#}"
    );
    assert!(t["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "worktree.ephemeral" && d["severity"] == "warning"));
    assert_eq!(t["tallies"]["ephemeral"], 1);
    // merged and clean as it is, a scratch checkout is never offered for cleanup
    let branch = t["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "pr74")
        .unwrap();
    assert_eq!(branch["merged_into_trunk"], true);
    assert_eq!(branch["cleanup_eligible"], false, "{branch:#}");

    // the guard still refuses a commit from there: a branch is being worked on where it
    // does not belong, and the remedy says what to do about it
    let refused = code(&agent, &["worktree", "guard"], 10);
    assert!(refused.contains("worktree.ephemeral"), "{refused}");

    // the migration leaves it alone unless asked
    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    assert_eq!(plan["steps"].as_array().unwrap().len(), 0);
    assert!(plan["exceptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "worktree.ephemeral"));
    ok(&root, &["worktree", "migrate"]);
    assert!(agent.join("wip.txt").is_file());

    // and moves it, work included, when asked
    let applied = json(&root, &["worktree", "migrate", "--include-ephemeral"]);
    assert_eq!(applied["moved"], 1, "{applied:#}");
    assert!(f.container().join("pr74/wip.txt").is_file());
    assert!(!agent.exists());
}

#[test]
fn the_primary_checkout_on_a_feature_branch_is_an_error_the_guard_refuses_and_migration_never_touches(
) {
    let f = Fixture::new();
    let root = f.root();
    git(&root, &["switch", "-q", "-c", "feature/oops"]);
    let refused = code(&root, &["worktree", "guard"], 10);
    assert!(
        refused.contains("worktree.primary_on_non_trunk"),
        "{refused}"
    );
    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    assert_eq!(plan["steps"].as_array().unwrap().len(), 0);
    assert!(plan["exceptions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "worktree.primary_on_non_trunk"));
    // applying changes nothing about the primary checkout
    std::fs::write(root.join("scratch.txt"), "work\n").unwrap();
    code(&root, &["worktree", "migrate"], 0);
    assert_eq!(
        git(&root, &["symbolic-ref", "--short", "HEAD"]),
        "feature/oops"
    );
    assert!(root.join("scratch.txt").is_file());
}

#[test]
fn a_detached_worktree_has_no_canonical_path_and_is_never_moved() {
    let f = Fixture::new();
    let root = f.root();
    let detached = f.parent().join("scratch");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            detached.to_str().unwrap(),
        ],
    );
    std::fs::write(detached.join("notes.txt"), "keep\n").unwrap();
    let t = json(&root, &["worktree", "topology"]);
    let w = t["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["detached"] == true)
        .unwrap();
    assert_eq!(w["standing"], "detached");
    assert!(w["label"].as_str().unwrap().starts_with("detached/"));
    assert!(w.get("expected_path").is_none());
    assert_eq!(
        t["valid"], true,
        "a detached worktree is not an error: {t:#}"
    );
    ok(&root, &["worktree", "migrate"]);
    assert!(detached.join("notes.txt").is_file());
    // and the guard from a detached worktree is exempt
    let out = ok(&detached, &["worktree", "guard"]);
    assert!(out.contains("exempt"), "{out}");
}

// ---------------------------------------------------------------- migration (E, K, O)

#[test]
fn a_misplaced_dirty_worktree_migrates_without_losing_any_work() {
    let f = Fixture::new();
    let root = f.root();
    let outside = f.parent().join("wrong-place");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature/foo",
            outside.to_str().unwrap(),
        ],
    );
    // modify a tracked file, stage another, add an untracked one, and an untracked dir
    std::fs::write(outside.join("README.md"), "modified, not staged\n").unwrap();
    std::fs::write(outside.join("docs/CLI.md"), "staged change\n").unwrap();
    git(&outside, &["add", "docs/CLI.md"]);
    std::fs::write(outside.join("untracked.txt"), "untracked work\n").unwrap();
    std::fs::create_dir_all(outside.join("scratch/deep")).unwrap();
    std::fs::write(outside.join("scratch/deep/file.bin"), b"\x00\x01\x02").unwrap();
    // and something ignored, which is where a checkout's own state lives
    std::fs::create_dir_all(outside.join(".ai/local/state")).unwrap();
    std::fs::write(outside.join(".ai/local/state/current.yaml"), "id: t-9\n").unwrap();
    let head = git(&outside, &["rev-parse", "HEAD"]);
    let staged_before = git(&outside, &["diff", "--cached"]);
    let unstaged_before = git(&outside, &["diff"]);

    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    let step = &plan["steps"][0];
    assert_eq!(step["branch"], "feature/foo");
    assert_eq!(step["action"], "move");
    assert_eq!(step["dirty"]["staged"], 1);
    assert_eq!(step["dirty"]["unstaged"], 1);
    assert_eq!(step["dirty"]["untracked"], 2);
    assert_eq!(plan["applied"], false);
    assert!(outside.is_dir(), "--plan moved something");

    let applied = json(&root, &["worktree", "migrate"]);
    let step = &applied["steps"][0];
    assert_eq!(step["outcome"], "moved", "{applied:#}");
    assert_eq!(step["differences"].as_array().unwrap().len(), 0);
    assert!(step["before"].is_object() && step["after"].is_object());
    assert_eq!(step["before"]["untracked_files"], 2);
    assert_eq!(applied["moved"], 1);

    let home = f.container().join("feature/foo");
    assert!(home.is_dir(), "not at its canonical path");
    assert!(!outside.exists(), "the old path survived");
    assert!(registered_paths(&root).contains(&canonical(&home)));
    assert_eq!(
        git(&home, &["symbolic-ref", "--short", "HEAD"]),
        "feature/foo"
    );
    assert_eq!(git(&home, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&home, &["diff", "--cached"]), staged_before);
    assert_eq!(git(&home, &["diff"]), unstaged_before);
    assert_eq!(
        std::fs::read_to_string(home.join("untracked.txt")).unwrap(),
        "untracked work\n"
    );
    assert_eq!(
        std::fs::read(home.join("scratch/deep/file.bin")).unwrap(),
        b"\x00\x01\x02"
    );
    assert_eq!(
        std::fs::read_to_string(home.join(".ai/local/state/current.yaml")).unwrap(),
        "id: t-9\n"
    );
    ok(&root, &["worktree", "validate"]);
}

#[test]
fn a_worktree_occupying_the_container_path_is_moved_out_and_then_in() {
    let f = Fixture::new();
    let root = f.root();
    let container = f.container();
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "int/occupant",
            container.to_str().unwrap(),
        ],
    );
    std::fs::write(container.join("wip.txt"), "in progress\n").unwrap();
    let other = f.parent().join("other");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "fix/other",
            other.to_str().unwrap(),
        ],
    );

    let t = json_exit(&root, &["worktree", "topology"], 10);
    assert!(t["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "worktree.container_occupied"));
    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    assert_eq!(
        plan["steps"][0]["branch"], "int/occupant",
        "the occupant moves first"
    );
    assert_eq!(plan["steps"][0]["action"], "move_via_staging");

    let applied = json(&root, &["worktree", "migrate"]);
    assert_eq!(applied["moved"], 2, "{applied:#}");
    assert!(container.join("int/occupant/wip.txt").is_file());
    assert!(container.join("fix/other").is_dir());
    assert!(!other.exists());
    assert!(!f.parent().join("repo-wt.staging").exists());
    assert_eq!(
        std::fs::read_dir(f.parent())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("migrating"))
            .count(),
        0,
        "a staging directory was left behind"
    );
    ok(&root, &["worktree", "validate"]);
}

#[test]
fn a_locked_worktree_blocks_its_own_step_and_not_the_others() {
    let f = Fixture::new();
    let root = f.root();
    let a = f.parent().join("a");
    let b = f.parent().join("b");
    git(
        &root,
        &["worktree", "add", "-q", "-b", "fix/a", a.to_str().unwrap()],
    );
    git(
        &root,
        &["worktree", "add", "-q", "-b", "fix/b", b.to_str().unwrap()],
    );
    git(
        &root,
        &[
            "worktree",
            "lock",
            "--reason",
            "in use",
            a.to_str().unwrap(),
        ],
    );
    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    let steps = plan["steps"].as_array().unwrap();
    let locked = steps.iter().find(|s| s["branch"] == "fix/a").unwrap();
    let free = steps.iter().find(|s| s["branch"] == "fix/b").unwrap();
    assert_eq!(locked["outcome"], "blocked");
    assert_eq!(locked["blockers"][0]["code"], "worktree.locked");
    assert_eq!(free["outcome"], "planned");
    code(&root, &["worktree", "migrate"], 10);
    assert!(a.is_dir(), "the locked worktree was moved");
    assert!(f.container().join("fix/b").is_dir());
}

// ---------------------------------------------------------------- conflicts (F)

#[test]
fn a_destination_conflict_is_detected_and_never_overwritten() {
    let f = Fixture::new();
    let root = f.root();
    let outside = f.parent().join("outside");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature/taken",
            outside.to_str().unwrap(),
        ],
    );
    let dest = f.container().join("feature/taken");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(dest.join("precious.txt"), "do not touch\n").unwrap();

    let plan = json(&root, &["worktree", "migrate", "--plan"]);
    let step = &plan["steps"][0];
    assert_eq!(step["outcome"], "blocked");
    assert_eq!(step["blockers"][0]["code"], "worktree.destination_conflict");
    assert!(step["blockers"][0]["message"]
        .as_str()
        .unwrap()
        .contains("unrelated directory"));
    code(&root, &["worktree", "migrate"], 10);
    assert_eq!(
        std::fs::read_to_string(dest.join("precious.txt")).unwrap(),
        "do not touch\n"
    );
    assert!(outside.is_dir());

    // create refuses the same collision for a new branch
    let dest2 = f.container().join("feature/file");
    std::fs::create_dir_all(dest2.parent().unwrap()).unwrap();
    std::fs::write(&dest2, "a file\n").unwrap();
    let out = code(&root, &["worktree", "create", "feature/file"], 10);
    assert!(out.contains("already exists"), "{out}");
    assert_eq!(std::fs::read_to_string(&dest2).unwrap(), "a file\n");
}

#[test]
fn a_branch_checked_out_elsewhere_is_named_rather_than_stolen() {
    let f = Fixture::new();
    let root = f.root();
    let outside = f.parent().join("outside");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature/held",
            outside.to_str().unwrap(),
        ],
    );
    let out = code(&root, &["worktree", "create", "feature/held"], 10);
    assert!(out.contains("checked out at"), "{out}");
    assert!(!f.container().join("feature/held").exists());
    let out = code(&root, &["worktree", "ensure", "feature/held"], 10);
    assert!(out.contains("checked out at"), "{out}");
}

#[test]
fn the_same_worktree_twice_is_refused_by_create_and_answered_by_ensure() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature/once"]);
    let before = git(&root, &["worktree", "list", "--porcelain"]);
    let out = code(&root, &["worktree", "create", "feature/once"], 10);
    assert!(out.contains("already has its canonical worktree"), "{out}");
    let out = ok(&root, &["worktree", "ensure", "feature/once"]);
    assert!(out.contains("exists"), "{out}");
    assert_eq!(git(&root, &["worktree", "list", "--porcelain"]), before);
}

#[test]
fn a_stale_registration_is_reported_and_repair_drops_it_without_deleting_anything() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature/gone"]);
    let home = f.container().join("feature/gone");
    std::fs::remove_dir_all(&home).unwrap();
    let t = json(&root, &["worktree", "topology"]);
    let w = t["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["branch"] == "feature/gone")
        .unwrap();
    assert_eq!(w["standing"], "missing");
    assert!(t["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "worktree.stale_registration" || d["code"] == "worktree.missing"));
    let dry = json(&root, &["worktree", "repair", "--dry-run"]);
    assert_eq!(dry["applied"], false);
    assert!(git(&root, &["worktree", "list", "--porcelain"]).contains("feature/gone"));
    ok(&root, &["worktree", "repair"]);
    assert!(!git(&root, &["worktree", "list", "--porcelain"]).contains("feature/gone"));
    assert!(git(&root, &["branch", "--list", "feature/gone"]).contains("feature/gone"));
}

// ---------------------------------------------------------------- safety

#[test]
fn a_dirty_worktree_is_never_removed_without_force_and_the_branch_survives() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature/dirty"]);
    let wt = f.container().join("feature/dirty");
    std::fs::write(wt.join("scratch.txt"), "notes\n").unwrap();
    let out = code(&root, &["worktree", "remove", "feature/dirty"], 10);
    assert!(out.contains("dirty"), "{out}");
    assert!(wt.join("scratch.txt").is_file());
    ok(&root, &["worktree", "remove", "feature/dirty", "--force"]);
    assert!(!wt.exists());
    assert!(git(&root, &["branch", "--list", "feature/dirty"]).contains("feature/dirty"));
    for selector in [root.to_str().unwrap(), "master", "main"] {
        let (_, out, err) = run_in(&root, &["worktree", "remove", selector], "");
        let text = format!("{out}{err}");
        assert!(
            text.contains("primary checkout") || text.contains("no worktree"),
            "removing the primary checkout by `{selector}` gave: {text}"
        );
    }
    assert!(root.join(".ai/manifest.yaml").is_file());
}

#[test]
fn a_symlink_cannot_make_a_misplaced_worktree_look_canonical() {
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
            "feature/sneaky",
            outside.to_str().unwrap(),
        ],
    );
    std::fs::create_dir_all(f.container().join("feature")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, f.container().join("feature/sneaky")).unwrap();
    let t = json_exit(&root, &["worktree", "topology"], 10);
    let w = t["worktrees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["branch"] == "feature/sneaky")
        .unwrap();
    assert_eq!(w["standing"], "misplaced");
    assert!(w["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == "worktree.destination_conflict"));
}

#[test]
fn invalid_branch_names_are_refused_before_git_is_asked() {
    let f = Fixture::new();
    let root = f.root();
    for bad in [
        "../escape",
        "feature/../../etc",
        ".hidden/x",
        "a b",
        "-x",
        "x.lock",
        "a//b",
    ] {
        let out = code(&root, &["worktree", "create", "--", bad], 10);
        assert!(out.contains("invalid branch name"), "{bad}: {out}");
        let out = code(&root, &["worktree", "path", "--", bad], 10);
        assert!(out.contains("invalid branch name"), "{bad}: {out}");
    }
    assert!(
        !f.container().exists(),
        "a refused create created the container"
    );
}

#[test]
fn the_offline_branch_rules_agree_with_git_over_many_names() {
    let f = Fixture::new();
    let root = f.root();
    let names = [
        "feature/x",
        "feature/x/y/z",
        "a.b",
        "a-b_c",
        "žluťoučký",
        "v1.2.3",
        "a/b.lock",
        "a..b",
        "a/.b",
        "@",
        "a@{b",
        "a:b",
        "a?b",
        "a*b",
        "a[b",
        "a\\b",
        "a~b",
        "a^b",
        "-a",
        "a/",
        "/a",
        "a//b",
        "a.",
        "with space",
        "ünïcode/ok",
        "x.lock/y",
        "a@b",
        "a{b}",
        "a+b",
        "a=b",
    ];
    for name in names {
        let ours = majordomus_cli::worktree::BranchName::parse(name).is_ok();
        // `--branch` is the form `git branch` and `git switch -c` apply: it also refuses a
        // leading `-`, which `refs/heads/-x` as a bare reference would accept.
        let theirs = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["check-ref-format", "--branch", name])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success();
        assert_eq!(ours, theirs, "{name:?}: offline {ours}, git {theirs}");
    }
}

// ---------------------------------------------------------------- concurrency

#[test]
fn two_processes_creating_at_once_produce_one_worktree_and_one_clear_refusal() {
    let f = Fixture::new();
    let root = f.root();
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || {
                run_in(&root, &["worktree", "create", "feature/contended"], "")
            })
        })
        .collect();
    let results: Vec<(i32, String, String)> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();
    let succeeded = results.iter().filter(|(c, _, _)| *c == 0).count();
    assert_eq!(succeeded, 1, "expected exactly one success: {results:#?}");
    let refused = results.iter().find(|(c, _, _)| *c != 0).unwrap();
    assert_eq!(
        refused.0, 10,
        "the loser should refuse cleanly: {refused:#?}"
    );
    let registered = registered_paths(&root)
        .iter()
        .filter(|p| p.ends_with("feature/contended"))
        .count();
    assert_eq!(registered, 1);
    ok(&root, &["worktree", "validate"]);
}

// ---------------------------------------------------------------- parity (H)

#[test]
fn the_command_line_and_the_capability_registry_see_one_topology() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature/parity"]);
    let outside = f.parent().join("outside");
    git(
        &root,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "fix/outside",
            outside.to_str().unwrap(),
        ],
    );
    let via_cli = json_exit(&root, &["worktree", "topology"], 10);

    let app = common::load_app(&f);
    let via_registry = app
        .context
        .execute("worktree.topology", serde_json::json!({}))
        .expect("the capability answers");
    // the observed_from differs only in that the CLI ran in the fixture; everything else is
    // one struct serialised twice
    let strip = |mut v: Value| {
        v.as_object_mut().unwrap().remove("observed_from");
        v
    };
    assert_eq!(strip(via_cli), strip(via_registry));

    let plan_cli = json(&root, &["worktree", "migrate", "--plan"]);
    let plan_registry = app
        .context
        .execute("worktree.migration_plan", serde_json::json!({}))
        .unwrap();
    assert_eq!(plan_cli, plan_registry);

    let status_registry = app
        .context
        .execute(
            "worktree.status",
            serde_json::json!({ "path": outside.to_str().unwrap() }),
        )
        .unwrap();
    assert_eq!(status_registry["worktree"]["branch"], "fix/outside");
    assert_eq!(status_registry["canonical"], false);

    // a path in another repository is refused: the topology answers only about its own
    let other = Fixture::new();
    let err = app
        .context
        .execute(
            "worktree.status",
            serde_json::json!({ "path": other.root().to_str().unwrap() }),
        )
        .unwrap_err();
    assert!(err.to_string().contains("refused"), "{err}");
}

// ---------------------------------------------------------------- cleanup and issues

#[test]
fn cleanup_eligibility_is_derived_and_nothing_is_deleted() {
    let f = Fixture::new();
    let root = f.root();
    ok(&root, &["worktree", "create", "feature/merged"]);
    let wt = f.container().join("feature/merged");
    std::fs::write(wt.join("new.txt"), "x\n").unwrap();
    git(&wt, &["add", "new.txt"]);
    git(
        &wt,
        &[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "merged work",
        ],
    );
    git(&root, &["merge", "-q", "--no-edit", "feature/merged"]);
    ok(&root, &["worktree", "create", "feature/unmerged"]);
    let unmerged_wt = f.container().join("feature/unmerged");
    std::fs::write(unmerged_wt.join("own.txt"), "own work\n").unwrap();
    git(&unmerged_wt, &["add", "own.txt"]);
    git(
        &unmerged_wt,
        &[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "own work",
        ],
    );
    // a branch created a moment ago with no commits of its own is, in git's sense, merged
    ok(&root, &["worktree", "create", "feature/fresh"]);
    let t = json(&root, &["worktree", "topology"]);
    let branches = t["branches"].as_array().unwrap();
    let merged = branches
        .iter()
        .find(|b| b["name"] == "feature/merged")
        .unwrap();
    let unmerged = branches
        .iter()
        .find(|b| b["name"] == "feature/unmerged")
        .unwrap();
    let fresh = branches
        .iter()
        .find(|b| b["name"] == "feature/fresh")
        .unwrap();
    assert_eq!(merged["cleanup_eligible"], true);
    assert_eq!(unmerged["cleanup_eligible"], false);
    assert_eq!(
        fresh["cleanup_eligible"], true,
        "no commits of its own, clean: nothing to lose"
    );
    assert_eq!(
        branches.iter().find(|b| b["trunk"] == true).unwrap()["cleanup_eligible"],
        false
    );
    let out = ok(&root, &["worktree", "cleanup"]);
    assert!(
        out.contains("feature/merged") && !out.contains("feature/unmerged"),
        "{out}"
    );
    assert!(wt.is_dir(), "cleanup deleted a worktree");
}

#[test]
fn an_issue_is_inferred_only_from_an_exact_id_component_and_create_names_it_that_way() {
    let f = Fixture::new();
    let root = f.root();
    std::fs::create_dir_all(root.join(".ai/repo/project/issues")).unwrap();
    std::fs::write(
        root.join(".ai/repo/project/issues/README.md"),
        "---\nschema: context/v1\nid: ai.repo.project.issues\nkind: context\ntitle: Issues\ndescription: Issues.\nstatus: active\nscope: subtree\nproviders: [\"*\"]\naudience: [human, agent]\ncomposition: extend\norder: 100\n---\n# Issues\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".ai/repo/project/issues/I0042.yaml"),
        "id: I0042\ntitle: Live worktree page\nslug: live-worktree-page\n",
    )
    .unwrap();
    git(&root, &["add", "-A"]);
    git(
        &root,
        &[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "issue",
        ],
    );
    ok(&root, &["worktree", "create", "feature/I0042-live-page"]);
    ok(&root, &["worktree", "create", "feature/I00420-not-it"]);
    ok(&root, &["worktree", "create", "feature/live-page"]);
    let t = json(&root, &["worktree", "topology"]);
    let issue_of = |branch: &str| {
        t["worktrees"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["branch"] == branch)
            .unwrap()["issue"]
            .clone()
    };
    assert_eq!(issue_of("feature/I0042-live-page"), "I0042");
    assert_eq!(issue_of("feature/I00420-not-it"), Value::Null);
    assert_eq!(issue_of("feature/live-page"), Value::Null);
}
