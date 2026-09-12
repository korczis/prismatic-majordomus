//! Exactly-once close, proved across **processes** rather than across threads.
//!
//! The unit tests in `session::store` spawn threads. That is the right harness for the
//! logic and the wrong one for the claim: a reader is entitled to ask whether the exclusion
//! is an in-process mutex that would not survive two `majordomus session close` invocations
//! racing from two provider windows. It is not — the claim is a `create_new(true)` open,
//! which is `O_EXCL` and is decided by the filesystem — and this file is the assertion that
//! says so with real processes.
//!
//! The shape is the standard one for a test that needs a second process and has no second
//! binary: the test executable re-runs itself with an environment variable set, and the
//! child does the work and exits with a code naming its outcome. A barrier file makes the
//! children race rather than queue, because a race nobody arranged is a race that passes by
//! accident.
//!
//! The incident behind it: episode `s-20260909152316-024f` has four immutable records in
//! this repository, written at 21:41:35 and then at 10:39:02, 10:39:18 and 10:39:31 the
//! next morning, because a provider's end event fires more than once and
//! `mj_publish_record` is designed never to collide.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use majordomus_cli::session::{CloseOutcome, Episode, EpisodeId, Outcome, SessionStore};

/// The environment variable that turns this test binary into one closer.
const CHILD: &str = "MJ_SESSION_DOMAIN_CLOSE_CHILD";

/// Exit codes the child speaks, chosen outside the range a test harness uses itself.
const WROTE: i32 = 20;
const ALREADY_CLOSED: i32 = 21;
const ALREADY_CLOSING: i32 = 22;

const EPISODE: &str = "s-20260909152316-024f";

fn record_body() -> String {
    format!("---\nschema: session/v1\nkind: session\nsession_id: {EPISODE}\n---\n")
}

fn episode() -> Episode {
    Episode::opening(
        EpisodeId::parse(EPISODE).expect("the id from the incident"),
        Path::new("/r"),
    )
    .opened_at("2026-09-09T15:23:16Z", "master", "9f3a2a0", 1_000)
}

/// One closer, in a process of its own. A no-op when the variable is unset, which is what
/// lets it sit in the same file as the test that drives it without running twice.
#[test]
fn close_one_episode_as_a_child_process() {
    let Ok(root) = std::env::var(CHILD) else {
        return;
    };
    let root = PathBuf::from(root);

    // Wait on the barrier so that every child is inside the window at once. Bounded: a
    // child that waited for ever would hang the suite rather than fail it.
    let barrier = root.join("go");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !barrier.exists() {
        if Instant::now() > deadline {
            std::process::exit(90);
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    let store = SessionStore::writable(&root);
    let report = match store.close(&episode(), Outcome::Closed, || {
        // the expensive part a real close does inside the claim: a `git status` and a walk
        // of the episode's ledger lines
        std::thread::sleep(Duration::from_millis(120));
        record_body()
    }) {
        Ok(report) => report,
        Err(_) => std::process::exit(91),
    };
    std::process::exit(match report.outcome {
        CloseOutcome::Closed => WROTE,
        CloseOutcome::AlreadyClosed => ALREADY_CLOSED,
        CloseOutcome::AlreadyClosing => ALREADY_CLOSING,
        CloseOutcome::Refused => 92,
    });
}

#[test]
fn six_processes_closing_one_episode_yield_exactly_one_record() {
    if std::env::var(CHILD).is_ok() {
        return;
    }
    let exe = std::env::current_exe().expect("this test binary");
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path().to_path_buf();

    let mut children = Vec::new();
    for _ in 0..6 {
        let child = std::process::Command::new(&exe)
            .args([
                "--exact",
                "close_one_episode_as_a_child_process",
                "--nocapture",
                "--test-threads",
                "1",
            ])
            .env(CHILD, &root)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("a closer");
        children.push(child);
    }
    // Every child is spawned and spinning on the barrier; release them together.
    std::fs::write(root.join("go"), b"").expect("the barrier");

    let mut wrote = 0usize;
    let mut already_closed = 0usize;
    let mut already_closing = 0usize;
    for mut child in children {
        let status = child.wait().expect("a closer to finish");
        match status.code() {
            Some(WROTE) => wrote += 1,
            Some(ALREADY_CLOSED) => already_closed += 1,
            Some(ALREADY_CLOSING) => already_closing += 1,
            other => panic!("a closer exited with {other:?}, which is not an outcome"),
        }
    }

    assert_eq!(
        wrote, 1,
        "exactly one of six processes published the record"
    );
    assert_eq!(
        already_closed + already_closing,
        5,
        "and the other five were told which case they were in"
    );

    let store = SessionStore::read_only(&root);
    let records = store.records_of(&EpisodeId::parse(EPISODE).expect("the id"));
    assert_eq!(
        records.len(),
        1,
        "one episode, one record — the incident of 2026-09-10 produced four"
    );
    assert!(
        !store
            .claim_path(&EpisodeId::parse(EPISODE).expect("the id"))
            .exists(),
        "and no claim was left behind to lock the episode out of ever being closed"
    );
}
