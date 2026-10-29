//! Replays of the runs of 2026-10-28 through [`super::supervise::decide`], from what the forge
//! recorded of them, and the wiring of the workflow that runs the supervisor (I3903, N8 of the
//! incident review of that day).
//!
//! The fixtures are the forge's own JSON (`GET repos/<repo>/actions/runs/<id>` and its
//! `/jobs`), cut to the fields the decision reads and read through the same
//! [`super::runs::run_of`] and [`super::runs::job_of`] the command uses, so a replay judges
//! what the supervisor would have been handed at that moment.
//!
//! - Train 10b's legitimate run 40764355688 was created at 20:33:33Z and waited in its pull
//!   request's group until 22:21:01Z; its first job started at 22:24:09Z. Its
//!   `run_started_at` is its creation, so a clock read from it says 256 minutes at 00:50Z, past
//!   validate's 255, while the run had worked for 145. Replayed at 00:50Z: no cancel.
//! - Train 10b's twin 40713048283 was created at 15:09:14Z, one second after 40713047404 of the
//!   same commit, both from opening #2631 with `--label train` (`opened` and `labeled`). It
//!   waited in the group until 19:40:32Z and ran until it was cancelled by hand at 22:21:01Z.
//!   Replayed at 15:10Z: cancelled as a duplicate.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::runs::{job_of, run_of};
use super::supervise::{
    decide, liveness, CancelReason, HoldReason, RunObservation, Situation, SupersededGroup,
    SuperviseModel, Supervision, STALE_MINUTES, SUPERVISE_STALE,
};

/// Train 10b's pull request.
const PR: u64 = 2631;
/// Its head from 20:33Z.
const HEAD_C: &str = "3fd1d48d330b92604d6068f61773b6fe660808f0";
/// Its head before.
const HEAD_B: &str = "9a7830723a1f780d9e3951ac1d99206de70a0459";

/// Run 40764355688 as the forge recorded it at 22:55Z, and so at 00:50Z save its clock: the
/// run `queued` with its plan, build and cockpit done and the rest waiting for runners.
const RUN_40764355688: &str = r#"{"id":40764355688,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"3fd1d48d330b92604d6068f61773b6fe660808f0","status":"queued","conclusion":null,
 "created_at":"2026-10-28T20:33:33Z","run_started_at":"2026-10-28T20:33:33Z",
 "pull_requests":[{"number":2631}]}"#;

const JOBS_40764355688: &str = r#"[
 {"name":"plan","status":"completed","conclusion":"success","started_at":"2026-10-28T22:24:09Z"},
 {"name":"cockpit","status":"completed","conclusion":"success","started_at":"2026-10-28T22:29:32Z"},
 {"name":"build","status":"completed","conclusion":"success","started_at":"2026-10-28T22:42:36Z"},
 {"name":"coverage","status":"in_progress","conclusion":null,"started_at":"2026-10-28T22:48:36Z"},
 {"name":"rust (3)","status":"in_progress","conclusion":null,"started_at":"2026-10-28T22:28:47Z"},
 {"name":"rust (2)","status":"queued","conclusion":null,"started_at":"2026-10-28T22:24:29Z"},
 {"name":"macos","status":"completed","conclusion":"skipped","started_at":"2026-10-28T22:24:29Z"},
 {"name":"structure","status":"queued","conclusion":null,"started_at":"2026-10-28T22:48:34Z"},
 {"name":"suite-shard (1)","status":"queued","conclusion":null,"started_at":"2026-10-28T22:54:27Z"}
]"#;

/// The two runs of the old head as they stood at 00:50Z: both completed.
const RUN_40713047404_DONE: &str = r#"{"id":40713047404,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"9a7830723a1f780d9e3951ac1d99206de70a0459","status":"completed",
 "conclusion":"failure","created_at":"2026-10-28T15:09:13Z",
 "run_started_at":"2026-10-28T15:09:13Z","pull_requests":[{"number":2631}]}"#;

const RUN_40713048283_DONE: &str = r#"{"id":40713048283,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"9a7830723a1f780d9e3951ac1d99206de70a0459","status":"completed",
 "conclusion":"cancelled","created_at":"2026-10-28T15:09:14Z",
 "run_started_at":"2026-10-28T15:09:14Z","pull_requests":[{"number":2631}]}"#;

