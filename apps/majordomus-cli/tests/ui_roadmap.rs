//! `/ui/roadmap` reads where the work is, the delivery of each issue and the releases
//! themselves (I2744, ADR 0166 decision 4), through a served executable over a fixture:
//!
//! - Now is the milestones with work in flight, never merely the first open one: the
//!   fixture's first milestone has no work (the "Now M002" of 2026-10-16), and the screen
//!   names the one whose issue has an open pull request and the one whose issue is ACTIVE.
//! - An issue shows its declared status beside its inferred stage, linked to the pull
//!   request, the merge commit and the release tag that prove it.
//! - The latest release is the highest the releases name: a tag of the executable's own
//!   version outranks an older record, so the screen never shows a release older than the
//!   executable that drew it (the "Latest release 0.26.0 under a 0.27.0 header").
//! - A source that was not read says so.
//! - `plan.flight` is one answer over HTTP and MCP: the tool a client calls is the reading
//!   the screen draws.

mod common;

use std::io::Write;
use std::process::Stdio;

use common::{Fixture, Served};
use serde_json::{json, Value};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn issue(id: &str, milestone: &str, slug: &str, title: &str) -> String {
    common::ISSUE
        .replace("id: I0001", &format!("id: {id}"))
        .replace(
            "milestone: fixture-milestone",
            &format!("milestone: {milestone}"),
        )
        .replace("slug: issue-I0001", &format!("slug: {slug}"))
        .replace(
            "title: The bounded piece of work",
            &format!("title: {title}"),
        )
}

fn milestone(id: &str, order: u32, title: &str) -> String {
    format!(
        "id: {id}\ntitle: {title}\nslug: {id}\norder: {order}\npriority: p1\n\
         problem: \"A problem.\"\noutcome: \"An outcome.\"\nacceptance_criteria:\n  - It is \
         reached\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\n"
    )
}

/// A release record of `version` naming `commit`, in the shape `scripts/release-record` writes.
fn record(version: &str, commit: &str) -> String {
    format!(
        "schema: release/v1\nversion: \"{version}\"\ntag: v{version}\nchannel: stable\n\
         commit: {commit}\npublished_at: \"2026-10-16T07:00:00Z\"\nartifacts:\n  \
         - target: linux-x86_64-gnu\n    name: m-v{version}.tar.gz\n    \
         url: https://example.invalid/m-v{version}.tar.gz\n    sha256: \"{}\"\n    size: 1\n",
        "0".repeat(64)
    )
}

fn observe_forge(f: &Fixture) {
    let observation = json!({
        "schema": 4,
        "repository": "example/fixture",
        "base": "master",
        "base_sha": "0000000000000000000000000000000000000000",
        "observed_at": "2026-10-16T08:00:00Z",
        "required_checks": null,
        "review_policy": null,
        "merge_methods": ["merge"],
        "pull_requests": [{
            "number": 9,
            "title": "feat: the busy piece (I0002)",
            "author": "someone",
            "head_ref": "feature/the-busy-piece",
            "head_sha": "1111111111111111111111111111111111111111",
            "base_ref": "master",
            "draft": false,
            "created_at": "2026-10-16T07:00:00Z",
            "updated_at": "2026-10-16T07:30:00Z"
        }]
    });
    f.write(
        ".ai/local/state/integration/observation.json",
        &serde_json::to_string_pretty(&observation).unwrap(),
    );
}

