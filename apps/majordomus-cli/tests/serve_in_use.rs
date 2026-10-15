//! A server someone is using is not idle, whatever the transport (I2131).
//!
//! The idle timer counted MCP sessions alone, so a person reading the Cockpit or a client of
//! the REST API watched the server stop under them at the idle timeout. Here a real `serve`
//! with a two-second idle timeout is read over plain HTTP for longer than that, and must still
//! be there; left alone, it must end by itself.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::Stdio;
use std::time::{Duration, Instant};

use common::{dist_share, Fixture};

fn get_home(address: &str) -> bool {
    let Ok(mut stream) = TcpStream::connect(address) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let request = format!("GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).is_ok() && raw.starts_with(b"HTTP/1.1 200")
}

#[test]
fn a_server_read_over_http_outlives_its_idle_timeout_and_ends_when_left_alone() {
    let f = Fixture::new();
    let mut child = common::majordomus()
        .args(["serve", "--port", "0", "--idle", "2"])
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "info")
        .env("MAJORDOMUS_SHARE", dist_share())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn majordomus serve");
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut address = None;
    for line in lines.by_ref() {
        let line = line.unwrap();
        if let Some(rest) = line.split("listening on http://").nth(1) {
            address = Some(rest.split_whitespace().next().unwrap().to_string());
            break;
        }
    }
    std::thread::spawn(move || for _ in lines {});
    let address = address.expect("serve reported its address");

    // read for twice the idle timeout: the server is in use, and stays
    let reading = Instant::now();
    while reading.elapsed() < Duration::from_secs(4) {
        assert!(
            get_home(&address),
            "the server stopped while it was being read"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
    assert!(
        child.try_wait().unwrap().is_none(),
        "the server ended while in use"
    );

    // left alone, it ends by itself within a timeout and some slack
    let left = Instant::now();
    while child.try_wait().unwrap().is_none() {
        assert!(
            left.elapsed() < Duration::from_secs(10),
            "the server did not end once nobody used it"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Open the live channel at `/events` and read the handshake and the first byte of the first
/// frame, which the channel's own thread writes once it is open. The socket is returned open.
fn open_events(address: &str) -> TcpStream {
    let mut stream = TcpStream::connect(address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    let request = format!(
        "GET /events HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        assert_eq!(
            stream.read(&mut byte).unwrap(),
            1,
            "closed in the handshake"
        );
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head);
    assert!(head.starts_with("HTTP/1.1 101"), "{head}");
    assert_eq!(
        stream.read(&mut byte).unwrap(),
        1,
        "the channel's first frame"
    );
    stream
}

/// A person watching executions over the live channel sends nothing after the handshake and
/// asks no other route, and is using the server for as long as the socket is open: a real
/// `serve` with a two-second idle timeout and no MCP session, watched over `/events` for more
/// than twice that, is still running. Once the socket closes it ends within one timeout and
/// scheduling slack — not at the channel's next heartbeat, twenty seconds on, which is when a
/// server that never looks at its channel's socket would first learn the client had gone.
#[test]
fn a_server_watched_over_its_live_channel_outlives_its_idle_timeout_and_ends_once_it_closes() {
    let f = Fixture::new();
    let mut child = common::majordomus()
        .args(["serve", "--port", "0", "--idle", "2"])
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "info")
        .env("MAJORDOMUS_SHARE", dist_share())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn majordomus serve");
    let mut lines = BufReader::new(child.stderr.take().unwrap()).lines();
    let mut address = None;
    for line in lines.by_ref() {
        let line = line.unwrap();
        if let Some(rest) = line.split("listening on http://").nth(1) {
            address = Some(rest.split_whitespace().next().unwrap().to_string());
            break;
        }
    }
    std::thread::spawn(move || for _ in lines {});
    let address = address.expect("serve reported its address");

    // open, and quiet, for more than twice the idle timeout
    let channel = open_events(&address);
    let watching = Instant::now();
    while watching.elapsed() < Duration::from_secs(5) {
        if child.try_wait().unwrap().is_some() {
            panic!("the server stopped while its live channel was open");
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    // closed, it ends within one timeout (2 s) and slack, far inside a heartbeat (20 s)
    drop(channel);
    let closed = Instant::now();
    let bound = Duration::from_secs(8);
    while child.try_wait().unwrap().is_none() && closed.elapsed() < bound {
        std::thread::sleep(Duration::from_millis(100));
    }
    let took = closed.elapsed();
    if child.try_wait().unwrap().is_none() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("the server was still running {took:?} after its only live channel closed");
    }
    eprintln!("the server ended {took:?} after its live channel closed");
}
