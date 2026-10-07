//! Intent realization through the built executable: one task carried by two providers across a
//! handover realises one intent, with every link's provenance, and the command line, HTTP and MCP
//! answer the same join. The ledger lines are the ones the shell lifecycle writes; case 388 drives
//! the lifecycle itself.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{run_in, Fixture, Served, BIN};
use majordomus_cli::evidence::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
use serde_json::{json, Value};

fn tool(f: &Fixture, name: &str, arguments: Value) -> Value {
    let mut child = Command::new(BIN)
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": arguments } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let last = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .last()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .unwrap();
    assert_eq!(last["result"]["isError"], false, "{name}: {last}");
    last["result"]["structuredContent"].clone()
}

fn cli_json(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

/// The lines the lifecycle appends when a Claude Code episode starts task t-1 and hands it over,
/// and a Codex episode resumes it and starts the issue.
fn two_provider_ledger(branch: &str, moved: bool) -> String {
    let env = |ts: &str, ev: &str, session: &str| {
        format!(
            r#""ts":"{ts}","event":"{ev}","head":"abc","branch":"{branch}","by":"majordomus/0.7.0","session":"{session}""#
        )
    };
    let mut lines = vec![
        format!(
            r#"{{{},"owner":"a","provider":"claude-code","provider_session":"win-a"}}"#,
            env("1", "session.started", "s-a")
        ),
        format!(
            r#"{{{},"task_id":"t-1","scope":"lib"}}"#,
            env("2", "task.started", "s-a")
        ),
        format!(
            r#"{{{},"task_id":"t-1","handover_path":".ai/local/state/handovers/h.md"}}"#,
            env("3", "task.handed_over", "s-a")
        ),
        format!(
            r#"{{{},"outcome":"closed"}}"#,
            env("4", "session.closed", "s-a")
        ),
        format!(
            r#"{{{},"owner":"b","provider":"codex","provider_session":"win-b"}}"#,
            env("5", "session.started", "s-b")
        ),
        format!(
            r#"{{{},"task_id":"t-1"}}"#,
            env("6", "task.checkpoint", "s-b")
        ),
    ];
    if moved {
        lines.push(format!(
            r#"{{{},"issue":"I0001"}}"#,
            env("7", "plan_start", "s-b")
        ));
    }
    lines.join("\n") + "\n"
}

#[test]
fn one_task_carried_by_two_providers_across_a_handover_realises_one_intent() {
    let f = Fixture::new();
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", true),
    );

    let (code, v) = cli_json(&f, &["intent", "realization"]);
    assert_eq!(code, 0, "{v:#}");
    let work = v["work"].as_array().unwrap();
    let task = work
        .iter()
        .find(|w| w["work"]["id"] == "t-1")
        .expect("the task is joined");
    assert_eq!(task["work"]["kind"], "task");
    let providers: Vec<&str> = task["work"]["episodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["provider"].as_str().unwrap())
        .collect();
    assert_eq!(
        providers,
        ["claude-code", "codex"],
        "both episodes, in order, each with its provider"
    );
    assert_eq!(
        task["work"]["handovers"],
        json!([".ai/local/state/handovers/h.md"])
    );
    assert_eq!(task["work"]["moved_issues"], json!(["I0001"]));

    // the Codex episode moved the issue, so the link is observed, not inferred from the scope
    let link = &task["links"][0];
    assert_eq!(link["intent"], "fixture-intent");
    assert_eq!(link["issue"], "I0001");
    assert_eq!(link["criteria"], json!(["the-case-passes"]));
    assert_eq!(link["via"], "moved_issue");
    assert_eq!(link["provenance"], "observed");

    let intent = &v["intents"][0];
    assert_eq!(intent["intent"], "fixture-intent");
    assert_eq!(intent["providers"], json!(["claude-code", "codex"]));
    assert_eq!(intent["work"][0]["id"], "t-1");
    assert_eq!(intent["work"][0]["handovers"], 1);
    assert_eq!(intent["unmet"][0]["id"], "the-case-passes");
    assert_eq!(intent["unmet"][0]["state"], "not_run");
    assert_eq!(intent["unmet"][0]["issues"], json!(["I0001"]));

    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/realization");
    assert_eq!(status, 200);
    assert_eq!(v, http, "the command line and HTTP disagree");
    let mcp = tool(&f, "majordomus_intent_realization", json!({}));
    assert_eq!(v, mcp, "the command line and MCP disagree");

    let (status, explained) = served.get("/api/v1/intents/explain?id=fixture-intent");
    assert_eq!(status, 200);
    let because: Vec<&str> = explained["because"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_str().unwrap())
        .collect();
    assert!(because[0].starts_with("planned"), "{because:#?}");
    assert!(
        because
            .iter()
            .any(|b| b.contains("`the-case-passes` is not met") && b.contains("no recorded run")),
        "{because:#?}"
    );
    assert!(
        because
            .iter()
            .any(|b| b.contains("across 1 handover(s), run by claude-code, codex")),
        "{because:#?}"
    );
    assert_eq!(
        explained,
        tool(
            &f,
            "majordomus_intent_explain",
            json!({"id": "fixture-intent"})
        )
    );
    let (status, _) = served.get("/api/v1/intents/explain?id=absent");
    assert_eq!(status, 404);
}

#[test]
fn each_link_says_how_it_is_known_and_the_strongest_wins() {
    let f = Fixture::new();
    // no plan transition and a branch that names nothing: only the scope overlaps
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", false),
    );
    let (_, v) = cli_json(&f, &["intent", "realization", "--intent", "fixture-intent"]);
    assert_eq!(v["work"][0]["links"][0]["provenance"], "inferred");
    assert_eq!(v["work"][0]["links"][0]["via"], "scope_overlap");

    // the branch names the issue: derived beats inferred, and replaces it
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("feature/I0001-work", false),
    );
    let (_, v) = cli_json(&f, &["intent", "realization", "--intent", "fixture-intent"]);
    let links = v["work"][0]["links"].as_array().unwrap();
    assert_eq!(links.len(), 1, "one link per issue: {links:#?}");
    assert_eq!(links[0]["provenance"], "derived");

    // the task's own words cite the issue: declared
    f.write(
        ".ai/local/state/archive/t-1.yaml",
        "id: t-1\ntask: \"finish I0001\"\nscope:\n  - lib\n",
    );
    f.write(".ai/local/state/current.yaml", "id: t-2\n");
    let (_, v) = cli_json(&f, &["intent", "realization", "--intent", "fixture-intent"]);
    let t1 = v["work"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["work"]["id"] == "t-1")
        .unwrap();
    assert_eq!(t1["links"][0]["provenance"], "declared");
    assert_eq!(t1["work"]["title"], "finish I0001");

    // an intent that is not there is refused, never an empty answer
    let (code, _, err) = run_in(
        &f.root(),
        &["intent", "realization", "--intent", "absent"],
        "",
    );
    assert_ne!(code, 0);
    assert!(err.contains("no intent 'absent'"), "{err}");
}

