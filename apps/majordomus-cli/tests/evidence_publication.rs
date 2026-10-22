//! The evidence publication is an allow-listed projection (I1955, ADR 0087 decisions 3 and 14).
//!
//! `evidence publish` writes what the site publishes about evidence. These tests drive the
//! built executable in a fixture repository whose one claim has a run recorded and committed,
//! and hold the four things the publication owes:
//!
//! * `publication_allow_listed`: every field it carries is on the declared allow-list, and a
//!   field off the list is not published;
//! * `publication_texts_pass_the_one_redactor`: every text passed the redactor, and the
//!   artifact passes `generate::forbidden_in`;
//! * `publication_states_defined`: its one state is the first matching row of the publication
//!   table, decided from what it observes;
//! * `publication_never_outranks_the_committed_ledger`: no verdict is stronger than the
//!   ledger committed at the presented commit.

mod common;

use std::path::Path;

use common::{run_in, Fixture};
use majordomus_cli::evidence::publication::{project, ALLOW_LIST, FILE, WITHHELD};
use majordomus_cli::evidence::ProofState;
use serde_json::{json, Value};

/// The fixture, with a passing run of its one claim's case recorded at C1 and committed in
/// C2, so that the claim is proven at C2 from the ledger C2 holds. Returns C2.
fn recorded() -> (Fixture, String) {
    let f = Fixture::new();
    let reports = tempfile::tempdir().unwrap();
    let tsv = reports.path().join("run.tsv");
    std::fs::write(&tsv, "00_x\tok\t1\tparallel\n").unwrap();
    let stamp = reports.path().join("stamp.json");
    let (tsv, stamp) = (tsv.display().to_string(), stamp.display().to_string());
    let (code, _, err) = run_in(
        &f.root(),
        &[
            "evidence",
            "stamp",
            "--producer",
            "suite",
            "--report",
            &tsv,
            "--out",
            &stamp,
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let provenance = format!("suite={stamp}");
    let (code, _, err) = run_in(
        &f.root(),
        &[
            "evidence",
            "record",
            "--suite",
            &tsv,
            "--provenance",
            &provenance,
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    f.commit("the recorded run");
    let c2 = head(&f);
    (f, c2)
}

fn head(f: &Fixture) -> String {
    f.git(&["rev-parse", "HEAD"]).trim().to_string()
}

/// One execution of the claim's case at `commit`, as a run record carries it.
fn execution(outcome: &str, commit: &str) -> Value {
    json!({
        "test": "suite:00_x", "runner": "suite", "source": "test/cases/00_x.sh",
        "outcome": outcome, "seconds": 1, "commit": commit, "working_tree": "clean",
        "digest": "sha256:0", "at": "2026-10-22T12:00:00Z", "origin": "ci",
        "command": "bash test/run.sh 00_x"
    })
}

/// A CI run record at `commit`, its tree `tree`, holding one execution of the claim's case.
fn record(id: &str, commit: &str, tree: &str, outcome: &str) -> Value {
    json!({
        "schema": 1, "id": id, "origin": "ci", "commit": commit, "working_tree": tree,
        "recorded_at": "2026-10-22T12:00:00Z", "ledger": "repo",
        "executions": [execution(outcome, commit)]
    })
}

/// A CI evidence artifact outside the checkout: each record as a file, beside a manifest
/// that is not a run record.
fn artifact(records: &[Value]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (i, r) in records.iter().enumerate() {
        std::fs::write(dir.path().join(format!("record-{i}.json")), r.to_string()).unwrap();
    }
    std::fs::write(dir.path().join("manifest.json"), "{\"schema\": 1}").unwrap();
    dir
}

/// `evidence publish` with `args` after the presented commit, into a directory outside the
/// checkout: the answer, and the file written, which must be the answer's publication.
fn publish(f: &Fixture, args: &[&str]) -> (Value, String) {
    let out = tempfile::tempdir().unwrap();
    let dir = out.path().join("published");
    let dir_s = dir.display().to_string();
    let mut argv = vec!["evidence", "publish", "--presented", "HEAD"];
    argv.extend_from_slice(args);
    argv.extend(["--out", dir_s.as_str(), "--format", "json"]);
    let (code, stdout, err) = run_in(&f.root(), &argv, "");
    assert_eq!(code, 0, "{argv:?}: {err}");
    let answer: Value = serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("{e}: {stdout}"));
    let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
    let written: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(written, answer["publication"], "the file is not the answer");
    (written, text)
}

fn from(dir: &tempfile::TempDir) -> String {
    dir.path().display().to_string()
}

/// The claim's published verdict.
fn claim(p: &Value) -> &Value {
    p["claims"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "policy-parse")
        .unwrap_or_else(|| panic!("no policy-parse claim in {p}"))
}

/// Every field path of `v`, in the allow-list's spelling: `a.b`, `a[]`, and the map keys of
/// `totals` as `totals.*`.
fn paths(v: &Value, at: &str, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                let key = if at == "totals" { "*" } else { k.as_str() };
                let p = if at.is_empty() {
                    key.to_string()
                } else {
                    format!("{at}.{key}")
                };
                paths(x, &p, out);
            }
        }
        Value::Array(items) => {
            for x in items {
                paths(x, &format!("{at}[]"), out);
            }
        }
        _ => out.push(at.to_string()),
    }
}

