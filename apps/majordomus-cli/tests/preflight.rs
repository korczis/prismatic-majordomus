//! `environment::preflight`: every verdict rests on evidence, every surface says the same.
//!
//! Two halves. The first holds [`derive`] to the semantics `project.entry-reports-only-evidence`
//! states — a rule on disk is not an enforced rule, a configured server that does not answer
//! is not a healthy one, a run recorded at another commit does not verify this tree — over
//! observations built by hand, so that each verdict is decided by a value rather than by a
//! machine. The second takes one fixture repository and reads the preflight from the command
//! line, the HTTP route, the MCP tool and resource, and the Cockpit page, and holds them to
//! one set of verdicts.

mod common;

use std::collections::BTreeMap;

use common::{run_in, Fixture, Served};
use majordomus_cli::capability::builtin::continuity::Freshness;
use majordomus_cli::capability::builtin::server::ServerStanding;
use majordomus_cli::environment::preflight::{
    derive, observe, observe_coverage, Check, CoverageObservation, DeploymentObservation,
    EpisodeObservation, GenerationObservation, GenerationRecord, GitObservation,
    HandoverObservation, LedgerObservation, Observations, Preflight, Probe, RulesObservation,
    RulesTally, ServerObservation, TaskObservation, Verdict, COVERAGE_RECORD, GENERATION_RECORD,
    PAGES_REF,
};
use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs};
use majordomus_cli::Repository;
use serde_json::{json, Value};

const HEAD: &str = "1111111111111111111111111111111111111111";
const OLD: &str = "2222222222222222222222222222222222222222";

fn at(head: &str) -> Observations {
    let mut o = Observations::empty("fixture", 0);
    o.git = Some(GitObservation {
        branch: Some("master".into()),
        head: Some(head.into()),
        clean: true,
        changed: 0,
    });
    o
}

fn verdict(o: &Observations, id: &str) -> Verdict {
    derive(o)
        .check(id)
        .unwrap_or_else(|| panic!("no check {id}"))
        .verdict
}

// ---------------------------------------------------------------- semantics

#[test]
fn a_verdict_of_force_without_evidence_is_not_one() {
    for claimed in [Verdict::Verified, Verdict::Active, Verdict::Fresh] {
        let c = Check::new("x.y", "y", claimed, "said so", vec![]);
        assert_eq!(c.verdict, Verdict::Unknown, "{claimed:?} was accepted bare");
    }
    let every = derive(&at(HEAD));
    for c in every.sections.iter().flat_map(|s| &s.checks) {
        assert!(
            !c.verdict.proves() || !c.evidence.is_empty(),
            "{} proves with no evidence",
            c.id
        );
    }
}

#[test]
fn rules_discovered_are_not_rules_enforced() {
    let mut o = at(HEAD);
    // 40 rules on disk, all gated by CI, none with a recorded verdict against this tree
    let mut states = BTreeMap::new();
    states.insert("gated".to_string(), 40);
    o.rules = RulesObservation::Counted(RulesTally {
        head: Some(HEAD.into()),
        working_tree: "clean".into(),
        rules: 40,
        blocking: 40,
        project: 40,
        gated: 40,
        states,
        ..Default::default()
    });
    assert_eq!(
        verdict(&o, "governance.rules"),
        Verdict::Active,
        "they are loaded"
    );
    assert_eq!(
        verdict(&o, "verification.enforcement"),
        Verdict::Degraded,
        "a gate that exists is not a verdict that passed"
    );
}

