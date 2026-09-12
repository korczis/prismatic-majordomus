//! The knowledge base surface, over a real repository with a real ledger and real git.
//!
//! // claim: knowledge-served-on-every-surface
//!
//! The behavioural claim these exist for is the one a reviewer of a candidate depends on
//! and cannot check alone: the candidates awaiting review, one record by id with its
//! references resolved, and the derivation status of the checkout are one declaration each
//! and answer with the same content over the command line, HTTP and MCP. Timestamps and
//! the ages derived from them are the only volatile fields and nothing here asserts on them.

mod common;

use common::{context_doc, run_in, Fixture, Served, POLICY, SOURCES};
use serde_json::{json, Value};

const ID: &str = "e1-0123456789ab";
const CANDIDATE_PATH: &str = ".ai/repo/knowledge/candidates/e1-0123456789ab.md";

/// The source classes of the fixture — which already declare the curated records — plus the
/// ones this surface reads: the candidates, the tracked session records, and the directory
/// contracts, in the order this repository's own `sources.yaml` declares them.
fn sources() -> String {
    format!(
        "{SOURCES}
  - id: context
    kind: context
    discovery: vcs
    pathspec: ':(glob).ai/**/README.md'
    required: false

  - id: session
    kind: session
    discovery: vcs
    pathspec: ':(glob).ai/repo/sessions/*.md'
    required: false

  - id: candidates
    kind: knowledge
    discovery: vcs
    pathspec: ':(glob).ai/repo/knowledge/candidates/*.md'
    required: false
"
    )
}

/// The fixture's policy with the two blocks this surface reads: the freshness thresholds
/// and the review-queue bounds. Declared once here as they are declared once there.
fn policy() -> String {
    format!(
        "{POLICY}
session:
  freshness:
    fresh_minutes: 720
    stale_minutes: 2880

knowledge:
  candidates_max_files: 40
  candidate_max_age_minutes: 20160
"
    )
}

/// One candidate as the deriver writes it: extracted, naming its episode, the task its
/// decision was recorded under, a `task:none` the resolver must refuse, the commit the
/// episode made, and a relation to a rule the fixture tracks.
fn candidate(head: &str) -> String {
    format!(
        "---
schema: knowledge/v1
id: {ID}
kind: knowledge
class: convention
title: \"Tests run in disposable repositories\"
description: \"Recorded as a decision under task t-1 in episode e1.\"
status: candidate
epistemics: decided
date: 2026-01-01
tags:
  - derived
  - decision
provenance:
  origin: extracted
  derived_from:
    - session:e1
    - decision:t-1
    - task:none
    - commit:{head}
relations:
  - type: relates_to
    target: file:.ai/repo/rules/project/alpha.v1.md
---

# Tests run in disposable repositories

Recorded as a decision under task t-1 in episode e1.
"
    )
}

/// The tracked record of the episode the candidate names, on the fixture's branch.
fn session_record(branch: &str) -> String {
    format!(
        "---
schema: session/v1
kind: session
session_id: e1
started_at: 2026-01-01T00:00:00Z
closed_at: 2026-01-01T01:00:00Z
outcome: closed
branch: {branch}
title: \"Session e1 on {branch}\"
---

# Session e1
"
    )
}

/// A ledger in which the deriver ran once, for an older episode, and the newest episode
/// closed long ago with no derivation following it: the stopped writer.
fn ledger(branch: &str) -> String {
    let line = |ts: &str, event: &str, extra: Value| {
        let mut v =
            json!({ "ts": ts, "event": event, "head": "h", "branch": branch, "by": "test" });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        v.to_string()
    };
    [
        line("2026-01-01T00:00:00Z", "session.started", json!({ "session": "e0", "owner": "t" })),
        line("2026-01-01T00:30:00Z", "task.started", json!({ "session": "e0", "task_id": "t-1" })),
        line(
            "2026-01-01T00:40:00Z",
            "knowledge.derived",
            json!({ "episode": "e0", "written": 1, "unchanged": 0, "skipped": 0, "paths": CANDIDATE_PATH }),
        ),
        line("2026-01-02T00:00:00Z", "session.closed", json!({ "session": "e1", "outcome": "closed" })),
    ]
    .join("\n")
        + "\n"
}

