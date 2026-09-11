//! The continuity surface, over real repositories with real git state.
//!
//! The behavioural claims these exist for are the ones a reader of a resumed session
//! depends on and cannot check for themselves:
//!
//! * a record from another branch is never offered, however new it is;
//! * a record whose commit is no longer in this history is labelled, not quietly served;
//! * a malformed record degrades the answer and never fails the call;
//! * absence is reported as absence.
//!
//! Each of those is the difference between a briefing that is merely unhelpful and one that
//! is confidently about somebody else's work. Timestamps are the only volatile field and
//! nothing here asserts on them.

mod common;

use common::{Fixture, Served};
use serde_json::Value;

/// A record of the local half: the front matter the resolver reads, over a body with the
/// section a resuming worker acts on.
fn record(
    created: &str,
    task: &str,
    branch: &str,
    head: &str,
    worktree: &str,
    next: &str,
) -> String {
    format!(
        "---\n\
         schema_version: 1\n\
         created_at: {created}\n\
         task_id: {task}\n\
         profile: implementation\n\
         owner: \"tester\"\n\
         repository_id: {worktree}/.git\n\
         worktree: {worktree}\n\
         branch: {branch}\n\
         head: {head}\n\
         working_tree: clean\n\
         changed_files:\n\
         ---\n\
         \n\
         # Objective\n\
         \n\
         Something.\n\
         \n\
         # Current State\n\
         \n\
         Somewhere.\n\
         \n\
         # Next Action\n\
         \n\
         {next}\n"
    )
}

/// Ask the running server for the continuity state.
fn state(s: &Served) -> Value {
    let (status, body) = s.get("/api/v1/continuity");
    assert_eq!(status, 200, "continuity route: {body}");
    body
}

#[test]
fn a_record_from_another_branch_is_never_offered_however_new_it_is() {
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();

    // The older record is this branch's. The newer one is another branch's, in the same
    // worktree, and is the one a "latest file wins" resolver would return.
    f.write(
        ".ai/local/state/handovers/a.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t-mine",
            &branch,
            &head,
            &root_s,
            "Continue mine.",
        ),
    );
    f.write(
        ".ai/local/state/handovers/b.md",
        &record(
            "2030-01-01T00:00:00Z",
            "t-theirs",
            "some/other-branch",
            &head,
            &root_s,
            "Continue theirs.",
        ),
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    let h = &c["handover"];
    assert_eq!(
        h["task_id"], "t-mine",
        "the newer record belongs to another branch and must not win: {c}"
    );
    assert_eq!(h["matched"], "same_worktree_same_branch");
    assert!(
        h["next_action"].as_str().unwrap().contains("Continue mine"),
        "the acted-on section travels with the record: {h}"
    );
    s.stop();
}

#[test]
fn a_record_written_on_a_commit_this_history_no_longer_has_is_labelled_diverged() {
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();

    // A commit id of the right shape that this repository has never seen. The resolver must
    // ask git rather than compare strings, and git must answer "not an ancestor".
    let orphan = "0123456789abcdef0123456789abcdef01234567";
    f.write(
        ".ai/local/state/handovers/a.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t-old",
            &branch,
            orphan,
            &root_s,
            "Pick up where the rewritten history left off.",
        ),
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(
        c["handover"]["divergence"], "diverged",
        "a commit outside this history is diverged, not exact: {c}"
    );
    let findings = c["findings"].as_array().unwrap();
    assert!(
        findings.iter().any(|f| f.as_str().unwrap().contains("trust git over it")),
        "the label must also be a finding, so a reader who skims the record still meets it: {findings:?}"
    );
    s.stop();
}

#[test]
fn a_record_written_at_this_commit_is_exact_and_one_behind_it_is_advanced() {
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();
    let first = f.git(&["rev-parse", "HEAD"]).trim().to_string();

    f.write(
        ".ai/local/state/handovers/a.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t",
            &branch,
            &first,
            &root_s,
            "Go on.",
        ),
    );

    let mut s = Served::start(&root, &[]);
    assert_eq!(state(&s)["handover"]["divergence"], "exact");
    s.stop();

    // Move git forward. The same record is now behind, which is a different instruction to
    // the reader: trust it, and expect some of it to be done.
    f.write("moved.txt", "one more commit\n");
    f.commit("move the branch on");

    let mut s = Served::start(&root, &[]);
    assert_eq!(
        state(&s)["handover"]["divergence"],
        "advanced",
        "git moving forward makes a record advanced, never diverged"
    );
    s.stop();
}

