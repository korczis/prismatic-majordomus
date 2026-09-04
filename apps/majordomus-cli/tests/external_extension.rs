//! The extensibility proof, end to end, through the built binary: a new object of a known
//! kind appears with no Rust change, disappears when removed, and a broken one is named.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{rule, Fixture, BIN};
use serde_json::{json, Value};

/// Ask a fresh server for its resource list and the repository diagnostics.
fn observe(f: &Fixture) -> (Vec<String>, Vec<Value>) {
    let mut child = Command::new(BIN)
        .arg("mcp")
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" })
        )
        .unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "majordomus_repository", "arguments": {} } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let uris = frames[1]["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap().to_string())
        .collect();
    let diags = frames[2]["result"]["structuredContent"]["diagnostics"]
        .as_array()
        .unwrap()
        .clone();
    (uris, diags)
}

#[test]
fn an_object_added_to_the_repository_appears_disappears_and_a_broken_one_is_named() {
    let f = Fixture::new();
    const A: &str = "majordomus://rule/project.alpha@1";
    const B: &str = "majordomus://rule/project.beta@1";
    const P: &str = "majordomus://prompt/review";
    const D: &str = "majordomus://document/docs/NEW.md";

    // 1. the built server sees A and nothing else of the kind
    let (uris, diags) = observe(&f);
    assert!(uris.contains(&A.to_string()));
    assert!(!uris.contains(&B.to_string()));
    assert!(diags.is_empty());

    // 2. add B (a rule), a prompt and a document: data only, no Rust, no registration
    f.write(
        ".ai/repo/rules/project/beta.v1.md",
        &rule("project.beta", 1, "Beta"),
    );
    f.write(
        ".ai/repo/prompts/review.md",
        "---\nname: review\ndescription: review a change\n---\n\nReview it.\n",
    );
    f.write("docs/NEW.md", "# New document\n");
    f.commit("add");
    let (uris, diags) = observe(&f);
    for u in [A, B, P, D] {
        assert!(
            uris.contains(&u.to_string()),
            "{u} missing after restart: {uris:?}"
        );
    }
    assert!(diags.is_empty());

    // 3. provenance of the new object is its own
    let (_, out, _) = common::run_in(&f.root(), &["mcp", "--inspect", "--format", "json"], "");
    let v: Value = serde_json::from_str(&out).unwrap();
    let beta = v["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["uri"] == B)
        .unwrap();
    assert_eq!(
        beta["meta"]["provenance"]["path"],
        ".ai/repo/rules/project/beta.v1.md"
    );
    assert_eq!(beta["meta"]["provenance"]["source_class"], "rule");
    assert_eq!(beta["meta"]["identity"], "project.beta@1");

    // 4. remove B: gone after restart, A stays
    f.remove(".ai/repo/rules/project/beta.v1.md");
    f.commit("remove");
    let (uris, _) = observe(&f);
    assert!(uris.contains(&A.to_string()));
    assert!(!uris.contains(&B.to_string()), "{uris:?}");

    // 5. a broken C is named, excluded, and the rest still serves
    f.write(
        ".ai/repo/rules/project/gamma.v1.md",
        &rule("project.gamma", 1, "Gamma")
            .replace("class: advisory", "class: advisory\nseverity: high"),
    );
    f.commit("broken");
    let (uris, diags) = observe(&f);
    assert!(uris.contains(&A.to_string()));
    assert!(!uris.iter().any(|u| u.contains("gamma")));
    let d = diags
        .iter()
        .find(|d| d["path"] == ".ai/repo/rules/project/gamma.v1.md")
        .expect("gamma is named");
    assert_eq!(d["code"], "unknown_key");
    assert_eq!(d["severity"], "error");
    assert!(d["message"].as_str().unwrap().contains("severity"));
}

#[test]
fn a_new_source_class_of_a_known_kind_is_data_too() {
    // The repository can declare a new class in sources.yaml — say, decisions kept as
    // documents under .ai/repo/adrs/ — and the executable serves it without a change.
    let f = Fixture::new();
    let sources = common::SOURCES.replace(
        "  - id: readme\n    kind: document",
        "  - id: adr\n    kind: document\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/adrs/*.md'\n    required: false\n\n  - id: readme\n    kind: document",
    );
    f.write(".ai/repo/knowledge/sources.yaml", &sources);
    f.write(
        ".ai/repo/adrs/0001-example.md",
        "# 1. Example decision\n\nContext.\n",
    );
    f.commit("adr class");
    let (uris, diags) = observe(&f);
    assert!(diags.is_empty(), "{diags:?}");
    assert!(
        uris.contains(&"majordomus://document/.ai/repo/adrs/0001-example.md".to_string()),
        "{uris:?}"
    );
    let (_, out, _) = common::run_in(&f.root(), &["mcp", "--inspect", "--format", "json"], "");
    let v: Value = serde_json::from_str(&out).unwrap();
    let adr = v["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["uri"].as_str().unwrap().contains("adrs"))
        .unwrap();
    assert_eq!(adr["meta"]["provenance"]["source_class"], "adr");
    assert_eq!(adr["meta"]["provenance"]["section"], "adrs");
    assert_eq!(adr["title"], "1. Example decision");
}
