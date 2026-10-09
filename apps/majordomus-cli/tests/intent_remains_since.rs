//! What changed since a reading of one intent (ADR 0117): the caller keeps the two values a
//! reading answered, passes them back later, and is told `unchanged`, `remains_moved` or
//! `plan_changed`; nothing records a reading, and one pair is refused for several intents.

// claims: intent-remains-derived

mod common;

use common::{run_in, Fixture};
use majordomus_cli::evidence::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
use serde_json::Value;

fn json_of(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

/// The two values a reading of the fixture's intent answers.
fn reading(f: &Fixture) -> (String, String) {
    let (_, v) = json_of(f, &["intent", "realization", "--intent", "fixture-intent"]);
    let view = &v["intents"][0];
    (
        view["review_revision"].as_str().unwrap().to_string(),
        view["remains_digest"].as_str().unwrap().to_string(),
    )
}

/// How the fixture's intent changed since `then`.
fn since(f: &Fixture, then: &(String, String)) -> String {
    let (_, v) = json_of(
        f,
        &[
            "intent",
            "realization",
            "--intent",
            "fixture-intent",
            "--since-review-revision",
            &then.0,
            "--since-remains-digest",
            &then.1,
        ],
    );
    v["intents"][0]["change"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Criterion `what-changed`.
#[test]
fn a_reading_is_compared_through_its_two_values() {
    let f = Fixture::new();
    let first = reading(&f);
    assert_eq!(since(&f, &first), "unchanged");

    // the case the criterion names is run and fails, recorded against the source in the tree:
    // what remains moved (the open work is now failing), and the plan a review judges did not
    let id = TestId::of("test/cases/00_x.sh").unwrap();
    let source = std::fs::read(f.path(&id.source())).unwrap();
    let mut ledger = Ledger::empty();
    ledger.merge([Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome: Outcome::Fail,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: "clean".into(),
        digest: digest_of(&source),
        at: "2026-10-09T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
        run: None,
    }]);
    ledger.save(&f.root()).unwrap();
    f.commit("the case fails");
    let (_, v) = json_of(&f, &["intent", "realization", "--intent", "fixture-intent"]);
    assert_eq!(
        v["intents"][0]["unmet"][0]["basis"], "work_open_failing",
        "{v}"
    );
    assert_eq!(since(&f, &first), "remains_moved");
    let second = reading(&f);
    assert_eq!(
        second.0, first.0,
        "a recorded run is not the plan a review judges"
    );

    // the criterion is reworded: the plan changed, which outranks the move
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &common::INTENT.replace(
            "The fixture's own case passes",
            "The fixture's own case passes on a clean tree",
        ),
    );
    f.commit("the criterion is reworded");
    assert_eq!(since(&f, &second), "plan_changed");
    assert_eq!(since(&f, &first), "plan_changed");
}

/// One pair of values cannot describe several intents.
#[test]
fn a_reading_without_an_intent_is_refused() {
    let f = Fixture::new();
    let (code, out, err) = run_in(
        &f.root(),
        &[
            "run",
            "intent_realization.work",
            "--input",
            r#"{"since_review_revision":"a","since_remains_digest":"b"}"#,
        ],
        "",
    );
    assert_ne!(code, 0, "{out}{err}");
    assert!(
        format!("{out}{err}").contains("name the intent"),
        "{out}{err}"
    );
}