#[test]
fn live_work_that_serves_no_intent_is_named_with_the_missing_link() {
    let f = Fixture::new();
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", false),
    );
    f.remove(".ai/repo/project/intents/fixture-intent.yaml");
    f.commit("no intent");
    let (code, v) = cli_json(&f, &["intent", "realization"]);
    assert_eq!(code, 0, "an orphan is a warning, not a refusal");
    assert_eq!(v["orphans"], 1);
    let finding = &v["findings"][0];
    assert_eq!(finding["code"], "work_serves_no_intent");
    assert_eq!(finding["subject"], "t-1");
    assert!(
        finding["message"]
            .as_str()
            .unwrap()
            .contains("milestones no intent names: fixture-milestone (I0001)"),
        "{finding:#}"
    );
}

/// The Cockpit's intent page is a projection of `intent_realization.explain`: each criterion
/// links to the test object it names and to the issue serving it, the work realising the intent
/// carries its providers and the provenance of its link, and an unknown intent is a 404: the
/// claim `intent-cockpit-pages` of docs/CLAIMS.yaml.
#[test]
fn the_cockpit_links_each_criterion_to_its_test_and_the_issue_serving_it() {
    let f = Fixture::new();
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", true),
    );
    let served = Served::start(&f.root(), &[]);
    let page = |target: &str| {
        let (status, _, body) = served.request("GET", target, None);
        (status, body)
    };

    let (status, list) = page("/cockpit/intents");
    assert_eq!(status, 200, "{list}");
    assert!(
        list.contains(r#"href="/cockpit/intents/fixture-intent""#),
        "{list}"
    );
    assert!(list.contains("claude-code, codex"), "{list}");

    let (status, body) = page("/cockpit/intents/fixture-intent");
    assert_eq!(status, 200, "{body}");
    let test_link = "/cockpit/object?uri=majordomus%3A%2F%2Ftest%2Ftest%2Fcases%2F00_x.sh";
    assert!(
        body.contains(test_link),
        "the criterion does not link its test:\n{body}"
    );
    assert!(
        body.contains("/cockpit/object?uri=majordomus%3A%2F%2Fissue%2FI0001"),
        "the criterion does not link the issue serving it:\n{body}"
    );
    for fragment in [
        "not run",
        "bash test/run.sh 00_x",
        "observed",
        "codex",
        "t-1",
    ] {
        assert!(body.contains(fragment), "missing {fragment}:\n{body}");
    }
    // the linked test is a page the Cockpit actually serves
    let (status, _) = page(test_link);
    assert_eq!(status, 200);

    let (status, _) = page("/cockpit/intents/absent");
    assert_eq!(status, 404);
}

