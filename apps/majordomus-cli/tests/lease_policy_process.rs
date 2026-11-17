//! The lease timings a repository declares are the ones its server elects by (I2129).
//!
//! The timings were a `OnceLock` filled with the compiled defaults on first read, and `mcp`
//! and `serve` read them during the election, before anything had read the policy. A
//! repository that declared `server.bind_grace_seconds` changed nothing a server did: the
//! declaration reached `env enter` alone. Here a real `serve` meets a lease a starting owner
//! wrote and never filled, under a declared grace of one second; and a real `serve` and a
//! real `mcp` each meet a live owner that never answers, under a declared busy grace of one
//! second, and give up on it after that second rather than after the compiled ten.

mod common;

use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use common::{Fixture, Served};
use majordomus_cli::lease;

/// Append a `server:` block declaring `timing` to the fixture's policy, which declares none.
fn declare(f: &Fixture, timing: &str) {
    let policy = std::fs::read_to_string(f.path(".ai/repo/policy.yaml")).expect("a policy");
    assert!(
        !policy.contains("\nserver:"),
        "the fixture declares no server block of its own"
    );
    f.write(
        ".ai/repo/policy.yaml",
        &format!("{policy}\nserver:\n  {timing}\n"),
    );
}

#[test]
fn serve_waits_on_a_starting_owner_only_as_long_as_the_policy_declares() {
    let f = Fixture::new();
    declare(&f, "bind_grace_seconds: 1");
    // a lease a starting owner wrote and never filled, older than the declared grace and far
    // younger than the compiled one (15 s)
    let lease = f.path(".ai/local/state/mcp/server.json");
    std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
    std::fs::write(&lease, "").unwrap();
    std::thread::sleep(Duration::from_secs(2));

    let started = Instant::now();
    let mut served = Served::start(&f.root(), &["--idle", "0"]);
    let took = started.elapsed();
    assert_eq!(served.stop(), 0);
    assert!(
        took < Duration::from_secs(8),
        "the server waited {took:?} on an abandoned lease: the declared one-second grace was not the one used"
    );
}

/// Declare a one-second busy grace and plant the lease of a busy owner: the address it names
/// accepts connections into its backlog and never answers one, and the pid it names is this
/// test's, which is alive. The election calls such an owner busy, not gone, and waits on it
/// for the busy grace before it takes the lease over. The listener is returned so that it
/// stays bound for as long as the caller holds it.
fn plant_busy_owner(f: &Fixture) -> TcpListener {
    declare(f, "busy_grace_seconds: 1");
    let silent = TcpListener::bind("127.0.0.1:0").expect("a free loopback port");
    let url = format!("http://{}", silent.local_addr().unwrap());
    let doc = serde_json::json!({
        "schema": lease::SCHEMA,
        "pid": std::process::id(),
        "token": "a-busy-owner",
        "root": f.root(),
        "url": url,
        "started_at": "2026-10-14T00:00:00Z",
        "version": majordomus_cli::VERSION,
    });
    let path = f.path(".ai/local/state/mcp/server.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, doc.to_string()).unwrap();
    silent
}

/// Start `majordomus <args>` in the fixture, with stdin a pipe so that closing it stops the
/// process, and read its stderr until it says where it listens. What it logged on the way,
/// and how long it took to get there.
fn elect(f: &Fixture, args: &[&str]) -> (Child, Vec<String>, Duration) {
    let started = Instant::now();
    let mut child = common::majordomus()
        .args(args)
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "info")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove(lease::JUDGED_STALE_ENV)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn majordomus");
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut log = Vec::new();
    for line in lines.by_ref() {
        let line = line.unwrap();
        let listening = line.contains("listening on http://");
        log.push(line);
        if listening {
            break;
        }
    }
    let took = started.elapsed();
    // keep draining stderr so the child never blocks on a full pipe
    std::thread::spawn(move || for _ in lines {});
    assert!(
        log.last()
            .is_some_and(|l| l.contains("listening on http://")),
        "{args:?} never listened; stderr:\n{}",
        log.join("\n")
    );
    (child, log, took)
}

/// What both entry points must show about one election against a busy owner: the take-over
/// names the declared grace, it came after that grace and well before the compiled one, the
/// lease now belongs to the process that elected, and the process ends cleanly.
fn assert_gave_up_after_the_declared_grace(
    f: &Fixture,
    silent: &TcpListener,
    (mut child, log, took): (Child, Vec<String>, Duration),
) {
    let log = log.join("\n");
    assert!(
        log.contains("has not answered for 1 seconds; taking it over"),
        "the take-over names the declared one-second grace: {log}"
    );
    // one silent probe (the compiled two-second timeout), then the grace, then one more
    // probe; under the compiled grace (10 s) the take-over cannot happen before 12 s
    assert!(
        took >= Duration::from_secs(1),
        "taken over before the declared grace: {took:?}"
    );
    assert!(
        took < Duration::from_secs(9),
        "the election waited {took:?} on a busy owner: the compiled busy grace was used, not the declared one"
    );
    let lease: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(f.path(".ai/local/state/mcp/server.json")).unwrap(),
    )
    .unwrap();
    assert_ne!(lease["token"], "a-busy-owner", "the lease was taken over");
    assert_ne!(
        lease["url"].as_str(),
        Some(format!("http://{}", silent.local_addr().unwrap()).as_str()),
        "the lease names the new server, not the silent one"
    );
    drop(child.stdin.take());
    let status = child.wait().expect("wait for the process");
    assert_eq!(status.code(), Some(0), "the process ended cleanly");
}

#[test]
fn serve_waits_on_a_busy_owner_only_as_long_as_the_policy_declares() {
    let f = Fixture::new();
    let silent = plant_busy_owner(&f);
    let elected = elect(&f, &["serve", "--port", "0", "--idle", "0"]);
    assert_gave_up_after_the_declared_grace(&f, &silent, elected);
}

#[test]
fn mcp_waits_on_a_busy_owner_only_as_long_as_the_policy_declares() {
    let f = Fixture::new();
    let silent = plant_busy_owner(&f);
    let elected = elect(&f, &["mcp", "--http-port", "0"]);
    assert_gave_up_after_the_declared_grace(&f, &silent, elected);
}
