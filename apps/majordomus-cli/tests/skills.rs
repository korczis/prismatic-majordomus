//! Skills through the built executable: the command line, HTTP and MCP answer the same
//! derivation; a recorded passing run of a naming test is the only thing that makes a skill
//! tested; and an orphan, a binding to nothing and a broken contract are each refused, naming
//! the finding.
//!
//! majordomus-skill: deploy-site implement repo-review assess-before-deleting report-verification-state pack-for-review
//!
//! The marker above binds this file to every active skill of this repository: the last test
//! reads each one from the repository itself and fails when one of them is not a valid,
//! invoked capability that names this file among its tests.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{run_in, Fixture, Served, BIN};
use majordomus_cli::evidence::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
use serde_json::{json, Value};

/// The marker, assembled so that this file's own fixtures do not bind it to their skills.
const MARKER: &str = concat!("majordomus-", "skill:");

/// One MCP tool call over stdio; the structured content of its answer.
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

fn cli_json(root: &std::path::Path, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(root, &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

fn skill_file(id: &str, status: &str, extra: &str) -> String {
    format!(
        "---\nschema: skill/v1\nid: {id}\nversion: 1\ntitle: Skill {id}\ndescription: The {id} procedure.\nstatus: {status}\n{extra}---\n# Purpose\n\nWhy.\n\n# Procedure\n\n1. Do it.\n\n# Output\n\nA report.\n"
    )
}

/// A fixture holding one active skill `alpha`, invoked by a workflow and named by a case.
fn skilled() -> Fixture {
    let f = Fixture::new();
    let sources = std::fs::read_to_string(f.path(".ai/repo/knowledge/sources.yaml")).unwrap();
    f.write(
        ".ai/repo/knowledge/sources.yaml",
        &format!(
            "{sources}\n  - id: skill\n    kind: skill\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/skills/*/SKILL.md'\n    required: false\n"
        ),
    );
    f.write(
        ".ai/repo/skills/alpha/SKILL.md",
        &skill_file("alpha", "active", ""),
    );
    f.write(
        ".ai/repo/workflows/uses-alpha.md",
        "# Uses alpha\n\nFollow majordomus://skill/alpha.\n",
    );
    f.write(
        "test/cases/01_alpha.sh",
        &format!("# {MARKER} alpha\n. \"$ROOT/test/lib.sh\"\ntrue\n"),
    );
    f.commit("a skill, its invocation and its test");
    f
}

fn codes(verify: &Value) -> Vec<String> {
    verify["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| {
            format!(
                "{}:{}",
                f["level"].as_str().unwrap(),
                f["code"].as_str().unwrap()
            )
        })
        .collect()
}

#[test]
fn the_command_line_http_and_mcp_answer_the_same_skills() {
    let f = skilled();
    let (code, cli) = cli_json(&f.root(), &["skills", "status"]);
    assert_eq!(code, 0);
    assert_eq!(cli["count"], 1);
    let alpha = &cli["skills"][0];
    assert_eq!(alpha["id"], "alpha");
    assert_eq!(alpha["used"]["used"], true);
    assert_eq!(
        alpha["used"]["invocations"][0]["path"],
        ".ai/repo/workflows/uses-alpha.md"
    );
    assert_eq!(
        alpha["tested"]["tests"][0]["path"],
        "test/cases/01_alpha.sh"
    );
    assert_eq!(alpha["tested"]["state"], "not_run");
    assert_eq!(alpha["standing"], "partial");

    let mcp = tool(&f, "majordomus_skills", json!({}));
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/skills");
    assert_eq!(status, 200);
    assert_eq!(cli, http, "the command line and HTTP disagree");
    assert_eq!(cli, mcp, "the command line and MCP disagree");

    let (status, one) = served.get("/api/v1/skills/explain?id=alpha");
    assert_eq!(status, 200);
    assert_eq!(
        &one, alpha,
        "skills.explain answers what skills.status lists"
    );
    let (status, _) = served.get("/api/v1/skills/explain?id=absent");
    assert_eq!(status, 404);
    let (code, _, _) = run_in(&f.root(), &["skills", "explain", "absent"], "");
    assert_eq!(
        code, 12,
        "an unknown skill is a missing thing, not an empty answer"
    );

    let (code, cli_verify) = cli_json(&f.root(), &["skills", "verify"]);
    assert_eq!(code, 0, "warnings alone do not refuse: {cli_verify}");
    let (_, http_verify) = served.get("/api/v1/skills/verify");
    let mcp_verify = tool(&f, "majordomus_skills_verify", json!({}));
    assert_eq!(cli_verify, http_verify);
    assert_eq!(cli_verify, mcp_verify);
    assert_eq!(cli_verify["valid"], true);
    assert_eq!(cli_verify["failures"], 0);
    // a fixture has no ledger run, no site page and no gate: three debts, each a warning
    assert_eq!(
        codes(&cli_verify),
        ["warn:unevidenced", "warn:undocumented", "warn:unenforced"]
    );
}

#[test]
fn a_skill_is_tested_only_by_a_recorded_passing_run_of_the_test_that_names_it() {
    let f = skilled();
    let tested = |f: &Fixture| {
        let (_, v) = cli_json(&f.root(), &["skills", "explain", "alpha"]);
        v["tested"].clone()
    };
    assert_eq!(tested(&f)["state"], "not_run");

    let id = TestId::of("test/cases/01_alpha.sh").unwrap();
    let source = std::fs::read(f.path(&id.source())).unwrap();
    let run = |outcome: Outcome, digest: String| Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: "clean".into(),
        digest,
        at: "2026-09-17T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
    };
    let record = |e: Execution| {
        let mut ledger = Ledger::empty();
        ledger.merge([e]);
        ledger.save(&f.root()).unwrap();
    };

    // recorded against the commit that is checked out, over the source that is there: the
    // only change since is the ledger itself
    record(run(Outcome::Pass, digest_of(&source)));
    let t = tested(&f);
    assert_eq!(t["state"], "proven", "{t}");
    assert_eq!(t["tests"][0]["recorded_at"], "2026-09-17T00:00:00Z");

    // the same pass over a test that has since changed proves nothing about it
    record(run(Outcome::Pass, digest_of(b"another test")));
    assert_eq!(tested(&f)["state"], "stale");

    record(run(Outcome::Fail, digest_of(&source)));
    assert_eq!(tested(&f)["state"], "failing");
    let (code, v) = cli_json(&f.root(), &["skills", "verify"]);
    assert_eq!(code, 10);
    assert!(codes(&v).contains(&"fail:failing".to_string()), "{v}");
}

