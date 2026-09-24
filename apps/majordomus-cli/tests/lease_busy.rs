//! A busy server is not a dead one: the election waits for a live server that is slow to
//! answer instead of taking its lease over.
//!
//! On 2026-09-15 ten MCP clients attached at once to the server a shell entry had started
//! (test/cases/192). Nine were bridged to it; the tenth probed while the server was
//! answering the other nine, heard nothing within the probe's timeout, called the lease
//! stale, took it over and started a second server for one checkout. These tests hold the
//! three sides of the distinction with a stand-in server whose answers they control.

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Fixture;
use majordomus_cli::lease::{self, Role, BUSY_GRACE, PROBE_TIMEOUT};
use majordomus_cli::Repository;

/// A stand-in for the checkout's server: every connection is read, then held silent for
/// `silence` before the index is answered — or never answered, when `silence` is `None`.
/// Only the first `slow` connections are held; the rest are answered at once.
fn stand_in(root: &Path, slow: usize, silence: Option<Duration>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
    let url = format!("http://{}", listener.local_addr().unwrap());
    let body = serde_json::json!({
        "name": "majordomus",
        "repository_id": majordomus_cli::repository::identity(root),
        lease::LEASEHOLDER_KEY: true,
    })
    .to_string();
    let seen = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let n = seen.fetch_add(1, Ordering::SeqCst);
            let body = body.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                if n < slow {
                    match silence {
                        Some(d) => std::thread::sleep(d),
                        None => loop {
                            std::thread::sleep(Duration::from_secs(60));
                        },
                    }
                }
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            });
        }
    });
    url
}

/// Publish a lease naming `url`, held by the process `pid`.
fn publish(repo: &Repository, url: &str, pid: u32) {
    let path = lease::lease_path(repo);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        serde_json::json!({
            "schema": lease::SCHEMA,
            "pid": pid,
            "token": "stand-in",
            "root": repo.root(),
            "url": url,
            "started_at": "2026-09-15T00:00:00Z",
            "version": majordomus_cli::VERSION,
        })
        .to_string(),
    )
    .unwrap();
}

#[test]
fn a_live_server_slow_to_answer_is_attached_to_not_taken_over() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).expect("the fixture is a repository");
    // the first probe hears nothing within the timeout; the server answers the next one
    let url = stand_in(repo.root(), 1, Some(PROBE_TIMEOUT + Duration::from_secs(1)));
    publish(&repo, &url, std::process::id());
    match lease::elect(&repo).expect("the election decides") {
        Role::Peer { url: found } => {
            assert_eq!(found, url, "attached to the server that holds the lease")
        }
        Role::Server(mine) => {
            mine.release();
            panic!("a live server that was slow to answer had its lease taken over");
        }
    }
}

#[test]
fn a_silent_server_whose_process_is_gone_is_taken_over_at_once() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).expect("the fixture is a repository");
    let url = stand_in(repo.root(), usize::MAX, None);
    // a pid that cannot be a live process of this host
    publish(&repo, &url, i32::MAX as u32);
    let started = Instant::now();
    match lease::elect(&repo).expect("the election decides") {
        Role::Server(mine) => {
            assert!(
                started.elapsed() < BUSY_GRACE,
                "a dead owner was waited on like a busy one: {:?}",
                started.elapsed()
            );
            mine.release();
        }
        Role::Peer { url } => panic!("attached to a silent server whose process is gone: {url}"),
    }
}

#[test]
fn a_live_server_that_stays_silent_loses_its_lease_after_the_grace() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).expect("the fixture is a repository");
    let url = stand_in(repo.root(), usize::MAX, None);
    publish(&repo, &url, std::process::id());
    let started = Instant::now();
    match lease::elect(&repo).expect("the election decides within its own bound") {
        Role::Server(mine) => {
            assert!(
                started.elapsed() >= BUSY_GRACE,
                "a live server was taken over before the grace: {:?}",
                started.elapsed()
            );
            mine.release();
        }
        Role::Peer { url } => panic!("attached to a server that never answers: {url}"),
    }
}