/// The fixture: `fixture-milestone` first in the plan's order and with no work in flight
/// (I0001 merged by pull request #5 and released in v0.1.0, declared READY); `m-busy`
/// second, whose I0002 has open pull request #9; `m-started` third, whose I0003 is ACTIVE;
/// `m-later` last, depending on `m-busy`. A `v<executable>` tag on the trunk, with no
/// record of it.
fn fixture() -> (Fixture, String) {
    let f = Fixture::new();
    f.write(
        ".ai/repo/project/milestones/m-busy.yaml",
        &milestone("m-busy", 1, "The busy milestone"),
    );
    f.write(
        ".ai/repo/project/milestones/m-started.yaml",
        &milestone("m-started", 2, "The started milestone"),
    );
    f.write(
        ".ai/repo/project/milestones/m-later.yaml",
        &format!(
            "{}depends_on:\n  - m-busy\n",
            milestone("m-later", 3, "The later milestone")
        ),
    );
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &issue("I0002", "m-busy", "the-busy-piece", "The busy piece"),
    );
    f.write(
        ".ai/repo/project/issues/I0003.yaml",
        &issue(
            "I0003",
            "m-started",
            "the-started-piece",
            "The started piece",
        ),
    );
    f.write(
        ".ai/repo/project/issues/I0004.yaml",
        &issue("I0004", "m-later", "the-later-piece", "The later piece"),
    );
    f.commit("plan: three more milestones");
    let trunk = f
        .git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    f.git(&["checkout", "-q", "-b", "feature/the-work"]);
    f.write("lib/work.sh", "echo work\n");
    f.commit("feat: the work (I0001)");
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
    let merge = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.write(".ai/repo/releases/v0.1.0.yaml", &record("0.1.0", &merge));
    f.commit("release: record v0.1.0");
    // the executable's own version is tagged, and its record has not merged yet
    f.git(&["tag", &format!("v{VERSION}")]);
    // I0003 is started, through the one writer of a stamp
    let mut writer = Served::start(&f.root(), &[]);
    let (status, _, out) = writer.request(
        "POST",
        "/api/v1/plan/transition",
        Some(r#"{"issue":"I0003","transition":"start"}"#),
    );
    assert_eq!(status, 200, "plan.transition start I0003: {out}");
    writer.stop();
    observe_forge(&f);
    (f, merge)
}

fn page(s: &Served, target: &str) -> String {
    let (status, _, html) = s.request("GET", target, None);
    assert_eq!(status, 200, "{target}: {html}");
    html
}

#[test]
fn now_is_where_the_work_is_and_the_release_is_the_releases() {
    let (f, merge) = fixture();
    let mut s = Served::start(&f.root(), &[]);

    // ---- the answer the screen reads
    let (status, flight) = s.get("/api/v1/plan/flight");
    assert_eq!(status, 200, "{flight}");
    assert_eq!(
        flight["now"],
        json!(["m-busy", "m-started"]),
        "now is the milestones with work in flight, in the plan's order: {flight}"
    );
    assert_eq!(
        flight["next"], "fixture-milestone",
        "next is the first open milestone the graph allows: {flight}"
    );
    let (_, roadmap) = s.get("/api/v1/plan/roadmap");
    assert_eq!(
        roadmap["now"], "fixture-milestone",
        "the fixture reproduces the order-only now this screen no longer shows"
    );
    let busy = &flight["milestones"][0];
    assert_eq!(busy["issues"][0]["issue"], "I0002");
    assert_eq!(busy["issues"][0]["stage"], "pr_open");
    assert_eq!(busy["issues"][0]["in_flight"], json!(["pr_open"]));
    let started = &flight["milestones"][1];
    assert_eq!(started["issues"][0]["status"], "ACTIVE");
    assert_eq!(started["issues"][0]["in_flight"], json!(["active"]));
    // The latest release is never a pre-release (#1313): an executable whose version carries a
    // pre-release part leaves the latest at the highest stable release, the fixture's recorded
    // 0.1.0, while its own tag still counts as tagged.
    let (latest, latest_source) = if VERSION.contains('-') {
        ("0.1.0", "records")
    } else {
        (VERSION, "tags")
    };
    assert_eq!(flight["release"]["latest"], latest, "{flight}");
    assert_eq!(flight["release"]["source"], latest_source, "{flight}");
    assert_eq!(flight["release"]["executable"], VERSION);
    assert_eq!(flight["release"]["executable_tagged"], true);
    let records = flight["release"]["readings"]
        .as_array()
        .and_then(|r| r.iter().find(|r| r["source"] == "records"))
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(records["version"], "0.1.0", "{flight}");

    // ---- the screen
    let html = page(&s, "/ui/roadmap");
    assert!(
        html.contains("Now m-busy, m-started · next fixture-milestone"),
        "{html}"
    );
    assert!(
        !html.contains("Now fixture-milestone"),
        "a milestone with no work in flight is not now: {html}"
    );
    assert!(html.contains("plan.flight"), "the screen cites its source");
    // the issue in flight: declared beside inferred, with the pull request that proves it
    assert!(html.contains("I0002") && html.contains("pr_open"), "{html}");
    assert!(
        html.contains("href=\"https://github.com/example/fixture/pull/9\""),
        "the open pull request is linked: {html}"
    );
    // the release, from the releases, never older than this executable
    assert!(
        html.contains(&format!("Latest release {latest} (from {latest_source})")),
        "{html}"
    );
    assert!(
        html.contains(&format!(
            "href=\"https://github.com/example/fixture/releases/tag/v{latest}\""
        )),
        "{html}"
    );
    // a source that was not read is said: no deployment was observed, no mesh runs here
    assert!(html.contains("publication"), "{html}");
    assert!(html.contains(">unread<"), "{html}");

    // ---- the inspected milestone: every issue, its stage and its proof
    let html = page(&s, "/ui/roadmap?milestone=fixture-milestone");
    assert!(html.contains("Delivery of fixture-milestone"), "{html}");
    assert!(html.contains("released"), "{html}");
    for href in [
        "https://github.com/example/fixture/pull/5".to_string(),
        format!("https://github.com/example/fixture/commit/{merge}"),
        "https://github.com/example/fixture/releases/tag/v0.1.0".to_string(),
    ] {
        assert!(
            html.contains(&format!("href=\"{href}\"")),
            "{href} is not linked: {html}"
        );
    }
    // a milestone the plan does not declare is said, not answered with every issue
    let html = page(&s, "/ui/roadmap?milestone=m-nope");
    assert!(
        html.contains("m-nope is not a milestone of this plan."),
        "{html}"
    );
    s.stop();
}