/// At 15:10Z: the `opened` run 40713047404 queued with its plan job waiting for a hosted
/// runner (it started at 15:50:59Z) ...
const RUN_40713047404_AT_1510: &str = r#"{"id":40713047404,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"9a7830723a1f780d9e3951ac1d99206de70a0459","status":"queued","conclusion":null,
 "created_at":"2026-10-28T15:09:13Z","run_started_at":"2026-10-28T15:09:13Z",
 "pull_requests":[{"number":2631}]}"#;

const JOBS_40713047404_AT_1510: &str = r#"[
 {"name":"plan","status":"queued","conclusion":null,"started_at":"2026-10-28T15:09:14Z"}
]"#;

/// ... and the `labeled` run 40713048283 pending in the group behind it, with no job.
const RUN_40713048283_AT_1510: &str = r#"{"id":40713048283,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"9a7830723a1f780d9e3951ac1d99206de70a0459","status":"pending","conclusion":null,
 "created_at":"2026-10-28T15:09:14Z","run_started_at":"2026-10-28T15:09:14Z",
 "pull_requests":[{"number":2631}]}"#;

fn t(stamp: &str) -> u64 {
    crate::peers::parse_rfc3339(stamp).unwrap()
}

fn run(json: &str, jobs: Option<&str>) -> RunObservation {
    let v: Value = serde_json::from_str(json).unwrap();
    let mut r = run_of(&v).unwrap();
    if let Some(j) = jobs {
        let a: Vec<Value> = serde_json::from_str(j).unwrap();
        r.jobs = a.iter().map(|j| job_of(j).unwrap()).collect();
    }
    r
}

/// The model as `.ai/repo/ci/supervise.yaml` declares validate.
fn model() -> SuperviseModel {
    let m = super::supervise::read_model(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
    )
    .unwrap();
    assert_eq!(m.bounds["validate"].running_minutes, 255);
    assert_eq!(m.bounds["validate"].queued_minutes, 240);
    assert_eq!(
        m.bounds["validate"].superseded,
        SupersededGroup::PerPullRequest
    );
    m
}

fn replay(runs: &[RunObservation], head: &str, full: &[u64], at: &str) -> Supervision {
    let m = model();
    let heads: BTreeMap<u64, String> = [(PR, head.to_string())].into_iter().collect();
    let full: BTreeSet<u64> = full.iter().copied().collect();
    decide(&Situation {
        runs,
        pr_heads: &heads,
        full: &full,
        default_branch: "master",
        model: &m,
        now: t(at),
        self_run: None,
    })
}

