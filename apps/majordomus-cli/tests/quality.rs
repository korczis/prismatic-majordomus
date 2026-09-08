//! The quality gate against the crate it is about, and against fixtures built to break it.
//!
//! Two things are proved here and they are different. The first is the **ratchet**: this
//! crate, measured through the real registry and the real command tree, carries no finding
//! outside the recorded baseline — so the debt can shrink and cannot grow. The second is
//! that the validator **can still fail**, which a gate measured only against a passing tree
//! never demonstrates: one fixture crate per way of getting it wrong, each producing its own
//! code and no other.
//!
//! Nothing here enumerates the crate's modules, items or commands. Every expectation is
//! derived from the measurement itself, so a module added tomorrow is held to the same
//! contract without this file being edited.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use majordomus_cli::capability::builtin::quality::{baseline_key, baseline_keys, BASELINE};
use majordomus_cli::quality::{self, ViolationCode};
use majordomus_cli::{capability::model::CRATE_DIR, cli};

/// The crate this test is compiled inside.
fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The repository the crate sits in.
fn repo_root() -> std::path::PathBuf {
    crate_dir()
        .join("../..")
        .canonicalize()
        .expect("the repository above the crate")
}

fn measure() -> quality::QualityReport {
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    let openapi =
        majordomus_cli::http::openapi::document(app.registry(), "test", None).expect("openapi");
    quality::report::inspect(
        crate_dir(),
        CRATE_DIR,
        app.registry(),
        &cli::tree(),
        &openapi,
    )
    .expect("the crate measures")
}

// ---------------------------------------------------------------- the ratchet

#[test]
fn the_crate_carries_no_finding_the_baseline_does_not_already_accept() {
    let report = measure();
    let accepted = baseline_keys(&repo_root());
    let new: Vec<String> = report
        .violations
        .iter()
        .filter(|v| !accepted.contains(&baseline_key(v)))
        .map(|v| v.line_summary())
        .collect();
    assert!(
        new.is_empty(),
        "{} finding(s) outside {BASELINE}:\n{}\n\nFix them, or — only if they were already \
         there — run: majordomus quality report --write-baseline",
        new.len(),
        new.join("\n")
    );
}

#[test]
fn the_baseline_records_nothing_that_is_no_longer_a_finding() {
    // debt that has been paid must leave the file, or the ratchet stops being one: a stale
    // key would silently accept a finding that comes back later under the same key
    let report = measure();
    let found: BTreeSet<String> = report.violations.iter().map(baseline_key).collect();
    let stale: Vec<String> = baseline_keys(&repo_root())
        .into_iter()
        .filter(|k| !found.contains(k))
        .collect();
    assert!(
        stale.is_empty(),
        "{} baseline entr(ies) name a finding that no longer exists; run: \
         majordomus quality report --write-baseline\n{}",
        stale.len(),
        stale.join("\n")
    );
}

// ---------------------------------------------------------------- what the compiler already holds

#[test]
fn every_item_this_crate_exports_is_documented_because_the_compiler_refuses_otherwise() {
    let report = measure();
    assert_eq!(
        report.public_api.items, report.public_api.documented,
        "missing_docs is warned in lib.rs and warnings are denied in the gate"
    );
    assert!(report.of(ViolationCode::RustPublicMissingDocs).is_empty());
    assert_eq!(report.modules.modules, report.modules.documented);
}

// ---------------------------------------------------------------- parity

