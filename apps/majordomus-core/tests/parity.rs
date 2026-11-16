//! Every row of the shared fixture, on the native host: the canonical bytes and the digest
//! the fixture records, which a hash library outside this crate computed over the expected
//! bytes. The wasm32 module is held to the same rows by `wasm-parity.mjs`, and the
//! executable by its own test, so that one fixture judges all three.

use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/canonical-digest.v1.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("JSON")
}

#[test]
fn every_case_has_the_canonical_bytes_and_digest_the_fixture_records() {
    let f = fixture();
    let cases = f["cases"].as_array().expect("cases");
    assert!(cases.len() >= 10, "the fixture lost cases");
    let mut wrong = Vec::new();
    for case in cases {
        let id = case["id"].as_str().expect("an id");
        let input: Value =
            serde_json::from_str(case["input"].as_str().expect("input")).expect("valid input");
        let canonical = String::from_utf8(majordomus_core::canonical_json(&input)).expect("UTF-8");
        let want = case["canonical"].as_str().expect("canonical");
        if canonical != want {
            wrong.push(format!("{id}: canonical {canonical} != {want}"));
        }
        let digest = case["digest"].as_str().expect("digest");
        if majordomus_core::canonical_digest_hex(&input) != digest {
            wrong.push(format!("{id}: digest"));
        }
        if majordomus_core::digest_of_json_text(case["input"].as_str().unwrap()).as_deref()
            != Ok(digest)
        {
            wrong.push(format!("{id}: digest from text"));
        }
    }
    assert!(
        wrong.is_empty(),
        "rows that disagree:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn text_that_is_not_json_is_refused_not_digested() {
    let f = fixture();
    for case in f["invalid"].as_array().expect("invalid") {
        let id = case["id"].as_str().expect("an id");
        assert!(
            majordomus_core::digest_of_json_text(case["input"].as_str().unwrap()).is_err(),
            "{id} was digested"
        );
    }
}