#[test]
fn enforcement_says_nothing_about_this_tree_from_another_or_from_nothing() {
    let tally = |head: &str, working_tree: &str| RulesTally {
        head: Some(head.into()),
        working_tree: working_tree.into(),
        rules: 3,
        proven: 2,
        failing: 1,
        ..Default::default()
    };
    // a failure counted at another commit is not a failure of this one
    let mut o = at(HEAD);
    o.rules = RulesObservation::Cached(tally(OLD, "clean"));
    let p = derive(&o);
    let c = p.check("verification.enforcement").unwrap();
    assert_eq!(c.verdict, Verdict::Stale, "{}", c.summary);
    assert!(c.summary.contains("were failing there"), "{}", c.summary);

    // counted over a dirty tree: whatever it found, it is not this tree's verdict
    o.rules = RulesObservation::Counted(RulesTally {
        failing: 0,
        proven: 3,
        ..tally(HEAD, "dirty")
    });
    assert_eq!(verdict(&o, "verification.enforcement"), Verdict::Stale);

    // nothing owes a proof: zero of zero is not verified
    o.rules = RulesObservation::Counted(RulesTally {
        head: Some(HEAD.into()),
        working_tree: "clean".into(),
        rules: 2,
        reviewed: 1,
        unproven: 1,
        ..Default::default()
    });
    assert_eq!(
        verdict(&o, "verification.enforcement"),
        Verdict::NotApplicable
    );
}

#[test]
fn a_briefing_over_a_dirty_tree_cannot_be_judged_fresh() {
    let mut o = at(HEAD);
    o.git.as_mut().unwrap().clean = false;
    o.episode = Some(EpisodeObservation {
        start_working_tree: "dirty".into(),
        ..episode(HEAD)
    });
    assert_eq!(verdict(&o, "session.context"), Verdict::Unknown);
    // and a move of HEAD is still reported as what it is
    o.episode = Some(EpisodeObservation {
        start_working_tree: "dirty".into(),
        ..episode(OLD)
    });
    assert_eq!(verdict(&o, "session.context"), Verdict::Stale);
}

#[test]
fn an_abbreviated_deployment_source_names_head_by_prefix() {
    let mut o = at(HEAD);
    o.deployment = DeploymentObservation::Published {
        commit: "d".repeat(40),
        at: "2026-09-15T15:24:31Z".into(),
        source: Some(HEAD[..12].into()),
    };
    assert_eq!(verdict(&o, "verification.deployment"), Verdict::Verified);
    o.deployment = DeploymentObservation::Published {
        commit: "d".repeat(40),
        at: "2026-09-15T15:24:31Z".into(),
        source: Some(OLD[..12].into()),
    };
    assert_eq!(verdict(&o, "verification.deployment"), Verdict::Stale);
}

#[test]
fn enforcement_is_verified_only_by_proof_at_this_tree() {
    let proven = |head: &str| RulesTally {
        head: Some(head.into()),
        working_tree: "clean".into(),
        rules: 3,
        proven: 3,
        ..Default::default()
    };
    let mut o = at(HEAD);
    o.rules = RulesObservation::Counted(proven(HEAD));
    assert_eq!(verdict(&o, "verification.enforcement"), Verdict::Verified);

    o.rules = RulesObservation::Cached(proven(OLD));
    assert_eq!(verdict(&o, "verification.enforcement"), Verdict::Stale);
    assert_eq!(verdict(&o, "governance.rules"), Verdict::Stale);

    o.rules = RulesObservation::Counted(RulesTally {
        failing: 1,
        proven: 2,
        ..proven(HEAD)
    });
    assert_eq!(verdict(&o, "verification.enforcement"), Verdict::Failed);

    o.rules = RulesObservation::Absent;
    assert_eq!(verdict(&o, "verification.enforcement"), Verdict::Unknown);
}

fn episode(start_head: &str) -> EpisodeObservation {
    EpisodeObservation {
        id: "s-1".into(),
        started_at: "2026-09-15T10:00:00Z".into(),
        provider: "claude-code".into(),
        start_head: start_head.into(),
        start_working_tree: "clean".into(),
        this_checkout: true,
    }
}