/// A plan with no work in flight says so, and an executable whose version no tag names is
/// said to be untagged.
#[test]
fn nothing_in_flight_is_said_and_an_untagged_executable_too() {
    let f = Fixture::new();
    let mut s = Served::start(&f.root(), &[]);
    let html = page(&s, "/ui/roadmap");
    let (status, refused) = s.get("/api/v1/plan/flight?milestone=m-nope");
    s.stop();
    assert!(html.contains("No work is in flight"), "{html}");
    assert!(html.contains("is not tagged"), "{html}");
    assert_eq!(status, 404, "{refused}");
}

/// Where the mesh runs, a live claim puts its issue in flight and is shown as a proof; a
/// cancelled milestone is never now, next is the first open milestone whose prerequisites are
/// all now, and every proof a stage has is shown: a remote-tracking branch linked on the
/// forge, a local one unlinked, and the published pointer.
#[test]
fn a_live_claim_puts_its_issue_in_flight_and_every_proof_is_shown() {
    let (f, merge) = fixture();
    f.write(
        ".ai/repo/project/milestones/m-dropped.yaml",
        &format!(
            "{}cancelled: true\n",
            milestone("m-dropped", 4, "The dropped milestone")
        ),
    );
    f.write(
        "site/static/releases/latest.json",
        "{\"schema\":\"release/v1\",\"version\":\"0.1.0\",\"tag\":\"v0.1.0\"}\n",
    );
    f.commit("plan: a dropped milestone; publish the pointer");
    let published = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.write(
        ".ai/local/state/integration/deployments.json",
        &serde_json::to_string_pretty(&json!({
            "schema": 1,
            "repository": "example/fixture",
            "repository_url": "https://github.com/example/fixture",
            "observed_at": "2026-10-16T09:00:00Z",
            "deployments": [{"id": "1", "environment": "github-pages", "sha": published,
                             "created_at": "2026-10-16T08:30:00Z", "state": "success"}],
            "releases": []
        }))
        .unwrap(),
    );
    f.git(&["branch", "fix/the-later-piece"]);
    f.git(&[
        "update-ref",
        "refs/remotes/origin/feature/the-busy-piece",
        &merge,
    ]);

    let state = f.parent().join("xdg-state");
    let key = majordomus_cli::mesh::identity::NodeIdentity::load_or_create(
        &state.join("majordomus").join("node.json"),
    )
    .expect("a node identity")
    .public
    .public_key;
    f.write(
        ".ai/repo/mesh/majordomus.yaml",
        &format!(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n\
             multicast:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n    - {key}\n\
             cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: fixture\n"
        ),
    );
    let mut s = Served::start_with_env(
        &f.root(),
        &["--discovery", "filesystem"],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    );
    let post = |path: &str, body: Value| {
        let (status, _, text) = s.request("POST", path, Some(&body.to_string()));
        assert_eq!(status, 200, "{path}: {text}");
    };
    post(
        "/api/v1/mesh/sessions",
        json!({"session": "s1", "client": "test", "intent": "the work", "issue": "I0001"}),
    );
    post(
        "/api/v1/mesh/claims",
        json!({"session": "s1", "scope": ["lib"], "issue": "I0001", "intent": "the work"}),
    );
    post(
        "/api/v1/mesh/claims",
        json!({"session": "s1", "scope": ["docs"], "intent": "no issue named"}),
    );

    let (status, flight) = s.get("/api/v1/plan/flight?milestone=m-later");
    assert_eq!(status, 200, "{flight}");
    assert_eq!(
        flight["now"],
        json!(["fixture-milestone", "m-busy", "m-started"]),
        "the claimed issue's milestone is now; the dropped one never is: {flight}"
    );
    assert_eq!(flight["next"], "m-later", "{flight}");
    let fixture_now = &flight["milestones"][0]["issues"][0];
    assert_eq!(fixture_now["in_flight"], json!(["claim"]), "{fixture_now}");
    let kinds: Vec<&str> = fixture_now["proofs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["pull_request", "merge", "release", "publication", "claim"],
        "{fixture_now}"
    );
    let busy = &flight["milestones"][1]["issues"][0]["proofs"][0];
    assert_eq!(
        busy["href"], "https://github.com/example/fixture/tree/feature/the-busy-piece",
        "{busy}"
    );
    let later = &flight["inspected"]["issues"][0]["proofs"][0];
    assert_eq!(
        (later["kind"].as_str(), later.get("href")),
        (Some("branch"), None),
        "{later}"
    );
    let publication = flight["release"]["readings"]
        .as_array()
        .and_then(|r| r.iter().find(|r| r["source"] == "publication"))
        .cloned()
        .unwrap_or(Value::Null);
    assert_eq!(publication["version"], "0.1.0", "{flight}");

    let html = page(&s, "/ui/roadmap?milestone=m-later");
    s.stop();
    assert!(html.contains("claim "), "the claim is shown: {html}");
    assert!(
        html.contains("branch fix/the-later-piece"),
        "a local branch is shown, unlinked: {html}"
    );
}

/// One `tools/call` of `name` over a fresh MCP session in the fixture, its structured answer.
fn mcp_tool(f: &Fixture, name: &str, arguments: Value) -> Value {
    let mut child = common::majordomus()
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

#[test]
fn plan_flight_is_one_answer_over_http_and_mcp() {
    let (f, _) = fixture();
    let mut s = Served::start(&f.root(), &[]);
    let (status, from_http) = s.get("/api/v1/plan/flight");
    s.stop();
    assert_eq!(status, 200, "{from_http}");
    let from_mcp = mcp_tool(&f, "majordomus_plan_flight", json!({}));
    // when it was observed differs between two readings; what was observed must not
    for key in ["schema", "now", "next", "milestones", "release"] {
        assert_eq!(
            from_mcp[key], from_http[key],
            "{key}: MCP {from_mcp} against HTTP {from_http}"
        );
    }
    assert_eq!(
        from_mcp["now"],
        json!(["m-busy", "m-started"]),
        "{from_mcp}"
    );
}