/// A committed repository holding one candidate, the session record it names, and the
/// ledger above.
fn repository() -> (Fixture, String, String) {
    let f = Fixture::new();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();
    f.write(".ai/repo/knowledge/sources.yaml", &sources());
    f.write(".ai/repo/policy.yaml", &policy());
    f.write(
        ".ai/repo/knowledge/README.md",
        &context_doc("ai.repo.knowledge", "Knowledge"),
    );
    f.write(
        ".ai/repo/knowledge/candidates/README.md",
        &context_doc("ai.repo.knowledge.candidates", "Candidate records"),
    );
    f.write(CANDIDATE_PATH, &candidate(&head));
    f.write(".ai/repo/sessions/e1.md", &session_record(&branch));
    f.commit("a candidate, its episode, and the rule it relates to");
    f.write(".ai/local/state/ledger.jsonl", &ledger(&branch));
    (f, head, branch)
}

/// The fields whose value depends on the clock, removed before two surfaces are compared.
fn stable(mut v: Value) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(m) => {
                m.remove("age_minutes");
                m.remove("freshness_reason");
                for (_, child) in m.iter_mut() {
                    strip(child);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut v);
    v
}

/// One MCP session over the served `/mcp`, enough to call a tool and read a resource.
struct Mcp<'a> {
    served: &'a Served,
    session: String,
    next: u64,
}

impl<'a> Mcp<'a> {
    fn open(served: &'a Served) -> Self {
        let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "knowledge-test", "version": "0" } } })
        .to_string();
        let (status, headers, body) = served.request("POST", "/mcp", Some(&init));
        assert_eq!(status, 200, "initialize: {body}");
        let session = headers
            .iter()
            .find(|(k, _)| k == "mcp-session-id")
            .map(|(_, v)| v.clone())
            .expect("initialize answers with a session id");
        Mcp {
            served,
            session,
            next: 2,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let body = json!({ "jsonrpc": "2.0", "id": self.next, "method": method, "params": params })
            .to_string();
        let (status, _, body) = self.served.request_with(
            "POST",
            "/mcp",
            Some(&body),
            &[("Mcp-Session-Id", &self.session)],
        );
        assert_eq!(status, 200, "{method}: {body}");
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v.get("error").is_none(), "{method} failed: {v}");
        v["result"].clone()
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let r = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        );
        assert_eq!(r["isError"], false, "{tool}: {r}");
        r["structuredContent"].clone()
    }

    fn read(&mut self, uri: &str) -> Value {
        let r = self.request("resources/read", json!({ "uri": uri }));
        let text = r["contents"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{uri}: no text content: {r}"));
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{uri}: not JSON ({e}): {text}"))
    }
}

#[test]
fn the_candidates_awaiting_review_are_listed_with_the_branch_of_their_episode() {
    let (f, _, branch) = repository();
    let mut s = Served::start(&f.root(), &[]);

    let (status, v) = s.get("/api/v1/knowledge/candidates");
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["total"], 1, "{v}");
    assert_eq!(v["on_this_branch"], 1);
    assert_eq!(v["unattributed"], 0);
    assert_eq!(v["branch"], branch);
    assert_eq!(v["cap"], 40, "the cap is the policy's, read from it");
    assert_eq!(v["over_cap"], false);
    let c = &v["candidates"][0];
    assert_eq!(c["id"], ID);
    assert_eq!(c["path"], CANDIDATE_PATH);
    assert_eq!(c["episode"], "e1");
    assert_eq!(
        c["branch"], branch,
        "the branch is the episode's, joined to the tracked session record"
    );
    assert_eq!(c["class"], "convention");
    assert_eq!(c["date"], "2026-01-01");
    assert_eq!(
        c["age_source"], "derivation",
        "the wait is measured from the derivation that wrote the file, not from its date: {c}"
    );
    assert_eq!(
        c["freshness"], "stale",
        "a candidate derived in January has waited past the stale threshold: {c}"
    );
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap().contains("waited longer")),
        "a candidate past knowledge.candidate_max_age_minutes is named: {v}"
    );
    s.stop();
}

