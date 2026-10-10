//! A server busy with slow calls still answers the probe an election sends (I2126).
//!
//! Every request was answered on one of four workers, so four calls that outlasted the probe
//! timeout made a healthy server look wedged to the next client, which then took its lease.
//! Here four slow calls hold a real socket's workers as they used to, and `GET /` — what the
//! lease probe asks — must still be answered promptly.

mod common;

use std::time::{Duration, Instant};

use common::Fixture;
use majordomus_cli::http::{server, Router};

fn get(url: &str, path: &str, timeout: Duration) -> Option<u16> {
    majordomus_cli::mcp::bridge::request(url, "GET", path, &[], None, timeout)
        .ok()
        .map(|r| r.status)
}

#[test]
fn the_probe_is_answered_while_every_worker_would_be_busy() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let url = bound.url();
    let running = bound.start(Router::new(app.context.clone(), "test"));

    // as many slow calls as there are workers, and one more
    let slow: Vec<_> = (0..=server::WORKERS)
        .map(|_| {
            let url = url.clone();
            std::thread::spawn(move || {
                get(
                    &url,
                    "/api/v1/executions/demonstrate?steps=1&delay_ms=4000",
                    Duration::from_secs(30),
                )
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(500));

    let asked = Instant::now();
    let probe = get(&url, "/", Duration::from_secs(10));
    let took = asked.elapsed();
    assert_eq!(probe, Some(200), "the probe was answered");
    assert!(
        took < Duration::from_secs(2),
        "the probe waited {took:?} behind the slow calls: a busy server looks wedged"
    );
    for call in slow {
        assert_eq!(
            call.join().unwrap(),
            Some(200),
            "every slow call is answered too"
        );
    }
    running.stop();
}
