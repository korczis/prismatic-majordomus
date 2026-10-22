//! Every test line is read, and a local run stays local (I1957, ADR 0087 decision 2).
//!
//! The recorder reads a run through one adapter per runner, and the web report reads the
//! same adapters. These tests drive the built executable in a fixture repository of its own,
//! over real output of both runners (`test/fixtures/evidence-adapters/`), and hold the two
//! things the issue owes:
//!
//! * `every_test_line_read`: every line a runner printed is either read into a case result
//!   or listed as dropped, with its number, its text and why, and the run record counts both;
//!   each execution carries its per-test results and what the runner said about it, and the
//!   tracked ledger keeps only the cases that did not pass, without their output;
//! * `local_run_stays_local`: `evidence record --run suite` runs the repository's own suite
//!   runner, stamps its own run, and keeps it only under `.ai/local/`: in the local ledger and
//!   the local run index, which is bounded; the tracked ledger is never written, nothing
//!   tracked changes, and every way of pointing a local run at the tracked ledger is refused.

mod common;

use std::path::{Path, PathBuf};

use common::{dist_share, majordomus, Fixture};
use majordomus_cli::evidence::local_runs::{append, entries};
use majordomus_cli::evidence::{
    read_libtest, read_suite, EvidenceRunRecord, Outcome, LEDGER_PATH, LOCAL_LEDGER_PATH,
    LOCAL_RUNS_BOUND, LOCAL_RUNS_DIR,
};
use serde_json::Value;

/// The adapter fixtures: real output of `cargo test` and of `test/run.sh`.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/fixtures/evidence-adapters")
}