#[test]
fn a_briefing_is_fresh_until_git_or_the_task_moves() {
    let mut o = at(HEAD);
    o.episode = Some(episode(HEAD));
    assert_eq!(verdict(&o, "session.context"), Verdict::Fresh);

    // HEAD moved
    o.episode = Some(episode(OLD));
    assert_eq!(verdict(&o, "session.context"), Verdict::Stale);

    // the tree changed under it
    o.episode = Some(episode(HEAD));
    o.git.as_mut().unwrap().clean = false;
    assert_eq!(verdict(&o, "session.context"), Verdict::Stale);

    // a task began after it
    o.git.as_mut().unwrap().clean = true;
    o.task = Some(TaskObservation {
        id: "t-2".into(),
        task: "later".into(),
        outcome: String::new(),
        started_at: "2026-09-15T11:00:00Z".into(),
    });
    let p = derive(&o);
    let c = p.check("session.context").unwrap();
    assert_eq!(c.verdict, Verdict::Stale);
    assert!(c.summary.contains("t-2"), "{}", c.summary);
}

#[test]
fn a_stale_handover_is_history() {
    let mut o = at(HEAD);
    o.handover = Some(HandoverObservation {
        path: ".ai/local/state/handovers/h.md".into(),
        freshness: Freshness::Stale,
        reason: "10d old".into(),
        divergence: "advanced".into(),
    });
    assert_eq!(verdict(&o, "session.handover"), Verdict::Stale);
}

fn surfaces() -> Vec<(String, String, bool)> {
    [("mcp", "/mcp"), ("api", "/api/v1"), ("cockpit", "/cockpit")]
        .into_iter()
        .map(|(i, p)| (i.to_string(), p.to_string(), true))
        .collect()
}

#[test]
fn a_configured_server_that_does_not_answer_is_not_healthy() {
    let mut o = at(HEAD);
    // a lease names an address, and nothing answers there
    o.server = ServerObservation {
        standing: Some(ServerStanding::Stale),
        reason: Some("the server the lease names does not answer".into()),
        url: Some("http://127.0.0.1:1".into()),
        pid: Some(1),
        ..Default::default()
    };
    assert_eq!(verdict(&o, "integration.server"), Verdict::Failed);
    assert_eq!(verdict(&o, "integration.mcp"), Verdict::Unavailable);
    assert_eq!(verdict(&o, "integration.cockpit"), Verdict::Unavailable);

    // no lease at all: stopped
    o.server = ServerObservation {
        standing: Some(ServerStanding::Absent),
        ..Default::default()
    };
    assert_eq!(verdict(&o, "integration.cockpit"), Verdict::Unavailable);
    assert_eq!(verdict(&o, "integration.peers"), Verdict::Unavailable);

    // answering, from other code: the connection is real and the mark is not a success
    o.server = ServerObservation {
        standing: Some(ServerStanding::Outdated),
        reason: Some("the server is serving version 0.6.1".into()),
        url: Some("http://127.0.0.1:8742".into()),
        surfaces: surfaces(),
        ..Default::default()
    };
    assert_eq!(verdict(&o, "integration.server"), Verdict::Degraded);
    assert_eq!(verdict(&o, "integration.mcp"), Verdict::Degraded);

    // answering, this version, surface listed not ready
    let mut listed = surfaces();
    listed[0].2 = false;
    o.server = ServerObservation {
        standing: Some(ServerStanding::Ready),
        url: Some("http://127.0.0.1:8742".into()),
        surfaces: listed,
        ..Default::default()
    };
    assert_eq!(verdict(&o, "integration.mcp"), Verdict::Failed);
    assert_eq!(verdict(&o, "integration.cockpit"), Verdict::Verified);
}

#[test]
fn test_evidence_for_another_commit_is_stale_and_for_this_one_verified() {
    let mut o = at(HEAD);
    o.ledger = LedgerObservation::Read {
        total: 6,
        current: 0,
        current_failing: 0,
        newest_commit: OLD.into(),
        newest_at: "2026-09-12T10:36:18Z".into(),
    };
    assert_eq!(verdict(&o, "verification.tests"), Verdict::Stale);
    o.ledger = LedgerObservation::Read {
        total: 6,
        current: 6,
        current_failing: 0,
        newest_commit: HEAD.into(),
        newest_at: "2026-09-15T10:00:00Z".into(),
    };
    assert_eq!(verdict(&o, "verification.tests"), Verdict::Verified);
    o.ledger = LedgerObservation::Read {
        total: 6,
        current: 6,
        current_failing: 1,
        newest_commit: HEAD.into(),
        newest_at: "2026-09-15T10:00:00Z".into(),
    };
    assert_eq!(verdict(&o, "verification.tests"), Verdict::Failed);
    o.ledger = LedgerObservation::Absent;
    assert_eq!(verdict(&o, "verification.tests"), Verdict::Unavailable);
}