#[test]
fn publication_allow_listed() {
    let (f, p2) = recorded();
    let a = artifact(&[record("ci:github:1:1", &p2, "clean", "pass")]);
    let (p, _) = publish(&f, &["--from", &from(&a), "--run", "concluded"]);
    let mut found = Vec::new();
    paths(&p, "", &mut found);
    assert!(!found.is_empty());
    for path in &found {
        assert!(
            ALLOW_LIST.contains(&path.as_str()),
            "`{path}` is published and is not on the allow-list"
        );
    }
    // what a run record and the report carry beyond the list is not published: no
    // execution, no command, no digest, no claim sentence; the ledger's count of its
    // executions is the one path that names them
    let executions: Vec<&String> = found.iter().filter(|p| p.contains("execution")).collect();
    assert_eq!(executions, ["ledger.executions"], "{p}");
    let text = p.to_string();
    for absent in [
        "\"execution\"",
        "\"command\"",
        "\"digest\"",
        "\"claim\"",
        "sha256:",
    ] {
        assert!(!text.contains(absent), "{absent} is published: {text}");
    }
    // and the list is the only thing deciding: a field off it is dropped wherever it stands
    let mut planted = p.clone();
    planted["host"] = json!("a machine");
    planted["claims"][0]["execution"] = json!({ "command": "x" });
    planted["presented"]["account"] = json!("someone");
    assert_eq!(project(&planted), p);
    // each entry of the list is spelled once
    let mut seen = std::collections::BTreeSet::new();
    assert!(ALLOW_LIST.iter().all(|a| seen.insert(*a)), "{ALLOW_LIST:?}");
}

#[test]
fn publication_texts_pass_the_one_redactor() {
    let (f, p2) = recorded();
    // assembled at run time, so that no committed file carries a credential's shape or a
    // machine path
    let token = format!("{}{}", "ghp_", "a".repeat(24));
    let elsewhere = format!("{}{}", "/ho", "me/someone/.cache");
    let mut r = record(&format!("ci:{token}"), &p2, "clean", "pass");
    r["run"] = json!({
        "provider": "github", "id": "1", "attempt": 1, "workflow": "validate", "job": "evidence",
        "url": format!("{elsewhere}/run")
    });
    let a = artifact(&[r]);
    let (p, text) = publish(&f, &["--from", &from(&a)]);
    assert!(!text.contains(&token), "a credential is published: {text}");
    assert_eq!(p["records"][0]["id"], "ci:[redacted:github-token]", "{p}");
    assert_eq!(p["records"][0]["url"], WITHHELD, "{p}");
    assert_eq!(p["withheld"], 1, "{p}");
    assert!(
        majordomus_cli::generate::forbidden_in(&text).is_none(),
        "the artifact carries a forbidden marker: {text}"
    );
    // a machine path of the checkout itself is normalised, never published
    let root = f.root().display().to_string();
    assert!(!text.contains(&root), "{text}");
}

