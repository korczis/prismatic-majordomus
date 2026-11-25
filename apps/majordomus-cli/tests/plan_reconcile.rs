//! `plan.reconcile` through the built executable (I4773, ADR 0238): in a fixture repository
//! whose READY issue a merged pull request names, the proposal is `start` and never `verify`;
//! `--apply` refuses while the forge is unread, writes the start through plan.transition once
//! it is read, proposes nothing the second time, and writes `done` with `--done` only once
//! the record carries evidence recorded after the merge. A `Delivers:` trailer is an edge,
//! and a BLOCKED issue is held with the guard's reason. Drift reports the VERIFY issue that
//! owes evidence as `merged_unaccepted`.

mod common;

use common::{run_in, Fixture, ISSUE};
use serde_json::{json, Value};

/// I0001 merged by pull request #5, whose title names it; I0002, which depends on it,
/// delivered by a commit whose subject names nothing and whose trailer says so.
fn fixture() -> Fixture {
    let f = Fixture::new();
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &ISSUE
            .replace("id: I0001", "id: I0002")
            .replace("slug: issue-I0001", "slug: issue-I0002")
            .replace(
                "title: The bounded piece of work",
                "title: The second piece of work",
            )
            .replace(
                "evidence_required:",
                "depends_on:\n  - I0001\nevidence_required:",
            ),
    );
    f.commit("docs(plan): I0002");
    let trunk = f
        .git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    f.git(&["checkout", "-q", "-b", "feature/the-work"]);
    f.write("lib/work.sh", "echo work\n");
    f.commit("wip");
    f.git(&["checkout", "-q", &trunk]);
    f.git(&[
        "merge",
        "-q",
        "--no-ff",
        "feature/the-work",
        "-m",
        "Merge pull request #5 from someone/feature/the-work",
        "-m",
        "feat: the work (I0001)",
    ]);
    f.write("lib/second.sh", "echo second\n");
    f.git(&["add", "-A"]);
    f.git(&[
        "commit",
        "-q",
        "-m",
        "Add: [Lib] The second piece",
        "-m",
        "A commit grammar with no room for an id in its subject.",
        "-m",
        "Delivers: I0002",
    ]);
    f
}

fn observe_forge(f: &Fixture) {
    f.write(
        ".ai/local/state/integration/observation.json",
        &json!({
            "schema": 4, "repository": "someone/fixture", "base": "master",
            "base_sha": "0000000000000000000000000000000000000000",
            "observed_at": "2026-11-25T08:00:00Z", "required_checks": null,
            "review_policy": null, "merge_methods": ["merge"], "pull_requests": []
        })
        .to_string(),
    );
}

fn reconcile(f: &Fixture, args: &[&str]) -> (i32, Value, String) {
    let mut argv = vec!["plan", "reconcile"];
    argv.extend_from_slice(args);
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}\n{err}"));
    (code, v, err)
}

/// One `plan.transition`, as any surface asks it.
fn transition(f: &Fixture, issue: &str, t: &str) {
    let input = json!({ "issue": issue, "transition": t }).to_string();
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "run",
            "plan.transition",
            "--input",
            &input,
            "--quiet",
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 0, "{t} {issue}: {out}{err}");
}

fn proposal<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["proposals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["issue"] == id)
        .unwrap_or_else(|| panic!("no proposal for {id} in {v}"))
}

