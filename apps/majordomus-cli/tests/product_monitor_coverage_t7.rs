//! Coverage of the one path of train 7 that needs a process of its own: a `git` that cannot
//! be started. `PATH` is a property of the whole process, so this file holds one test and
//! nothing else runs beside it.

mod common;

use common::Fixture;
use serde_json::json;

#[test]
fn a_git_that_cannot_be_started_is_said_by_the_gate_state_which_still_answers() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let nowhere = tempfile::tempdir().unwrap();
    let original = std::env::var_os("PATH").expect("a PATH");
    std::env::set_var("PATH", nowhere.path());
    let answer = app.context.execute("gates.state", json!({}));
    std::env::set_var("PATH", &original);
    let answer = answer.expect("the gate state answers without git");
    assert!(answer["head"].is_null(), "no commit could be read");
    let findings: Vec<&str> = answer["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        findings.iter().any(|f| f.starts_with(
            "the current commit could not be read, so no run is said to be on it: git rev-parse HEAD:"
        )),
        "{findings:?}"
    );
}
