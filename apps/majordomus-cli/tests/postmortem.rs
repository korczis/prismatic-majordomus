//! `majordomus postmortem`, driven end to end through the built executable.
//!
//! The rules themselves are proved beside them in `src/postmortem/mod.rs`. Here the command
//! is run whole in a disposable repository that holds two records: the exit statuses a
//! script reads, the runs handed in on standard input in the shape the forge prints them,
//! and the property the whole group shares — nothing it does changes the repository. The
//! forge is never reached: every run comes from a file or from standard input.

mod common;

use common::{run_in, Fixture, SOURCES};
use serde_json::{json, Value};

/// The source class the records are discovered through, as this repository declares it.
const POST_MORTEM_SOURCE: &str = "
  - id: post-mortem
    kind: post-mortem
    discovery: vcs
    pathspec: ':(glob).ai/repo/post-mortems/*.md'
    required: false
";

const BODY: &str = "## What failed\n\nIt failed.\n\n## Root cause\n\nA cause.\n\n## Fix\n\nA fix.\n\n## Prevention\n\nA test.\n";

const RESOLVED: &str = "---
schema: post-mortem/v1
id: pm-20261013-lease
kind: post-mortem
title: Two writers lost the lease
description: A lease held by two writers is lost by both.
status: resolved
date: 2026-10-13
runs:
  - workflow: validate
    id: \"100\"
    commit: \"abcdef1\"
    jobs: [\"rust (3)\"]
    step: cargo test
cause:
  class: race
  summary: Two writers, one file.
components:
  - file:lib/a.sh
  - rule:project.alpha@1
signals:
  - id: lease-lost
    text: \"the lease was lost\"
fixed_by: [\"pr:1078\"]
prevention: [\"test:test/cases/00_x.sh\"]
related: [\"issue:I0001\"]
---

";

const DRAFT: &str = "---
schema: post-mortem/v1
id: pm-20261014-validate-200
kind: post-mortem
title: validate run 200 ended in failure
description: A draft.
status: draft
date: 2026-10-14
runs:
  - workflow: validate
    id: \"200\"
    commit: \"abcdef2\"
    jobs: [\"structure\"]
cause:
  class: undetermined
  summary: Not yet established.
---

";

/// A repository with one resolved record answering run 100 and one draft answering run 200.
fn recorded() -> Fixture {
    let f = Fixture::new();
    f.write(
        ".ai/repo/knowledge/sources.yaml",
        &format!("{}{POST_MORTEM_SOURCE}", SOURCES.trim_end()),
    );
    f.write(
        ".ai/repo/post-mortems/pm-20261013-lease.md",
        &format!("{RESOLVED}{BODY}"),
    );
    f.write(
        ".ai/repo/post-mortems/pm-20261014-validate-200.md",
        &format!("{DRAFT}{BODY}"),
    );
    f.commit("two post-mortems");
    f
}

fn run(id: u64, conclusion: &str, at: &str) -> Value {
    json!({"databaseId": id, "workflowName": "validate", "conclusion": conclusion,
        "headSha": "0123456789abcdef0123456789abcdef01234567", "createdAt": at,
        "updatedAt": at, "headBranch": "master",
        "url": format!("https://forge.example/o/r/actions/runs/{id}")})
}

fn runs() -> String {
    json!([
        run(100, "failure", "2026-10-13T05:00:00Z"),
        run(200, "failure", "2026-10-14T05:00:00Z"),
        run(400, "failure", "2026-10-15T05:00:00Z"),
        run(402, "cancelled", "2026-10-15T07:00:00Z"),
        run(90, "failure", "2026-10-12T05:00:00Z"),
    ])
    .to_string()
}

fn parsed(stdout: &str) -> Value {
    serde_json::from_str(stdout).unwrap_or_else(|e| panic!("not JSON ({e}):\n{stdout}"))
}

#[test]
fn the_records_are_listed_and_a_broken_rule_is_exit_10() {
    let f = recorded();
    let root = f.root();

    let (code, out, err) = run_in(&root, &["postmortem", "list", "--format", "json"], "");
    assert_eq!(code, 0, "{err}");
    let v = parsed(&out);
    assert_eq!(v["count"], 2, "{out}");
    assert_eq!(
        v["records"][0]["id"], "pm-20261014-validate-200",
        "newest first"
    );
    assert_eq!(v["records"][1]["related"], json!(["issue:I0001"]));
    assert_eq!(v["by_cause"], json!({"race": 1, "undetermined": 1}));
    assert_eq!(v["unreadable"], json!([]));

    let (code, out, err) = run_in(&root, &["postmortem", "check"], "");
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(out, "2 post-mortem(s) judged, 0 finding(s)\n");

    // the draft claims a cause and a fix: two findings, each with its code, and exit 10
    f.write(
        ".ai/repo/post-mortems/pm-20261014-validate-200.md",
        &format!(
            "{}{BODY}",
            DRAFT
                .replace("class: undetermined", "class: defect")
                .replace("---\n\n", "fixed_by: [\"pr:1\"]\n---\n\n")
        ),
    );
    f.commit("a draft that is not one");
    let (code, out, _) = run_in(&root, &["postmortem", "check", "--format", "json"], "");
    assert_eq!(code, 10, "{out}");
    let v = parsed(&out);
    assert_eq!(v["ok"], false);
    let codes: Vec<&str> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["draft-names-a-cause", "fix-before-resolved"]);
    let (code, out, _) = run_in(&root, &["postmortem", "check"], "");
    assert_eq!(code, 10);
    assert!(
        out.starts_with("FAIL  pm-20261014-validate-200  draft-names-a-cause  "),
        "{out}"
    );
}

#[test]
fn the_runs_on_standard_input_are_owed_until_a_record_names_them() {
    let f = recorded();
    let root = f.root();
    let before = f.snapshot();

    // inside the grace: owed, said, and exit 0
    let (code, out, err) = run_in(
        &root,
        &[
            "postmortem",
            "owed",
            "--runs-json",
            "-",
            "--now",
            "2026-10-15T12:00:00Z",
            "--format",
            "json",
        ],
        &runs(),
    );
    assert_eq!(code, 0, "{out}{err}");
    let v = parsed(&out);
    assert_eq!(v["observed"], true);
    assert_eq!(v["source"], "input");
    assert_eq!(
        (v["failed"].as_u64(), v["answered"].as_u64()),
        (Some(3), Some(2))
    );
    assert_eq!(v["owed"].as_array().unwrap().len(), 1);
    assert_eq!(v["owed"][0]["id"], "400");
    assert_eq!(v["owed"][0]["within_grace"], true);
    assert_eq!(v["owed"][0]["age_seconds"], 7 * 3600);

    // past it: the same run is late, and the exit status says so
    let late = [
        "postmortem",
        "owed",
        "--runs-json",
        "-",
        "--now",
        "2026-10-17T12:00:00Z",
    ];
    let (code, out, _) = run_in(&root, &late, &runs());
    assert_eq!(code, 10, "{out}");
    assert!(
        out.starts_with("LATE  validate 400  failure  2d ago  "),
        "{out}"
    );
    assert!(
        out.contains(
            "3 failed run(s) of master since 2026-10-13: 2 answered, 1 owed, 1 past the 24h grace"
        ),
        "{out}"
    );

    // a longer grace, or a later start of the rule, and nothing is late
    let mut patient = late.to_vec();
    patient.extend(["--grace-hours", "72"]);
    assert_eq!(run_in(&root, &patient, &runs()).0, 0);
    let mut recent = late.to_vec();
    recent.extend(["--since", "2026-10-16"]);
    let (code, out, _) = run_in(&root, &recent, &runs());
    assert_eq!(code, 0);
    assert!(
        out.contains("0 failed run(s) of master since 2026-10-16"),
        "{out}"
    );

    // what is not a list of runs is a usage error
    let (code, _, err) = run_in(&root, &late, "{}");
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("not a list of runs"), "{err}");

    assert_eq!(f.snapshot(), before, "the question changed the repository");
}

#[test]
fn a_failing_log_is_matched_and_a_draft_is_printed_and_never_written() {
    let f = recorded();
    let root = f.root();
    let before = f.snapshot();

    // the log on standard input, in a case the record did not write it in
    let (code, out, err) = run_in(
        &root,
        &["postmortem", "diagnose", "--format", "json"],
        "step 4\nerror: The Lease Was Lost\n",
    );
    assert_eq!(code, 0, "{err}");
    let v = parsed(&out);
    assert_eq!(v["matches"].as_array().unwrap().len(), 1, "{out}");
    let m = &v["matches"][0];
    assert_eq!(m["id"], "pm-20261013-lease");
    assert_eq!(m["signals"], json!(["lease-lost"]));
    assert_eq!(m["cause"]["class"], "race");
    assert_eq!(m["fixed_by"], json!(["pr:1078"]));
    assert_eq!(m["prevention"], json!(["test:test/cases/00_x.sh"]));
    assert_eq!(m["related"], json!(["issue:I0001"]));

    f.write("failed.log", "nothing anyone has recorded\n");
    let (code, out, _) = run_in(
        &root,
        &["postmortem", "diagnose", "--file", "failed.log"],
        "",
    );
    assert_eq!(code, 0, "no match is an answer");
    assert!(
        out.starts_with("no recorded failure shows its signals"),
        "{out}"
    );
    f.remove("failed.log");

    // a draft for the run nobody answered, from the runs on standard input
    let draft = [
        "postmortem",
        "draft",
        "--run",
        "400",
        "--runs-json",
        "-",
        "--job",
        "rust (3)",
    ];
    let (code, out, err) = run_in(&root, &draft, &runs());
    assert_eq!(code, 0, "{err}");
    assert!(
        out.starts_with("---\nschema: post-mortem/v1\nid: pm-20261015-validate-400\n"),
        "{out}"
    );
    assert!(out.contains("status: draft\n") && out.contains("  class: undetermined\n"));
    assert!(out.contains("      - \"rust (3)\"\n"), "{out}");
    for heading in ["## What failed", "## Root cause", "## Fix", "## Prevention"] {
        assert!(out.contains(heading), "{heading} is missing:\n{out}");
    }
    assert_eq!(f.snapshot(), before, "a draft is printed, never written");

    // saved where the answer says, the draft is a record: indexed, accepted, and the run
    // it was drafted for is owed no longer
    f.write(".ai/repo/post-mortems/pm-20261015-validate-400.md", &out);
    f.commit("the draft, saved");
    let (code, listed, _) = run_in(&root, &["postmortem", "list", "--format", "json"], "");
    assert_eq!(code, 0);
    let listed = parsed(&listed);
    assert_eq!(listed["count"], 3);
    assert_eq!(listed["records"][0]["id"], "pm-20261015-validate-400");
    assert_eq!(listed["records"][0]["status"], "draft");
    assert_eq!(run_in(&root, &["postmortem", "check"], "").0, 0);
    let owed = [
        "postmortem",
        "owed",
        "--runs-json",
        "-",
        "--now",
        "2026-10-17T12:00:00Z",
    ];
    assert_eq!(run_in(&root, &owed, &runs()).0, 0);

    // a run the input does not hold is a missing artifact, and a run that is no run is usage
    let mut absent = draft.to_vec();
    absent[3] = "999";
    assert_eq!(run_in(&root, &absent, &runs()).0, 12);
    let mut hostile = draft.to_vec();
    hostile[3] = "12a";
    assert_eq!(run_in(&root, &hostile, &runs()).0, 2);
}