/// One MCP session over HTTP that announces a claim, the way a peer joins the board; the
/// session id, for closing it.
fn announce(served: &Served, client: &str, arguments: Value) -> String {
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": client, "version": "0.0.0" },
        },
    });
    let (status, headers, _) =
        served.request("POST", "/mcp", Some(&serde_json::to_string(&init).unwrap()));
    assert_eq!(status, 200, "initialize was not accepted");
    let id = headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.clone())
        .expect("the server mints a session id on initialize");
    let send = |message: Value| {
        served
            .request_with(
                "POST",
                "/mcp",
                Some(&serde_json::to_string(&message).unwrap()),
                &[("Mcp-Session-Id", &id)],
            )
            .2
    };
    send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    let body = send(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "majordomus_announce", "arguments": arguments },
    }));
    let v: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}"));
    assert_eq!(
        v["result"]["isError"], false,
        "announce was refused: {body}"
    );
    id
}

/// Every record a checkout holds is a unit of work: the ledger's tasks with the providers the
/// open episodes name, the active task the ledger has not seen, a closed session record of
/// another checkout, and every claim on the peer board, named or not. A ledger line that cannot
/// be read is named rather than silently dropped.
#[test]
fn every_record_the_checkout_holds_is_gathered_with_how_it_links() {
    let f = Fixture::new();
    // the shared layer's closed session records are a source class of this repository
    let sources = std::fs::read_to_string(f.path(".ai/repo/knowledge/sources.yaml")).unwrap();
    f.write(
        ".ai/repo/knowledge/sources.yaml",
        &format!(
            "{sources}  - id: session\n    kind: session\n    discovery: vcs\n    pathspec: \
             ':(glob).ai/repo/sessions/*.md'\n    required: false\n"
        ),
    );
    let record = |sid: &str, extra: &str| {
        format!(
            "---\nschema: session/v1\nkind: session\nsession_id: {sid}\n\
             started_at: 2026-09-01T00:00:00Z\nclosed_at: 2026-09-01T01:00:00Z\n\
             outcome: closed\ntitle: \"Session {sid}\"\nbranch: feature/elsewhere\n{extra}---\n\n\
             # Session {sid}\n"
        )
    };
    f.write(
        ".ai/repo/sessions/20260901T010000Z--s-remote.md",
        &record(
            "s-remote",
            "changed_files:\n  - lib/a.sh\nhandovers:\n  - .ai/repo/handovers/h.md\n\
             issues:\n  - I0001\n",
        ),
    );
    // a record of an episode this checkout's ledger already holds is that task, not a second
    // unit of work
    f.write(
        ".ai/repo/sessions/20260901T020000Z--s-a.md",
        &record("s-a", ""),
    );
    f.commit("two closed session records");

    let mut ledger = two_provider_ledger("master", true);
    ledger.push_str("this line is not a ledger entry\n");
    f.write(".ai/local/state/ledger.jsonl", &ledger);
    // the open episode file names the provider of s-b before the ledger's own event does
    f.write(
        ".ai/local/state/sessions-open/s-b.yaml",
        "session_id: s-b\nprovider: gemini\nprovider_session: g-1\n",
    );
    f.write(
        ".ai/local/state/sessions-open/broken.yaml",
        "owner: nobody\n",
    );
    // the active task predates the ledger's reader, and carries its own outcome and words
    f.write(
        ".ai/local/state/current.yaml",
        "id: t-9\noutcome: paused\ntask: \"carry I0001 home\"\nscope:\n  - lib\n",
    );

    let served = Served::start(&f.root(), &[]);
    let codex = announce(
        &served,
        "codex-mcp-client",
        json!({ "intent": "finish I0001", "scope": ["lib"], "claim": "I0001-work" }),
    );
    let gemini = announce(
        &served,
        "gemini-cli",
        json!({ "intent": "tidy the docs", "scope": ["docs"] }),
    );
    let (status, v) = served.get("/api/v1/intents/realization");
    assert_eq!(status, 200, "{v:#}");
    let unit = |id: &str| {
        v["work"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["work"]["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("no unit {id}: {v:#}"))
            .clone()
    };

    let t1 = unit("t-1");
    let providers: Vec<&str> = t1["work"]["episodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["provider"].as_str().unwrap())
        .collect();
    assert_eq!(providers, ["claude-code", "gemini"]);
    assert_eq!(t1["work"]["episodes"][1]["provider_session"], "g-1");

    let t9 = unit("t-9");
    assert_eq!(t9["work"]["outcome"], "paused");
    assert_eq!(t9["work"]["title"], "carry I0001 home");
    assert_eq!(t9["work"]["named_issues"], json!(["I0001"]));
    assert_eq!(t9["links"][0]["provenance"], "declared");

    let remote = unit("s-remote");
    assert_eq!(remote["work"]["kind"], "session_record");
    assert_eq!(remote["work"]["outcome"], "closed");
    assert_eq!(remote["work"]["title"], "Session s-remote");
    assert_eq!(remote["work"]["branches"], json!(["feature/elsewhere"]));
    assert_eq!(remote["work"]["scope"], json!(["lib/a.sh"]));
    assert_eq!(
        remote["work"]["handovers"],
        json!([".ai/repo/handovers/h.md"])
    );
    assert_eq!(remote["work"]["moved_issues"], json!(["I0001"]));
    assert_eq!(remote["links"][0]["provenance"], "observed");
    assert!(
        !v["work"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w["work"]["id"] == "s-a"),
        "the episode the ledger holds is not a second unit: {v:#}"
    );

    let claims: Vec<&Value> = v["work"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["work"]["kind"] == "peer_claim")
        .collect();
    assert_eq!(claims.len(), 2, "{v:#}");
    let named = claims
        .iter()
        .find(|w| w["work"]["id"].as_str().unwrap().ends_with("/I0001-work"))
        .expect("the named claim is <peer>/<claim>");
    assert_eq!(named["work"]["outcome"], "attached");
    assert_eq!(named["work"]["title"], "finish I0001");
    assert_eq!(named["work"]["named_issues"], json!(["I0001"]));
    assert_eq!(named["work"]["episodes"][0]["provider"], "codex-mcp-client");
    assert_eq!(named["links"][0]["provenance"], "declared");
    let unnamed = claims
        .iter()
        .find(|w| !w["work"]["id"].as_str().unwrap().contains('/'))
        .expect("an unnamed claim is the peer itself");
    assert_eq!(unnamed["work"]["scope"], json!(["docs"]));
    assert!(unnamed["links"].as_array().unwrap().is_empty());
    assert!(
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "work_serves_no_intent" && f["subject"] == unnamed["work"]["id"]),
        "a live claim that serves no intent is named: {v:#}"
    );
    let skipped = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "ledger_lines_skipped")
        .unwrap_or_else(|| panic!("an unreadable ledger line is named: {v:#}"));
    assert_eq!(skipped["subject"], ".ai/local/state/ledger.jsonl");
    assert!(skipped["message"]
        .as_str()
        .unwrap()
        .starts_with("1 ledger line(s) could not be read"));

    // a peer whose session ended has departed: its claim is history, not live work
    let (status, _, _) =
        served.request_with("DELETE", "/mcp", None, &[("Mcp-Session-Id", &gemini)]);
    assert!(status < 300, "the session closes: {status}");
    let (_, v) = served.get("/api/v1/intents/realization");
    let departed: Vec<&Value> = v["work"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|w| w["work"]["kind"] == "peer_claim" && w["work"]["outcome"] == "departed")
        .collect();
    assert_eq!(departed.len(), 1, "{v:#}");
    assert_eq!(departed[0]["work"]["scope"], json!(["docs"]));
    assert!(
        !v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["code"] == "work_serves_no_intent"),
        "a departed claim is not live work that serves nothing: {v:#}"
    );
    served.request_with("DELETE", "/mcp", None, &[("Mcp-Session-Id", &codex)]);
    drop(served);

    // the text the command line prints carries every unit, its link, and who ran it
    let (code, out, err) = run_in(&f.root(), &["intent", "realization"], "");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("  work      task t-1  active  observed  handovers 1  by claude-code, gemini"),
        "{out}"
    );
    // a closed session record names no provider
    assert!(
        out.contains("  work      session_record s-remote  closed  observed  handovers 1\n"),
        "{out}"
    );
    assert!(
        out.contains("task t-9  paused  fixture-intent via I0001 named_issue (declared)"),
        "{out}"
    );
}