#[test]
fn one_record_answers_with_every_reference_resolved_and_names_the_one_that_dangles() {
    let (f, head, _) = repository();
    let mut s = Served::start(&f.root(), &[]);

    let (status, v) = s.get(&format!("/api/v1/knowledge/record?id={ID}"));
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["id"], ID);
    assert_eq!(v["source_class"], "candidates");
    assert_eq!(v["status"], "candidate");
    assert_eq!(v["origin"], "extracted");
    assert!(v["content"]
        .as_str()
        .unwrap()
        .contains("schema: knowledge/v1"));

    let refs = v["derived_from"].as_array().unwrap();
    assert_eq!(refs.len(), 4, "{v}");
    assert_eq!(refs[0]["reference"], "session:e1");
    assert_eq!(
        refs[0]["resolution"], "object",
        "the episode has a tracked record: {}",
        refs[0]
    );
    assert_eq!(refs[0]["target"], "majordomus://session/e1");
    assert_eq!(refs[1]["reference"], "decision:t-1");
    assert_eq!(
        refs[1]["resolution"], "external",
        "the task is vouched for by the ledger: {}",
        refs[1]
    );
    assert_eq!(refs[1]["target"], "ledger");
    assert_eq!(refs[2]["reference"], "task:none");
    assert_eq!(
        refs[2]["resolution"], "missing",
        "`none` is not a task and is refused by name: {}",
        refs[2]
    );
    assert!(refs[2]["reason"].as_str().unwrap().contains("none"));
    assert_eq!(refs[3]["reference"], format!("commit:{head}"));
    assert_eq!(refs[3]["resolution"], "external", "{}", refs[3]);
    assert_eq!(refs[3]["target"], "git");

    let rel = &v["relations"][0];
    assert_eq!(rel["relation_type"], "relates_to");
    assert_eq!(rel["resolution"], "object", "{rel}");
    assert_eq!(rel["target"], "majordomus://rule/project.alpha@1");
    assert_eq!(
        v["resolved"], false,
        "one reference dangles, and the record says so"
    );

    // a record the repository does not hold is not found, not malformed
    let (status, v) = s.get("/api/v1/knowledge/record?id=nope");
    assert_eq!(status, 404, "{v}");
    s.stop();
}

#[test]
fn a_closed_episode_no_derivation_followed_is_a_stopped_writer_once_the_deriver_has_run() {
    let (f, _, branch) = repository();
    let mut s = Served::start(&f.root(), &[]);

    let (status, v) = s.get("/api/v1/knowledge/status");
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["present"], true);
    assert_eq!(v["branch"], branch);
    assert_eq!(v["last_derived"]["episode"], "e0");
    assert_eq!(v["last_closed"]["episode"], "e1");
    assert_eq!(v["closed_without_derivation"], 1);
    assert_eq!(v["candidates"], 1);
    assert_eq!(v["on_this_branch"], 1);
    assert_eq!(v["knowledge_on_end"], true);
    assert_eq!(v["knowledge_on_compact"], true);
    assert_eq!(v["judged"], true, "{v}");
    assert_eq!(
        v["stopped_writer"], true,
        "e1 closed in January and nothing derived it: {v}"
    );
    assert_eq!(v["freshness"], "stale");
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap().contains("--episode e1")),
        "the finding names the remedy: {v}"
    );
    s.stop();
}