fn fixture(name: &str) -> String {
    let path = fixtures().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Run the executable in `root`, with the distribution beside the crate and `MJ_TEST_JOBS=1`,
/// returning the exit code, stdout and stderr.
fn run(root: &Path, args: &[&str]) -> (i32, String, String) {
    let out = majordomus()
        .args(args)
        .current_dir(root)
        .env("MAJORDOMUS_SHARE", dist_share())
        .env("MJ_TEST_JOBS", "1")
        .env_remove("MJ_TEST_REPORT")
        .output()
        .expect("the executable runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The answer of a recording that must succeed.
fn recorded(root: &Path, args: &[&str]) -> Value {
    let (code, out, err) = run(root, args);
    assert_eq!(code, 0, "{args:?}: {err}\n{out}");
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"))
}

/// Every stream a run record read is accounted for: the lines it read and the lines it
/// listed add up to what the stream held, and each listed line is the line at its number.
fn every_line_accounted(run: &Value, streams: &[(&str, &str, &str)]) {
    let lines = run["lines"]
        .as_array()
        .expect("the run record counts its lines");
    assert_eq!(lines.len(), streams.len(), "{lines:?}");
    for (producer, stream, text) in streams {
        let count = lines
            .iter()
            .find(|l| l["producer"] == *producer && l["stream"] == *stream)
            .unwrap_or_else(|| panic!("no count of the {producer} {stream}"));
        let held = text.lines().count();
        assert_eq!(count["lines"], held, "{producer} {stream}");
        let dropped: Vec<&Value> = run["dropped_lines"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|d| d["producer"] == *producer && d["stream"] == *stream)
            .collect();
        assert_eq!(
            count["read"].as_u64().unwrap() as usize + dropped.len(),
            held,
            "{producer} {stream}: a line neither read nor listed"
        );
        let texts: Vec<&str> = text.lines().collect();
        for d in dropped {
            let n = d["line"].as_u64().unwrap() as usize;
            let plain = d["text"].as_str().unwrap();
            assert!(
                texts[n - 1].ends_with(plain) || texts[n - 1].contains('\u{1b}'),
                "line {n} listed as {plain:?}, and is {:?}",
                texts[n - 1]
            );
            assert!(!d["reason"].as_str().unwrap().is_empty(), "{d}");
        }
    }
}

/// The fixture with the integration binaries the cargo fixture ran and the suite cases the
/// suite fixture ran, committed, so that none of them is unknown.
fn with_tests() -> Fixture {
    let f = Fixture::new();
    for name in ["empty", "green", "red"] {
        f.write(
            &format!("apps/majordomus-cli/tests/{name}.rs"),
            &format!("// {name}\n"),
        );
    }
    for name in [
        "01_passes",
        "02_fails",
        "03_declines",
        "04_exclusive",
        "05_hangs",
    ] {
        f.write(&format!("test/cases/{name}.sh"), "true\n");
    }
    f.commit("the tests the fixtures ran");
    f
}

#[test]
fn every_test_line_read() {
    // the adapters, over every fixture: each line read into a case result or listed
    for name in [
        "cargo-test.txt",
        "cargo-test-color.txt",
        "cargo-test-filtered.txt",
    ] {
        let text = fixture(name);
        let read = read_libtest(&text);
        assert_eq!(read.lines.lines, text.lines().count(), "{name}");
        assert_eq!(
            read.lines.read + read.dropped.len(),
            read.lines.lines,
            "{name}"
        );
        assert!(!read.units.is_empty(), "{name}");
    }
    for mode in ["serial", "parallel", "no-skips"] {
        let (tsv, out) = (
            fixture(&format!("suite-{mode}.tsv")),
            fixture(&format!("suite-{mode}.txt")),
        );
        let read = read_suite(&tsv, Some(&out)).unwrap();
        for count in &read.lines {
            let mine = read
                .dropped
                .iter()
                .filter(|d| d.stream == count.stream)
                .count();
            assert_eq!(count.read + mine, count.lines, "{mode} {:?}", count.stream);
        }
        assert!(
            read.cases.iter().any(|c| !c.log.is_empty()),
            "{mode}: the logs were read"
        );
    }

    // the recorder, through the executable: the run record accounts for every line
    let f = with_tests();
    let root = f.root();
    let reports = tempfile::tempdir().unwrap();
    let out = reports.path().join("run.json");
    let (cargo, out_s) = (
        fixtures().join("cargo-test.txt").display().to_string(),
        out.display().to_string(),
    );
    let v = recorded(
        &root,
        &[
            "evidence",
            "--format",
            "json",
            "record",
            "--crate-output",
            &cargo,
            "--ledger",
            "local",
            "--run-record",
            &out_s,
        ],
    );
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(
        written, v["run_record"],
        "the run record written is the one answered"
    );
    every_line_accounted(
        &written,
        &[("crate", "output", fixture("cargo-test.txt").as_str())],
    );
    let typed: EvidenceRunRecord = serde_json::from_value(written).unwrap();
    let red = typed
        .executions
        .iter()
        .find(|e| e.test == "crate:red")
        .expect("the failing binary is recorded");
    let cases: Vec<(&str, Outcome)> = red
        .cases
        .iter()
        .map(|c| (c.name.as_str(), c.outcome))
        .collect();
    assert_eq!(
        cases,
        [
            ("ignored_plain", Outcome::Skip),
            ("passes", Outcome::Pass),
            ("fails_too", Outcome::Fail),
            ("fails_with_output", Outcome::Fail)
        ]
    );
    assert!(red.cases[3]
        .detail
        .as_deref()
        .unwrap()
        .contains("arithmetic is broken"));
    assert!(red
        .detail
        .as_deref()
        .unwrap()
        .starts_with("test result: FAILED."));
    // what no claim can name is listed with every test it ran
    let unit_tests = typed
        .dropped
        .iter()
        .find(|d| d.what == "unittests src/lib.rs")
        .unwrap();
    assert_eq!(unit_tests.cases.len(), 2);
    // and the ledger keeps one line's worth per test: what did not pass, never its output
    let ledger: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(LOCAL_LEDGER_PATH)).unwrap())
            .unwrap();
    let row = ledger["executions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["test"] == "crate:red")
        .unwrap();
    let kept: Vec<&str> = row["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(kept, ["ignored_plain", "fails_too", "fails_with_output"]);
    assert!(
        row.get("detail").is_none() && row["cases"][2].get("detail").is_none(),
        "{row}"
    );
    let green = ledger["executions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["test"] == "crate:green")
        .unwrap();
    assert!(
        green.get("cases").is_none(),
        "a green row is what it always was: {green}"
    );

    // the text rendering says how much was read and why the rest was not
    let (code, text, err) = run(
        &root,
        &[
            "evidence",
            "record",
            "--crate-output",
            &cargo,
            "--ledger",
            "local",
        ],
    );
    assert_eq!(code, 0, "{err}");
    assert!(text.contains("lines        crate output ("), "{text}");
    assert!(text.contains("not read     "), "{text}");
}

/// A suite runner the fixture carries: the repository's own `test/run.sh`, so the run is a
/// real run of the real runner, over the fixture's case 00_x and two of its own.
fn with_suite(f: &Fixture) {
    let runner = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test/run.sh");
    f.write("test/run.sh", &std::fs::read_to_string(runner).unwrap());
    // the library the fixture's own case 00_x sources
    f.write("test/lib.sh", "# the fixture's case library\n");
    f.write(
        "test/cases/01_passes.sh",
        "echo \"    a line it printed\"\n",
    );
    f.write(
        "test/cases/02_fails.sh",
        "echo \"    expected /proven/ in output:\"\necho \"not run\"\nexit 1\n",
    );
    f.commit("a suite");
}

#[test]
fn local_run_stays_local() {
    let f = Fixture::new();
    with_suite(&f);
    let root = f.root();
    let before = f.snapshot();
    assert!(!root.join(LEDGER_PATH).exists());

    let v = recorded(
        &root,
        &["evidence", "--format", "json", "record", "--run", "suite"],
    );
    // it recorded both cases, into the local ledger, and kept the run in the local index
    assert_eq!(v["recorded"], 3, "{v}");
    assert_eq!(v["passed"], 2, "{v}");
    assert_eq!(v["ledger"], LOCAL_LEDGER_PATH, "{v}");
    let kept = v["local_run"]
        .as_str()
        .expect("the run is kept in the index");
    assert!(
        kept.starts_with(&format!("{LOCAL_RUNS_DIR}/run-local-")),
        "{kept}"
    );
    let rec = &v["run_record"];
    assert_eq!(rec["ledger"], "local");
    assert_eq!(rec["origin"], "local");
    // it stamped its own run: the suite's measurement, of a clean tree, naming its report
    assert_eq!(rec["provenance"][0]["producer"], "suite", "{rec}");
    assert_eq!(rec["provenance"][0]["working_tree"], "clean", "{rec}");
    assert_eq!(rec["working_tree"], "clean");
    // the runner's report and its output, every line read or listed
    let streams: Vec<(String, String)> = rec["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let read = l["read"].as_u64().unwrap() as usize;
            let listed = rec["dropped_lines"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| d["stream"] == l["stream"])
                .count();
            assert_eq!(read + listed, l["lines"].as_u64().unwrap() as usize, "{l}");
            (
                l["producer"].as_str().unwrap().to_string(),
                l["stream"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        streams,
        [
            ("suite".to_string(), "report".to_string()),
            ("suite".to_string(), "output".to_string())
        ]
    );
    let fails = rec["executions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["test"] == "suite:02_fails")
        .unwrap();
    assert_eq!(fails["outcome"], "fail");
    assert_eq!(
        fails["detail"], "    expected /proven/ in output:\nnot run",
        "the case's log, read from what the runner printed: {fails}"
    );
    assert_eq!(fails["cases"][0]["phase"], "serial", "{fails}");

    // only the ignored local layer moved: the tracked ledger is not written, nothing tracked
    // changed, and the index holds the run
    assert!(!root.join(LEDGER_PATH).exists(), "the tracked ledger");
    assert_eq!(f.snapshot(), before, "a local run changed the checkout");
    assert!(root.join(kept).is_file());
    assert_eq!(entries(&root).len(), 1);
    let local: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join(LOCAL_LEDGER_PATH)).unwrap())
            .unwrap();
    assert_eq!(local["executions"].as_array().unwrap().len(), 3);

    // one case, named
    let v = recorded(
        &root,
        &[
            "evidence",
            "--format",
            "json",
            "record",
            "--run",
            "suite",
            "--case",
            "01_passes",
        ],
    );
    assert_eq!(v["recorded"], 1, "{v}");
    assert_eq!(entries(&root).len(), 2);

    // every way of pointing a local run at the tracked ledger, or at another run, is refused
    for (args, said) in [
        (vec!["--run", "suite", "--ledger", "repo"], "tracked ledger"),
        (vec!["--run", "suite", "--origin", "ci"], "local run"),
        (
            vec!["--run", "suite", "--suite", "x.tsv"],
            "reads no report",
        ),
        (vec!["--case", "01_passes"], "--run suite"),
        (vec!["--run", "suite", "--case", "--no-skips"], ""),
    ] {
        let mut all = vec!["evidence", "record"];
        all.extend(args.iter());
        let (code, _, err) = run(&root, &all);
        assert_ne!(code, 0, "{args:?}");
        assert!(err.contains(said), "{args:?}: {err}");
    }
    assert!(
        !root.join(LEDGER_PATH).exists(),
        "a refusal wrote the tracked ledger"
    );
    assert_eq!(entries(&root).len(), 2, "a refusal kept a run");

    // the index is bounded: the newest LOCAL_RUNS_BOUND are kept, the oldest dropped
    let newest = entries(&root).last().unwrap().record.clone();
    for n in 0..LOCAL_RUNS_BOUND {
        let mut later = newest.clone();
        later.recorded_at = format!("2999-01-01T00:{:02}:{:02}Z", n / 60, n % 60);
        later.id = format!("local:bound:{n}");
        append(&root, &later).unwrap();
    }
    let kept_now = entries(&root);
    assert_eq!(kept_now.len(), LOCAL_RUNS_BOUND);
    assert!(
        kept_now
            .iter()
            .all(|r| r.record.id.starts_with("local:bound:")),
        "the two oldest, the real runs, were the ones dropped"
    );
    assert_eq!(
        f.snapshot(),
        before,
        "the index stays inside the ignored layer"
    );
}