/// The evidence a done transition owes, appended the way `plan evidence` appends it.
fn proven(f: &Fixture, rel: &str) {
    let record = std::fs::read_to_string(f.path(rel)).unwrap();
    f.write(
        rel,
        &format!(
            "{record}evidence:\n  - covers: proof\n    type: test\n    command: \"bash test/cases/00_x.sh\"\n    \
             result: \"ok\"\n    commit: abc\n    recorded_at: 2026-09-15T00:00:00Z\n"
        ),
    );
}

/// Run the executable with its stdout a socket whose reader is already gone, so that writing
/// the answer fails; the exit code and stderr.
#[cfg(unix)]
fn with_stdout_gone(f: &Fixture, args: &[&str]) -> (i32, String) {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    let (stdout, reader) = UnixStream::pair().expect("a socket pair");
    drop(reader);
    let out = Command::new(BIN)
        .args(args)
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .stdin(Stdio::null())
        .stdout(Stdio::from(OwnedFd::from(stdout)))
        .stderr(Stdio::piped())
        .output()
        .expect("spawn majordomus");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stderr).expect("stderr is UTF-8"),
    )
}

/// Closed work that reality contradicts: every issue and the milestone are DONE, the case the
/// criterion names fails, and every surface says so — the command line exits 10 and names the
/// contradiction, the explanation and the plan's coverage say why, and the Cockpit page shows
/// the finding beside each criterion, its evidence and the issues serving it.
#[test]
fn closed_work_reality_contradicts_is_named_on_every_surface_and_exits_10() {
    let f = Fixture::new();
    // two more criteria: one no issue serves, and one settled by a claim rather than a test
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &format!(
            "{}  - id: nobody-serves\n    criterion: Nothing serves this yet\n    evidence: test\n    \
             ref: test/cases/00_x.sh\n  - id: the-claim-holds\n    criterion: The policy claim \
             holds\n    evidence: claim\n    ref: policy-parse\ngovernance:\n  - rule:project.alpha\n",
            common::INTENT.replace("governance:\n  - rule:project.alpha\n", "")
        ),
    );
    // a second issue serving the same criterion
    let i1 = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).unwrap();
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &i1.replace("id: I0001", "id: I0002")
            .replace("slug: issue-I0001", "slug: issue-I0002"),
    );
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", true),
    );
    f.commit("three criteria, two issues");

    // close every issue and the milestone through the transition, with the evidence each owes
    for issue in ["I0001", "I0002"] {
        let moved = tool(
            &f,
            "majordomus_plan_transition",
            json!({ "issue": issue, "transition": "start" }),
        );
        assert_eq!(moved["to"], "ACTIVE", "{moved}");
        proven(&f, &format!(".ai/repo/project/issues/{issue}.yaml"));
        let moved = tool(
            &f,
            "majordomus_plan_transition",
            json!({ "issue": issue, "transition": "done" }),
        );
        assert_eq!(moved["to"], "DONE", "{moved}");
    }
    proven(&f, ".ai/repo/project/milestones/fixture-milestone.yaml");
    f.commit("close the work");

    // and the case the criterion names fails, recorded against the source in the tree
    let id = TestId::of("test/cases/00_x.sh").unwrap();
    let source = std::fs::read(f.path(&id.source())).unwrap();
    let mut ledger = Ledger::empty();
    ledger.merge([Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome: Outcome::Fail,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: "clean".into(),
        digest: digest_of(&source),
        at: "2026-09-15T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
        run: None,
    }]);
    ledger.save(&f.root()).unwrap();
    f.commit("the case fails");

    let (_, v) = cli_json(&f, &["intent", "show", "fixture-intent"]);
    assert_eq!(v["stage"], "verifying", "{v:#}");

    let (code, out, err) = run_in(&f.root(), &["intent", "realization"], "");
    assert_eq!(
        code, 10,
        "closed work reality contradicts is a refusal:\n{out}\n{err}"
    );
    assert!(
        out.contains("closed_work_contradicted  fixture-intent: "),
        "{out}"
    );
    // an unmet criterion nothing serves names no issue
    assert!(
        out.contains("  unmet     nobody-serves  failing\n"),
        "{out}"
    );
    assert!(
        out.contains("  unmet     the-case-passes  failing  served by I0001 I0002"),
        "{out}"
    );

    let (code, out, err) = run_in(&f.root(), &["intent", "explain", "fixture-intent"], "");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.starts_with("fixture-intent  The fixture's outcome is true\n"),
        "{out}"
    );
    assert!(
        out.contains("  - verifying: every milestone is DONE and"),
        "{out}"
    );
    assert!(out.contains("  - closed_work_contradicted: "), "{out}");

    let (code, out, err) = run_in(&f.root(), &["intent", "coverage"], "");
    assert_eq!(code, 0, "{err}");
    let row = out
        .lines()
        .find(|l| l.starts_with("fixture-intent#nobody-serves"))
        .unwrap_or_else(|| panic!("{out}"));
    assert!(
        row.ends_with("—"),
        "a criterion nothing serves names no issue: {row}"
    );
    assert!(
        out.lines()
            .any(|l| l.starts_with("fixture-intent#the-case-passes") && l.ends_with("I0001 I0002")),
        "{out}"
    );

    // an answer that cannot be written is a transport failure, never a success
    #[cfg(unix)]
    for args in [
        &["intent", "coverage"][..],
        &["intent", "realization"][..],
        &["intent", "explain", "fixture-intent"][..],
    ] {
        let (code, err) = with_stdout_gone(&f, args);
        assert_eq!(code, 13, "{args:?}: {err}");
        assert!(err.contains("majordomus: transport:"), "{args:?}: {err}");
    }

    let served = Served::start(&f.root(), &[]);
    let (status, _, body) = served.request("GET", "/cockpit/intents/fixture-intent", None);
    assert_eq!(status, 200, "{body}");
    for fragment in [
        // the finding beside the criteria
        "closed_work_contradicted — fixture-intent: ",
        // a criterion settled by a claim names the claim, not a test page
        "claim policy-parse",
        // two issues serving one criterion are both linked
        "/cockpit/object?uri=majordomus%3A%2F%2Fissue%2FI0002",
        "failing",
        "verifying",
    ] {
        assert!(body.contains(fragment), "missing {fragment}:\n{body}");
    }
}

