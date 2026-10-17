//! A JSON field of the UI is an editor (I2810).
//!
//! The editor is the section of `share/ui/ui.js` between `editor:begin` and `editor:end`. Its
//! promise is that formatting re-indents tokens and never re-writes a value, so the text a
//! reader formats is the text that is computed. That is held here with the code itself: the
//! section is run by node over every case of the portable core's shared fixture, and the
//! digest of each formatted text — computed by the portable core — must be the fixture's.
//! Node is required, as the portable-core gate requires it: a run without it fails, it never
//! passes by skipping.
//!
//! The page is held too: /ui/compute renders its input as a `data-editor="json"` field, and
//! the script and stylesheet that make it an editor are what the UI serves.
//!
//! They prove the criterion
//! `a-browser-computes-what-the-server-verifies#the-reader-writes-json-in-an-editor`.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Fixture, Served};
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root")
}

/// The editor's section of ui.js, as the browser loads it.
fn editor_source() -> String {
    let script = std::fs::read_to_string(repo().join("share/ui/ui.js")).expect("ui.js");
    let start = script
        .find("// editor:begin")
        .expect("ui.js marks where the editor begins");
    let end = script
        .find("// editor:end")
        .expect("ui.js marks where the editor ends");
    script[start..end].to_string()
}

fn fixture() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            repo().join("apps/majordomus-core/fixtures/canonical-digest.v1.json"),
        )
        .expect("the shared fixture"),
    )
    .expect("the fixture is JSON")
}

/// Run `body` under node with the editor's functions in scope and `inputs` as `INPUTS`;
/// answer what it prints, as JSON.
fn node(inputs: &Value, body: &str) -> Value {
    let program = format!("{}\nconst INPUTS = {};\n{}", editor_source(), inputs, body);
    let out = Command::new("node")
        .args(["--input-type=module", "-e", &program])
        .output()
        .expect("node runs the editor's own code; it is required, as the portable-core gate requires it");
    assert!(
        out.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("node answers JSON")
}

#[test]
fn formatting_any_fixture_case_keeps_its_digest() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases");
    let inputs: Vec<&str> = cases
        .iter()
        .map(|c| c["input"].as_str().expect("an input"))
        .collect();
    let answer = node(
        &serde_json::json!(inputs),
        "console.log(JSON.stringify(INPUTS.map((t) => ({ \
           formatted: formatted(t), \
           rejoined: jsonTokens(t).map((x) => x.text).join(''), \
           ok: judge(t).ok }))));",
    );
    for (case, out) in cases
        .iter()
        .zip(answer.as_array().expect("one answer per case"))
    {
        let id = case["id"].as_str().expect("an id");
        let input = case["input"].as_str().expect("an input");
        assert_eq!(out["rejoined"], input, "{id}: tokenising lost a character");
        assert_eq!(
            out["ok"], true,
            "{id}: the editor judged valid JSON invalid"
        );
        let formatted = out["formatted"].as_str().expect("a formatted text");
        let digest = majordomus_core::digest_of_json_text(formatted)
            .unwrap_or_else(|e| panic!("{id}: the formatted text is not JSON: {e}\n{formatted}"));
        assert_eq!(
            Some(digest.as_str()),
            case["digest"].as_str(),
            "{id}: formatting changed what is computed:\n{formatted}"
        );
    }
}

#[test]
fn a_syntax_error_is_named_with_its_line_and_column() {
    let fixture = fixture();
    let invalid: Vec<&str> = fixture["invalid"]
        .as_array()
        .expect("invalid rows")
        .iter()
        .map(|c| c["input"].as_str().expect("an input"))
        .collect();
    let mut inputs = invalid.clone();
    inputs.push("{\n  \"a\": 1,\n  \"b\": ]\n}");
    let answer = node(
        &serde_json::json!(inputs),
        "console.log(JSON.stringify(INPUTS.map((t) => judge(t))));",
    );
    let verdicts = answer.as_array().expect("one verdict per input");
    for verdict in verdicts {
        assert_eq!(verdict["ok"], false, "{verdict}");
    }
    let last = verdicts.last().expect("the multi-line case")["message"]
        .as_str()
        .expect("a message")
        .to_string();
    assert!(
        last.contains("line 3"),
        "the error on the third line is not placed there: {last}"
    );
}

#[test]
fn the_compute_page_renders_an_editor_and_the_ui_serves_what_makes_it_one() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, _, page) = s.request("GET", "/ui/compute", None);
    assert_eq!(status, 200, "{page}");
    let area = page
        .split("<textarea")
        .skip(1)
        .find(|tag| tag.contains("mj-ui-compute-input"))
        .and_then(|tag| tag.split('>').next())
        .expect("the compute input");
    assert!(area.contains("data-editor=\"json\""), "<textarea{area}>");

    let script = page
        .split("src=\"")
        .find_map(|rest| {
            rest.split('"')
                .next()
                .filter(|u| u.starts_with("/ui/assets/ui.js"))
        })
        .expect("the page loads ui.js");
    let (status, _, ui_js) = s.request("GET", script, None);
    assert_eq!(status, 200);
    assert!(
        ui_js.contains("function enhanceEditors"),
        "ui.js holds no editor"
    );
    let sheet = page
        .split("href=\"")
        .find_map(|rest| {
            rest.split('"')
                .next()
                .filter(|u| u.starts_with("/ui/assets/ui.css"))
        })
        .expect("the page links ui.css");
    let (status, _, css) = s.request("GET", sheet, None);
    assert_eq!(status, 200);
    assert!(css.contains(".mj-ui-editor"), "ui.css styles no editor");
}