#[test]
fn a_malformed_record_degrades_the_answer_and_never_fails_the_call() {
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();

    f.write(
        ".ai/local/state/handovers/broken.md",
        "no front matter here\n",
    );
    f.write(
        ".ai/local/state/handovers/unsupported.md",
        "---\nschema_version: 99\ncreated_at: 2026-01-01T00:00:00Z\nhead: abc\n---\n",
    );
    f.write(
        ".ai/local/state/handovers/good.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t-good",
            &branch,
            &head,
            &root_s,
            "The one readable record wins.",
        ),
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(c["handover"]["task_id"], "t-good");
    let findings = c["findings"].as_array().unwrap();
    assert!(
        findings
            .iter()
            .any(|f| f.as_str().unwrap().contains("could not be read as records")),
        "skipping is reported, never silent: {findings:?}"
    );
    s.stop();
}

#[test]
fn a_future_schema_version_is_refused_rather_than_read_as_this_one() {
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();

    // Everything about this record is readable except the number that says what its fields
    // mean. Reading it anyway is how a field that changed meaning is quietly misread.
    let mut newer = record(
        "2030-01-01T00:00:00Z",
        "t-future",
        &branch,
        &head,
        &root_s,
        "From a later Majordomus.",
    );
    newer = newer.replace("schema_version: 1", "schema_version: 2");
    f.write(".ai/local/state/handovers/future.md", &newer);
    f.write(
        ".ai/local/state/handovers/present.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t-now",
            &branch,
            &head,
            &root_s,
            "From this one.",
        ),
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(
        c["handover"]["task_id"], "t-now",
        "a newer schema version is skipped, not guessed at: {c}"
    );
    s.stop();
}

