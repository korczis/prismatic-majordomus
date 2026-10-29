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
//!
//! The review of #2645 on 2026-10-29 added four runs that reached their verdict and that a
//! clock from the run's first job would have cut (40576851731, 40680986818, release
//! 38013111243's attempt 2, 40747313397's attempt 3), the queue clock of a re-run, the
//! protected runs that wait, and the limits of the duplicate rule: a re-run, a reopen, a head
//! that goes A, B, A, and a train and its member at one sha.
//!
//! Its adversarial review added two pairs. #2678's twin 40811621641 was admitted when
//! 40811610939 passed, and its cancel ran its `ci` job over the pass: replayed at 05:35Z, no
//! cancel. #2674's 40801560222, with a job past its bound, is ended while a twin of it is held to
//! take its place, and its real second run 2.5 hours later is no twin at all.
//!
//! The verification of that fix carried both pairs on to the twin's admission. A twin pending in
//! its group for hours is admitted with its creation and `run_started_at` hours old, so the queue
//! clock starts at its admission (40811621641: created 03:20:59Z, admitted 05:31:53Z); and the
//! twin of #2674's ended run is held while that run runs only its `ci`, then waits for a runner,
//! and is cancelled on no tick from 06:06Z to 06:50Z.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::runs::{job_of, run_of};
use super::supervise::{
    decide, liveness_of, CancelReason, HoldReason, JobObservation, RunObservation, Situation,
    SupersededGroup, SuperviseModel, Supervision, STALE_MINUTES, SUPERVISE_STALE,
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
 "pull_requests":[{"number":2631,"head":{"ref":"integration/train-10b-20261028"}}]}"#;

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
 "run_started_at":"2026-10-28T15:09:13Z","pull_requests":[{"number":2631,"head":{"ref":"integration/train-10b-20261028"}}]}"#;

const RUN_40713048283_DONE: &str = r#"{"id":40713048283,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"9a7830723a1f780d9e3951ac1d99206de70a0459","status":"completed",
 "conclusion":"cancelled","created_at":"2026-10-28T15:09:14Z",
 "run_started_at":"2026-10-28T15:09:14Z","pull_requests":[{"number":2631,"head":{"ref":"integration/train-10b-20261028"}}]}"#;

/// At 15:10Z: the `opened` run 40713047404 queued with its plan job waiting for a hosted
/// runner (it started at 15:50:59Z) ...
const RUN_40713047404_AT_1510: &str = r#"{"id":40713047404,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"9a7830723a1f780d9e3951ac1d99206de70a0459","status":"queued","conclusion":null,
 "created_at":"2026-10-28T15:09:13Z","run_started_at":"2026-10-28T15:09:13Z",
 "pull_requests":[{"number":2631,"head":{"ref":"integration/train-10b-20261028"}}]}"#;

const JOBS_40713047404_AT_1510: &str = r#"[
 {"name":"plan","status":"queued","conclusion":null,"started_at":"2026-10-28T15:09:14Z"}
]"#;