#[test]
fn a_deployment_of_another_commit_is_stale_and_of_this_one_verified() {
    let mut o = at(HEAD);
    o.deployment = DeploymentObservation::Published {
        commit: "d".repeat(40),
        at: "2026-09-15T15:24:31Z".into(),
        source: Some(OLD.into()),
    };
    assert_eq!(verdict(&o, "verification.deployment"), Verdict::Stale);
    o.deployment = DeploymentObservation::Published {
        commit: "d".repeat(40),
        at: "2026-09-15T15:24:31Z".into(),
        source: Some(HEAD.into()),
    };
    assert_eq!(verdict(&o, "verification.deployment"), Verdict::Verified);
    o.deployment = DeploymentObservation::NoRef;
    assert_eq!(verdict(&o, "verification.deployment"), Verdict::Unavailable);
}

/// A record as `scripts/rust-coverage` writes it, at `commit`, on a tree it calls `tree`.
fn coverage_record(commit: &str, tree: &str, outcome: &str, suite_exit: i32) -> String {
    json!({
        "schema": 1,
        "measurement": "scripts/rust-coverage",
        "commit": commit,
        "working_tree": tree,
        "at": "2026-09-29T20:00:00Z",
        "mode": "gate",
        "outcome": outcome,
        "suite_exit": suite_exit,
        "crate": {
            "lines": {"covered": 90, "total": 100, "percent": 90.0},
            "functions": {"covered": 8, "total": 10, "percent": 80.0},
            "regions": {"covered": 7, "total": 10, "percent": 70.0}
        },
        "domain": {
            "lines": {"covered": 1, "total": 1, "percent": 100.0},
            "functions": {"covered": 1, "total": 1, "percent": 100.0},
            "regions": {"covered": 1, "total": 1, "percent": 100.0}
        }
    })
    .to_string()
}

