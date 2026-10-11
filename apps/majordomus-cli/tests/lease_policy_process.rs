//! The lease timings a repository declares are the ones its server elects by (I2129).
//!
//! The timings were a `OnceLock` filled with the compiled defaults on first read, and `mcp`
//! and `serve` read them during the election, before anything had read the policy. A
//! repository that declared `server.bind_grace_seconds` changed nothing a server did: the
//! declaration reached `env enter` alone. Here a real `serve` meets a lease a starting owner
//! wrote and never filled, under a declared grace of one second.

mod common;

use std::time::{Duration, Instant};

use common::{Fixture, Served};

#[test]
fn serve_waits_on_a_starting_owner_only_as_long_as_the_policy_declares() {
    let f = Fixture::new();
    let policy = std::fs::read_to_string(f.path(".ai/repo/policy.yaml")).expect("a policy");
    assert!(
        !policy.contains("\nserver:"),
        "the fixture declares no server block of its own"
    );
    f.write(
        ".ai/repo/policy.yaml",
        &format!("{policy}\nserver:\n  bind_grace_seconds: 1\n"),
    );
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
