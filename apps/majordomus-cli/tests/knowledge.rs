//! The repository knowledge system, end to end: a brownfield repository adopted with its
//! debt recorded, a curated claim that goes stale when its evidence moves, a contradiction
//! that stays an open conflict until a person accepts it, the check that refuses new debt
//! and passes the recorded, the canonicality audit finding an orphan projection, a
//! hand-kept mirror and an expired exception, the public projection carrying nothing
//! restricted, the same answers over MCP and HTTP, a scan that is deterministic and
//! bounded, a baseline of an older schema migrated and one of a newer refused, and two
//! worktrees of one repository judged apart.
//!
//! Every test builds its own repository (`Fixture::brownfield`) and never reads the
//! developer's checkout.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{dist_share, run_in, Fixture, Served, BIN};
use serde_json::{json, Value};

fn json_of(f: &Fixture, args: &[&str]) -> Value {
    let (code, out, err) = run_in(&f.root(), args, "");
    assert_eq!(code, 0, "{args:?} failed:\n{err}\n{out}");
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("{args:?} printed no JSON ({e}):\n{out}"))
}

fn exit_of(f: &Fixture, args: &[&str]) -> (i32, String, String) {
    run_in(&f.root(), args, "")
}

#[test]
fn a_brownfield_repository_is_adopted_with_its_debt_recorded_and_the_ratchet_holds() {
    let f = Fixture::brownfield();

    // before adoption: the curated record is unverified, the contradiction is open, the
    // link to nowhere is a gap, and the check refuses all of it
    let status = json_of(&f, &["knowledge", "status", "--format", "json"]);
    assert_eq!(status["schema"], "majordomus/knowledge/v1");
    assert!(status["nodes"].as_u64().unwrap() > 10);
    assert_eq!(status["baseline"]["recorded"], false);
    assert_eq!(status["check"]["verdict"], "fail");
    let conflicts = json_of(&f, &["knowledge", "conflicts", "--format", "json"]);
    assert_eq!(conflicts["open"], 1, "{conflicts}");
    let c = &conflicts["conflicts"][0];
    assert_eq!(c["id"], "component:alpha#version");
    assert_eq!(c["resolution"], "open");
    assert_eq!(c["severity"], "high");
    let says: Vec<&str> = c["sides"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["value"].as_str().unwrap())
        .collect();
    assert!(says.contains(&"1.2.3") && says.contains(&"2.0.0"), "{says:?}");
    let stale = json_of(&f, &["knowledge", "stale", "--format", "json"]);
    let ids: Vec<&str> = stale["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"knowledge:alpha-version"), "{ids:?}");
    assert!(ids.contains(&"document:docs/ALPHA.md"), "{ids:?}");
    let gaps = json_of(&f, &["knowledge", "gaps", "--format", "json"]);
    assert!(
        gaps["gaps"].as_array().unwrap().iter().any(|g| g["category"] == "unresolved_reference" && g["reason"].as_str().unwrap().contains("runbooks/alpha.md")),
        "{gaps}"
    );
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("no baseline recorded"), "{out}");

    // adoption: one command, the baseline written, the check passes, the conflict is
    // accepted by that act with the reason saying so
    let (code, out, err) = exit_of(&f, &["knowledge", "bootstrap"]);
    assert_eq!(code, 0, "{err}\n{out}");
    assert!(out.contains("baseline recorded at .ai/repo/knowledge/baseline.yaml"), "{out}");
    assert!(f.path(".ai/repo/knowledge/baseline.yaml").is_file());
    let baseline = std::fs::read_to_string(f.path(".ai/repo/knowledge/baseline.yaml")).unwrap();
    assert!(baseline.starts_with("# The knowledge baseline"), "{baseline}");
    assert!(baseline.contains("schema: majordomus/knowledge-baseline/v1"));
    assert!(baseline.contains("component:alpha#version"), "the conflict is accepted:\n{baseline}");
    assert!(!baseline.contains(&f.root().display().to_string()), "no machine path in a tracked file");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("knowledge check: pass"), "{out}");
    let (code, _, _) = exit_of(&f, &["knowledge", "bootstrap"]);
    assert_eq!(code, 10, "a second bootstrap refuses to overwrite");
    let conflicts = json_of(&f, &["knowledge", "conflicts", "--format", "json"]);
    assert_eq!(conflicts["conflicts"][0]["resolution"], "accepted");

    // the evidence moves: the curated record verified against the manifest is stale, the
    // check refuses it by name, and nothing else regressed
    f.write(
        "apps/alpha/Cargo.toml",
        "[package]\nname = \"alpha\"\nversion = \"1.3.0\"\nedition = \"2021\"\nlicense = \"MIT\"\n\n[dependencies]\nserde = \"1\"\n",
    );
    f.commit("bump alpha");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("knowledge:alpha-version"), "{out}");
    assert!(out.contains("stale"), "{out}");
    let explain = json_of(&f, &["knowledge", "explain", "knowledge:alpha-version", "--format", "json"]);
    assert_eq!(explain["provenance"], "curated");
    assert_eq!(explain["freshness"], "stale");
    assert!(explain["freshness_reason"].as_str().unwrap().contains("changed since this was verified"), "{explain}");
    assert!(explain["remedies"].as_array().unwrap().iter().any(|r| r.as_str().unwrap().contains("reconcile")));
    let impact = json_of(&f, &["knowledge", "impact", "apps/alpha/Cargo.toml", "--format", "json"]);
    assert!(impact["direct"].as_array().unwrap().iter().any(|a| a["id"] == "component:alpha"), "{impact}");
    assert!(impact["direct"].as_array().unwrap().iter().any(|a| a["id"] == "knowledge:alpha-version"), "{impact}");

    // a person checks the record and accepts: the baseline is rewritten, the check passes
    let (code, out, _) = exit_of(&f, &["knowledge", "reconcile"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("reverify"), "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "reconcile", "--accept"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("baseline written"), "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 0, "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "stale"]);
    assert_eq!(code, 0);
    assert!(!out.contains("knowledge:alpha-version"), "{out}");

    // a new contradiction is new debt until a person accepts it by name with a reason
    f.write(
        ".ai/repo/knowledge/curated/alpha-license.md",
        "---\nschema: knowledge/v1\nid: alpha-license\nkind: knowledge\nclass: fact\ntitle: alpha is Apache licensed\ndescription: The licence the legal review recorded.\nstatus: candidate\nepistemics: observed\ndate: 2026-02-01\nprovenance:\n  origin: authored\nasserts:\n  - subject: component:alpha\n    predicate: license\n    value: Apache-2.0\n---\n\n# Licence\n\nRecorded at the legal review.\n",
    );
    f.commit("a contradicting note");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("component:alpha#license"), "{out}");
    let (code, _, err) = exit_of(&f, &["knowledge", "accept", "component:alpha#nothing", "--reason", "x"]);
    assert_eq!(code, 12, "{err}");
    assert!(err.contains("component:alpha#license"), "the open ones are named: {err}");
    let (code, out, _) = exit_of(&f, &["knowledge", "accept", "component:alpha#license", "--reason", "the manifest says MIT for the crate and legal says Apache for the product"]);
    assert_eq!(code, 0, "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    // the new record's own content claims are unverified until accepted through reconcile
    let (code2, _, _) = exit_of(&f, &["knowledge", "reconcile", "--accept"]);
    assert_eq!(code2, 0);
    let (code3, out3, _) = exit_of(&f, &["knowledge", "check"]);
    assert!(code == 0 || code3 == 0, "after accepting the conflict and verifying the note the check passes:\n{out}\n{out3}");
    assert_eq!(code3, 0, "{out3}");
}

#[test]
fn a_scan_is_deterministic_bounded_and_the_public_projection_carries_nothing_restricted() {
    let f = Fixture::brownfield();
    let started = std::time::Instant::now();
    let a = json_of(&f, &["knowledge", "scan"]);
    let elapsed = started.elapsed();
    let b = json_of(&f, &["knowledge", "scan"]);
    assert_eq!(a["fingerprint"], b["fingerprint"]);
    assert_eq!(a, b, "two scans of one tree are one document");
    assert!(
        elapsed < std::time::Duration::from_secs(20),
        "a scan of the fixture took {elapsed:?}; the bound is generous for a cold debug build and still a bound"
    );
    let text = a.to_string();
    assert!(text.contains("knowledge:alpha-secret"), "the whole model holds the restricted record");
    assert!(!text.contains("PARTNER-ZETA"), "no node carries a record's prose");

    let public = json_of(&f, &["knowledge", "scan", "--public"]);
    let text = public.to_string();
    assert!(!text.contains("alpha-secret"), "the public projection drops the restricted node:\n{text}");
    assert!(!text.contains("PARTNER-ZETA"));
    assert!(text.contains("component:alpha"), "and keeps the public one");
    for e in public["evidence"].as_array().unwrap() {
        assert_eq!(e["visibility"], "public", "{e}");
        assert_eq!(e["remote_processing"], true, "{e}");
    }
    // the same rule on the way to a provider: the dry run withholds what may not leave
    let (code, _, err) = exit_of(&f, &["knowledge", "derive", "--dry-run"]);
    assert_eq!(code, 10, "off until the policy says otherwise: {err}");
    assert!(err.contains("knowledge.semantic.enabled"), "{err}");
    let policy = std::fs::read_to_string(f.path(".ai/repo/policy.yaml")).unwrap();
    f.write(
        ".ai/repo/policy.yaml",
        &format!("{policy}\nknowledge:\n  mode: protect\n  semantic:\n    enabled: true\n    allow_remote: false\n    provider: local\n"),
    );
    let (code, out, err) = exit_of(&f, &["knowledge", "derive", "--kind", "document", "--dry-run"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("provider local"), "{out}");
    assert!(out.contains("nothing was run"), "{out}");
    let (code, out, err) = exit_of(&f, &["knowledge", "derive", "--kind", "document"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("derivation(s) written to .ai/local/knowledge/semantic-cache.json"), "{out}");
    let cache = std::fs::read_to_string(f.path(".ai/local/knowledge/semantic-cache.json")).unwrap();
    assert!(cache.contains("\"schema\": \"majordomus/semantic-cache/v1\""));
    assert!(cache.contains("\"provider\": \"local\""));
    // the next scan reads the cache as derived knowledge, possibly stale until verified
    let after = json_of(&f, &["knowledge", "status", "--format", "json"]);
    assert!(after["semantic"]["derived_claims"].as_u64().unwrap() > 0, "{after}");
    let readme = json_of(&f, &["knowledge", "show", "document:README.md", "--format", "json"]);
    assert!(
        readme["node"]["claims"].as_array().unwrap().iter().any(|c| c["provenance"] == "derived" && c["predicate"] == "summary"),
        "{readme}"
    );
    // a remote provider is refused even when enabled, unless allowed
    f.write(
        ".ai/repo/policy.yaml",
        &format!("{policy}\nknowledge:\n  semantic:\n    enabled: true\n    provider: cloud\n"),
    );
    let (code, _, err) = exit_of(&f, &["knowledge", "derive", "--dry-run"]);
    assert_eq!(code, 12, "{err}");
    assert!(err.contains("no semantic provider `cloud`"), "{err}");
}

#[test]
fn the_canonicality_audit_names_an_orphan_a_mirror_and_an_expired_exception() {
    let f = Fixture::new();
    let (code, out, _) = exit_of(&f, &["canonicality"]);
    assert_eq!(code, 0, "a fixture with no generated tree and no mirror passes:\n{out}");
    assert!(out.contains("MMS") && out.contains("verdict: pass"), "{out}");

    // a generated artifact the manifest declares with no source it was derived from
    f.write("docs/generated/orphan.json", "{\"schema\":\"x/v1\",\"generated\":\"GENERATED FILE\"}\n");
    f.write(
        "docs/generated/artifacts.json",
        &json!({
            "schema": "majordomus/generated-artifacts/v1",
            "generated": "GENERATED FILE",
            "generator": "majordomus-cli test",
            "documents": [{"id": "orphan", "source": "nothing in particular", "formats": ["json"]}],
            "artifacts": [{"path": "docs/generated/orphan.json", "document": "orphan", "format": "json", "source": "nothing in particular", "bytes": 1, "sha256": "0000000000000000000000000000000000000000000000000000000000000000"}]
        })
        .to_string(),
    );
    // and a document that lists capability identifiers by hand
    f.write(
        "docs/TOOLS.md",
        "# Tools\n\n| id | what |\n|---|---|\n| `objects.get` | one |\n| `objects.list` | two |\n| `objects.search` | three |\n| `capabilities.list` | four |\n| `capabilities.describe` | five |\n| `health.report` | six |\n",
    );
    f.commit("an orphan and a mirror");
    let (code, out, _) = exit_of(&f, &["canonicality", "check"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("orphan_projection") && out.contains("docs/generated/orphan.json"), "{out}");
    assert!(out.contains("suspected_mirror") && out.contains("docs/TOOLS.md"), "{out}");
    let audit = json_of(&f, &["canonicality", "check", "--format", "json"]);
    assert_eq!(audit["verdict"], "fail");
    let row = audit["capabilities"].as_array().unwrap().iter().find(|c| c["id"] == "objects.get").unwrap();
    assert!(row["mms"].as_u64().unwrap() >= 2, "the mirror counts against the capability: {row}");
    assert!(row["mentions"].as_array().unwrap().iter().any(|m| m["path"] == "docs/TOOLS.md"));
    assert!(row["surfaces"].as_array().unwrap().iter().all(|s| s["derived"] == true));

    // an exception with a date covers a violation; an expired one is a violation itself
    f.write(
        ".ai/repo/knowledge/exceptions.yaml",
        "schema: majordomus/knowledge-exceptions/v1\nexceptions:\n  - id: orphan_projection:docs/generated/orphan.json\n    reason: the target gains derived_from in the next release\n    owner: the maintainers\n    expires: 2999-01-01\n    validation: the artifact names a source in docs/generated/artifacts.json\n  - id: suspected_mirror:docs/TOOLS.md\n    reason: the table is being derived\n    owner: the maintainers\n    expires: 2999-01-01\n    validation: docs/TOOLS.md names no id by hand\n",
    );
    let (code, out, _) = exit_of(&f, &["canonicality", "check"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("excepted by"), "{out}");
    f.write(
        ".ai/repo/knowledge/exceptions.yaml",
        "schema: majordomus/knowledge-exceptions/v1\nexceptions:\n  - id: orphan_projection:docs/generated/orphan.json\n    reason: expired long ago\n    owner: the maintainers\n    expires: 2001-01-01\n    validation: never\n",
    );
    let (code, out, _) = exit_of(&f, &["canonicality", "check"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("expired_exception"), "{out}");
    // an exception file that breaks its contract is refused, not skipped
    f.write(".ai/repo/knowledge/exceptions.yaml", "schema: majordomus/knowledge-exceptions/v1\nexceptions:\n  - id: x\n    reason: r\n    owner: o\n    expires: tomorrow\n    validation: v\n");
    let (code, _, err) = exit_of(&f, &["knowledge", "validate"]);
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("knowledge validate") || err.contains("exceptions"), "{err}");
    f.remove(".ai/repo/knowledge/exceptions.yaml");

    // the baseline tolerates what the repository had; the ratchet refuses more
    let (code, out, _) = exit_of(&f, &["knowledge", "bootstrap"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("tolerated canonicality"), "{out}");
    let (code, out, _) = exit_of(&f, &["canonicality", "check"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("tolerated"), "{out}");
    f.write("docs/TOOLS2.md", "# More\n\n- `objects.get`\n- `objects.list`\n- `objects.search`\n- `capabilities.list`\n- `capabilities.describe`\n- `health.report`\n");
    f.commit("a second mirror");
    let (code, out, _) = exit_of(&f, &["canonicality", "check"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("docs/TOOLS2.md"), "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 10, "the knowledge check carries the same violation:\n{out}");
    assert!(out.contains("canonicality:suspected_mirror:docs/TOOLS2.md"), "{out}");
    // and the inspection of the change names it
    let inspect = json_of(&f, &["change", "inspect", "--base", "HEAD~1", "--format", "json"]);
    assert_eq!(inspect["verdict"], "fail");
    assert!(inspect["impact"]["changed"].as_array().unwrap().iter().any(|c| c["path"] == "docs/TOOLS2.md"), "{inspect}");
    assert_eq!(inspect["added_capabilities"].as_array().unwrap().len(), 0);
    // removing the mirror resolves the debt and the check says the baseline can shrink
    f.remove("docs/TOOLS2.md");
    f.remove("docs/TOOLS.md");
    f.commit("derived instead");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("resolved") && out.contains("record the baseline again"), "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "baseline", "record", "--force"]);
    assert_eq!(code, 0, "{out}");
    let baseline = std::fs::read_to_string(f.path(".ai/repo/knowledge/baseline.yaml")).unwrap();
    assert!(!baseline.contains("docs/TOOLS.md"), "the baseline shrank:\n{baseline}");
}

#[test]
fn every_capability_explains_its_canonical_source_and_derived_surfaces() {
    let f = Fixture::new();
    let (code, out, _) = exit_of(&f, &["explain", "capability", "objects.get"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("capability:objects.get"), "{out}");
    assert!(out.contains("canonical source: apps/majordomus-cli/src/capability/builtin/objects.rs"), "{out}");
    for surface in ["mcp", "http", "openapi", "docs", "cockpit", "benchmark"] {
        assert!(out.contains(&format!("✓ {surface}")), "{surface} is a derived surface:\n{out}");
    }
    assert!(out.contains("manual maintenance surface"), "{out}");
    let (code, out, _) = exit_of(&f, &["canonicality", "explain", "objects.get"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("canonical source:"), "{out}");
    // a subject that is a path, an object URI or a node id resolves the same way
    for subject in ["README.md", "document:README.md", "majordomus://rule/project.alpha@1", "rule:project.alpha@1"] {
        let (code, out, err) = exit_of(&f, &["explain", subject]);
        assert_eq!(code, 0, "{subject}: {err}");
        assert!(out.contains("evidence"), "{subject}:\n{out}");
    }
    let (code, _, err) = exit_of(&f, &["explain", "nothing:here"]);
    assert_eq!(code, 12, "{err}");
    assert!(err.contains("no knowledge node"), "{err}");
}

#[test]
fn the_same_scan_answers_over_http_mcp_and_the_cockpit_and_the_health_report_carries_it() {
    let f = Fixture::brownfield();
    let (code, _, _) = exit_of(&f, &["knowledge", "bootstrap"]);
    assert_eq!(code, 0);
    f.commit("baseline");
    let s = Served::start(&f.root(), &[]);

    let (status, body) = s.get("/api/v1/knowledge");
    assert_eq!(status, 200);
    assert_eq!(body["schema"], "majordomus/knowledge/v1");
    assert_eq!(body["check"]["verdict"], "pass");
    assert_eq!(body["baseline"]["recorded"], true);
    let (status, explain) = s.get("/api/v1/knowledge/explain?id=component:alpha");
    assert_eq!(status, 200, "{explain}");
    assert_eq!(explain["provenance"], "observed");
    assert!(explain["why"][0].as_str().unwrap().contains("read off the tree"), "{explain}");
    let (status, audit) = s.get("/api/v1/canonicality");
    assert_eq!(status, 200);
    assert!(audit["capabilities"].as_array().unwrap().iter().any(|c| c["id"] == "rks.status"));
    let (status, graph) = s.get("/api/v1/knowledge/graph?root=component:alpha&depth=1");
    assert_eq!(status, 200, "{graph}");
    assert!(graph["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "component:alpha"));
    assert!(graph["nodes"].as_array().unwrap().iter().all(|n| n["route"].as_str().unwrap().starts_with("/cockpit/knowledge/node?id=")));
    let (status, missing) = s.get("/api/v1/knowledge/node?id=nothing:here");
    assert_eq!(status, 404, "{missing}");
    let (status, context) = s.get("/api/v1/knowledge/context?paths=apps/alpha&public_only=true");
    assert_eq!(status, 200, "{context}");
    assert!(context["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "component:alpha"), "{context}");
    assert!(!context.to_string().contains("alpha-secret"));
    let (status, health) = s.get("/api/v1/health");
    assert_eq!(status, 200);
    let checks: Vec<&str> = health["checks"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(checks.contains(&"knowledge") && checks.contains(&"canonicality"), "{checks:?}");
    let knowledge = health["checks"].as_array().unwrap().iter().find(|c| c["id"] == "knowledge").unwrap();
    assert_eq!(knowledge["status"], "ok", "{knowledge}");
    assert_eq!(knowledge["evidence"][0], "majordomus knowledge check");

    // the Cockpit: the node page names why, and the integrity page names the source
    let (status, _, page) = s.request("GET", "/cockpit/knowledge/node?id=knowledge:alpha-version", None);
    assert_eq!(status, 200);
    assert!(page.contains("Why the model holds this"), "{page}");
    assert!(page.contains("a person wrote"), "{page}");
    assert!(page.contains("Conflicts"), "the accepted conflict is still shown");
    let (status, _, page) = s.request("GET", "/cockpit/integrity/rks.status", None);
    assert_eq!(status, 200);
    assert!(page.contains("apps/majordomus-cli/src/capability/builtin/knowledge.rs"), "{page}");
    assert!(page.contains("derived"), "{page}");
    let (status, _, page) = s.request("GET", "/cockpit/knowledge/node?id=nothing:here", None);
    assert_eq!(status, 404, "{page}");
    // repository content reaches a page as text, never as markup
    assert!(!page.contains("<script>alert"), "{page}");

    // the same over MCP, on stdio
    let mut child = Command::new(BIN)
        .args(["mcp", "--standalone"])
        .env("MAJORDOMUS_SHARE", dist_share())
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn majordomus mcp");
    {
        let mut stdin = child.stdin.take().unwrap();
        for r in [
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } } }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "majordomus_knowledge_explain", "arguments": { "id": "component:alpha" } } }),
            json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "majordomus_knowledge_context", "arguments": { "paths": ["docs"], "public_only": true } } }),
            json!({ "jsonrpc": "2.0", "id": 5, "method": "resources/read", "params": { "uri": "majordomus://knowledge" } }),
        ] {
            writeln!(stdin, "{r}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let frames: Vec<Value> = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let by_id = |id: u64| frames.iter().find(|f| f["id"] == id).unwrap_or_else(|| panic!("no frame {id}"));
    let tools: Vec<&str> = by_id(2)["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for tool in ["majordomus_knowledge", "majordomus_knowledge_explain", "majordomus_knowledge_context", "majordomus_canonicality", "majordomus_change_inspect"] {
        assert!(tools.contains(&tool), "{tools:?}");
    }
    assert_eq!(by_id(3)["result"]["structuredContent"]["provenance"], "observed");
    let ctx = &by_id(4)["result"]["structuredContent"];
    assert_eq!(ctx["public_only"], true);
    assert!(!ctx.to_string().contains("alpha-secret"));
    let res = &by_id(5)["result"]["contents"][0]["text"];
    assert!(res.as_str().unwrap().contains("majordomus/knowledge/v1"), "{res}");
}

#[test]
fn a_baseline_of_an_older_schema_is_migrated_and_one_of_a_newer_schema_is_refused() {
    let f = Fixture::new();
    f.write(".ai/repo/knowledge/baseline.yaml", "tolerated:\n  - node: document:docs/CLI.md\n    freshness: unverified\n");
    let (code, out, _) = exit_of(&f, &["knowledge", "validate"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("knowledge_baseline_migration"), "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "baseline", "migrate"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("v0 -> v1"), "{out}");
    let text = std::fs::read_to_string(f.path(".ai/repo/knowledge/baseline.yaml")).unwrap();
    assert!(text.contains("schema: majordomus/knowledge-baseline/v1"), "{text}");
    assert!(text.contains("document:docs/CLI.md"), "{text}");
    let (code, out, _) = exit_of(&f, &["knowledge", "baseline", "migrate"]);
    assert_eq!(code, 0);
    assert!(out.contains("is current"), "{out}");
    f.write(".ai/repo/knowledge/baseline.yaml", "schema: majordomus/knowledge-baseline/v9\n");
    let (code, _, err) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("newer Majordomus"), "{err}");
    let (code, _, err) = exit_of(&f, &["knowledge", "status"]);
    assert_eq!(code, 10, "every read refuses a baseline it cannot read: {err}");
}

#[test]
fn two_worktrees_of_one_repository_are_judged_apart() {
    let f = Fixture::brownfield();
    let (code, _, _) = exit_of(&f, &["knowledge", "bootstrap"]);
    assert_eq!(code, 0);
    f.commit("baseline");
    let other = f.container().join("feature").join("other");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    f.git(&["worktree", "add", "-q", "-b", "feature/other", other.to_str().unwrap()]);
    // the worktree passes on the committed baseline
    let (code, out, _) = run_in(&other, &["knowledge", "check"], "");
    assert_eq!(code, 0, "{out}");
    // a change in the worktree is judged there and not in the primary checkout
    std::fs::write(
        other.join("apps/alpha/Cargo.toml"),
        "[package]\nname = \"alpha\"\nversion = \"9.9.9\"\nedition = \"2021\"\nlicense = \"MIT\"\n\n[dependencies]\nserde = \"1\"\n",
    )
    .unwrap();
    let (code, out, _) = run_in(&other, &["knowledge", "check"], "");
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("knowledge:alpha-version"), "{out}");
    let (code, out, _) = exit_of(&f, &["knowledge", "check"]);
    assert_eq!(code, 0, "the primary checkout is untouched:\n{out}");
    let status = json_of(&f, &["knowledge", "status", "--format", "json"]);
    let other_status: Value = {
        let (code, out, err) = run_in(&other, &["knowledge", "status", "--format", "json"], "");
        assert_eq!(code, 0, "{err}");
        serde_json::from_str(&out).unwrap()
    };
    assert_eq!(status["repository"]["branch"], "master");
    assert_eq!(other_status["repository"]["branch"], "feature/other");
    assert_ne!(status["fingerprint"], other_status["fingerprint"]);
    assert!(!other_status.to_string().contains(f.root().to_str().unwrap()), "no machine path in an answer");
}

#[test]
fn the_extractors_declare_their_vocabulary_and_nothing_else_names_a_kind() {
    let f = Fixture::new();
    let report = json_of(&f, &["knowledge", "extractors", "--format", "json"]);
    let ids: Vec<&str> = report["extractors"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["git", "layer", "cargo", "docs", "registry", "workflows", "semantic"]);
    for e in report["extractors"].as_array().unwrap() {
        assert_eq!(e["deterministic"], true, "{e}");
        assert_eq!(e["reads_sensitive"], false, "{e}");
    }
    let kinds: Vec<&str> = report["kinds"].as_array().unwrap().iter().map(|k| k["kind"].as_str().unwrap()).collect();
    for k in ["document", "rule", "capability", "artifact", "component", "file", "directory"] {
        assert!(kinds.contains(&k), "{kinds:?}");
    }
    let providers = report["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["id"], "local");
    assert_eq!(providers[0]["remote"], false);
    assert!(report["schemas"][0]["families"].as_array().unwrap().iter().any(|s| s == "majordomus/knowledge-baseline/v1"));
    // the model validates against its own contract
    let (code, out, _) = exit_of(&f, &["knowledge", "validate"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("0 error(s)"), "{out}");
}