/// The verdict of the coverage check over a record written to a real directory and read back
/// by the same function `observe` calls.
fn coverage_verdict(head: &str, record: Option<&str>) -> (Verdict, CoverageObservation) {
    let dir = std::env::temp_dir().join(format!(
        "mj-preflight-coverage-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = dir.join(COVERAGE_RECORD);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if let Some(text) = record {
        std::fs::write(&path, text).unwrap();
    }
    let mut o = at(head);
    o.coverage = observe_coverage(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    (verdict(&o, "verification.coverage"), o.coverage)
}

#[test]
fn coverage_is_judged_by_the_record_the_measurement_left() {
    let (v, c) = coverage_verdict(HEAD, None);
    assert_eq!((v, c), (Verdict::Unavailable, CoverageObservation::Absent));
    let pass = coverage_record(HEAD, "clean", "pass", 0);
    assert_eq!(coverage_verdict(HEAD, Some(&pass)).0, Verdict::Verified);
    // another commit, or a tree that moved during the run, proves nothing about this one
    assert_eq!(coverage_verdict(OLD, Some(&pass)).0, Verdict::Stale);
    let dirty = coverage_record(HEAD, "dirty", "pass", 0);
    assert_eq!(coverage_verdict(HEAD, Some(&dirty)).0, Verdict::Stale);
    let fail = coverage_record(HEAD, "clean", "fail", 0);
    assert_eq!(coverage_verdict(HEAD, Some(&fail)).0, Verdict::Failed);
    // thresholds that hold over a suite that failed are not a verified measurement
    let broken = coverage_record(HEAD, "clean", "pass", 101);
    assert_eq!(coverage_verdict(HEAD, Some(&broken)).0, Verdict::Failed);
    let report = coverage_record(HEAD, "clean", "report", 0);
    assert_eq!(coverage_verdict(HEAD, Some(&report)).0, Verdict::Active);
    let (v, c) = coverage_verdict(HEAD, Some("{\"schema\": 2}"));
    assert_eq!(v, Verdict::Unknown);
    assert!(matches!(c, CoverageObservation::Unreadable(_)));
}

#[test]
fn a_verified_coverage_check_names_its_record() {
    let mut o = at(HEAD);
    o.coverage = CoverageObservation::Read(
        serde_json::from_str(&coverage_record(HEAD, "clean", "pass", 0)).unwrap(),
    );
    let p = derive(&o);
    let c = p.check("verification.coverage").unwrap();
    assert_eq!(c.verdict, Verdict::Verified);
    assert!(c.summary.contains("lines 90.00% (90/100)"), "{}", c.summary);
    assert!(
        c.evidence.iter().any(|e| e.source == COVERAGE_RECORD),
        "{c:?}"
    );
}

const TREE: &str = "3333333333333333333333333333333333333333";

/// The generation check's verdict when the record says `outcome` about `tree`, HEAD's tree is
/// `head_tree`, and the working tree is `clean`.
fn generation_verdict(outcome: &str, tree: &str, head_tree: Option<&str>, clean: bool) -> Check {
    let mut o = at(HEAD);
    o.git.as_mut().unwrap().clean = clean;
    o.generation = GenerationObservation::Read {
        record: GenerationRecord {
            schema: 1,
            commit: OLD.into(),
            tree: tree.into(),
            at: "2026-09-30T00:00:00Z".into(),
            outcome: outcome.into(),
        },
        head_tree: head_tree.map(str::to_string),
    };
    derive(&o).check("verification.docs").unwrap().clone()
}

#[test]
fn generated_docs_are_judged_by_the_tree_the_check_measured() {
    // the hook checks the index before the commit exists: the record names the parent commit,
    // and it is the tree that must match
    let c = generation_verdict("pass", TREE, Some(TREE), true);
    assert_eq!(c.verdict, Verdict::Verified, "{}", c.summary);
    assert!(
        c.evidence.iter().any(|e| e.source == GENERATION_RECORD),
        "{c:?}"
    );
    assert_eq!(
        generation_verdict("fail", TREE, Some(TREE), true).verdict,
        Verdict::Failed
    );
    // the registry half skipped is a check that did not finish
    assert_eq!(
        generation_verdict("partial", TREE, Some(TREE), true).verdict,
        Verdict::Unknown
    );
    // another tree, a working tree that was not its index, or edits since: nothing about HEAD
    assert_eq!(
        generation_verdict("pass", OLD, Some(TREE), true).verdict,
        Verdict::Stale
    );
    assert_eq!(
        generation_verdict("pass", "", Some(TREE), true).verdict,
        Verdict::Stale
    );
    assert_eq!(
        generation_verdict("pass", TREE, Some(TREE), false).verdict,
        Verdict::Stale
    );
    assert_eq!(
        generation_verdict("pass", TREE, None, true).verdict,
        Verdict::Stale
    );
    let mut o = at(HEAD);
    o.generation = GenerationObservation::Unreadable("schema 2".into());
    assert_eq!(verdict(&o, "verification.docs"), Verdict::Unknown);
}

#[test]
fn coverage_and_generated_docs_are_never_claimed_on_nothing() {
    let p = derive(&at(HEAD));
    assert_eq!(
        p.check("verification.coverage").unwrap().verdict,
        Verdict::Unavailable
    );
    assert_eq!(
        p.check("verification.docs").unwrap().verdict,
        Verdict::Unknown
    );
}

// ---------------------------------------------------------------- observations of a real tree

/// A fixture with a deployment ref and a ledger, both about commits the fixture really has.
fn fixture_with_evidence() -> (Fixture, String) {
    let f = Fixture::new();
    let old = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    // the deployment commit names the source it was built from, the way scripts/pages writes it
    // an empty tree: a deployment commit's content is not what this reads
    let empty_tree = f.git(&["mktree"]).trim().to_string();
    let deploy = |source: &str| {
        f.git(&[
            "commit-tree",
            &empty_tree,
            "-m",
            &format!("deploy: site from {}\n\nsource: {source}", &source[..7]),
        ])
        .trim()
        .to_string()
    };
    let d = deploy(&old);
    f.git(&["update-ref", PAGES_REF, &d]);
    (f, old)
}

fn observed(f: &Fixture) -> Preflight {
    let repository = Repository::open(&f.root()).expect("the fixture opens");
    let environment = resolve(
        &Inputs {
            repository: &repository,
            share: None,
            index: None,
            registry: None,
            policy: None,
        },
        &EnvironmentQuery::fast().sealed(),
    );
    derive(&observe(
        &f.root(),
        &environment,
        Err(String::new()),
        None,
        Probe::sealed(),
    ))
}

#[test]
fn the_deployment_ref_is_read_and_judged_against_head() {
    let (f, deployed) = fixture_with_evidence();
    assert_eq!(
        observed(&f)
            .check("verification.deployment")
            .unwrap()
            .verdict,
        Verdict::Verified,
        "deployed from the commit HEAD names"
    );
    f.write("more.md", "# more\n");
    f.commit("later");
    let p = observed(&f);
    let c = p.check("verification.deployment").unwrap();
    assert_eq!(c.verdict, Verdict::Stale, "{c:?}");
    assert!(c.summary.contains(&deployed[..12]), "{}", c.summary);
}

#[test]
fn the_generation_record_is_read_against_heads_tree() {
    let (f, parent) = fixture_with_evidence();
    // what the pre-commit hook records: the parent commit, and the tree the commit then has
    let record = |tree: &str, outcome: &str| {
        f.write(
            GENERATION_RECORD,
            &json!({ "schema": 1, "check": "scripts/pages current", "commit": parent,
                     "tree": tree, "at": "2026-09-30T10:00:00Z", "outcome": outcome })
            .to_string(),
        );
    };
    let head_tree = f.git(&["rev-parse", "HEAD^{tree}"]).trim().to_string();
    record(&head_tree, "pass");
    let p = observed(&f);
    let c = p.check("verification.docs").unwrap();
    assert_eq!(c.verdict, Verdict::Verified, "{c:?}");
    // a later commit has another tree, and the record says nothing about it
    f.write("more.md", "# more\n");
    f.commit("later");
    assert_eq!(
        observed(&f).check("verification.docs").unwrap().verdict,
        Verdict::Stale
    );
    let head_tree = f.git(&["rev-parse", "HEAD^{tree}"]).trim().to_string();
    record(&head_tree, "fail");
    assert_eq!(
        observed(&f).check("verification.docs").unwrap().verdict,
        Verdict::Failed
    );
}

#[test]
fn a_ledger_run_verifies_only_the_tree_it_measured() {
    let (f, old) = fixture_with_evidence();
    let source = "echo ok\n";
    f.write("test/cases/01_ok.sh", source);
    f.commit("a case");
    let measured = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let ledger = |commit: &str, outcome: &str| {
        json!({ "version": 1, "executions": [{
            "test": "suite:01_ok", "runner": "suite", "source": "test/cases/01_ok.sh",
            "outcome": outcome, "seconds": 1, "commit": commit, "working_tree": "clean",
            "digest": majordomus_cli::evidence::digest_of(source.as_bytes()),
            "at": "2026-09-15T10:00:00Z", "origin": "local",
            "command": "bash test/run.sh 01_ok"
        }]})
        .to_string()
    };
    let tests = |f: &Fixture| observed(f).check("verification.tests").unwrap().verdict;

    // recorded at a commit that did not have the case: another tree
    f.write(".ai/repo/evidence/ledger.json", &ledger(&old, "pass"));
    assert_eq!(tests(&f), Verdict::Stale);

    // recorded at the commit the tree is: recording dirtied only the ledger itself
    f.write(".ai/repo/evidence/ledger.json", &ledger(&measured, "pass"));
    assert_eq!(tests(&f), Verdict::Verified);

    // committing the record moves HEAD past the commit it names, and changes nothing measured
    f.commit("the record");
    assert_eq!(tests(&f), Verdict::Verified);

    // the same run, failing
    f.write(".ai/repo/evidence/ledger.json", &ledger(&measured, "fail"));
    assert_eq!(tests(&f), Verdict::Failed);

    // a passing run, and then the tree changes under it
    f.write(".ai/repo/evidence/ledger.json", &ledger(&measured, "pass"));
    f.write("lib/a.sh", "echo changed\n");
    assert_eq!(tests(&f), Verdict::Stale);
}

// ---------------------------------------------------------------- one value, every surface

fn mcp_call(served: &Served, method: &str, params: Value) -> Value {
    let (status, headers, body) = served.request(
        "POST",
        "/mcp",
        Some(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
                "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": {"name": "preflight-test", "version": "0"}}})
            .to_string(),
        ),
    );
    assert_eq!(status, 200, "{body}");
    let session = headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.clone())
        .expect("a session id");
    let (status, _, body) = served.request_with(
        "POST",
        "/mcp",
        Some(&json!({"jsonrpc": "2.0", "id": 2, "method": method, "params": params}).to_string()),
        &[("Mcp-Session-Id", session.as_str())],
    );
    assert_eq!(status, 200, "{body}");
    serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}"))
}