#[test]
fn publication_states_defined() {
    let (f, p2) = recorded();

    // row 3: P's own record, every tree clean, nothing capped
    let own = artifact(&[record("ci:own", &p2, "clean", "pass")]);
    let (p, _) = publish(&f, &["--from", &from(&own), "--run", "concluded"]);
    assert_eq!(p["state"], "current", "{p}");
    assert_eq!(p["records"][0]["relation"], "own", "{p}");
    assert_eq!(claim(&p)["state"], "proven", "{p}");
    assert_eq!(p["unread"][0]["file"], "manifest.json", "{p}");

    // row 5: P's own record carries a tree that is not clean, and says which
    let unclean = artifact(&[record("ci:unclean", &p2, "unknown", "pass")]);
    let (p, _) = publish(&f, &["--from", &from(&unclean), "--run", "concluded"]);
    assert_eq!(p["state"], "unknown", "{p}");
    assert!(
        p["reasons"][0].as_str().unwrap().contains("ci:unclean"),
        "{p}"
    );

    // row 2: a record that contradicts a verdict caps it at stale, and is named
    let failed = artifact(&[record("ci:failed", &p2, "clean", "fail")]);
    let (p, _) = publish(&f, &["--from", &from(&failed)]);
    assert_eq!(p["state"], "stale", "{p}");
    assert_eq!(p["capped_by"], json!(["ci:failed"]), "{p}");
    assert_eq!(claim(&p)["state"], "stale", "{p}");
    assert_eq!(claim(&p)["capped_by"], json!(["ci:failed"]), "{p}");
    assert_eq!(p["records"][0]["capped"], json!(["policy-parse"]), "{p}");

    // row 1: a dirty build that a record also contradicts reads dirty, and names the record
    let (p, _) = publish(&f, &["--from", &from(&failed), "--presented-tree", "dirty"]);
    assert_eq!(p["state"], "dirty", "{p}");
    assert_eq!(p["presented"]["tree"], "dirty", "{p}");
    assert_eq!(p["capped_by"], json!(["ci:failed"]), "{p}");
    assert!(
        p["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_str().unwrap().contains("ci:failed")),
        "{p}"
    );
    // and a build the caller cannot vouch for is dirty too: `build.json.dirty` is not false
    let (p, _) = publish(&f, &["--presented-tree", "unknown", "--run", "pending"]);
    assert_eq!(p["state"], "dirty", "{p}");

    // row 4: no record of P, and its run has not concluded
    let (p, _) = publish(&f, &["--run", "pending"]);
    assert_eq!(p["state"], "pending", "{p}");

    // row 5: a run concluded without a record reads unknown, and so does a run not found or
    // not described
    let (p, _) = publish(&f, &["--run", "concluded"]);
    assert_eq!(p["state"], "unknown", "{p}");
    assert!(
        p["reasons"][0]
            .as_str()
            .unwrap()
            .contains("concluded without a run record"),
        "{p}"
    );
    let (p, _) = publish(&f, &["--run", "absent"]);
    assert_eq!(p["state"], "unknown", "{p}");
    let (p, _) = publish(&f, &[]);
    assert_eq!(p["state"], "unknown", "{p}");

    // a record of an ancestor only never reads current: a passing one leaves the
    // publication pending or unknown, listed by its commit with what lies between
    f.write("docs/OTHER.md", "# Other\n");
    f.commit("another commit");
    let p3 = head(&f);
    let ancestor = artifact(&[record("ci:ancestor", &p2, "clean", "pass")]);
    for (run, want) in [
        ("concluded", "unknown"),
        ("pending", "pending"),
        ("absent", "unknown"),
    ] {
        let (p, _) = publish(&f, &["--from", &from(&ancestor), "--run", run]);
        assert_eq!(p["state"], want, "{run}: {p}");
        assert_eq!(p["presented"]["commit"], p3.as_str(), "{p}");
        let r = &p["records"][0];
        assert_eq!(
            (r["relation"].clone(), r["commit"].clone()),
            (json!("ancestor"), json!(p2)),
            "{p}"
        );
        assert_eq!(r["commits_between"], 1, "{p}");
        assert_eq!(r["changed"], json!(["docs/OTHER.md"]), "{p}");
    }
    // and a failing one can cap a verdict, which is all it can do
    let ancestor_failed = artifact(&[record("ci:ancestor-failed", &p2, "clean", "fail")]);
    let (p, _) = publish(
        &f,
        &["--from", &from(&ancestor_failed), "--run", "concluded"],
    );
    assert_eq!(p["state"], "stale", "{p}");
    assert_eq!(claim(&p)["capped_by"], json!(["ci:ancestor-failed"]), "{p}");

    // a record of another history is listed and says nothing about this one
    let elsewhere = artifact(&[record("ci:elsewhere", &"e".repeat(40), "clean", "fail")]);
    let (p, _) = publish(&f, &["--from", &from(&elsewhere), "--run", "pending"]);
    assert_eq!(p["state"], "pending", "{p}");
    assert_eq!(p["records"][0]["relation"], "unrelated", "{p}");
    assert_eq!(p["capped_by"], json!([]), "{p}");
}

/// The proof state of every claim of `evidence show --presented HEAD`, by its id.
fn committed_verdicts(f: &Fixture) -> Vec<(String, ProofState)> {
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "evidence",
            "show",
            "--presented",
            "HEAD",
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    v["claims"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_string(),
                serde_json::from_value(c["state"].clone()).unwrap(),
            )
        })
        .collect()
}