/// ... and the `labeled` run 40713048283 pending in the group behind it, with no job.
const RUN_40713048283_AT_1510: &str = r#"{"id":40713048283,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"integration/train-10b-20261028",
 "head_sha":"9a7830723a1f780d9e3951ac1d99206de70a0459","status":"pending","conclusion":null,
 "created_at":"2026-10-28T15:09:14Z","run_started_at":"2026-10-28T15:09:14Z",
 "pull_requests":[{"number":2631,"head":{"ref":"integration/train-10b-20261028"}}]}"#;

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
    // the job clock: its longest running job, rust (3), has run 141 minutes
    let (job, minutes) = c
        .longest_running_job(t("2026-10-29T00:50:00Z"))
        .unwrap()
        .unwrap();
    assert_eq!((job.name.as_str(), minutes), ("rust (3)", 141));
    let s = replay(&runs, HEAD_C, &[], "2026-10-29T00:50:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // and so once the forge calls it in_progress
    let mut running = c.clone();
    running.status = "in_progress".into();
    let s = replay(&[running.clone()], HEAD_C, &[], "2026-10-29T00:50:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // a job past its bound is named, from that job's start: rust (3) at 22:28:47Z + 255 min
    // is 02:43:47Z; an integration top is held for it, never ended
    let s = replay(&[running], HEAD_C, &[], "2026-10-29T02:45:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    assert_eq!(
        s.held[0].reason,
        HoldReason::IntegrationTopRuns {
            job: "rust (3)".into(),
            minutes: 256,
            bound: 255
        }
    );
    // with no job started it is never running past its bound, however old; its queue is
    // measured from its admission at 22:21:01Z, when its plan job was created, not from its
    // creation at 20:33:33Z: 148 minutes at 00:50Z, so nothing to say
    let mut waiting = c;
    waiting.jobs = vec![JobObservation {
        name: "plan".into(),
        status: "queued".into(),
        started_at: Some("2026-10-28T22:21:01Z".into()),
        created_at: Some("2026-10-28T22:21:01Z".into()),
        conclusion: None,
    }];
    waiting.status = "in_progress".into();
    let s = replay(
        std::slice::from_ref(&waiting),
        HEAD_C,
        &[],
        "2026-10-29T00:50:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    assert!(s.held.is_empty(), "{s:?}");
    // and 241 minutes after its admission it is past the queue bound, and held
    let s = replay(&[waiting], HEAD_C, &[], "2026-10-29T02:22:01Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    assert_eq!(
        s.held[0].reason,
        HoldReason::IntegrationTopWaits {
            minutes: 241,
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
    assert!(!runs[0].any_job_started());
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
    // once the older run is judged, failed or passed, its twin is never a duplicate: the group
    // admits the twin then, and a cancel of an admitted run runs its `ci` job over the verdict
    let judged = run(RUN_40713047404_DONE, None);
    assert_eq!(judged.conclusion.as_deref(), Some("failure"));
    let s = replay(
        &[judged.clone(), run(RUN_40713048283_AT_1510, None)],
        HEAD_B,
        &[],
        "2026-10-28T19:41:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    let mut passed = judged;
    passed.conclusion = Some("success".into());
    let s = replay(
        &[passed, run(RUN_40713048283_AT_1510, None)],
        HEAD_B,
        &[],
        "2026-10-28T19:41:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
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

/// The liveness of any lane's supervision, over every run.
fn live(newest: Result<Option<&str>, String>, now: u64) -> super::supervise::Liveness {
    liveness_of(newest, now, None, None)
}

#[test]
fn the_newest_supervise_success_31_minutes_old_is_stale_and_9_is_not() {
    let now = t("2026-10-28T23:00:00Z");
    let l = live(Ok(Some("2026-10-28T22:29:00Z")), now);
    assert_eq!(l.minutes, Some(31));
    assert_eq!(l.finding.as_deref(), Some(SUPERVISE_STALE));
    assert_eq!(l.bound_minutes, STALE_MINUTES);
    let l = live(Ok(Some("2026-10-28T22:51:00Z")), now);
    assert_eq!(l.minutes, Some(9));
    assert_eq!(l.finding, None, "{l:?}");
    // none at all, or one that cannot be read, is a finding: silence is not alive
    assert_eq!(
        live(Ok(None), now).finding.as_deref(),
        Some(SUPERVISE_STALE)
    );
    assert!(live(Err("HTTP 502".into()), now).finding.is_some());
    assert!(live(Ok(Some("yesterday")), now).finding.is_some());
    // the bound itself is not past it
    assert_eq!(live(Ok(Some("2026-10-28T22:30:00Z")), now).finding, None);
}

// ---------------------------------------------------------------- the clocks of real runs
//
// The review of #2645 (2026-10-29) replayed four more of this repository's runs, each of which
// reached its verdict, against a clock read from the run's first job: each would have been
// force-cancelled. A bound is a job's own timeout plus thirty minutes, so it is a job that is
// held to it, and only a job that is running: the waits between a run's stages, and the jobs
// an earlier attempt left behind, are no job's time.
//
// Each fixture is the forge's record of the run's jobs once it ended (`name|started_at|
// completed_at|conclusion`, the matrix names cut to their first value), and `jobs_at` turns it
// into what the forge listed at a moment: done by then, running, or not yet taken.

/// 40576851731 (pull_request, fix/a-followers-read-is-its-own-servers, success): its plan
/// started at 20:52:10Z and its last job ended at 06:31:57Z the next day, 579 min later; its
/// longest job ran 97 min, and interaction-probe-shard (2) waited 184 min for a runner.
const RUN_40576851731: &str = r#"{"id":40576851731,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"fix/a-followers-read-is-its-own-servers",
 "head_sha":"81ef931a3e156907f1784c64c509884f565c17f0","status":"in_progress",
 "conclusion":null,"created_at":"2026-10-27T20:18:26Z","run_started_at":"2026-10-27T20:18:26Z",
 "run_attempt":1,"pull_requests":[]}"#;

/// 40680986818 (workflow_dispatch on feature/lundra-is-the-first-wave, one attempt): its plan
/// started at 11:42:14Z, its last stage between 15:59:34Z and 16:02:45Z, its verdict at
/// 16:03:12Z.
const RUN_40680986818: &str = r#"{"id":40680986818,"name":"validate",
 "path":".github/workflows/validate.yml","event":"workflow_dispatch",
 "head_branch":"feature/lundra-is-the-first-wave",
 "head_sha":"f37ab83f1e51e30953f0276441198e85c2e350af","status":"in_progress",
 "conclusion":null,"created_at":"2026-10-28T11:34:18Z","run_started_at":"2026-10-28T11:34:18Z",
 "run_attempt":1,"pull_requests":[{"number":2596,
 "head":{"ref":"feature/lundra-is-the-first-wave"}}]}"#;

/// 38013111243 (release of tag v0.19.1, push, attempt 2, success): attempt 2 started at
/// 07:15:55Z; its plan, builds and publish are attempt 1's, carried over and listed with their
/// first start, 01:25:21Z; attempt 2's smoke jobs ran from 07:19Z to 07:34Z.
const RUN_38013111243: &str = r#"{"id":38013111243,"name":"release",
 "path":".github/workflows/release.yml","event":"push","head_branch":"v0.19.1",
 "head_sha":"fe0b58b67b63d7a95b15236d9de222ea2dc2fab5","status":"in_progress",
 "conclusion":null,"created_at":"2026-10-10T01:25:18Z","run_started_at":"2026-10-10T07:15:55Z",
 "run_attempt":2,"pull_requests":[]}"#;

/// 40747313397 (pull_request #2349, attempt 3): created at 18:40:29Z, attempt 3 started at
/// 22:30:09Z; plan, build, rust and cockpit are carried from 20:54:39Z on, and attempt 3's
/// site-probe shards ran until 01:24Z.
const RUN_40747313397: &str = r#"{"id":40747313397,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"feature/the-site-names-the-newest-prerelease",
 "head_sha":"d560cbc10417177f017a29414d2d0830d81b96e1","status":"in_progress",
 "conclusion":null,"created_at":"2026-10-28T18:40:29Z","run_started_at":"2026-10-28T22:30:09Z",
 "run_attempt":3,"pull_requests":[{"number":2349,
 "head":{"ref":"feature/the-site-names-the-newest-prerelease"}}]}"#;

const JOBS_40576851731: &str = "\
 plan|2026-10-27T20:52:10Z|2026-10-27T20:52:30Z|success\n\
 cockpit|2026-10-27T22:12:41Z|2026-10-27T22:31:34Z|success\n\
 rustdoc-surface|2026-10-27T22:02:58Z|2026-10-27T22:06:13Z|success\n\
 rust (2)|2026-10-27T21:43:51Z|2026-10-27T22:07:02Z|success\n\
 coverage|2026-10-27T22:49:38Z|2026-10-27T23:33:40Z|success\n\
 rust (3)|2026-10-27T22:35:01Z|2026-10-27T23:19:34Z|success\n\
 build|2026-10-27T22:25:12Z|2026-10-27T22:30:11Z|success\n\
 rust (1)|2026-10-27T22:36:14Z|2026-10-27T23:23:37Z|success\n\
 macos|2026-10-27T20:52:30Z|2026-10-27T20:52:30Z|skipped\n\
 bench|2026-10-27T20:52:30Z|2026-10-27T20:52:30Z|skipped\n\
 install|2026-10-27T20:52:30Z|2026-10-27T20:52:30Z|skipped\n\
 ui-checks|2026-10-27T23:57:44Z|2026-10-28T00:06:06Z|success\n\
 suite-shard (3)|2026-10-27T23:53:00Z|2026-10-28T01:30:15Z|success\n\
 site|2026-10-28T00:15:54Z|2026-10-28T00:45:21Z|success\n\
 structure|2026-10-28T00:33:33Z|2026-10-28T01:25:14Z|success\n\
 mesh-lab|2026-10-28T00:41:02Z|2026-10-28T00:42:50Z|success\n\
 suite-shard (2)|2026-10-28T00:13:54Z|2026-10-28T01:40:42Z|success\n\
 ui-shard (2)|2026-10-28T00:27:22Z|2026-10-28T00:51:02Z|success\n\
 suite-shard (1)|2026-10-28T01:07:45Z|2026-10-28T02:08:21Z|success\n\
 ui-shard (3)|2026-10-28T00:40:58Z|2026-10-28T01:05:29Z|success\n\
 suite-shard (4)|2026-10-28T00:45:24Z|2026-10-28T02:10:11Z|success\n\
 ui-shard (1)|2026-10-28T01:11:30Z|2026-10-28T01:33:11Z|success\n\
 ui-shard (4)|2026-10-28T01:25:34Z|2026-10-28T01:46:06Z|success\n\
 site-probe-shard (1)|2026-10-28T02:11:09Z|2026-10-28T02:18:54Z|success\n\
 interaction-probe-shard (3)|2026-10-28T02:15:49Z|2026-10-28T02:35:49Z|success\n\
 site-probe-shard (2)|2026-10-28T02:21:44Z|2026-10-28T02:30:20Z|success\n\
 interaction-probe-shard (5)|2026-10-28T02:03:31Z|2026-10-28T02:21:41Z|success\n\
 site-probe-shard (6)|2026-10-28T02:41:00Z|2026-10-28T02:50:41Z|success\n\
 site-probe-shard (3)|2026-10-28T02:20:52Z|2026-10-28T02:27:43Z|success\n\
 interaction-probe-shard (6)|2026-10-28T03:04:15Z|2026-10-28T03:24:33Z|success\n\
 interaction-probe-shard (1)|2026-10-28T03:02:02Z|2026-10-28T03:23:54Z|success\n\
 site-probe-shard (4)|2026-10-28T03:26:45Z|2026-10-28T03:35:42Z|success\n\
 site-probe-shard (5)|2026-10-28T03:27:39Z|2026-10-28T03:36:29Z|success\n\
 interaction-probe-shard (4)|2026-10-28T02:49:57Z|2026-10-28T03:11:47Z|success\n\
 interaction-probe-shard (2)|2026-10-28T03:49:42Z|2026-10-28T04:15:58Z|success\n\
 ui|2026-10-28T03:50:45Z|2026-10-28T03:51:26Z|success\n\
 suite|2026-10-28T03:56:44Z|2026-10-28T03:57:06Z|success\n\
 site-probe|2026-10-28T04:31:06Z|2026-10-28T04:31:21Z|success\n\
 evidence|2026-10-28T04:52:11Z|2026-10-28T04:57:49Z|success\n\
 interactions|2026-10-28T05:13:25Z|2026-10-28T05:13:43Z|success\n\
 pages|2026-10-28T04:57:50Z|2026-10-28T04:57:49Z|skipped\n\
 ci|2026-10-28T06:31:37Z|2026-10-28T06:31:57Z|success\n\
";

const JOBS_40680986818: &str = "\
 plan|2026-10-28T11:42:14Z|2026-10-28T11:42:33Z|success\n\
 cockpit|2026-10-28T11:42:36Z|2026-10-28T12:10:31Z|failure\n\
 rust (3)|2026-10-28T11:51:45Z|2026-10-28T12:34:04Z|success\n\
 rustdoc-surface|2026-10-28T12:03:00Z|2026-10-28T12:04:52Z|success\n\
 rust (2)|2026-10-28T11:52:21Z|2026-10-28T12:12:50Z|success\n\
 coverage|2026-10-28T11:42:36Z|2026-10-28T12:08:30Z|failure\n\
 build|2026-10-28T11:57:56Z|2026-10-28T12:00:56Z|success\n\
 rust (1)|2026-10-28T12:06:11Z|2026-10-28T12:50:53Z|success\n\
 bench|2026-10-28T11:42:33Z|2026-10-28T11:42:33Z|skipped\n\
 macos|2026-10-28T11:42:33Z|2026-10-28T11:42:33Z|skipped\n\
 install|2026-10-28T11:42:33Z|2026-10-28T11:42:33Z|skipped\n\
 mesh-lab|2026-10-28T12:01:58Z|2026-10-28T12:05:54Z|failure\n\
 structure|2026-10-28T12:06:26Z|2026-10-28T12:58:08Z|failure\n\
 site|2026-10-28T12:11:12Z|2026-10-28T12:34:17Z|success\n\
 ui-shard (2)|2026-10-28T12:12:53Z|2026-10-28T12:33:44Z|success\n\
 ui-shard (4)|2026-10-28T12:22:09Z|2026-10-28T12:52:22Z|success\n\
 ui-checks|2026-10-28T12:29:03Z|2026-10-28T12:37:31Z|success\n\
 ui-shard (3)|2026-10-28T12:29:37Z|2026-10-28T13:01:27Z|success\n\
 suite-shard (2)|2026-10-28T12:31:25Z|2026-10-28T14:33:31Z|failure\n\
 ui-shard (1)|2026-10-28T12:27:28Z|2026-10-28T12:47:50Z|success\n\
 suite-shard (1)|2026-10-28T12:33:50Z|2026-10-28T13:32:55Z|success\n\
 suite-shard (3)|2026-10-28T12:29:11Z|2026-10-28T13:39:36Z|success\n\
 suite-shard (4)|2026-10-28T12:34:27Z|2026-10-28T13:26:48Z|success\n\
 site-probe-shard (2)|2026-10-28T12:42:49Z|2026-10-28T12:53:06Z|success\n\
 interaction-probe-shard (6)|2026-10-28T12:43:50Z|2026-10-28T13:01:51Z|success\n\
 interaction-probe-shard (3)|2026-10-28T12:42:02Z|2026-10-28T13:01:14Z|success\n\
 interaction-probe-shard (1)|2026-10-28T12:55:27Z|2026-10-28T13:13:00Z|success\n\
 site-probe-shard (6)|2026-10-28T13:01:13Z|2026-10-28T13:10:13Z|success\n\
 site-probe-shard (4)|2026-10-28T13:02:41Z|2026-10-28T13:10:51Z|success\n\
 interaction-probe-shard (2)|2026-10-28T12:54:57Z|2026-10-28T13:15:02Z|success\n\
 site-probe-shard (5)|2026-10-28T12:46:54Z|2026-10-28T12:56:06Z|success\n\
 site-probe-shard (1)|2026-10-28T12:59:22Z|2026-10-28T13:09:09Z|success\n\
 interaction-probe-shard (5)|2026-10-28T12:48:12Z|2026-10-28T13:09:12Z|success\n\
 interaction-probe-shard (4)|2026-10-28T13:10:53Z|2026-10-28T13:38:44Z|success\n\
 site-probe-shard (3)|2026-10-28T13:12:28Z|2026-10-28T13:19:30Z|success\n\
 ui|2026-10-28T15:59:34Z|2026-10-28T16:01:03Z|success\n\
 site-probe|2026-10-28T15:59:34Z|2026-10-28T15:59:50Z|success\n\
 interactions|2026-10-28T15:59:37Z|2026-10-28T15:59:54Z|success\n\
 suite|2026-10-28T15:59:53Z|2026-10-28T16:00:17Z|failure\n\
 evidence|2026-10-28T16:00:20Z|2026-10-28T16:02:41Z|success\n\
 ci|2026-10-28T16:02:45Z|2026-10-28T16:03:12Z|failure\n\
 pages|2026-10-28T16:02:42Z|2026-10-28T16:02:42Z|skipped\n\
";

const JOBS_38013111243: &str = "\
 plan|2026-10-10T01:25:21Z|2026-10-10T01:28:54Z|success\n\
 build (linux-x86_64-gnu)|2026-10-10T01:28:57Z|2026-10-10T01:36:07Z|success\n\
 smoke (macos-x86_64)|2026-10-10T07:26:05Z|2026-10-10T07:26:28Z|success\n\
 smoke (macos-aarch64)|2026-10-10T07:19:54Z|2026-10-10T07:20:14Z|success\n\
 smoke (linux-aarch64-gnu)|2026-10-10T07:26:15Z|2026-10-10T07:26:27Z|success\n\
 smoke (linux-x86_64-gnu)|2026-10-10T07:34:13Z|2026-10-10T07:34:22Z|success\n\
 smoke (linux-x86_64-musl)|2026-10-10T07:26:05Z|2026-10-10T07:26:18Z|success\n\
 smoke (linux-aarch64-musl)|2026-10-10T07:34:27Z|2026-10-10T07:34:36Z|success\n\
 build (linux-x86_64-musl)|2026-10-10T01:28:57Z|2026-10-10T01:34:42Z|success\n\
 publish|2026-10-10T01:42:45Z|2026-10-10T01:56:22Z|success\n\
 build (macos-x86_64)|2026-10-10T01:28:58Z|2026-10-10T01:42:42Z|success\n\
 build (linux-aarch64-musl)|2026-10-10T01:29:00Z|2026-10-10T01:35:33Z|success\n\
 build (linux-aarch64-gnu)|2026-10-10T01:29:00Z|2026-10-10T01:35:21Z|success\n\
 build (macos-aarch64)|2026-10-10T01:29:01Z|2026-10-10T01:38:55Z|success\n\
";

const JOBS_40747313397: &str = "\
 coverage|2026-10-28T22:48:22Z|2026-10-28T23:26:30Z|success\n\
 ui-shard (3)|2026-10-28T23:15:13Z|2026-10-28T23:46:33Z|success\n\
 structure|2026-10-28T23:00:54Z|2026-10-28T23:47:57Z|success\n\
 ui-shard (4)|2026-10-28T22:30:11Z|2026-10-29T00:24:28Z|cancelled\n\
 suite-shard (1)|2026-10-28T23:20:12Z|2026-10-29T00:25:27Z|cancelled\n\
 suite-shard (4)|2026-10-28T23:36:44Z|2026-10-29T00:25:57Z|cancelled\n\
 site|2026-10-28T22:30:11Z|2026-10-29T00:24:28Z|cancelled\n\
 ui-checks|2026-10-29T00:19:34Z|2026-10-29T00:24:30Z|cancelled\n\
 suite-shard (3)|2026-10-28T23:48:37Z|2026-10-29T00:25:16Z|cancelled\n\
 rust (3)|2026-10-28T23:37:24Z|2026-10-29T00:19:31Z|success\n\
 suite-shard (2)|2026-10-28T22:30:11Z|2026-10-29T00:24:28Z|cancelled\n\
 ui-shard (2)|2026-10-28T22:30:11Z|2026-10-29T00:24:28Z|cancelled\n\
 ui-shard (1)|2026-10-29T00:16:42Z|2026-10-29T00:25:45Z|cancelled\n\
 macos|2026-10-28T22:30:12Z|2026-10-28T20:55:01Z|skipped\n\
 install|2026-10-28T22:30:12Z|2026-10-28T20:55:01Z|skipped\n\
 cockpit|2026-10-28T21:19:08Z|2026-10-28T21:42:38Z|success\n\
 build|2026-10-28T21:26:52Z|2026-10-28T21:32:56Z|success\n\
 rustdoc-surface|2026-10-28T20:59:17Z|2026-10-28T21:01:25Z|success\n\
 bench|2026-10-28T22:30:18Z|2026-10-28T20:55:01Z|skipped\n\
 plan|2026-10-28T20:54:39Z|2026-10-28T20:55:01Z|success\n\
 mesh-lab|2026-10-28T21:48:55Z|2026-10-28T21:50:41Z|success\n\
 rust (2)|2026-10-28T21:35:17Z|2026-10-28T21:59:12Z|success\n\
 rust (1)|2026-10-28T21:20:04Z|2026-10-28T21:57:21Z|success\n\
 site-probe-shard (4)|2026-10-29T00:51:19Z|2026-10-29T00:51:40Z|failure\n\
 site-probe-shard (5)|2026-10-29T01:08:29Z|2026-10-29T01:08:53Z|failure\n\
 site-probe-shard (1)|2026-10-29T00:53:37Z|2026-10-29T00:54:02Z|failure\n\
 interaction-probe-shard (6)|2026-10-29T00:53:37Z|2026-10-29T00:53:55Z|failure\n\
 site-probe-shard (6)|2026-10-29T01:01:16Z|2026-10-29T01:01:35Z|failure\n\
 interaction-probe-shard (3)|2026-10-29T00:43:40Z|2026-10-29T00:43:59Z|failure\n\
 site-probe-shard (3)|2026-10-29T01:23:49Z|2026-10-29T01:24:17Z|failure\n\
 interaction-probe-shard (2)|2026-10-29T01:18:46Z|2026-10-29T01:19:05Z|failure\n\
 site-probe-shard (2)|2026-10-29T01:24:20Z|2026-10-29T01:24:37Z|failure\n\
 interaction-probe-shard (5)|2026-10-29T01:12:46Z|2026-10-29T01:13:08Z|failure\n\
 interaction-probe-shard (4)|2026-10-29T01:14:45Z|2026-10-29T01:15:02Z|failure\n\
 interaction-probe-shard (1)|2026-10-29T01:26:42Z|2026-10-29T01:27:08Z|failure\n\
 ui|2026-10-29T01:26:46Z|2026-10-29T01:27:18Z|failure\n\
 suite|2026-10-29T01:28:13Z|2026-10-29T01:28:32Z|failure\n\
 site-probe|2026-10-29T01:31:57Z|2026-10-29T01:32:15Z|failure\n\
 interactions|2026-10-29T01:35:29Z|2026-10-29T01:35:46Z|failure\n\
 evidence|2026-10-29T01:39:29Z|2026-10-29T01:44:28Z|success\n\
 ci|2026-10-29T01:47:19Z|2026-10-29T01:47:34Z|failure\n\
 pages|2026-10-29T01:44:29Z|2026-10-29T01:44:28Z|cancelled\n\
";

/// The jobs of `record` as the forge listed them at `at`: completed by then with their
/// conclusion, in progress from their start, or queued (whose `started_at` the forge fills
/// with their creation, so the recorded start stands in for it).
fn jobs_at(record: &str, at: &str) -> Vec<JobObservation> {
    let now = t(at);
    record
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split('|').collect();
            let (name, started, completed, conclusion) = (f[0], f[1], f[2], f[3]);
            let v = if t(completed) <= now {
                serde_json::json!({"name": name, "status": "completed",
                    "conclusion": conclusion, "started_at": started})
            } else if t(started) <= now {
                serde_json::json!({"name": name, "status": "in_progress",
                    "conclusion": null, "started_at": started})
            } else {
                serde_json::json!({"name": name, "status": "queued",
                    "conclusion": null, "started_at": started})
            };
            job_of(&v).unwrap()
        })
        .collect()
}

/// `json`'s run with `record`'s jobs as they stood at `at`.
fn run_at(json: &str, record: &str, at: &str) -> RunObservation {
    let mut r = run(json, None);
    r.jobs = jobs_at(record, at);
    r
}

fn decide_at(
    runs: &[RunObservation],
    heads: &[(u64, &str)],
    full: &[u64],
    at: &str,
) -> Supervision {
    let m = model();
    let heads: BTreeMap<u64, String> = heads.iter().map(|(n, s)| (*n, s.to_string())).collect();
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

/// A hold's kind, as the report words it.
fn kind(h: &HoldReason) -> String {
    serde_json::to_value(h).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Every ten minutes from `from` to `to`.
fn ticks(from: &str, to: &str) -> Vec<String> {
    (t(from)..=t(to))
        .step_by(600)
        .map(|s| crate::peers::rfc3339(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s)))
        .collect()
}

#[test]
fn a_chain_of_jobs_that_waited_for_runners_replayed_at_01_10z_is_not_cancelled() {
    let at = "2026-10-28T01:10:00Z";
    let r = run_at(RUN_40576851731, JOBS_40576851731, at);
    // what a clock from its first job says: past validate's 255 minutes
    assert!((t(at) - t("2026-10-27T20:52:10Z")) / 60 > 255);
    let s = decide_at(&[r], &[], &[], at);
    assert!(s.decisions.is_empty(), "{s:?}");
    // nor at any tick until its verdict: no job of it ever ran 255 minutes
    for at in ticks("2026-10-28T01:10:00Z", "2026-10-28T06:31:00Z") {
        let r = run_at(RUN_40576851731, JOBS_40576851731, &at);
        let s = decide_at(&[r], &[], &[], &at);
        assert!(s.decisions.is_empty(), "{at}: {s:?}");
    }
    // a job that hangs is still ended: suite-shard (1) still running at 05:30Z, 262 minutes
    // after it started at 01:07:45Z
    let mut hung = run_at(RUN_40576851731, JOBS_40576851731, "2026-10-28T05:30:00Z");
    let shard = hung
        .jobs
        .iter_mut()
        .find(|j| j.name == "suite-shard (1)")
        .unwrap();
    shard.status = "in_progress".into();
    shard.conclusion = None;
    let s = decide_at(&[hung], &[], &[], "2026-10-28T05:30:00Z");
    assert_eq!(s.decisions.len(), 1, "{s:?}");
    assert_eq!(s.decisions[0].reason.word(), "running_past_bound");
    assert!(
        s.decisions[0].reason.explain().contains("suite-shard (1)"),
        "{}",
        s.decisions[0].reason.explain()
    );
}

#[test]
fn a_dispatched_run_between_its_stages_replayed_at_15_58z_is_not_cancelled() {
    let at = "2026-10-28T15:58:30Z";
    let r = run_at(RUN_40680986818, JOBS_40680986818, at);
    assert!((t(at) - t("2026-10-28T11:42:14Z")) / 60 > 255);
    let s = decide_at(
        &[r],
        &[(2596, "f37ab83f1e51e30953f0276441198e85c2e350af")],
        &[],
        at,
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    // its last stage, running at 16:01Z, is minutes old
    let at = "2026-10-28T16:01:00Z";
    let r = run_at(RUN_40680986818, JOBS_40680986818, at);
    let s = decide_at(
        &[r],
        &[(2596, "f37ab83f1e51e30953f0276441198e85c2e350af")],
        &[],
        at,
    );
    assert!(s.decisions.is_empty(), "{s:?}");
}

#[test]
fn a_release_re_run_replayed_at_07_20z_is_not_cancelled() {
    let at = "2026-10-10T07:20:00Z";
    let r = run_at(RUN_38013111243, JOBS_38013111243, at);
    // its listed plan is attempt 1's: from it, 354 minutes against release's 75
    assert!((t(at) - t("2026-10-10T01:25:21Z")) / 60 > 75);
    assert_eq!(r.run_started_at.as_deref(), Some("2026-10-10T07:15:55Z"));
    let s = decide_at(&[r], &[], &[], at);
    assert!(s.decisions.is_empty(), "{s:?}");
    for at in ticks("2026-10-10T07:20:00Z", "2026-10-10T07:34:00Z") {
        let r = run_at(RUN_38013111243, JOBS_38013111243, &at);
        let s = decide_at(&[r], &[], &[], &at);
        assert!(s.decisions.is_empty(), "{at}: {s:?}");
    }
}

#[test]
fn a_third_attempt_replayed_at_01_11z_is_not_cancelled() {
    let at = "2026-10-29T01:11:00Z";
    let r = run_at(RUN_40747313397, JOBS_40747313397, at);
    // carried jobs from 20:54:39Z: 256 minutes by 01:11Z, while attempt 3 had run 160
    assert!((t(at) - t("2026-10-28T20:54:39Z")) / 60 > 255);
    let head = "d560cbc10417177f017a29414d2d0830d81b96e1";
    let s = decide_at(&[r], &[(2349, head)], &[], at);
    assert!(s.decisions.is_empty(), "{s:?}");
    for at in ticks("2026-10-29T00:30:00Z", "2026-10-29T01:40:00Z") {
        let r = run_at(RUN_40747313397, JOBS_40747313397, &at);
        let s = decide_at(&[r], &[(2349, head)], &[], &at);
        assert!(s.decisions.is_empty(), "{at}: {s:?}");
    }
}

#[test]
fn an_integration_top_whose_job_runs_past_its_bound_is_held() {
    let at = "2026-10-29T00:50:00Z";
    let mut top = run(RUN_40764355688, Some(JOBS_40764355688));
    top.status = "in_progress".into();
    // rust (3) started at 22:28:47Z; read at 02:50Z it has run 261 minutes
    let at_late = "2026-10-29T02:50:00Z";
    let s = decide_at(&[top.clone()], &[(PR, HEAD_C)], &[], at);
    assert!(s.decisions.is_empty(), "{s:?}");
    let s = decide_at(&[top.clone()], &[(PR, HEAD_C)], &[], at_late);
    assert!(s.decisions.is_empty(), "{s:?}");
    assert_eq!(kind(&s.held[0].reason), "integration_top_runs", "{s:?}");
    // the same job on any other branch is ended
    top.head_branch = "fix/x".into();
    top.pull_request = Some(7);
    let s = decide_at(&[top], &[(7, HEAD_C)], &[], at_late);
    assert_eq!(s.decisions[0].reason.word(), "running_past_bound", "{s:?}");
}

// ---------------------------------------------------------------- the queue's clock

/// "Re-run all jobs" of a pull request's run created at 08:00Z, asked for at 14:00Z: the
/// forge keeps `created_at` and resets `run_started_at`, and every job is the new attempt's.
const RUN_RERUN_ALL: &str = r#"{"id":40800000001,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request","head_branch":"fix/x",
 "head_sha":"aaaa","status":"queued","conclusion":null,"created_at":"2026-10-29T08:00:00Z",
 "run_started_at":"2026-10-29T14:00:00Z","run_attempt":2,
 "pull_requests":[{"number":7,"head":{"ref":"fix/x"}}]}"#;

const JOBS_RERUN_ALL: &str = r#"[
 {"name":"plan","status":"queued","conclusion":null,"started_at":"2026-10-29T14:00:01Z"}
]"#;

#[test]
fn a_re_run_created_six_hours_earlier_and_queued_two_minutes_is_not_cancelled() {
    let r = run(RUN_RERUN_ALL, Some(JOBS_RERUN_ALL));
    let s = decide_at(
        std::slice::from_ref(&r),
        &[(7, "aaaa")],
        &[],
        "2026-10-29T14:02:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    // the attempt is held to the bound from its admission, its plan job's creation at 14:00:01Z
    let s = decide_at(&[r], &[(7, "aaaa")], &[], "2026-10-29T18:01:01Z");
    assert_eq!(s.decisions.len(), 1, "{s:?}");
    assert_eq!(
        s.decisions[0].reason,
        CancelReason::QueuedPastBound {
            minutes: 241,
            bound: 240
        }
    );
}

/// Validate's push of master 40796031054, queued since 00:44:35Z on a starved pool: its
/// concurrency group is its own run id, so it holds nothing up.
const RUN_40796031054: &str = r#"{"id":40796031054,"name":"validate",
 "path":".github/workflows/validate.yml","event":"push","head_branch":"master",
 "head_sha":"6c79dcf68cf0000000000000000000000000000","status":"queued","conclusion":null,
 "created_at":"2026-10-29T00:44:35Z","run_started_at":"2026-10-29T00:44:35Z",
 "run_attempt":1,"pull_requests":[]}"#;

/// Its plan job, waiting for a runner. A push's group is its own run, so the run is admitted as
/// it is created; the job's creation is taken to be the run's.
const JOBS_40796031054: &str = r#"[
 {"name":"plan","status":"queued","conclusion":null,"started_at":"2026-10-29T00:44:35Z",
  "created_at":"2026-10-29T00:44:35Z"}
]"#;

