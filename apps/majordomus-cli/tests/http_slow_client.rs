//! A slow or silent HTTP client cannot hold the shared server (I2156).
//!
//! `tiny_http` never times a read out, so a client that opened a connection and sent one byte
//! a second held a thread forever: on a request head, on a body, or kept alive between
//! requests. Enough of them held every request handler, and past the bound a worker read the
//! next slow body itself. Here slow clients meet a real socket under a declared read deadline:
//! each is closed at the deadline and counted, a fifth client is answered, a request that
//! runs longer than the deadline is not cut short, and past the handler bound a request is
//! refused at once while the probe is still answered.
//!
//! "Answered" and not "answered at once": `tiny_http` gives each connection a thread of its
//! pool for as long as the connection lives, and a connection that arrives in a burst can wait
//! in the pool's queue until one of those threads is free — behind slow clients, until the
//! deadline frees one. Without the deadline that wait had no end. The slow clients here
//! connect a little apart, as separate clients do, so that each is given a thread.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use common::Fixture;
use majordomus_cli::http::{deadline, server, Router};
use majordomus_cli::lease;
use majordomus_cli::policy::ServerPolicy;

/// The deadline every test of this binary declares.
const DEADLINE: Duration = Duration::from_secs(6);

/// The handler bound and the counters are the process's: one test at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn declare() {
    lease::declare_timings(&ServerPolicy {
        read_deadline_seconds: Some(DEADLINE.as_secs()),
        ..ServerPolicy::default()
    });
    assert_eq!(lease::timings().read_deadline, DEADLINE);
}

fn get(url: &str, path: &str, timeout: Duration) -> Option<u16> {
    majordomus_cli::mcp::bridge::request(url, "GET", path, &[], None, timeout)
        .ok()
        .map(|r| r.status)
}

fn closed_by_deadline() -> u64 {
    deadline::counts().map_or(0, |c| c.closed_by_deadline)
}

/// A client that sends `head` at once and then one byte a second, until the server closes
/// the connection. Answers how long the connection lived, or nothing past `give_up`.
fn trickle(address: &str, head: &'static [u8], give_up: Duration) -> Option<Duration> {
    let opened = Instant::now();
    let mut stream = TcpStream::connect(address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let _ = stream.write_all(head);
    let mut next = Instant::now();
    let mut buffer = [0u8; 512];
    while opened.elapsed() < give_up {
        if Instant::now() >= next {
            if stream.write_all(b"a").is_err() {
                return Some(opened.elapsed());
            }
            next += Duration::from_secs(1);
        }
        match stream.read(&mut buffer) {
            Ok(0) => return Some(opened.elapsed()),
            // a 408 or a 503 written before the close is still a close
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return Some(opened.elapsed()),
        }
    }
    None
}

#[test]
fn slow_client_released_test() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    declare();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let url = bound.url();
    let address = bound.address().to_string();
    let running = bound.start(Router::new(app.context.clone(), "test"));
    let before = closed_by_deadline();

    // a call that runs longer than the deadline is answered in full: the deadline is on
    // reading a request, never on answering one
    let long = {
        let url = url.clone();
        std::thread::spawn(move || {
            get(
                &url,
                "/api/v1/executions/demonstrate?steps=1&delay_ms=7500",
                Duration::from_secs(30),
            )
        })
    };
    // four clients that never finish a head, four that never finish a body
    let heads: &'static [u8] = b"GET / HTTP/1.1\r\nHost: x\r\n";
    let bodies: &'static [u8] = b"POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Length: 5000\r\n\r\n";
    let slow: Vec<_> = (0..8)
        .map(|n| {
            let address = address.clone();
            let head = if n < 4 { heads } else { bodies };
            std::thread::sleep(Duration::from_millis(25));
            std::thread::spawn(move || trickle(&address, head, Duration::from_secs(20)))
        })
        .collect();
    std::thread::sleep(Duration::from_millis(500));

    // a fifth client is answered: on a quiet machine at once, and never later than the
    // deadline lets a slow client keep a connection thread
    let asked = Instant::now();
    assert_eq!(get(&url, "/", DEADLINE + Duration::from_secs(5)), Some(200));
    assert!(
        asked.elapsed() < DEADLINE + Duration::from_secs(3),
        "the fifth client waited {:?}",
        asked.elapsed()
    );

    for (n, client) in slow.into_iter().enumerate() {
        let lived = client
            .join()
            .unwrap()
            .unwrap_or_else(|| panic!("slow client {n} was never closed"));
        assert!(
            lived >= DEADLINE - Duration::from_millis(100),
            "slow client {n} was closed after {lived:?}, before the deadline"
        );
        assert!(
            lived < DEADLINE + Duration::from_secs(3),
            "slow client {n} held its connection for {lived:?}, past the deadline"
        );
    }
    assert_eq!(
        long.join().unwrap(),
        Some(200),
        "the long call was cut short"
    );
    assert!(
        closed_by_deadline() >= before + 8,
        "every close is counted: {} then {}",
        before,
        closed_by_deadline()
    );
    let counts = deadline::counts().expect("a serving process reports its counts");
    assert_eq!(counts.read_deadline_seconds, DEADLINE.as_secs());
    running.stop();
}

