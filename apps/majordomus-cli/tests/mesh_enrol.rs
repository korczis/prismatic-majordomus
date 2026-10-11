//! `mesh.enrol`, executed in this process: the declaration a repository commits gains this
//! machine's key once, and a repository that declares no mesh is refused before an identity
//! is created.
//!
//! One test, in a binary of its own, because it points `XDG_STATE_HOME` — where the node
//! identity lives — at a temporary directory for this whole process: a second test beside
//! it, or a test in a shared binary, would read or create an identity there by accident.

mod common;

use common::Fixture;
use serde_json::json;

#[test]
fn enrolment_in_process_lists_the_key_once_and_refuses_a_repository_without_a_mesh() {
    let state = tempfile::tempdir().unwrap();
    std::env::set_var("XDG_STATE_HOME", state.path());

    // no declaration: refused, and no identity was created for nothing
    let bare = Fixture::new();
    let app = common::load_app(&bare);
    let refused = app.context.execute("mesh.enrol", json!({}));
    assert!(refused.is_err(), "{refused:?}");
    assert!(
        !state.path().join("majordomus").join("node.json").exists(),
        "a refused enrolment creates no identity"
    );

    // the declaration init seeds: the key is added, under a comment naming the machine
    let f = Fixture::new();
    let seeded = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../share/skeleton/ai/repo/mesh/mesh.yaml"),
    )
    .unwrap();
    f.write(".ai/repo/mesh/mesh.yaml", &seeded);
    f.commit("the seeded mesh declaration");
    let app = common::load_app(&f);

    // before enrolment: the machine's identity exists and is not listed. The doctor fails
    // `trust` with the key, and a runtime activated from this declaration stays closed,
    // which the doctor's `runtime` check reports as that, not as a second failure.
    let identity = majordomus_cli::mesh::NodeIdentity::load_or_create(
        &state.path().join("majordomus").join("node.json"),
    )
    .unwrap();
    let unlisted = identity.public.public_key.clone();
    let doctor = app
        .context
        .execute("mesh.doctor", json!({}))
        .expect("the doctor answers");
    let check = |report: &serde_json::Value, name: &str| -> serde_json::Value {
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["check"] == json!(name))
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    };
    let trust = check(&doctor, "trust");
    assert_eq!(trust["ok"], json!(false), "{doctor}");
    assert!(
        trust["detail"].as_str().unwrap().contains(&unlisted),
        "{trust}"
    );
    let object = app
        .context
        .index
        .objects
        .iter()
        .find(|o| o.kind == majordomus_cli::mesh::KIND)
        .expect("the declaration is in the index");
    let config = majordomus_cli::mesh::MeshConfig::parse(object).expect("and parses");
    let runtime = majordomus_cli::mesh::MeshRuntime::new();
    runtime
        .activate(&config, identity, vec![], vec![], "test")
        .unwrap();
    let status = runtime.status();
    assert!(!status.active);
    let report =
        majordomus_cli::mesh::doctor::doctor_at(Some(Ok(config.clone())), None, Some(&status));
    let report = serde_json::to_value(report).unwrap();
    let closed = check(&report, "runtime");
    assert_eq!(closed["ok"], json!(true), "{report}");
    assert!(
        closed["detail"].as_str().unwrap().starts_with("closed: "),
        "{closed}"
    );

    let enrolled = app
        .context
        .execute("mesh.enrol", json!({}))
        .expect("enrolment answers");
    assert_eq!(enrolled["added"], json!(true), "{enrolled}");
    assert_eq!(enrolled["path"], json!(".ai/repo/mesh/mesh.yaml"));
    let key = enrolled["key"].as_str().unwrap().to_string();
    let written = std::fs::read_to_string(f.path(".ai/repo/mesh/mesh.yaml")).unwrap();
    assert!(written.contains(&format!("    - \"{key}\"")), "{written}");
    assert!(
        written.contains("    # ") && written.contains(" — "),
        "the entry is labelled with the machine: {written}"
    );
    assert!(
        written.contains(std::env::consts::OS),
        "the label names the platform: {written}"
    );
    assert!(
        enrolled["next"].as_str().unwrap().contains("commit"),
        "{enrolled}"
    );

    // once: the same process, the same machine, nothing changes
    let again = app
        .context
        .execute("mesh.enrol", json!({}))
        .expect("enrolment answers");
    assert_eq!(again["added"], json!(false), "{again}");
    assert_eq!(
        std::fs::read_to_string(f.path(".ai/repo/mesh/mesh.yaml")).unwrap(),
        written
    );
    assert_eq!(
        again["key"],
        json!(unlisted),
        "the identity made before is the one listed"
    );

    // a review command in this process has no cooperation to act on: refused, by name
    let refused = app.context.execute(
        "mesh.review.withdraw",
        json!({ "request": "none/r-000000000000", "session": "s1" }),
    );
    assert!(refused.is_err(), "{refused:?}");

    // listed now: the doctor's trust check passes on the enrolled declaration
    let app = common::load_app(&f);
    let doctor = app
        .context
        .execute("mesh.doctor", json!({}))
        .expect("the doctor answers");
    assert_eq!(check(&doctor, "trust")["ok"], json!(true), "{doctor}");
    std::env::remove_var("XDG_STATE_HOME");
}
