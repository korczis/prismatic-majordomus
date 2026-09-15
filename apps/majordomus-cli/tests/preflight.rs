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
    derive, observe, AdrRelation, AdrRelevance, AdrRelevanceObservation, Check, DeploymentObservation, EpisodeObservation, GitObservation,
    HandoverObservation, LedgerObservation, Observations, Preflight, Probe, RulesObservation,
    RulesTally, ServerObservation, TaskObservation, Verdict, PAGES_REF,
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
        ..Default::default()
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

// ---------------------------------------------------------------- decisions and the task

fn in_progress(id: &str, scope: &[&str]) -> TaskObservation {
    TaskObservation {
        id: id.into(),
        task: "the work".into(),
        outcome: "active".into(),
        scope: scope.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn joined(task: &str, scope: &[&str], head: &str) -> AdrRelevance {
    AdrRelevance {
        task: task.into(),
        scope: scope.iter().map(|s| s.to_string()).collect(),
        head: Some(head.into()),
        relations: vec![
            AdrRelation {
                adr: "adr-0007".into(),
                status: Some("accepted".into()),
                reference: "file:lib/session.sh".into(),
                scope: "lib".into(),
            },
            AdrRelation {
                adr: "adr-0009".into(),
                status: None,
                reference: "test:lib/session_test.sh".into(),
                scope: "lib".into(),
            },
        ],
    }
}

#[test]
fn a_decision_is_relevant_only_by_a_declared_relation_to_the_task_in_progress() {
    let mut o = at(HEAD);
    o.adrs = Some(12);

    // no task: counted, and nothing claimed relevant
    let p = derive(&o);
    let c = p.check("governance.adrs").unwrap();
    assert_eq!(c.verdict, Verdict::Active);
    assert!(c.summary.contains("no task is in progress"), "{}", c.summary);

    // a task in progress and no join: relevance is unknown, not zero
    o.task = Some(in_progress("t-1", &["lib"]));
    assert_eq!(verdict(&o, "governance.adrs"), Verdict::Unknown);

    // a join for this task, its scope and HEAD: each relation is a piece of evidence
    o.adr_relevance = AdrRelevanceObservation::Joined(joined("t-1", &["lib"], HEAD));
    let p = derive(&o);
    let c = p.check("governance.adrs").unwrap();
    assert_eq!(c.verdict, Verdict::Active);
    assert!(
        c.summary
            .starts_with("12 indexed · 2 relevant to task t-1: adr-0007, adr-0009"),
        "{}",
        c.summary
    );
    assert!(c
        .evidence
        .iter()
        .any(|e| e.observed == "adr-0007 (accepted) → file lib/session.sh → scope lib"));

    // the same join taken from the cache at another commit is stale
    o.adr_relevance = AdrRelevanceObservation::Cached(joined("t-1", &["lib"], OLD));
    assert_eq!(verdict(&o, "governance.adrs"), Verdict::Stale);

    // a join for another task, or for a scope the task no longer declares, says nothing
    o.adr_relevance = AdrRelevanceObservation::Cached(joined("t-0", &["lib"], HEAD));
    assert_eq!(verdict(&o, "governance.adrs"), Verdict::Unknown);
    o.adr_relevance = AdrRelevanceObservation::Cached(joined("t-1", &["lib"], HEAD));
    o.task = Some(in_progress("t-1", &["lib", "docs"]));
    assert_eq!(verdict(&o, "governance.adrs"), Verdict::Unknown);

    // and a task that ended is not a task anything is relevant to
    o.task = Some(TaskObservation {
        outcome: "completed".into(),
        ..in_progress("t-1", &["lib"])
    });
    let p = derive(&o);
    assert!(p
        .check("governance.adrs")
        .unwrap()
        .summary
        .contains("no task is in progress"));
}

#[test]
fn the_full_preflight_joins_decisions_to_the_task_and_entry_reads_the_join() {
    let f = Fixture::new();
    let adr = |n: &str, status: &str, extra: &str| {
        format!(
            "---\nschema: adr/v1\nid: adr-{n}\nkind: adr\ntitle: Decision {n}\nstatus: {status}\n\
             date: 2026-09-15\ntags:\n  - entry\n{extra}---\n\n## Context\n\nA reason.\n\n\
             ## Decision\n\nDecided.\n\n## Consequences\n\nFollows.\n"
        )
    };
    // one names a file inside the scope, one a file outside it, one is superseded by the first
    f.write(
        ".ai/repo/adrs/0001-inside.md",
        &adr(
            "0001",
            "accepted",
            "supersedes:\n  - adr-0003\nrelated:\n  - file:lib/a.sh\n",
        ),
    );
    f.write(
        ".ai/repo/adrs/0002-outside.md",
        &adr("0002", "accepted", "related:\n  - file:docs/elsewhere.md\n"),
    );
    f.write(
        ".ai/repo/adrs/0003-superseded.md",
        &adr("0003", "accepted", "related:\n  - file:lib/a.sh\n"),
    );
    f.commit("decisions");
    f.write(
        ".ai/local/state/current.yaml",
        "id: t-9\ntask: \"Change lib\"\nscope:\n  - lib\nstarted_at: 2026-09-15T00:00:00Z\n\
         outcome: active\n",
    );

    let adrs = |args: &[&str]| -> Value {
        let (code, out, err) = run_in(&f.root(), args, "");
        assert_eq!(code, 0, "{err}");
        let doc: Value = serde_json::from_str(&out).expect("JSON");
        doc["sections"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|s| s["checks"].as_array().unwrap().clone())
            .find(|c| c["id"] == "governance.adrs")
            .expect("the ADR check")
    };

    let full = adrs(&["env", "preflight", "--full", "--format", "json"]);
    assert_eq!(full["verdict"], "active", "{full}");
    let summary = full["summary"].as_str().unwrap();
    assert!(
        summary.ends_with("· 1 relevant to task t-9: adr-0001"),
        "{summary}"
    );
    assert!(
        full["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["observed"] == "adr-0001 (accepted) → file lib/a.sh → scope lib"),
        "{full}"
    );

    // entry never builds the index; it reads the join the full preflight left, at this commit
    let fast = adrs(&["env", "preflight", "--format", "json"]);
    assert_eq!(fast["verdict"], "active", "{fast}");
    assert!(fast["summary"].as_str().unwrap().contains("adr-0001"));

    // and once HEAD moves the join is about another commit
    f.write("lib/b.sh", "echo b\n");
    f.commit("more");
    assert_eq!(
        adrs(&["env", "preflight", "--format", "json"])["verdict"],
        "stale"
    );
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