/// An intent naming a milestone the plan does not hold reads as unresolved on its page, and
/// an evidence ledger this executable cannot read refuses the realization on every surface
/// rather than reading as an empty one.
#[test]
fn an_unresolved_milestone_and_an_unreadable_ledger_are_stated_not_hidden() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/project/intents/elsewhere.yaml",
        "id: elsewhere\ntitle: Somewhere else\nstatement: \"It is true somewhere else.\"\n\
         invariants: []\nmilestones:\n  - no-such-milestone\nsatisfaction:\n  - id: c\n    \
         criterion: It holds\n    evidence: test\n    ref: test/cases/00_x.sh\n",
    );
    f.commit("an intent of a milestone that is not there");
    let served = Served::start(&f.root(), &[]);
    let (status, _, body) = served.request("GET", "/cockpit/intents/elsewhere", None);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("does not resolve"), "{body}");
    drop(served);

    f.write(
        ".ai/repo/evidence/ledger.json",
        "{ \"this is\": not a ledger",
    );
    f.commit("an unreadable ledger");
    for args in [
        &["intent", "realization"][..],
        &["intent", "explain", "fixture-intent"][..],
    ] {
        let (code, out, err) = run_in(&f.root(), args, "");
        assert_eq!(
            code, 13,
            "{args:?} answered over an unreadable ledger:\n{out}"
        );
        assert!(out.is_empty(), "{args:?}:\n{out}");
        assert!(
            err.contains("is not a ledger this version can read"),
            "{args:?}: {err}"
        );
    }
    let served = Served::start(&f.root(), &[]);
    let (_, _, body) = served.request("GET", "/cockpit/intents", None);
    assert!(
        body.contains("is not a ledger this version can read"),
        "the list page states the failure:\n{body}"
    );
}