#[test]
fn past_the_handler_bound_a_request_is_refused_and_the_probe_is_answered() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    declare();
    let f = Fixture::new();
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let url = bound.url();
    let address = bound.address().to_string();
    let running = bound.start(Router::new(app.context.clone(), "test"));
    let refused_before = deadline::counts().map_or(0, |c| c.refused_busy);

    // every handler reads a body that never comes; a few more than there are handlers, so
    // that they fill whichever request takes a slot first, and the extra ones are refused
    let bodies: &'static [u8] = b"POST /mcp HTTP/1.1\r\nHost: x\r\nContent-Length: 5000\r\n\r\n";
    let opened = Instant::now();
    let slow: Vec<_> = (0..server::MAX_HANDLERS + 4)
        .map(|_| {
            let address = address.clone();
            std::thread::sleep(Duration::from_millis(25));
            std::thread::spawn(move || trickle(&address, bodies, Duration::from_secs(20)))
        })
        .collect();
    // a request past the bound is refused at once, not read by a worker that would be held:
    // asked until every handler has reached its slow body, which a loaded machine takes a
    // moment to arrange, and well before the deadline frees them
    let refused = loop {
        let asked = Instant::now();
        let reply = majordomus_cli::mcp::bridge::request(
            &url,
            "GET",
            "/api/v1/server",
            &[],
            None,
            Duration::from_secs(5),
        )
        .expect("an answer");
        if reply.status == 503 {
            assert!(
                asked.elapsed() < Duration::from_secs(1),
                "the refusal waited"
            );
            break reply;
        }
        assert_eq!(reply.status, 200, "{}", reply.body);
        assert!(
            opened.elapsed() < DEADLINE - Duration::from_secs(1),
            "the handlers never all held a slow body"
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    assert!(refused.body.contains("busy"), "{}", refused.body);
    // and the probe, which decides whether this server keeps its lease, is answered
    assert_eq!(get(&url, "/", Duration::from_secs(5)), Some(200));
    assert!(deadline::counts().unwrap().refused_busy > refused_before);

    for client in slow {
        assert!(
            client.join().unwrap().is_some(),
            "a slow body outlived the deadline"
        );
    }
    // the handlers are free again, and the status names what the deadline did
    let status = majordomus_cli::mcp::bridge::request(
        &url,
        "GET",
        "/api/v1/server",
        &[],
        None,
        Duration::from_secs(10),
    )
    .expect("an answer");
    assert_eq!(status.status, 200, "{}", status.body);
    let body: serde_json::Value = serde_json::from_str(&status.body).expect("JSON");
    let connections = body
        .pointer("/data/connections")
        .or_else(|| body.pointer("/connections"))
        .unwrap_or_else(|| panic!("no connections in {body}"));
    assert_eq!(connections["read_deadline_seconds"], DEADLINE.as_secs());
    assert!(connections["closed_by_deadline"].as_u64().unwrap() >= server::MAX_HANDLERS as u64);
    running.stop();
}
