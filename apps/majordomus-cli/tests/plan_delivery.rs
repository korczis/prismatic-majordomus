//! `plan.delivery` through the built executable (I2740, ADR 0166): in a fixture repository an
//! issue named by a merged pull request reads `merged` with the merge commit and `released`
//! once a `v*` tag contains it; an issue with only an open pull request reads `pr_open` with
//! its number, and with the forge unread the answer says unread, never none. The command
//! line, HTTP and MCP answer the same document, because all three are projections of the one
//! capability declaration.

mod common;

use std::io::Write;
use std::process::Stdio;

use common::{run_in, without_observation_times, Fixture, Served};
use serde_json::{json, Value};

fn issue(id: &str, slug: &str) -> String {
    common::ISSUE
        .replace("id: I0001", &format!("id: {id}"))
        .replace("slug: issue-I0001", &format!("slug: {slug}"))
}

/// The fixture: I0001 merged by a pull request whose title names it, I0002 on an unmerged
/// branch named by its slug, I0003 named only by an open pull request, I0004 named by nothing.
fn fixture() -> (Fixture, String) {
    let f = Fixture::new();
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &issue("I0002", "a-slugged-piece"),
    );
    f.write(
        ".ai/repo/project/issues/I0003.yaml",
        &issue("I0003", "an-open-piece"),
    );
    f.write(
        ".ai/repo/project/issues/I0004.yaml",
        &issue("I0004", "an-untouched-piece"),
    );
    f.commit("plan: three more issues");
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

    f.git(&["branch", "fix/a-slugged-piece"]);
    f.git(&["checkout", "-q", "fix/a-slugged-piece"]);
    f.write("lib/slug.sh", "echo slug\n");
    f.commit("fix: a slugged piece");
    f.git(&["checkout", "-q", &trunk]);
    (f, merge)
}

fn cli(f: &Fixture, args: &[&str]) -> (i32, Value, String) {
    let mut argv = vec!["plan", "delivery"];
    argv.extend_from_slice(args);
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out).unwrap_or(Value::Null);
    (code, v, err)
}

fn by_id<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["issues"]
        .as_array()
        .and_then(|a| a.iter().find(|i| i["issue"] == id))
        .unwrap_or_else(|| panic!("{id} is not in {v}"))
}

fn source<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["sources"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["name"] == name))
        .unwrap_or_else(|| panic!("no source {name} in {v}"))
}