#[test]
fn every_runnable_command_is_accounted_for_exactly_once() {
    let report = measure();
    let o = &report.operations;
    assert_eq!(
        o.cli_from_capability + o.cli_local,
        o.cli_commands,
        "a command is the projection of a capability or is classified, and never both or neither"
    );
    for code in [
        ViolationCode::OperationCliUnclassified,
        ViolationCode::OperationClassificationStale,
        ViolationCode::OperationClassificationConflict,
    ] {
        assert!(
            report.of(code).is_empty(),
            "{}: {:?}",
            code.as_str(),
            report
                .of(code)
                .iter()
                .map(|v| &v.symbol)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn every_capability_that_declares_a_route_is_described_by_the_generated_document() {
    let report = measure();
    assert_eq!(report.operations.http, report.operations.openapi);
    assert!(report.of(ViolationCode::OperationMissingOpenapi).is_empty());
    assert!(report
        .of(ViolationCode::OperationProjectionOrphan)
        .is_empty());
}

// ---------------------------------------------------------------- the validator can still fail

/// A crate with one module, whose body is what the caller gives.
fn fixture(body: &str) -> tempfile::TempDir {
    const HEADER: &str = "//! A fixture crate whose header is long enough to be one: it says what the module\n\
                          //! owns, which is the single function below it and nothing whatever besides.\n\
                          //!\n\
                          //! ```\n\
                          //! assert_eq!(majordomus_cli::two(), 2);\n\
                          //! ```\n";
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("lib.rs"), format!("{HEADER}{body}")).unwrap();
    dir
}

const TESTED: &str =
    "#[cfg(test)]\nmod tests { #[test] fn t() { assert_eq!(super::two(), 2); } }\n";

fn codes(dir: &tempfile::TempDir) -> Vec<&'static str> {
    let report = quality::report::rust_only(dir.path(), "fixture").expect("measures");
    report
        .violations
        .iter()
        .map(|v| v.code.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn a_public_function_without_an_example_fails_the_gate() {
    let dir = fixture(&format!(
        "/// Answers with two, which is more than the signature promises.\npub fn two() -> usize {{ 2 }}\n{TESTED}"
    ));
    assert_eq!(codes(&dir), ["RUST_PUBLIC_MISSING_EXAMPLE"]);
}

#[test]
fn an_ignored_example_does_not_count_as_one() {
    let dir = fixture(&format!(
        "/// Answers with two, which is more than the signature promises.\n///\n/// ```ignore\n/// assert_eq!(two(), 2);\n/// ```\npub fn two() -> usize {{ 2 }}\n{TESTED}"
    ));
    assert_eq!(codes(&dir), ["RUST_EXAMPLE_NOT_EXECUTABLE"]);
}

#[test]
fn an_example_that_asserts_only_what_is_true_of_every_program_does_not_count() {
    let dir = fixture(&format!(
        "/// Answers with two, which is more than the signature promises.\n///\n/// ```\n/// let _ = majordomus_cli::two;\n/// assert!(true);\n/// ```\npub fn two() -> usize {{ 2 }}\n{TESTED}"
    ));
    assert_eq!(codes(&dir), ["RUST_EXAMPLE_PLACEHOLDER"]);
}

#[test]
fn an_example_that_never_names_the_item_it_documents_does_not_count() {
    let dir = fixture(&format!(
        "/// Answers with two, which is more than the signature promises.\n///\n/// ```\n/// let v: Vec<u8> = Vec::new();\n/// assert!(v.is_empty());\n/// ```\npub fn two() -> usize {{ 2 }}\n{TESTED}"
    ));
    assert_eq!(codes(&dir), ["RUST_EXAMPLE_DOES_NOT_NAME_SUBJECT"]);
}

#[test]
fn a_module_nothing_exercises_fails_the_gate() {
    let dir = fixture(
        "/// Answers with two, which is more than the signature promises.\n///\n/// ```\n/// assert_eq!(majordomus_cli::two(), 2);\n/// ```\npub fn two() -> usize { 2 }\n",
    );
    assert_eq!(codes(&dir), ["RUST_MODULE_MISSING_BEHAVIOURAL_TEST"]);
}

#[test]
fn a_module_whose_header_names_the_boundary_without_explaining_it_fails_the_gate() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("lib.rs"),
        "//! Things.\n//!\n//! ```\n//! assert_eq!(majordomus_cli::two(), 2);\n//! ```\n/// Answers with two, which is more than the signature promises.\n///\n/// ```\n/// assert_eq!(majordomus_cli::two(), 2);\n/// ```\npub fn two() -> usize { 2 }\n".to_string() + TESTED,
    )
    .unwrap();
    assert_eq!(codes(&dir), ["RUST_MODULE_MISSING_DOCS"]);
}

#[test]
fn a_pub_item_no_module_chain_reaches_is_not_measured_at_all() {
    // the pressure the rule is meant to apply: reducing visibility is a way of satisfying it,
    // and the measurement has to agree that it is
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("lib.rs"),
        "//! A fixture crate whose header is long enough to be one: it says what the module\n//! owns, which is a private module nothing outside this crate can reach at all.\n//!\n//! ```\n//! assert!(true);\n//! ```\nmod hidden;\n",
    )
    .unwrap();
    std::fs::write(
        src.join("hidden.rs"),
        "//! Private.\npub fn undocumented_thing() {}\n",
    )
    .unwrap();
    let report = quality::report::rust_only(dir.path(), "fixture").unwrap();
    assert!(
        report
            .violations
            .iter()
            .all(|v| !v.symbol.contains("undocumented_thing")),
        "a pub item in a private module is not exported and is not measured"
    );
}

// ---------------------------------------------------------------- the whole stack

#[test]
fn the_measurement_is_answered_identically_through_the_command_line_and_the_capability() {
    // the parity claim this subsystem makes about itself: one execution, several renderings
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    let registry = app.registry();

    let cap = registry
        .get("quality.report")
        .expect("the capability is composed into the application");
    assert!(cap.exposure.http.is_some(), "and reaches HTTP");
    assert!(
        cap.exposure
            .mcp
            .as_ref()
            .and_then(|m| m.tool.as_ref())
            .is_some(),
        "and MCP"
    );
    let cli_path = cap.exposure.cli.as_ref().expect("and the command line");
    assert_eq!(
        registry.by_cli(&cli_path.path).map(|c| c.id.to_string()),
        Some("quality.report".to_string()),
        "the command line's route back to it is the registry's own"
    );

    // and the fixture repository, which carries no crate, is answered rather than failed
    let answer = app
        .context
        .execute("quality.report", serde_json::json!({}))
        .expect("answers");
    assert_eq!(answer["measured"], false);
    assert_eq!(answer["passes"], true);
    assert!(
        answer["reason"].as_str().unwrap_or("").contains(CRATE_DIR),
        "and says which crate it looked for: {answer:?}"
    );
}

#[test]
fn an_unknown_violation_code_is_refused_rather_than_answered_with_an_empty_list() {
    let f = common::Fixture::new();
    let app = common::load_app(&f);
    let err = app
        .context
        .execute("quality.report", serde_json::json!({ "code": "RUST_NOPE" }))
        .expect_err("a typo must not read as a clean report");
    let message = err.to_string();
    assert!(message.contains("RUST_NOPE"), "{message}");
    assert!(
        message.contains("RUST_PUBLIC_MISSING_EXAMPLE"),
        "and says what the codes are: {message}"
    );
}