fn verdicts_of(doc: &Value) -> BTreeMap<String, String> {
    doc["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .flat_map(|s| s["checks"].as_array().cloned().unwrap_or_default())
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_string(),
                c["verdict"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn the_command_line_the_api_mcp_and_the_cockpit_agree() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let (code, out, err) = run_in(
        &f.root(),
        &["env", "preflight", "--full", "--format", "json"],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let cli: Value = serde_json::from_str(&out).expect("the command line answers JSON");
    assert_eq!(cli["schema"], "majordomus/preflight/v1");

    let (status, api) = served.get("/api/v1/environment/preflight");
    assert_eq!(status, 200, "{api}");

    let tool = mcp_call(
        &served,
        "tools/call",
        json!({"name": "majordomus_preflight", "arguments": {}}),
    );
    let tool: Value = serde_json::from_str(
        tool["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("a text result: {tool}")),
    )
    .expect("the tool's text is the document");

    let resource = mcp_call(
        &served,
        "resources/read",
        json!({"uri": "majordomus://environment/preflight"}),
    );
    let resource: Value = serde_json::from_str(
        resource["result"]["contents"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("a text resource: {resource}")),
    )
    .expect("the resource's text is the document");

    let (status, _, html) = served.request("GET", "/cockpit", None);
    assert_eq!(status, 200);
    let cockpit: BTreeMap<String, String> = html
        .split("data-check=\"")
        .skip(1)
        .map(|rest| {
            let id = rest.split('"').next().unwrap().to_string();
            let verdict = rest
                .split("data-verdict=\"")
                .nth(1)
                .and_then(|r| r.split('"').next())
                .unwrap()
                .to_string();
            (id, verdict)
        })
        .collect();

    let reference = verdicts_of(&api);
    assert!(!reference.is_empty());
    for (surface, got) in [
        ("MCP tool", verdicts_of(&tool)),
        ("MCP resource", verdicts_of(&resource)),
        ("Cockpit", cockpit),
        ("command line", verdicts_of(&cli)),
    ] {
        assert_eq!(
            got.keys().collect::<Vec<_>>(),
            reference.keys().collect::<Vec<_>>(),
            "{surface} renders a different set of checks"
        );
        for (id, v) in &reference {
            assert_eq!(&got[id], v, "{surface} disagrees with the API about {id}");
        }
    }
    // and the fixture's server is really this checkout's, on every surface that says so
    assert_eq!(reference["integration.server"], "verified");
    assert_eq!(reference["integration.cockpit"], "verified");
    assert_eq!(reference["integration.peers"], "active");
}