fn observe_forge(f: &Fixture, title: &str, head: &str) {
    let observation = json!({
        "schema": 4,
        "repository": "someone/fixture",
        "base": "master",
        "base_sha": "0000000000000000000000000000000000000000",
        "observed_at": "2026-10-16T08:00:00Z",
        "required_checks": null,
        "review_policy": null,
        "merge_methods": ["merge"],
        "pull_requests": [{
            "number": 9,
            "title": title,
            "author": "someone",
            "head_ref": head,
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

fn observe_deployment(f: &Fixture, sha: &str, state: &str) {
    let observation = json!({
        "schema": 1,
        "repository": "someone/fixture",
        "repository_url": "https://example.invalid/someone/fixture",
        "observed_at": "2026-10-16T09:00:00Z",
        "deployments": [{
            "id": "1",
            "environment": "github-pages",
            "sha": sha,
            "created_at": "2026-10-16T08:30:00Z",
            "state": state
        }],
        "releases": []
    });
    f.write(
        ".ai/local/state/integration/deployments.json",
        &serde_json::to_string_pretty(&observation).unwrap(),
    );
}

#[test]
fn plan_delivery_reads_each_stage_with_the_object_that_proved_it() {
    let (f, merge) = fixture();

    // ---- live, with the forge never observed
    let (code, v, err) = cli(&f, &[]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(v["observed"]["mode"], "live");
    let i1 = by_id(&v, "I0001");
    assert_eq!(i1["stage"], "merged", "{i1}");
    assert_eq!(i1["merged"]["commit"], merge.as_str());
    assert_eq!(i1["merged"]["pull_request"], 5);
    assert_eq!(i1["merged"]["branch"], "feature/the-work");
    assert_eq!(i1["merged"]["via"], "title_or_branch");
    assert_eq!(
        i1["status"], "READY",
        "the declared status rides beside the stage"
    );
    let i2 = by_id(&v, "I0002");
    assert_eq!(i2["stage"], "branch", "{i2}");
    assert_eq!(i2["branch"]["name"], "fix/a-slugged-piece");
    assert_eq!(i2["branch"]["via"], "slug");
    // the forge was never read: I0003 is not "none and settled", it is unread
    let i3 = by_id(&v, "I0003");
    assert_eq!(i3["stage"], "none");
    assert_eq!(i3["settled"], false);
    assert_eq!(i3["unread"], json!(["forge"]));
    assert_eq!(source(&v, "forge")["state"], "unread");
    assert!(source(&v, "forge")["detail"]
        .as_str()
        .unwrap()
        .contains("prs refresh"));
    assert_eq!(v["tallies"]["merged"], 1);
    assert_eq!(
        v["unsettled"].as_u64(),
        Some(3),
        "I0002, I0003, I0004 could still have a pull request"
    );

    // ---- the forge observed: I0003 has an open pull request that names it
    observe_forge(&f, "feat: an open piece (I0003)", "feature/something");
    let (_, v, _) = cli(&f, &[]);
    let i3 = by_id(&v, "I0003");
    assert_eq!(i3["stage"], "pr_open", "{i3}");
    assert_eq!(i3["pr_open"]["number"], 9);
    assert_eq!(i3["pr_open"]["observed_at"], "2026-10-16T08:00:00Z");
    assert_eq!(i3["settled"], true);
    let i4 = by_id(&v, "I0004");
    assert_eq!(
        (i4["stage"].as_str(), i4["settled"].as_bool()),
        (Some("none"), Some(true))
    );

    // ---- a tag alone releases nothing: the release records are the source, not refs
    f.git(&["tag", "v0.1.0", &merge]);
    let (_, v, _) = cli(&f, &["I0001"]);
    assert_eq!(
        by_id(&v, "I0001")["stage"],
        "merged",
        "a tag is not a release record"
    );

    // ---- release records: the first one whose commit contains the merge
    f.write(".ai/repo/releases/v0.1.0.yaml", &record("0.1.0", &merge));
    f.write("lib/later.sh", "echo later\n");
    f.commit("release: record v0.1.0");
    let later = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.write(".ai/repo/releases/v0.2.0.yaml", &record("0.2.0", &later));
    f.commit("release: record v0.2.0");
    let (_, v, _) = cli(&f, &["I0001"]);
    assert_eq!(v["issues"].as_array().unwrap().len(), 1);
    let i1 = by_id(&v, "I0001");
    assert_eq!(i1["stage"], "released", "{i1}");
    assert_eq!(i1["released"]["tag"], "v0.1.0");
    // no deployment observed: the publication is unread, and the stage says so
    assert_eq!(i1["unread"], json!(["publication"]));
    assert!(source(&v, "publication")["detail"]
        .as_str()
        .unwrap()
        .contains("served forge"));

    // ---- a Pages deployment observed: its commit's latest.json names 0.2.0, so v0.1.0 is out
    f.write(
        "site/static/releases/latest.json",
        "{\"schema\":\"release/v1\",\"version\":\"0.2.0\",\"tag\":\"v0.2.0\"}\n",
    );
    f.commit("release: publish the pointer");
    let deployed = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    observe_deployment(&f, &deployed, "success");
    let (_, v, _) = cli(&f, &["I0001"]);
    let i1 = by_id(&v, "I0001");
    assert_eq!(i1["stage"], "deployed", "{i1}");
    assert_eq!(i1["deployed"]["version"], "0.2.0");
    assert_eq!(
        i1["deployed"]["latest_json"],
        format!("{deployed}:site/static/releases/latest.json").as_str()
    );
    assert_eq!(i1["deployed"]["observed_at"], "2026-10-16T09:00:00Z");
    let (_, out, _) = run_in(&f.root(), &["plan", "delivery", "I0001"], "");
    assert!(out.contains("v0.1.0 published as 0.2.0"), "{out}");
    // a deployment that failed publishes nothing
    observe_deployment(&f, &deployed, "failure");
    let (_, v, _) = cli(&f, &["I0001"]);
    assert_eq!(by_id(&v, "I0001")["stage"], "released");
    assert!(source(&v, "publication")["detail"]
        .as_str()
        .unwrap()
        .contains("no successful"));

    // ---- a commit on the trunk whose subject names the issue: merged, by that commit
    f.write("lib/direct.sh", "echo direct\n");
    f.commit("fix: the untouched piece, directly (I0004)");
    let direct = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let (_, v, _) = cli(&f, &["I0004"]);
    let i4 = by_id(&v, "I0004");
    assert_eq!(i4["stage"], "merged", "{i4}");
    assert_eq!(i4["merged"]["commit"], direct.as_str());
    assert_eq!(i4["merged"]["via"], "subject");
    // a body that mentions an issue names nothing
    f.write("lib/body.sh", "echo body\n");
    f.git(&["add", "-A"]);
    f.git(&[
        "commit",
        "-q",
        "-m",
        "chore: unrelated",
        "-m",
        "I0002 builds on this later.",
    ]);
    let (_, v, _) = cli(&f, &["I0002"]);
    assert_eq!(
        by_id(&v, "I0002")["stage"],
        "branch",
        "a commit body named I0002"
    );

    // ---- an id the plan does not declare is not found, never "none"
    let (code, _, err) = cli(&f, &["I9999"]);
    assert_ne!(code, 0);
    assert!(err.contains("I9999"), "{err}");
}

#[test]
fn plan_delivery_at_a_commit_reads_the_history_and_nothing_live() {
    let (f, merge) = fixture();
    f.write(".ai/repo/releases/v0.1.0.yaml", &record("0.1.0", &merge));
    f.write(
        "site/static/releases/latest.json",
        "{\"schema\":\"release/v1\",\"version\":\"0.1.0\",\"tag\":\"v0.1.0\"}\n",
    );
    f.commit("release: record v0.1.0");
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    // live state that an answer at a commit must not read
    observe_forge(&f, "feat: an open piece (I0003)", "feature/something");
    observe_deployment(&f, &head, "success");
    let (code, v, err) = cli(&f, &["--at", &head]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(v["observed"]["mode"], "at");
    assert_eq!(v["observed"]["commit"], head.as_str());
    let i1 = by_id(&v, "I0001");
    assert_eq!(
        i1["stage"], "released",
        "the publication is live state: {i1}"
    );
    assert_eq!(i1["released"]["tag"], "v0.1.0");
    assert_eq!(i1["unread"], json!(["publication"]));
    // neither the slugged branch nor the open pull request is read at a commit: unread, not none
    for id in ["I0002", "I0003"] {
        let i = by_id(&v, id);
        assert_eq!(i["stage"], "none", "{i}");
        assert_eq!(i["unread"], json!(["branches", "forge"]), "{i}");
    }
    for name in ["branches", "forge", "publication"] {
        assert_eq!(source(&v, name)["state"], "unread", "{name}");
    }

    // the answer is a function of the commit: the same bytes, whatever is tagged later
    f.git(&["tag", "v9.0.0"]);
    let (_, again, _) = cli(&f, &["--at", &head]);
    assert_eq!(v, again);

    // the text rendering names the proof and the commit
    let (code, out, _) = run_in(&f.root(), &["plan", "delivery", "--at", &head], "");
    assert_eq!(code, 0);
    assert!(out.contains("v0.1.0 contains"), "{out}");
    assert!(out.contains("(unread: branches, forge)"), "{out}");
    assert!(
        out.contains(&head[..11]),
        "the text names the commit it observed: {out}"
    );

    // a revision that resolves to nothing is refused, with its name
    let (code, _, err) = cli(&f, &["--at", "no-such-rev"]);
    assert_ne!(code, 0);
    assert!(err.contains("no-such-rev"), "{err}");
}

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
fn plan_delivery_is_one_answer_on_the_command_line_http_and_mcp() {
    let (f, _) = fixture();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let (_, from_cli, _) = cli(&f, &["--at", &head]);
    let from_mcp = mcp_tool(&f, "majordomus_plan_delivery", json!({ "at": head }));
    let mut served = Served::start(&f.root(), &[]);
    let (status, from_http) = served.get(&format!("/api/v1/plan/delivery?at={head}"));
    served.stop();
    assert_eq!(status, 200, "{from_http}");
    let norm = |v: &Value| without_observation_times(&v.to_string());
    assert_eq!(norm(&from_cli), norm(&from_mcp));
    assert_eq!(norm(&from_cli), norm(&from_http));
}