#[test]
fn a_trunk_push_or_a_tag_release_queued_300_minutes_is_held() {
    let at = "2026-10-29T05:44:35Z";
    let master = run(RUN_40796031054, Some(JOBS_40796031054));
    let s = decide_at(std::slice::from_ref(&master), &[], &[], at);
    assert!(s.decisions.is_empty(), "{s:?}");
    assert_eq!(kind(&s.held[0].reason), "protected_run_waits", "{s:?}");
    // a tag's release, queued 241 minutes
    let mut tag = master;
    tag.workflow = "release".into();
    tag.head_branch = "v1.0.0-alpha.10".into();
    let s = decide_at(&[tag], &[], &[], "2026-10-29T04:46:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    assert_eq!(kind(&s.held[0].reason), "protected_run_waits", "{s:?}");
}

// ---------------------------------------------------------------- the duplicate rule's limits

#[test]
fn a_re_run_of_the_labelled_twin_is_not_cancelled() {
    // the owner re-runs 40713048283 after its twin was judged: attempt 2, pending, no job
    let v: Value = serde_json::from_str(RUN_40713048283_AT_1510).unwrap();
    let mut v = v.as_object().unwrap().clone();
    v.insert("run_attempt".into(), 2.into());
    v.insert("run_started_at".into(), "2026-10-28T23:00:00Z".into());
    let again_json = Value::Object(v).to_string();
    let again2 = run(&again_json, None);
    let s = decide_at(
        &[run(RUN_40713047404_DONE, None), again2.clone()],
        &[(PR, HEAD_B)],
        &[],
        "2026-10-28T23:01:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    // and while its twin is still in flight, the re-run is held, never ended
    let s = decide_at(
        &[
            run(RUN_40713047404_AT_1510, Some(JOBS_40713047404_AT_1510)),
            again2,
        ],
        &[(PR, HEAD_B)],
        &[],
        "2026-10-28T23:01:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    let held = s.held.iter().find(|h| h.run == 40713048283).unwrap();
    assert_eq!(
        held.reason,
        HoldReason::RerunRequested {
            pull_request: PR,
            earlier: 40713047404,
            attempt: 2
        }
    );
}

/// A pull request's run `id` of `sha`, created at `created`, from the forge's JSON.
fn pr_run(id: u64, sha: &str, status: &str, conclusion: Option<&str>, created: &str) -> Value {
    serde_json::json!({"id": id, "name": "validate", "path": ".github/workflows/validate.yml",
        "event": "pull_request", "head_branch": "fix/x", "head_sha": sha, "status": status,
        "conclusion": conclusion, "created_at": created, "run_started_at": created,
        "run_attempt": 1, "pull_requests": [{"number": 7, "head": {"ref": "fix/x"}}]})
}

fn of(v: Value) -> RunObservation {
    run(&v.to_string(), None)
}

#[test]
fn a_reopened_pull_requests_run_of_a_judged_head_is_not_cancelled() {
    // run 100 of head A judged success at 10:00Z; closed and reopened at 14:00Z, run 140 of
    // the same head tests it against today's master
    let runs = [
        of(pr_run(
            100,
            "A",
            "completed",
            Some("success"),
            "2026-10-29T09:00:00Z",
        )),
        of(pr_run(140, "A", "queued", None, "2026-10-29T14:00:00Z")),
    ];
    let s = decide_at(&runs, &[(7, "A")], &[], "2026-10-29T14:01:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // a judged failure is never the twin of a later run, even one second later
    let runs = [
        of(pr_run(
            100,
            "A",
            "completed",
            Some("failure"),
            "2026-10-29T14:00:00Z",
        )),
        of(pr_run(101, "A", "pending", None, "2026-10-29T14:00:01Z")),
    ];
    let s = decide_at(&runs, &[(7, "A")], &[], "2026-10-29T15:00:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // nor a judged success one second earlier: the twin of a judged run has been admitted
    let runs = [
        of(pr_run(
            100,
            "A",
            "completed",
            Some("success"),
            "2026-10-29T14:00:00Z",
        )),
        of(pr_run(101, "A", "pending", None, "2026-10-29T14:00:01Z")),
    ];
    let s = decide_at(&runs, &[(7, "A")], &[], "2026-10-29T15:00:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // and while run 100 is in flight, a second run created two hours later was asked for
    let runs = [
        of(pr_run(
            100,
            "A",
            "in_progress",
            None,
            "2026-10-29T12:00:00Z",
        )),
        of(pr_run(101, "A", "pending", None, "2026-10-29T14:00:00Z")),
    ];
    let s = decide_at(&runs, &[(7, "A")], &[], "2026-10-29T14:01:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
}

// ---------------------------------------------------------------- #2678 and #2674, 2026-10-29

/// #2678's run 40811610939 as the forge recorded it: passed, its `ci` job a success at
/// 05:31:52Z, the run completed at 05:31:53Z.
const RUN_40811610939: &str = r#"{"id":40811610939,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"fix/the-incidents-issues-serve-their-intents",
 "head_sha":"3a45c8c73163c6213767048ee8c7888ec641be25","status":"completed",
 "conclusion":"success","created_at":"2026-10-29T03:20:53Z",
 "run_started_at":"2026-10-29T03:20:53Z","run_attempt":1,
 "pull_requests":[{"number":2678,"head":{"ref":"fix/the-incidents-issues-serve-their-intents"}}]}"#;

/// Its twin 40811621641, six seconds younger (the `labeled` run), as it stood at 05:35Z:
/// admitted when 40811610939 completed, `queued`, its `plan` job created at 05:31:53Z and
/// waiting for a runner (it started at 05:38:46Z). It was cancelled at 05:58Z, and its
/// `if: always()` job `ci` then ran and failed at 05:58:37Z, newer than the pass: #2678 read
/// BLOCKED.
const RUN_40811621641_AT_0535: &str = r#"{"id":40811621641,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"fix/the-incidents-issues-serve-their-intents",
 "head_sha":"3a45c8c73163c6213767048ee8c7888ec641be25","status":"queued","conclusion":null,
 "created_at":"2026-10-29T03:20:59Z","run_started_at":"2026-10-29T03:20:59Z","run_attempt":1,
 "pull_requests":[{"number":2678,"head":{"ref":"fix/the-incidents-issues-serve-their-intents"}}]}"#;

const JOBS_40811621641_AT_0535: &str = r#"[
 {"name":"plan","status":"queued","conclusion":null,"started_at":"2026-10-29T05:31:53Z",
  "created_at":"2026-10-29T05:31:53Z"}
]"#;

#[test]
fn the_admitted_twin_of_2678_replayed_at_05_35z_is_not_cancelled() {
    let head = "3a45c8c73163c6213767048ee8c7888ec641be25";
    let twin = run(RUN_40811621641_AT_0535, Some(JOBS_40811621641_AT_0535));
    assert!(!twin.any_job_started(), "its plan job waits for a runner");
    assert!(
        !twin.jobs.is_empty(),
        "but it exists: the group admitted the run"
    );
    let runs = [run(RUN_40811610939, None), twin.clone()];
    let s = decide_at(&runs, &[(2678, head)], &[], "2026-10-29T05:35:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
    // nor on any tick until a runner took its plan job
    for at in ticks("2026-10-29T05:32:00Z", "2026-10-29T05:38:00Z") {
        let s = decide_at(&runs, &[(2678, head)], &[], &at);
        assert!(s.decisions.is_empty(), "{at}: {s:?}");
    }
    // and an admitted twin is not cancelled even with its older run in flight
    let mut older = run(RUN_40811610939, None);
    older.status = "in_progress".into();
    older.conclusion = None;
    let s = decide_at(&[older, twin], &[(2678, head)], &[], "2026-10-29T05:35:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
}

#[test]
fn the_twin_of_2678_waits_for_a_runner_from_its_admission() {
    let head = "3a45c8c73163c6213767048ee8c7888ec641be25";
    let twin = run(RUN_40811621641_AT_0535, Some(JOBS_40811621641_AT_0535));
    // created, and its attempt started, at 03:20:59Z; its group admitted it at 05:31:53Z, when
    // 40811610939 passed: that is when it began to wait for a runner
    assert_eq!(twin.run_started_at.as_deref(), Some("2026-10-29T03:20:59Z"));
    assert_eq!(twin.queued_since(), Ok(Some(t("2026-10-29T05:31:53Z"))));
    let runs = [run(RUN_40811610939, None), twin];
    // at its admission and for the whole bound after it, no cancel: a clock from its creation
    // would have ended it at 07:21Z with its plan job still waiting
    for at in [
        "2026-10-29T05:31:53Z",
        "2026-10-29T07:22:00Z",
        "2026-10-29T09:31:53Z",
        "2026-10-29T09:32:52Z",
    ] {
        let s = decide_at(&runs, &[(2678, head)], &[], at);
        assert!(s.decisions.is_empty(), "{at}: {s:?}");
    }
    // past the bound from its admission, a plan job that never got a runner is ended
    let s = decide_at(&runs, &[(2678, head)], &[], "2026-10-29T09:32:53Z");
    assert_eq!(
        s.decisions
            .iter()
            .map(|d| (d.run, d.reason.clone()))
            .collect::<Vec<_>>(),
        vec![(
            40811621641,
            CancelReason::QueuedPastBound {
                minutes: 241,
                bound: 240
            }
        )],
        "{s:?}"
    );
    // and before its admission, pending in its group with no job, it is never ended for waiting
    let mut pending = run(RUN_40811621641_AT_0535, None);
    pending.status = "pending".into();
    assert_eq!(pending.queued_since(), Ok(None));
    let s = decide_at(&[pending], &[(2678, head)], &[], "2026-10-29T09:00:00Z");
    assert!(s.decisions.is_empty(), "{s:?}");
}

/// #2674's run 40801560222 of f565f564, created 01:37:47Z, with `suite-shard (3)` in progress
/// from 01:47:58Z: the older run of the pair.
const RUN_40801560222: &str = r#"{"id":40801560222,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"fix/the-reaper-bounds-an-untagged-build-dir",
 "head_sha":"f565f56407dbbd91f00225192da439b6f7e37b7d","status":"in_progress",
 "conclusion":null,"created_at":"2026-10-29T01:37:47Z",
 "run_started_at":"2026-10-29T01:37:47Z","run_attempt":1,
 "pull_requests":[{"number":2674,"head":{"ref":"fix/the-reaper-bounds-an-untagged-build-dir"}}]}"#;

const JOBS_40801560222: &str = "\
 plan|2026-10-29T01:47:36Z|2026-10-29T01:47:58Z|success\n\
 suite-shard (3)|2026-10-29T01:47:58Z|2026-10-29T23:00:00Z|cancelled\n\
";

/// Its second run 40816501443, created at 04:13:08Z when the `hold` label was added, pending in
/// the group with no job.
const RUN_40816501443: &str = r#"{"id":40816501443,"name":"validate",
 "path":".github/workflows/validate.yml","event":"pull_request",
 "head_branch":"fix/the-reaper-bounds-an-untagged-build-dir",
 "head_sha":"f565f56407dbbd91f00225192da439b6f7e37b7d","status":"pending","conclusion":null,
 "created_at":"2026-10-29T04:13:08Z","run_started_at":"2026-10-29T04:13:08Z","run_attempt":1,
 "pull_requests":[{"number":2674,"head":{"ref":"fix/the-reaper-bounds-an-untagged-build-dir"}}]}"#;

#[test]
fn a_twin_of_2674s_run_past_its_bound_is_never_cancelled_with_it() {
    let head = "f565f56407dbbd91f00225192da439b6f7e37b7d";
    let older_at = |at: &str| run_at(RUN_40801560222, JOBS_40801560222, at);
    // the real pair at 04:14Z: the label two and a half hours later asked for a second run
    let s = decide_at(
        &[older_at("2026-10-29T04:14:00Z"), run(RUN_40816501443, None)],
        &[(2674, head)],
        &[],
        "2026-10-29T04:14:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    // the same commit's twin from one burst, eight seconds younger, pending with no job
    let twin = || {
        let mut r = run(RUN_40816501443, None);
        r.created_at = "2026-10-29T01:37:55Z".into();
        r.run_started_at = Some("2026-10-29T01:37:55Z".into());
        r
    };
    // at 02:00Z the older run will reach its verdict: the twin is the duplicate
    let s = decide_at(
        &[older_at("2026-10-29T02:00:00Z"), twin()],
        &[(2674, head)],
        &[],
        "2026-10-29T02:00:00Z",
    );
    let words: Vec<_> = s
        .decisions
        .iter()
        .map(|d| (d.run, d.reason.word()))
        .collect();
    assert_eq!(words, vec![(40816501443, "duplicate_run")], "{s:?}");
    // at 06:05Z suite-shard (3) has run 257 minutes, past 255: the older run is ended, and the
    // twin, which would take its place, is held rather than ended with it
    let s = decide_at(
        &[older_at("2026-10-29T06:05:00Z"), twin()],
        &[(2674, head)],
        &[],
        "2026-10-29T06:05:00Z",
    );
    let words: Vec<_> = s
        .decisions
        .iter()
        .map(|d| (d.run, d.reason.word()))
        .collect();
    assert_eq!(words, vec![(40801560222, "running_past_bound")], "{s:?}");
    assert_eq!(
        s.held[0].reason,
        HoldReason::EarlierRunEnds {
            pull_request: 2674,
            earlier: 40801560222,
            ends_for: "running_past_bound".into()
        }
    );
    // and so on every tick within the horizon before the bound: the next pass would end the
    // older run, so this one does not end its twin
    for at in ticks("2026-10-29T05:40:00Z", "2026-10-29T06:00:00Z") {
        let s = decide_at(&[older_at(&at), twin()], &[(2674, head)], &[], &at);
        assert!(
            s.decisions.iter().all(|d| d.run != 40816501443),
            "{at}: {s:?}"
        );
    }
}

/// 40801560222's jobs, carried on from the replay above: ended at 06:05:30Z for running past its
/// bound, its `if: always()` `ci` then waits eleven minutes for a runner (as a cancelled run's
/// `ci` did on 2026-10-29, 04:16:36Z to 04:27:58Z) and fails, and the run completes, cancelled,
/// at 06:17:00Z.
const JOBS_40801560222_ENDED: &str = "\
 plan|2026-10-29T01:47:36Z|2026-10-29T01:47:58Z|success\n\
 suite-shard (3)|2026-10-29T01:47:58Z|2026-10-29T06:05:30Z|cancelled\n\
 ci|2026-10-29T06:16:30Z|2026-10-29T06:17:00Z|failure\n\
";

/// When the group admits the twin, the older run having completed: the twin's plan job is created
/// then and waits 27 minutes for a runner, as 40811610939's did (03:20:53Z to 03:48:13Z).
const TWIN_ADMITTED: &str = "2026-10-29T06:17:01Z";
const TWIN_PLAN_STARTS: &str = "2026-10-29T06:44:01Z";

#[test]
fn a_twin_of_2674s_ended_run_is_not_cancelled_until_it_has_its_verdict() {
    let head = "f565f56407dbbd91f00225192da439b6f7e37b7d";
    let older_at = |at: &str| {
        let mut r = run_at(RUN_40801560222, JOBS_40801560222_ENDED, at);
        if t(at) >= t("2026-10-29T06:17:00Z") {
            r.status = "completed".into();
            r.conclusion = Some("cancelled".into());
        }
        r
    };
    // the same commit's twin from one burst, eight seconds younger: pending with no job until the
    // group admits it, then queued with its plan job created at the admission
    let twin_at = |at: &str| {
        let mut r = run(RUN_40816501443, None);
        r.created_at = "2026-10-29T01:37:55Z".into();
        r.run_started_at = Some("2026-10-29T01:37:55Z".into());
        if t(at) >= t(TWIN_ADMITTED) {
            let started = t(at) >= t(TWIN_PLAN_STARTS);
            r.status = if started { "in_progress" } else { "queued" }.into();
            r.jobs = vec![JobObservation {
                name: "plan".into(),
                status: if started { "in_progress" } else { "queued" }.into(),
                started_at: Some(
                    if started {
                        TWIN_PLAN_STARTS
                    } else {
                        TWIN_ADMITTED
                    }
                    .into(),
                ),
                created_at: Some(TWIN_ADMITTED.into()),
                conclusion: None,
            }];
        }
        r
    };
    // the clock the twin used to be held to: its creation, 279 minutes before its admission
    assert!((t(TWIN_ADMITTED) - t("2026-10-29T01:37:55Z")) / 60 > 240);
    assert_eq!(
        twin_at(TWIN_ADMITTED).queued_since(),
        Ok(Some(t(TWIN_ADMITTED)))
    );
    // every minute from the older run's end until the twin's plan job runs: no cancel of either
    let mut at = t("2026-10-29T06:06:00Z");
    while at <= t("2026-10-29T06:50:00Z") {
        let stamp =
            crate::peers::rfc3339(std::time::UNIX_EPOCH + std::time::Duration::from_secs(at));
        let runs = [older_at(&stamp), twin_at(&stamp)];
        let s = decide_at(&runs, &[(2674, head)], &[], &stamp);
        assert!(s.decisions.is_empty(), "{stamp}: {s:?}");
        // nor when the pull request asked for the second run (ci:full)
        let s = decide_at(&runs, &[(2674, head)], &[2674], &stamp);
        assert!(s.decisions.is_empty(), "{stamp} (ci:full): {s:?}");
        at += 60;
    }
    // while the cancelled older run runs only its `ci`, the twin waits to take its place
    let s = decide_at(
        &[
            older_at("2026-10-29T06:10:00Z"),
            twin_at("2026-10-29T06:10:00Z"),
        ],
        &[(2674, head)],
        &[],
        "2026-10-29T06:10:00Z",
    );
    assert_eq!(
        s.held
            .iter()
            .find(|h| h.run == 40816501443)
            .map(|h| h.reason.clone()),
        Some(HoldReason::EarlierRunEnds {
            pull_request: 2674,
            earlier: 40801560222,
            ends_for: "cancelled".into()
        }),
        "{s:?}"
    );
}

#[test]
fn a_head_that_goes_a_b_a_keeps_a_run_of_a() {
    let mut b = of(pr_run(
        120,
        "B",
        "in_progress",
        None,
        "2026-10-29T10:00:00Z",
    ));
    b.jobs = jobs_at(
        "plan|2026-10-29T10:01:00Z|2026-10-29T10:02:00Z|success",
        "2026-10-29T11:01:00Z",
    );
    let runs = [
        of(pr_run(
            100,
            "A",
            "completed",
            Some("success"),
            "2026-10-29T09:00:00Z",
        )),
        b,
        of(pr_run(130, "A", "pending", None, "2026-10-29T11:00:00Z")),
    ];
    let s = decide_at(&runs, &[(7, "A")], &[], "2026-10-29T11:01:00Z");
    let words: Vec<_> = s
        .decisions
        .iter()
        .map(|d| (d.run, d.reason.word()))
        .collect();
    assert_eq!(words, vec![(120, "superseded_blocks_successor")], "{s:?}");
}

#[test]
fn a_train_and_its_member_at_one_sha_are_two_pull_requests() {
    // the forge lists every open pull request whose head matches: the member's first
    let both = r#"[{"number":2600,"head":{"ref":"fix/x"}},
        {"number":2631,"head":{"ref":"integration/10a"}}]"#;
    let member: Value = serde_json::from_str(&format!(
        r#"{{"id":500,"name":"validate","path":".github/workflows/validate.yml",
        "event":"pull_request","head_branch":"fix/x","head_sha":"S","status":"in_progress",
        "conclusion":null,"created_at":"2026-10-29T10:00:00Z",
        "run_started_at":"2026-10-29T10:00:00Z","run_attempt":1,"pull_requests":{both}}}"#
    ))
    .unwrap();
    let train: Value = serde_json::from_str(&format!(
        r#"{{"id":510,"name":"validate","path":".github/workflows/validate.yml",
        "event":"pull_request","head_branch":"integration/10a","head_sha":"S",
        "status":"queued","conclusion":null,"created_at":"2026-10-29T10:30:00Z",
        "run_started_at":"2026-10-29T10:30:00Z","run_attempt":1,"pull_requests":{both}}}"#
    ))
    .unwrap();
    let mut member = run_of(&member).unwrap();
    member.jobs = jobs_at(
        "plan|2026-10-29T10:01:00Z|2026-10-29T10:02:00Z|success",
        "2026-10-29T10:31:00Z",
    );
    let train = run_of(&train).unwrap();
    assert_eq!(member.pull_request, Some(2600));
    assert_eq!(
        train.pull_request,
        Some(2631),
        "the train's run is the train's"
    );
    let s = decide_at(
        &[member.clone(), train.clone()],
        &[(2600, "S"), (2631, "S")],
        &[],
        "2026-10-29T10:31:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
    // and a run in another branch is in another concurrency group, whatever it is attributed to
    let mut misattributed = train;
    misattributed.pull_request = Some(2600);
    let s = decide_at(
        &[member, misattributed],
        &[(2600, "S"), (2631, "S")],
        &[],
        "2026-10-29T10:31:00Z",
    );
    assert!(s.decisions.is_empty(), "{s:?}");
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

/// The job `name` of a workflow's non-comment lines: from its key to the next job's.
fn job_of_workflow(code: &str, name: &str) -> String {
    let mut out = Vec::new();
    let mut inside = false;
    for l in code.lines() {
        let key = l.starts_with("  ") && !l.starts_with("   ") && l.trim_end().ends_with(':');
        if key {
            inside = l.trim() == format!("{name}:");
        } else if !l.is_empty() && !l.starts_with(' ') {
            inside = false;
        }
        if inside {
            out.push(l);
        }
    }
    assert!(!out.is_empty(), "no job `{name}`");
    out.join("\n")
}

/// A job's `timeout-minutes`, and each of its steps' in order.
fn bounds_of(job: &str) -> (u64, Vec<u64>) {
    let mut bound = 0;
    let mut steps = Vec::new();
    for l in job.lines() {
        let indent = l.len() - l.trim_start().len();
        if let Some(n) = l.trim().strip_prefix("timeout-minutes:") {
            let n: u64 = n.trim().parse().unwrap();
            if indent == 4 {
                bound = n;
            } else {
                steps.push(n);
            }
        }
    }
    (bound, steps)
}

/// The two lanes of supervise.yml as GitHub runs them: each trigger at `at` (seconds) starts a
/// run whose lundra job enters its lane's group (one runs, one waits, a newer one replaces the
/// waiting one, which is cancelled), and whose hosted job runs once that lundra job has ended,
/// unless it supervised. `lundra_takes` is how long a lundra job runs once it has a runner;
/// `None` is a lundra that takes no job. For each run, whether its hosted lane applied.
fn sequenced(at: &[u64], lundra_takes: Option<u64>) -> Vec<bool> {
    let mut hosted = vec![false; at.len()];
    let mut running: Option<(usize, u64)> = None; // (run, started)
    let mut pending: Option<usize> = None;
    for (run, &t) in at.iter().enumerate() {
        // what ended before this trigger
        if let (Some((_, started)), Some(takes)) = (running, lundra_takes) {
            if started + takes <= t {
                // it supervised: its hosted lane does nothing; the waiting one starts
                running = pending.take().map(|p| (p, started + takes));
            }
        }
        if running.is_none() {
            running = Some((run, t));
        } else if let Some(replaced) = pending.replace(run) {
            // cancelled in the group: `always()` and no `supervised` output, so it applies
            hosted[replaced] = true;
        }
    }
    hosted
}

#[test]
fn a_lundra_that_takes_no_job_is_covered_by_the_hosted_lane_from_the_third_trigger() {
    // lundra offline, or every runner busy with validate's first wave (vars.MJ_RUNNER): its
    // first job waits for a runner and holds the group; the second waits behind it; the third
    // replaces the second, whose run's hosted lane then applies, and so on with every trigger
    let ticks: Vec<u64> = (0..6).map(|k| k * 600).collect();
    assert_eq!(
        sequenced(&ticks, None),
        vec![false, true, true, true, true, false]
    );
    // a lundra that supervises every tick leaves the hosted lane nothing to do
    assert!(sequenced(&ticks, Some(60)).iter().all(|h| !h));
    // the wiring this simulates
    let c = code(&workflow("supervise.yml"));
    let hosted = job_of_workflow(&c, "hosted");
    assert!(hosted.contains("needs: lundra") && hosted.contains("always()"));
    assert!(job_of_workflow(&c, "lundra").contains("cancel-in-progress: false"));
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
    assert!(
        !c.contains("cargo build"),
        "the supervisor compiles instead of downloading"
    );
    assert!(c.contains("prs supervise \"$@\""));
    assert!(c.contains("set -- --apply"));
    // two jobs of one run: lundra first, the hosted fallback after it
    let lundra = job_of_workflow(&c, "lundra");
    let hosted = job_of_workflow(&c, "hosted");
    assert!(
        lundra.contains(r#"'["self-hosted","lundra"]'"#),
        "not on lundra: {lundra}"
    );
    assert!(
        lundra.contains("vars.MJ_SUPERVISE_RUNNER"),
        "no way to name a runner of its own"
    );
    assert!(
        hosted.contains("runs-on: ubuntu-latest"),
        "no hosted fallback"
    );
    // the hosted lane sees the lundra lane of its own trigger: it needs it, runs whatever
    // became of it, and only when it did not supervise
    assert!(hosted.contains("needs: lundra"), "{hosted}");
    assert!(
        hosted.contains("if: ${{ always() && needs.lundra.outputs.supervised != 'true' }}"),
        "{hosted}"
    );
    assert!(
        lundra.contains("supervised: ${{ steps.supervise.outputs.supervised }}"),
        "{lundra}"
    );
    // `supervised` is written only after the exit was checked: a failed lane never claims it
    let step = lundra.split("- id: supervise").nth(1).unwrap();
    let check = step.find("[ \"$rc\" = 0 ] ||").expect("no exit check");
    let claim = step
        .find("echo \"supervised=true\" >> \"$GITHUB_OUTPUT\"")
        .expect("no supervised output");
    assert!(
        check < claim,
        "it claims to have supervised before its exit is known"
    );
    // each lane its own group, so a lane with no runner never holds the other
    assert!(lundra.contains("group: supervise-lundra"), "{lundra}");
    assert!(hosted.contains("group: supervise-hosted"), "{hosted}");
    // the hosted lane covers, and stands aside after a recent success in another run
    assert!(c.contains("--unless-supervised-within \"$COVER\""), "{c}");
    assert!(hosted.contains("COVER: '15'"), "{hosted}");
    // the window outlasts the schedule's period with a margin: after a quiet stretch the lundra
    // success before a burst is 8 to 10 minutes old, and it must still stand the hosted lane aside
    let period: u64 = c
        .lines()
        .find_map(|l| l.trim().strip_prefix("- cron: '*/"))
        .and_then(|r| r.split(' ').next())
        .and_then(|n| n.parse().ok())
        .expect("a */N schedule");
    let cover: u64 = hosted
        .lines()
        .find_map(|l| l.trim().strip_prefix("COVER: '"))
        .and_then(|r| r.trim_end_matches('\'').parse().ok())
        .expect("a hosted stand-aside window");
    assert!(
        cover >= period + 2,
        "window {cover} min, period {period} min"
    );
    assert!(
        lundra.contains("COVER: ''"),
        "the lundra lane always applies"
    );
    assert!(
        c.contains("--supervised-by lundra"),
        "a covering lane that counted its own stand-aside would stand aside for ever"
    );
    // an executable is taken only if it has the command: every build before I3903 runs
    // `--version` and fails `prs supervise`, so both the artifact and the release are asked,
    // in each lane
    for job in [&lundra, &hosted] {
        assert_eq!(
            job.matches("prs supervise --help >/dev/null").count(),
            2,
            "the artifact and the release are each asked whether they have prs supervise"
        );
        assert!(
            job.contains("exit 12"),
            "an executable without the command is not a silent pass"
        );
    }
    // every step bounded on its own, the bounds adding up under the job's, and the supervise
    // step outlasting the executable's forge budget: the job is never killed mid-act
    let mut timeouts = Vec::new();
    for job in [&lundra, &hosted] {
        let (bound, steps) = bounds_of(job);
        assert_eq!(steps.len(), 3, "a step with no bound of its own: {job}");
        assert!(
            steps.iter().sum::<u64>() < bound,
            "the steps' bounds {steps:?} add up past the job's {bound}"
        );
        let supervise_step = *steps.last().unwrap();
        assert!(
            supervise_step * 60 > super::runs::BUDGET_SECONDS + super::runs::GH_CALL_SECONDS,
            "the supervise step ({supervise_step} min) ends before the forge budget"
        );
        timeouts.push(bound);
    }
    assert!(
        c.contains("timeout 45 gh run download"),
        "a download with no bound"
    );
    assert!(
        c.contains("timeout 40 sh"),
        "the installer's own download is not bounded"
    );

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
    // the liveness is its own job, off the pool the supervisor waits on, compiling nothing
    let alive = c
        .split("\n  alive:")
        .nth(1)
        .expect("no `alive` job in queue.yml");
    assert!(alive.contains("[self-hosted, lundra]"), "{alive}");
    assert!(alive.contains("ubuntu-latest"), "{alive}");
    assert!(alive.contains("fail-fast: false"), "{alive}");
    assert!(
        !alive.contains("cargo build"),
        "it compiles before it reports"
    );
    assert!(alive.contains("prs supervise --help >/dev/null"), "{alive}");
    assert!(alive.contains("--unless-supervised-within 30"), "{alive}");
    // it names the lane it holds to account: any lane's success hid a lundra that always failed
    assert!(
        alive.contains("--unless-supervised-within 30 --supervised-by lundra"),
        "{alive}"
    );
    let (bound, steps) = bounds_of(alive);
    assert_eq!(
        steps.len(),
        3,
        "a step of the alive job with no bound: {alive}"
    );
    assert!(steps.iter().sum::<u64>() < bound, "{steps:?} past {bound}");
    assert!(
        alive.contains("group: queue-alive-${{ matrix.lane }}"),
        "{alive}"
    );
    assert!(alive.contains(".liveness.finding"), "{alive}");
    // and a lane waiting for a runner holds no measurement: the group is the job's
    let head = c.split("\njobs:").next().unwrap();
    assert!(
        !head.contains("concurrency:"),
        "a workflow-level group holds the next measurement behind a waiting lane"
    );
}