#[test]
fn an_orphan_a_binding_to_nothing_and_a_broken_contract_are_refused_naming_each_finding() {
    let f = skilled();
    let verify = |f: &Fixture| cli_json(&f.root(), &["skills", "verify"]);

    // nothing invokes it
    f.remove(".ai/repo/workflows/uses-alpha.md");
    f.commit("the invocation goes");
    let (code, v) = verify(&f);
    assert_eq!(code, 10, "{v}");
    assert!(codes(&v).contains(&"fail:unused".to_string()), "{v}");
    let (_, s) = cli_json(&f.root(), &["skills", "explain", "alpha"]);
    assert_eq!(s["standing"], "orphan");

    // and no test names it
    f.write(
        ".ai/repo/workflows/uses-alpha.md",
        "# Uses alpha\n\nmajordomus://skill/alpha\n",
    );
    f.write("test/cases/01_alpha.sh", ". \"$ROOT/test/lib.sh\"\ntrue\n");
    f.commit("the marker goes");
    let (code, v) = verify(&f);
    assert_eq!(code, 10);
    assert_eq!(
        codes(&v)
            .iter()
            .filter(|c| c.starts_with("fail:"))
            .collect::<Vec<_>>(),
        ["fail:untested"],
        "{v}"
    );

    // a marker written as data, not on a comment line, binds nothing
    f.write(
        "test/cases/01_alpha.sh",
        &format!("cat > fixture.sh <<'FIXTURE'\n{MARKER} alpha\nFIXTURE\n"),
    );
    f.commit("a marker as data");
    let (code, _) = verify(&f);
    assert_eq!(
        code, 10,
        "a string that contains the marker is not a binding"
    );

    // a test and an invocation naming a skill that does not exist
    f.write(
        "test/cases/01_alpha.sh",
        &format!("# {MARKER} alpha ghost\ntrue\n"),
    );
    f.write(
        ".ai/repo/workflows/uses-alpha.md",
        "# Uses alpha\n\nmajordomus://skill/alpha, then majordomus://skill/phantom\n",
    );
    f.commit("bindings to nothing");
    let (code, v) = verify(&f);
    assert_eq!(code, 10);
    let c = codes(&v);
    assert!(c.contains(&"fail:unknown_skill_in_test".to_string()), "{v}");
    assert!(c.contains(&"fail:unknown_skill_invoked".to_string()), "{v}");
    assert!(!c.contains(&"fail:untested".to_string()), "{v}");

    // a draft owes nothing
    f.write(
        ".ai/repo/skills/beta/SKILL.md",
        &skill_file("beta", "draft", ""),
    );
    // a provenance marker that is not the opaque form, and a missing required field
    f.write(
        ".ai/repo/skills/gamma/SKILL.md",
        &skill_file(
            "gamma",
            "draft",
            "provenance:\n  origin: prior-art\n  ledger: some/private/path.md\n  decision: copied\n",
        )
        .replace("title: Skill gamma\n", ""),
    );
    f.write(
        "test/cases/01_alpha.sh",
        &format!("# {MARKER} alpha\ntrue\n"),
    );
    f.write(
        ".ai/repo/workflows/uses-alpha.md",
        "majordomus://skill/alpha\n",
    );
    f.commit("a draft, and a broken contract");
    let (code, v) = verify(&f);
    assert_eq!(code, 10, "{v}");
    let contract: Vec<&Value> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["code"] == "contract")
        .collect();
    assert!(
        contract.iter().all(|x| x["skill"] == "gamma"),
        "only gamma breaks the contract: {v}"
    );
    let text: String = contract
        .iter()
        .map(|x| x["message"].as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("ledger"),
        "the ledger pattern is named: {text}"
    );
    assert!(text.contains("title"), "the missing title is named: {text}");
    let (_, s) = cli_json(&f.root(), &["skills", "explain", "beta"]);
    assert_eq!(s["standing"], "not_required");
    assert_eq!(s["findings"], json!([]));
    // a file the index refused is still listed, as invalid, so the listing never shrinks
    let (_, s) = cli_json(&f.root(), &["skills", "explain", "gamma"]);
    assert_eq!(s["standing"], "invalid");

    // two files claiming one id: both are refused, and both are named
    f.remove(".ai/repo/skills/gamma/SKILL.md");
    f.write(
        ".ai/repo/skills/twin/SKILL.md",
        &skill_file("beta", "draft", "").replace("The beta procedure.", "Another."),
    );
    f.commit("a duplicate id");
    let (code, v) = verify(&f);
    assert_eq!(code, 10, "{v}");
    let duplicated: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["code"] == "contract")
        .map(|x| x["subject"].as_str().unwrap())
        .collect();
    assert_eq!(
        duplicated,
        [
            ".ai/repo/skills/beta/SKILL.md",
            ".ai/repo/skills/twin/SKILL.md"
        ],
        "{v}"
    );
}

#[test]
fn every_active_skill_of_this_repository_is_a_valid_invoked_capability_this_file_tests() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = root.canonicalize().unwrap();
    let (_, v) = cli_json(&root, &["skills", "status"]);
    let this = "apps/majordomus-cli/tests/skills.rs";
    let named: Vec<String> = std::fs::read_to_string(root.join(this))
        .unwrap()
        .lines()
        .flat_map(majordomus_cli::skill::marker_ids)
        .collect();
    assert!(!named.is_empty(), "this file names the skills it proves");
    for id in &named {
        let s = v["skills"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["id"] == id.as_str())
            .unwrap_or_else(|| {
                panic!("this file names skill '{id}', which the repository does not hold")
            });
        assert_eq!(s["valid"], true, "{id} breaks the skill contract: {s}");
        assert_eq!(s["used"]["used"], true, "nothing invokes {id}: {s}");
        assert!(
            s["tested"]["tests"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["path"] == this),
            "{id} does not count this file among its tests: {s}"
        );
    }
    for s in v["skills"].as_array().unwrap() {
        if s["status"] == "active" {
            assert!(
                named.iter().any(|n| s["id"] == n.as_str()),
                "active skill {} is not named by this file",
                s["id"]
            );
        }
    }
}
