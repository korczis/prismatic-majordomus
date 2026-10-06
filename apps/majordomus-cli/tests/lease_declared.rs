//! What a lease contest is judged by is the declaration, not the compiled constant.
//!
//! A process reads one declaration (`lease::declare_timings` is a `OnceLock`), so these tests
//! live in a binary of their own: every one of them declares the same short timings before
//! anything opens a repository, and whichever runs first is the one that sets them. The
//! values are chosen so that each outcome is impossible under the constants — a busy owner
//! given up on in well under `BUSY_GRACE`, an election refused in well under `JOIN_TIMEOUT` —
//! and each test also reads the number back from what the election said.

mod common;

use std::io::Write;
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Fixture;
use majordomus_cli::lease::{self, Role, BUSY_GRACE, JOIN_TIMEOUT};
use majordomus_cli::policy::ServerPolicy;
use majordomus_cli::Repository;

/// The declaration every test of this binary runs under.
fn declare() {
    lease::declare_timings(&ServerPolicy {
        probe_timeout_seconds: Some(1),
        busy_grace_seconds: Some(1),
        join_timeout_seconds: Some(4),
        ..ServerPolicy::default()
    });
    assert_eq!(lease::timings().busy_grace, Duration::from_secs(1));
    assert_eq!(lease::timings().join_timeout, Duration::from_secs(4));
}

/// Write `lease` as the checkout's lease file.
fn publish(repo: &Repository, lease: serde_json::Value) {
    let path = lease::lease_path(repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, lease.to_string()).unwrap();
}

/// What a subscriber wrote, for a test that reads what the election said.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_busy_owner_is_given_up_on_after_the_declared_grace_and_the_log_names_it() {
    declare();
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).expect("the fixture is a repository");
    // a listener that accepts connections into its backlog and never answers one: the
    // owner's process (this one) is alive, so the election calls it busy and waits
    let silent = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", silent.local_addr().unwrap());
    publish(
        &repo,
        serde_json::json!({
            "schema": lease::SCHEMA,
            "pid": std::process::id(),
            "token": "busy-under-a-declaration",
            "root": repo.root(),
            "url": url,
            "started_at": "2026-09-15T00:00:00Z",
            "version": majordomus_cli::VERSION,
        }),
    );
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .finish();
    let started = Instant::now();
    let role = tracing::subscriber::with_default(subscriber, || lease::elect(&repo))
        .expect("the election decides within the declared join timeout");
    let took = started.elapsed();
    match role {
        Role::Server(mine) => mine.release(),
        Role::Peer { url } => panic!("attached to a server that never answers: {url}"),
    }
    assert!(
        took >= Duration::from_secs(1),
        "taken over before the declared grace: {took:?}"
    );
    assert!(
        took < BUSY_GRACE,
        "the compiled grace was waited out, not the declared one: {took:?}"
    );
    let log = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    assert!(
        log.contains("has not answered for 1 seconds; taking it over"),
        "the take-over names the declared grace: {log}"
    );
    drop(silent);
}

#[test]
fn an_election_that_cannot_join_is_refused_after_the_declared_timeout() {
    declare();
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).expect("the fixture is a repository");
    // a fresh lease that names no address yet: an owner still binding, waited on until the
    // election's own bound
    publish(
        &repo,
        serde_json::json!({
            "schema": lease::SCHEMA,
            "pid": std::process::id(),
            "token": "binding-under-a-declaration",
            "root": repo.root(),
            "started_at": "2026-09-15T00:00:00Z",
            "version": majordomus_cli::VERSION,
        }),
    );
    let started = Instant::now();
    let refused = match lease::elect(&repo) {
        Err(e) => e.to_string(),
        Ok(Role::Server(mine)) => {
            mine.release();
            panic!("took over a lease whose owner is still binding");
        }
        Ok(Role::Peer { url }) => panic!("attached to an owner with no address: {url}"),
    };
    let took = started.elapsed();
    assert!(
        refused.contains("within 4 seconds"),
        "the refusal names the declared timeout: {refused}"
    );
    assert!(
        took >= Duration::from_secs(4) && took < JOIN_TIMEOUT,
        "the declared timeout bounded the election, not the compiled one: {took:?}"
    );
    std::fs::remove_file(lease::lease_path(&repo)).unwrap();
}
