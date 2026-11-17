//! Stopping a served socket finishes the requests already handed to a thread of their own
//! before it returns: a process that stops while a tool call runs answers that call first.

mod common;

use std::time::{Duration, Instant};

use common::Fixture;
use majordomus_cli::http::{server, Router};

/// How long the request in flight runs.
const DELAY: Duration = Duration::from_millis(3000);

#[test]
fn stopping_waits_for_a_request_in_flight_to_be_answered() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let url = bound.url();
    let running = bound.start(Router::new(app.context.clone(), "test"));
    assert!(
        !server::used_within(Duration::from_secs(3600)),
        "nothing was asked of this process yet"
    );

    let sent = Instant::now();
    let call = {
        let url = url.clone();
        std::thread::spawn(move || {
            let reply = majordomus_cli::mcp::bridge::request(
                &url,
                "GET",
                &format!(
                    "/api/v1/executions/demonstrate?steps=1&delay_ms={}",
                    DELAY.as_millis()
                ),
                &[],
                None,
                Duration::from_secs(120),
            );
            (reply.map(|r| r.status).ok(), Instant::now())
        })
    };
    // the request has reached a handler once the process notes it as used
    let waited = Instant::now();
    while !server::used_within(Duration::from_secs(3600)) {
        assert!(
            waited.elapsed() < Duration::from_secs(60),
            "the request never reached a handler"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    running.stop();
    let stopped = Instant::now();
    let (status, answered) = call.join().unwrap();
    assert_eq!(status, Some(200), "the request in flight was answered");
    assert!(
        stopped.duration_since(sent) >= DELAY,
        "stop returned {:?} after the request was sent, before it could have been answered",
        stopped.duration_since(sent)
    );
    assert!(answered >= sent + DELAY);
}