/// Each published verdict is the committed ledger's or weaker, by the declared order of the
/// states.
fn never_stronger(p: &Value, committed: &[(String, ProofState)]) {
    let published: Vec<(String, ProofState)> = p["claims"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_string(),
                serde_json::from_value(c["state"].clone()).unwrap(),
            )
        })
        .collect();
    assert_eq!(published.len(), committed.len(), "{p}");
    for ((id, at), (cid, base)) in published.iter().zip(committed) {
        assert_eq!(id, cid);
        assert!(
            at >= base,
            "{id} is published {at:?}, stronger than the committed {base:?}"
        );
    }
}

#[test]
fn publication_never_outranks_the_committed_ledger() {
    let (f, p2) = recorded();
    let committed = committed_verdicts(&f);
    let (p, _) = publish(&f, &["--run", "pending"]);
    never_stronger(&p, &committed);
    let failed = artifact(&[record("ci:failed", &p2, "clean", "fail")]);
    let (p, _) = publish(&f, &["--from", &from(&failed)]);
    never_stronger(&p, &committed);

    // a run the working ledger holds and the commit's does not cannot strengthen anything:
    // at C3 the committed ledger says inputs unchanged, and a pass recorded at C3 into the
    // working copy leaves the publication there
    f.write("docs/OTHER.md", "# Other\n");
    f.commit("another commit");
    let committed = committed_verdicts(&f);
    let base = committed
        .iter()
        .find(|(id, _)| id == "policy-parse")
        .unwrap()
        .1;
    assert_eq!(base, ProofState::InputsUnchanged);
    let reports = tempfile::tempdir().unwrap();
    let tsv = reports.path().join("run.tsv");
    std::fs::write(&tsv, "00_x\tok\t1\tparallel\n").unwrap();
    let tsv = tsv.display().to_string();
    let (code, _, err) = run_in(&f.root(), &["evidence", "record", "--suite", &tsv], "");
    assert_eq!(code, 0, "{err}");
    assert!(
        !f.git(&["status", "--porcelain"]).is_empty(),
        "the working ledger holds the uncommitted run"
    );
    let (p, _) = publish(&f, &["--run", "pending"]);
    never_stronger(&p, &committed);
    assert_eq!(claim(&p)["state"], "inputs_unchanged", "{p}");
    assert_eq!(
        p["state"], "pending",
        "the ledger's working copy does not dirty the build"
    );
    assert!(Path::new(&f.root())
        .join(".ai/repo/evidence/ledger.json")
        .is_file());
}