#[test]
fn the_command_line_http_and_mcp_answer_with_the_same_content() {
    let (f, _, _) = repository();
    let root = f.root();
    let mut s = Served::start(&root, &[]);

    let (_, http_status) = s.get("/api/v1/knowledge/status");
    let (_, http_candidates) = s.get("/api/v1/knowledge/candidates");
    let (_, http_record) = s.get(&format!("/api/v1/knowledge/record?id={ID}"));

    // MCP: the tools and the resources, over the served /mcp
    let mut mcp = Mcp::open(&s);
    let tools = mcp.request("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for tool in [
        "majordomus_knowledge_candidates",
        "majordomus_knowledge_record",
        "majordomus_knowledge_status",
    ] {
        assert!(names.contains(&tool), "{tool} is not announced: {names:?}");
    }
    assert_eq!(
        stable(mcp.call("majordomus_knowledge_status", json!({}))),
        stable(http_status.clone()),
        "the MCP tool and the HTTP route answer differently"
    );
    assert_eq!(
        stable(mcp.call("majordomus_knowledge_candidates", json!({}))),
        stable(http_candidates.clone())
    );
    assert_eq!(
        stable(mcp.call("majordomus_knowledge_record", json!({ "id": ID }))),
        stable(http_record.clone())
    );
    assert_eq!(
        stable(mcp.read("majordomus://knowledge-status")),
        stable(http_status.clone()),
        "the resource is the same answer as a document"
    );
    assert_eq!(
        stable(mcp.read("majordomus://knowledge-candidates")),
        stable(http_candidates.clone())
    );
    s.stop();

    // the command line: the same value, printed
    let (code, out, err) = run_in(&root, &["knowledge", "status", "--format", "json"], "");
    assert_eq!(code, 0, "{err}");
    let cli: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"));
    assert_eq!(stable(cli), stable(http_status));

    let (code, out, err) = run_in(&root, &["knowledge", "candidates", "--format", "json"], "");
    assert_eq!(code, 0, "{err}");
    let cli: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(stable(cli), stable(http_candidates));

    let (code, out, err) = run_in(&root, &["knowledge", "record", ID, "--format", "json"], "");
    assert_eq!(code, 0, "{err}");
    let cli: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(stable(cli), stable(http_record));

    // and the text renderings carry the same facts
    let (code, out, _) = run_in(&root, &["knowledge", "status"], "");
    assert_eq!(code, 0);
    assert!(out.contains("STOPPED"), "{out}");
    assert!(out.contains("last closed  e1"), "{out}");
    let (code, out, _) = run_in(&root, &["knowledge", "candidates"], "");
    assert_eq!(code, 0);
    assert!(out.contains(ID), "{out}");
    let (code, out, _) = run_in(&root, &["knowledge", "record", ID], "");
    assert_eq!(code, 0);
    assert!(out.contains("task:none  missing"), "{out}");

    // a record that does not exist exits with the not-found code, never the usage code
    let (code, _, err) = run_in(&root, &["knowledge", "record", "nope"], "");
    assert_eq!(code, 12, "{err}");
    // and the group runs nothing of its own
    let (code, _, _) = run_in(&root, &["knowledge"], "");
    assert_eq!(code, 2);
}

#[test]
fn a_repository_with_no_candidates_and_no_ledger_answers_with_absence_not_an_error() {
    let f = Fixture::new();
    let mut s = Served::start(&f.root(), &[]);

    let (status, v) = s.get("/api/v1/knowledge/candidates");
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["total"], 0);
    assert_eq!(v["on_this_branch"], 0);
    assert!(
        v.get("cap").is_none(),
        "a policy without the key declares no cap: {v}"
    );
    assert_eq!(v["over_cap"], false);

    let (status, v) = s.get("/api/v1/knowledge/status");
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["present"], false, "{v}");
    assert_eq!(v["judged"], false);
    assert_eq!(v["stopped_writer"], false);
    assert!(v.get("last_closed").is_none());
    assert_eq!(v["freshness"], "unknown");
    assert!(
        v["findings"][0]
            .as_str()
            .unwrap()
            .contains("no episode has closed"),
        "{v}"
    );
    s.stop();
}

#[test]
fn a_close_before_any_derivation_is_withheld_rather_than_reported_as_stopped() {
    let f = Fixture::new();
    let branch = f
        .git(&["symbolic-ref", "--short", "HEAD"])
        .trim()
        .to_string();
    f.write(".ai/repo/policy.yaml", &policy());
    f.commit("thresholds");
    // the day the deriver arrives: episodes have closed here and none was ever derived
    f.write(
        ".ai/local/state/ledger.jsonl",
        &(json!({ "ts": "2026-01-02T00:00:00Z", "event": "session.closed", "head": "h",
                  "branch": branch, "by": "test", "session": "e1", "outcome": "closed" })
        .to_string()
            + "\n"),
    );
    let mut s = Served::start(&f.root(), &[]);
    let (status, v) = s.get("/api/v1/knowledge/status");
    assert_eq!(status, 200, "{v}");
    assert_eq!(v["present"], true);
    assert_eq!(v["last_closed"]["episode"], "e1");
    assert_eq!(
        v["closed_without_derivation"], 1,
        "counted even while not judged"
    );
    assert_eq!(v["judged"], false, "{v}");
    assert_eq!(v["stopped_writer"], false, "{v}");
    assert!(
        v["findings"][0]
            .as_str()
            .unwrap()
            .contains("no knowledge derivation has run in this checkout yet"),
        "{v}"
    );
    s.stop();
}