#[test]
fn train_10bs_waiting_run_replayed_at_00_50z_is_not_cancelled() {
    let c = run(RUN_40764355688, Some(JOBS_40764355688));
    let runs = vec![
        run(RUN_40713047404_DONE, None),
        run(RUN_40713048283_DONE, None),
        c.clone(),
    ];
    // the old clock: from run_started_at, which is its creation, it reads past the bound
    let by_run_started = (t("2026-10-29T00:50:00Z") - t(c.run_started_at.as_deref().unwrap())) / 60;
    assert!(by_run_started > 255, "{by_run_started}");
    // the job-start clock: 145 minutes of work
    assert_eq!(
        c.first_job_start().unwrap(),
        Some(t("2026-10-28T22:24:09Z"))
    );
    let s = replay(&runs, HEAD_C, &[], "2026-10-29T00:50:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // and so once the forge calls it in_progress
    let mut running = c.clone();
    running.status = "in_progress".into();
    let s = replay(&[running.clone()], HEAD_C, &[], "2026-10-29T00:50:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // it still ends at its bound, from its first job: 22:24:09Z + 255 min = 02:39:09Z
    let s = replay(&[running], HEAD_C, &[], "2026-10-29T02:41:00Z");
    assert_eq!(
        s.decisions[0].reason,
        CancelReason::RunningPastBound {
            minutes: 256,
            bound: 255
        }
    );
    // with no job started it is never running past its bound, however old
    let mut waiting = c;
    waiting.jobs.clear();
    waiting.status = "in_progress".into();
    let s = replay(&[waiting], HEAD_C, &[], "2026-10-29T00:50:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    assert_eq!(
        s.held[0].reason,
        HoldReason::IntegrationTopWaits {
            minutes: 256,
            bound: 240
        },
        "an integration top past the queue bound waits, it is not cancelled"
    );
}

#[test]
fn train_10bs_labelled_twin_replayed_by_15_10z_is_cancelled() {
    let runs = vec![
        run(RUN_40713047404_AT_1510, Some(JOBS_40713047404_AT_1510)),
        run(RUN_40713048283_AT_1510, None),
    ];
    // the queued plan job's started_at is filled, and it is not started
    assert_eq!(runs[0].first_job_start().unwrap(), None);
    for at in ["2026-10-28T15:09:30Z", "2026-10-28T15:10:00Z"] {
        let s = replay(&runs, HEAD_B, &[], at);
        assert_eq!(s.decisions.len(), 1, "{at}: {s:?}");
        let d = &s.decisions[0];
        assert_eq!(d.run, 40713048283);
        assert_eq!(
            d.reason,
            CancelReason::DuplicateRun {
                pull_request: PR,
                earlier: 40713047404,
                earlier_state: "queued".into()
            }
        );
        assert_eq!(d.reason.word(), "duplicate_run");
    }
    // the older of the two is never the duplicate
    let s = replay(&runs[..1], HEAD_B, &[], "2026-10-28T15:10:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // a twin of a commit already judged is a duplicate too
    let mut judged = run(RUN_40713047404_DONE, None);
    judged.status = "completed".into();
    let s = replay(
        &[judged, run(RUN_40713048283_AT_1510, None)],
        HEAD_B,
        &[],
        "2026-10-28T19:41:00Z",
    );
    assert_eq!(
        s.decisions[0].reason,
        CancelReason::DuplicateRun {
            pull_request: PR,
            earlier: 40713047404,
            earlier_state: "failure".into()
        }
    );
    // and of one that was cancelled, it is not: a cancelled run judged nothing
    let s = replay(
        &[run(RUN_40713048283_DONE, None), {
            let mut again = run(RUN_40713048283_AT_1510, None);
            again.id = 40713099999;
            again.created_at = "2026-10-28T22:30:00Z".into();
            again
        }],
        HEAD_B,
        &[],
        "2026-10-28T22:31:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
}

#[test]
fn a_same_commit_pair_labelled_ci_full_is_held() {
    let runs = vec![
        run(RUN_40713047404_AT_1510, Some(JOBS_40713047404_AT_1510)),
        run(RUN_40713048283_AT_1510, None),
    ];
    let s = replay(&runs, HEAD_B, &[PR], "2026-10-28T15:10:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    assert_eq!(
        s.held[0].reason,
        HoldReason::FullRunRequested {
            pull_request: PR,
            earlier: 40713047404
        }
    );
    // held past the queue bound too: an integration top is never ended for waiting
    let mut twin = run(RUN_40713048283_AT_1510, None);
    twin.head_branch = "integration/train-10b-20261028".into();
    let s = replay(&[twin], HEAD_B, &[PR], "2026-10-28T19:20:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
}

#[test]
fn the_newest_supervise_success_31_minutes_old_is_stale_and_9_is_not() {
    let now = t("2026-10-28T23:00:00Z");
    let l = liveness(Ok(Some("2026-10-28T22:29:00Z")), now);
    assert_eq!(l.minutes, Some(31));
    assert_eq!(l.finding.as_deref(), Some(SUPERVISE_STALE));
    assert_eq!(l.bound_minutes, STALE_MINUTES);
    let l = liveness(Ok(Some("2026-10-28T22:51:00Z")), now);
    assert_eq!(l.minutes, Some(9));
    assert_eq!(l.finding, None, "{l:?}");
    // none at all, or one that cannot be read, is a finding: silence is not alive
    assert_eq!(
        liveness(Ok(None), now).finding.as_deref(),
        Some(SUPERVISE_STALE)
    );
    assert!(liveness(Err("HTTP 502".into()), now).finding.is_some());
    assert!(liveness(Ok(Some("yesterday")), now).finding.is_some());
    // the bound itself is not past it
    assert_eq!(
        liveness(Ok(Some("2026-10-28T22:30:00Z")), now).finding,
        None
    );
}

// ---------------------------------------------------------------- the workflow's wiring

fn workflow(name: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.github/workflows")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The non-comment lines.
fn code(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// GitHub's concurrency group, as far as a burst is concerned: one entry runs, at most one
/// waits, a newer request replaces the waiting one, and with `cancel_in_progress` it also
/// cancels the running one. Requests at `at` (seconds), each apply taking `takes` seconds;
/// the applies that complete, as (requested at, completed at).
fn concurrency(at: &[u64], takes: u64, cancel_in_progress: bool) -> Vec<(u64, u64)> {
    let mut done = Vec::new();
    let mut running: Option<(u64, u64)> = None; // (requested, started)
    let mut pending: Option<u64> = None;
    let mut queue: Vec<u64> = at.to_vec();
    queue.sort_unstable();
    let mut i = 0;
    loop {
        let finish = running.map(|(_, s)| s + takes);
        let next = queue.get(i).copied();
        match (finish, next) {
            // the running apply completes, and the waiting one starts then
            (Some(f), Some(n)) if f <= n => {
                done.push((running.unwrap().0, f));
                running = pending.take().map(|r| (r, f));
            }
            (_, Some(n)) => {
                i += 1;
                if running.is_none() || cancel_in_progress {
                    running = Some((n, n));
                    pending = None;
                } else {
                    pending = Some(n);
                }
            }
            (Some(f), None) => {
                done.push((running.unwrap().0, f));
                running = pending.take().map(|r| (r, f));
            }
            (None, None) => break,
        }
    }
    done
}

#[test]
fn five_validate_requests_within_a_minute_let_a_supervise_apply_complete() {
    let text = workflow("supervise.yml");
    let c = code(&text);
    // every requested validate run starts it, and the clock is its watchdog
    assert!(c.contains("workflow_run:"), "no workflow_run trigger");
    assert!(
        c.contains("workflows: [validate]"),
        "not started by validate"
    );
    assert!(
        c.contains("types: [requested]"),
        "not on a requested validate run"
    );
    assert!(c.contains("- cron: "), "no schedule");
    // a burst never cancels the apply that runs
    assert!(c.contains("cancel-in-progress: false"), "{c}");
    assert!(!c.contains("cancel-in-progress: true"), "{c}");
    let cancel_in_progress = c.contains("cancel-in-progress: true");
    // what one apply takes: a downloaded executable and a handful of API calls, bounded
    let timeouts: Vec<u64> = c
        .lines()
        .filter_map(|l| l.trim().strip_prefix("timeout-minutes:"))
        .map(|n| n.trim().parse().unwrap())
        .collect();
    assert!(!timeouts.is_empty(), "no job bound");
    assert!(timeouts.iter().all(|m| *m <= 5), "{timeouts:?}");
    assert!(
        !c.contains("cargo build"),
        "the supervisor compiles instead of downloading"
    );
    assert!(c.contains("prs supervise --apply"));
    // on the pool that cannot be starved by validate, with the hosted pool as the fallback
    assert!(c.contains("[self-hosted, lundra]"), "not on lundra");
    assert!(c.contains("ubuntu-latest"), "no hosted fallback");
    assert!(c.contains("continue-on-error: true"));
    assert!(c.contains("fail-fast: false"));
    // each lane its own group, so a lane with no runner never holds the other
    assert!(c.contains("group: supervise-${{ matrix.lane }}"), "{c}");

    // five requests in 60 s, each apply taking its whole bound: the first apply completes,
    // and so does the one the last request leaves waiting
    let burst = [0, 12, 24, 36, 48];
    let takes = timeouts.iter().max().unwrap() * 60;
    let done = concurrency(&burst, takes, cancel_in_progress);
    assert!(!done.is_empty(), "no apply completed");
    assert_eq!(
        done[0],
        (0, takes),
        "the first apply was cancelled by the burst"
    );
    assert_eq!(
        done.last().unwrap().0,
        48,
        "the newest request's observation never ran"
    );
    // what the old wiring did: each request cancelled the apply before it, and an apply
    // that compiles for twenty minutes under a request every ten never completes
    let ticks: Vec<u64> = (0..12).map(|k| k * 600).collect();
    assert!(concurrency(&ticks[..11], 20 * 60, true)
        .iter()
        .all(|(r, _)| *r == 6000));
    assert_eq!(concurrency(&burst, takes, true)[0].0, 48);
}

#[test]
fn the_queue_workflow_fails_on_a_stale_supervisor() {
    let c = code(&workflow("queue.yml"));
    assert!(c.contains("prs supervise --format json"), "{c}");
    assert!(c.contains(".liveness.finding"), "{c}");
    assert!(
        c.contains("actions: read"),
        "listing runs needs actions read"
    );
    assert!(!c.contains("--apply"), "the queue never acts");
}
