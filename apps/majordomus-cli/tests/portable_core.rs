//! The executable answers the portable core's fixture (ADR 0170): the canonical bytes the
//! mesh signs (`mesh::journal::canonical_json`) and the cache key the executor takes
//! (`capability::executor::canonical_json`) are the core's, row for row, with this crate's
//! own `serde_json` features unified in. The wasm32 module is held to the same rows by the
//! `portable-core` gate, so one fixture judges the host, the executable and the browser.

use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../majordomus-core/fixtures/canonical-digest.v1.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the fixture")).expect("JSON")
}

#[test]
fn the_mesh_and_the_executor_write_the_bytes_the_fixture_records() {
    let f = fixture();
    let mut wrong = Vec::new();
    for case in f["cases"].as_array().expect("cases") {
        let id = case["id"].as_str().expect("an id");
        let input: Value =
            serde_json::from_str(case["input"].as_str().expect("input")).expect("valid input");
        let want = case["canonical"].as_str().expect("canonical");
        let mesh = majordomus_cli::mesh::journal::canonical_json(&input);
        if mesh != want.as_bytes() {
            wrong.push(format!("{id}: mesh {}", String::from_utf8_lossy(&mesh)));
        }
        let executor = majordomus_cli::capability::executor::canonical_json(&input);
        if executor != want {
            wrong.push(format!("{id}: executor {executor}"));
        }
        if majordomus_cli::policy::sha256_bytes_hex(&mesh) != case["digest"].as_str().unwrap() {
            wrong.push(format!("{id}: digest"));
        }
    }
    assert!(
        wrong.is_empty(),
        "rows that disagree:\n{}",
        wrong.join("\n")
    );
}
