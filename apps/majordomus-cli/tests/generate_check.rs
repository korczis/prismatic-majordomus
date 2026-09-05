//! `majordomus generate` and `--check`: the committed projections are caches of the
//! registry, written on request, and reported stale when they no longer match.

mod common;

use common::{run_in, Fixture};

#[test]
fn generate_writes_check_agrees_tampering_is_detected_and_check_never_writes() {
    let f = Fixture::new();
    let out = f.path("out");
    let out_s = out.to_str().unwrap();
    // check before any generation: missing is stale
    let (code, _, err) = run_in(&f.root(), &["generate", "--check", "--out", out_s], "");
    assert_eq!(code, 10, "{err}");
    assert!(
        err.contains("openapi.json (missing)") && err.contains("capabilities.md (missing)"),
        "{err}"
    );
    assert!(!out.exists(), "check mode created the output directory");

    let (code, stdout, _) = run_in(&f.root(), &["generate", "--out", out_s], "");
    assert_eq!(code, 0);
    assert!(
        stdout.contains("docs/generated/openapi.json")
            && stdout.contains("docs/generated/capabilities.md")
    );
    let openapi = out.join("docs/generated/openapi.json");
    let first = std::fs::read_to_string(&openapi).unwrap();
    assert!(first.ends_with('\n'));

    let (code, stdout, _) = run_in(&f.root(), &["generate", "--check", "--out", out_s], "");
    assert_eq!(code, 0);
    assert!(stdout.contains("in sync"));

    // regenerate: byte-identical
    run_in(&f.root(), &["generate", "--out", out_s], "");
    assert_eq!(std::fs::read_to_string(&openapi).unwrap(), first);

    // tamper with the committed snapshot
    std::fs::write(
        &openapi,
        first.replace("\"openapi\": \"3.1.0\"", "\"openapi\": \"3.0.3\""),
    )
    .unwrap();
    let (code, _, err) = run_in(&f.root(), &["generate", "--check", "--out", out_s], "");
    assert_eq!(code, 10);
    assert!(
        err.contains("openapi.json (differs)") && !err.contains("capabilities.md (differs)"),
        "{err}"
    );
    assert!(
        err.contains("majordomus generate"),
        "names the remedy: {err}"
    );
    assert_ne!(
        std::fs::read_to_string(&openapi).unwrap(),
        first,
        "check mode rewrote the file"
    );

    // only one target
    let (code, stdout, _) = run_in(&f.root(), &["generate", "openapi", "--out", out_s], "");
    assert_eq!(code, 0);
    assert!(stdout.contains("openapi.json") && !stdout.contains("capabilities.md"));
    assert_eq!(std::fs::read_to_string(&openapi).unwrap(), first);
}