#[test]
fn a_merged_ready_issue_is_started_through_the_transition_and_done_only_on_fresh_evidence() {
    let f = fixture();

    // the forge was never observed: listed, unsettled, and nothing is applied
    let (code, v, err) = reconcile(&f, &[]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(v["settled"], false);
    let i1 = proposal(&v, "I0001");
    assert_eq!(i1["class"], "recordable");
    assert_eq!(
        i1["moves"],
        json!([{"issue": "I0001", "transition": "start",
                                     "command": "majordomus plan start I0001"}])
    );
    assert_eq!(i1["merge"]["pull_request"], 5);
    assert_eq!(
        i1["evidence"][0],
        json!({"token": "proof", "covered": false, "fresh": false})
    );
    assert_eq!(i1["validation"], json!(["true"]));
    assert_eq!(i1["acceptance"][0]["standing"], "unjudged");
    let i2 = proposal(&v, "I0002");
    assert_eq!(
        i2["merge"]["via"], "trailer",
        "the trailer is the only edge to I0002"
    );
    assert_eq!(i2["class"], "held");
    assert!(i2["refused"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("waiting on I0001"));
    let (code, v, _) = reconcile(&f, &["--apply"]);
    assert_eq!(code, 12, "an unsettled proposal is not applied");
    assert_eq!(v["applied"], json!([]));

    // once the forge is read, the start is written with its seal, and nothing else
    observe_forge(&f);
    let (code, v, err) = reconcile(&f, &["I0001", "--apply"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        v["proposals"].as_array().unwrap().len(),
        1,
        "only the issue asked"
    );
    assert_eq!(
        v["applied"],
        json!([{"issue": "I0001", "transition": "start", "outcome": "written", "status": "ACTIVE"}])
    );
    let record = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).unwrap();
    assert!(
        record.contains("started_at: ") && record.contains("started_event: "),
        "{record}"
    );
    assert!(!record.contains("verified_at"), "verify is never written");

    // ACTIVE and owing its evidence: unaccepted, no move, and --done writes nothing
    let (code, v, _) = reconcile(&f, &["I0001", "--apply", "--done"]);
    assert_eq!(code, 0);
    assert_eq!(proposal(&v, "I0001")["class"], "unaccepted");
    assert_eq!(v["applied"], json!([]));

    // evidence recorded before the merge does not accept it
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &format!("{record}evidence:\n  - covers: proof\n    recorded_at: 2001-01-01T00:00:00Z\n"),
    );
    let (_, v, _) = reconcile(&f, &["I0001"]);
    assert_eq!(proposal(&v, "I0001")["evidence"][0]["fresh"], false);
    assert!(proposal(&v, "I0001")["refused"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("not recorded after the merge"));

    // evidence recorded since: done is admitted, and written only with --done
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &format!("{record}evidence:\n  - covers: proof\n    recorded_at: 2099-01-01T00:00:00Z\n"),
    );
    let (_, v, _) = reconcile(&f, &["I0001"]);
    assert_eq!(proposal(&v, "I0001")["class"], "acceptable");
    let (code, v, _) = reconcile(&f, &["I0001", "--apply"]);
    assert_eq!(
        (code, v["applied"].clone()),
        (0, json!([])),
        "done needs --done"
    );
    let (code, v, err) = reconcile(&f, &["I0001", "--apply", "--done"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(v["applied"][0]["status"], "DONE");

    // the text names the moves, what holds the rest, and the evidence
    let (code, out, _) = run_in(&f.root(), &["plan", "reconcile"], "");
    assert_eq!(code, 0);
    assert!(out.contains("plan reconcile — live"), "{out}");
    assert!(
        out.contains("MOVE     majordomus plan start I0002"),
        "{out}"
    );
    assert!(out.contains("EVIDENCE proof missing"), "{out}");
    assert!(
        out.contains("CRITERION (unjudged) The work is done"),
        "{out}"
    );
    assert!(out.contains("RUN      true"), "{out}");
}

#[test]
fn an_unknown_issue_is_not_found_and_a_move_that_cannot_be_written_is_refused() {
    let f = fixture();
    let (code, _, err) = run_in(&f.root(), &["plan", "reconcile", "I9999"], "");
    assert_ne!(code, 0);
    assert!(err.contains("no issue 'I9999'"), "{err}");

    // the text of an unsettled proposal says why, and an unsettled apply writes nothing
    let (code, out, _) = run_in(&f.root(), &["plan", "reconcile", "--apply"], "");
    assert_eq!(code, 12);
    assert!(out.contains("unsettled"), "{out}");
    assert!(out.contains("nothing: the proposal is unsettled"), "{out}");

    // a move that cannot be written is reported refused, stops its issue and not the batch,
    // and the run exits 10: here the record cannot be rewritten
    observe_forge(&f);
    let record = f.path(".ai/repo/project/issues/I0001.yaml");
    let mut perms = std::fs::metadata(&record).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&record, perms.clone()).unwrap();
    let (code, v, _) = reconcile(&f, &["--apply"]);
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    std::fs::set_permissions(&record, perms).unwrap();
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["applied"][0]["issue"], "I0001");
    assert_eq!(v["applied"][0]["outcome"], "refused");
    assert!(
        v["applied"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("I0001"),
        "{v}"
    );
    let (_, out, _) = run_in(&f.root(), &["plan", "reconcile", "I0001", "--apply"], "");
    assert!(out.contains("written  start I0001 -> ACTIVE"), "{out}");
}

#[test]
fn drift_reports_a_merged_verify_issue_that_owes_evidence_and_never_refuses_it() {
    let f = fixture();
    f.write(
        ".ai/repo/ci/plan.yaml",
        "version: 1\ndrift:\n  window: 60\n  enforcement: blocking\n",
    );
    transition(&f, "I0001", "start");
    transition(&f, "I0001", "verify");
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "plan", "drift", "I0001", "--gate", "trunk", "--format", "json",
        ],
        "",
    );
    assert_eq!(code, 0, "reported, never refused: {err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    let finding = &v["findings"][0];
    assert_eq!(finding["class"], "merged_unaccepted");
    assert_eq!(finding["verdict"], "reported");
    assert_eq!(
        finding["remedy"],
        "majordomus plan evidence I0001 --covers proof"
    );
    assert_eq!(v["tallies"]["merged_unaccepted"], 1);
}