#[test]
fn an_open_session_belonging_to_another_checkout_is_reported_and_not_adopted() {
    let f = Fixture::new();
    let root = f.root();

    f.write(
        ".ai/local/state/session-current.yaml",
        "session_id: s-elsewhere\n\
         started_at: 2026-01-01T00:00:00Z\n\
         owner: \"someone\"\n\
         repository_id: /somewhere/else/.git\n\
         worktree: /somewhere/else\n\
         branch: main\n\
         start_head: 0123456789abcdef0123456789abcdef01234567\n\
         start_working_tree: clean\n",
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(c["session"]["foreign"], true, "{c}");
    assert!(
        c["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap().contains("belongs to /somewhere/else")),
        "a foreign episode is named, with the checkout it belongs to: {c}"
    );
    s.stop();
}

#[test]
fn absence_is_an_answer_and_a_checkout_with_no_records_says_so() {
    // The fixture seeds a task record and nothing else, which is the shape of a checkout
    // that has started work and produced no continuation yet — the case a resuming worker
    // meets most often, and the one where inventing a briefing would be worst.
    let f = Fixture::new();
    let root = f.root();

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert!(
        c.get("handover").is_none(),
        "an absent record is an absent key, never a null a reader might render: {c}"
    );
    assert!(c.get("checkpoint").is_none(), "{c}");
    assert!(c.get("session").is_none(), "{c}");
    assert_eq!(c["tallies"]["handovers"], 0);
    assert_eq!(
        c["findings"].as_array().map(Vec::len).unwrap_or(0),
        0,
        "absence is an answer, not a finding: {c}"
    );
    s.stop();
}

#[test]
fn a_checkout_with_no_local_half_at_all_says_so_rather_than_failing() {
    let f = Fixture::new();
    let root = f.root();
    f.remove(".ai/local/state/current.yaml");
    std::fs::remove_dir_all(root.join(".ai/local")).ok();

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(c["present"], false, "a fresh clone has no local half: {c}");
    assert!(c.get("task").is_none(), "{c}");
    s.stop();
}

#[test]
fn the_stores_own_documentation_is_not_read_as_a_blocker() {
    // The template documents its line format inside an HTML comment. A reader that does not
    // skip the comment reports a blocker in every fresh checkout — and a phantom blocker is
    // worse than a missed one, because it refuses work nobody can unblock.
    let f = Fixture::new();
    let root = f.root();
    f.write(
        ".ai/local/state/open-questions.md",
        "# Open questions\n\n\
         Things blocked on a human.\n\n\
         <!--\n\
         - [unresolved] <task id> — <question> (<date>)\n\
         - [resolved YYYY-MM-DD] <task id> — <question> — <answer>\n\
         -->\n",
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(
        c["tallies"]["blockers"], 0,
        "the template's own example is documentation, not a blocker: {c}"
    );
    assert!(c.get("blockers").is_none(), "{c}");
    s.stop();
}

#[test]
fn every_unresolved_question_is_reported_as_refusing_completion() {
    let f = Fixture::new();
    let root = f.root();

    f.write(
        ".ai/local/state/open-questions.md",
        "# Open questions\n\n\
         <!--\n- [unresolved] <task id> — <question> (<date>)\n-->\n\
         - [unresolved] t-1 — Does the cache survive a rebase? (2026-01-01)\n\
         - [resolved 2026-01-02] t-1 — Answered one. (2026-01-01)\n\
         - [unresolved] t-2 — Which budget applies here? (2026-01-01)\n",
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    let blockers = c["blockers"].as_array().unwrap();
    assert_eq!(blockers.len(), 2, "resolved entries are not blockers: {c}");
    assert_eq!(c["tallies"]["blockers"], 2);
    assert!(c["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f.as_str().unwrap().contains("refuses")));
    s.stop();
}

#[test]
fn the_local_half_is_served_here_and_projected_nowhere() {
    // The guarantee that makes this module safe: what it reads names this machine, so no
    // generated artifact may carry it. A committed projection that mentioned the state
    // directory's records would publish the one part of the layer no other clone can
    // reproduce.
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();
    f.write(
        ".ai/local/state/handovers/a.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t-secret-ish",
            &branch,
            &head,
            &root_s,
            "Not for publication.",
        ),
    );

    let (code, out, err) = common::run_in(&root, &["generate"], "");
    assert_eq!(code, 0, "generate: {out}{err}");

    for rel in ["docs/generated", "site/data"] {
        let dir = root.join(rel);
        if !dir.exists() {
            continue;
        }
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                assert!(
                    !text.contains("t-secret-ish"),
                    "{} carries a record of the local half; nothing generated may",
                    p.display()
                );
            }
        }
    }
}

#[test]
fn the_ledger_is_counted_and_a_detached_checkout_says_detached_rather_than_guessing() {
    // Two fields of the answer that nothing reached until now, and both of them are about
    // saying what is true rather than what is convenient.
    //
    // `tallies.ledger_lines` is how a reader learns whether this checkout's lifecycle has
    // written anything at all — the distinction ADR 0041 was written after, where events kept
    // arriving and derived state stopped advancing. Blank lines are not lines: a store that
    // ends with a newline would otherwise be reported one event richer than it is.
    //
    // `branch` is `DETACHED` when git has no branch to name. It matters beyond the label:
    // the resolver's second tier is "another worktree of this repository, on this branch",
    // and a detached checkout has no branch, so that tier must be unreachable rather than
    // matching everything. A reader handed another worktree's record because its own branch
    // was the empty string cannot tell the record is not about its work.
    let f = Fixture::new();
    let root = f.root();
    let root_s = root.to_string_lossy().to_string();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();

    // a record on the branch, in another worktree of this repository: tier 1, and therefore
    // offered while a branch is checked out and not offered once there is none
    f.write(
        ".ai/local/state/handovers/theirs.md",
        &record(
            "2026-01-01T00:00:00Z",
            "t-1",
            &branch,
            &head,
            "/somewhere/else",
            "Theirs.",
        ),
    );
    f.write(
        ".ai/local/state/ledger.jsonl",
        "{\"ts\":\"2026-01-01T00:00:00Z\",\"event\":\"session.started\"}\n\
         \n\
         {\"ts\":\"2026-01-01T00:01:00Z\",\"event\":\"session.closed\"}\n\
         \n",
    );

    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(c["branch"], branch, "{c}");
    assert_eq!(c["tallies"]["ledger_lines"], 2, "blank lines are not events: {c}");
    assert_eq!(c["tallies"]["handovers"], 1, "{c}");
    assert_eq!(
        c["handover"]["matched"], "same_branch",
        "a record from another worktree on this branch is offered, and labelled: {c}"
    );
    assert_eq!(c["present"], true, "{c}");
    s.stop();

    // ...and with no branch at all, the second tier has nothing to match on
    f.git(&["checkout", "--detach"]);
    let mut s = Served::start(&root, &[]);
    let c = state(&s);
    assert_eq!(c["branch"], "DETACHED", "{c}");
    assert!(
        c.get("handover").is_none() || c["handover"].is_null(),
        "a detached checkout was handed another worktree's record: {c}"
    );
    assert_eq!(c["tallies"]["ledger_lines"], 2, "{c}");
    let _ = root_s;
    s.stop();
}
