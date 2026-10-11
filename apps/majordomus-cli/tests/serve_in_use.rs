//! A server someone is using is not idle, whatever the transport (I2131).
//!
//! The idle timer counted MCP sessions alone, so a person reading the Cockpit or a client of
//! the REST API watched the server stop under them at the idle timeout. Here a real `serve`
//! with a two-second idle timeout is read over plain HTTP for longer than that, and must still
//! be there; left alone, it must end by itself.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{dist_share, Fixture, BIN};

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
    let mut child = Command::new(BIN)
        .args(["serve", "--port", "0", "--idle", "2"])
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "info")
        .env("MAJORDOMUS_SHARE", dist_share())
        .env_remove("MAJORDOMUS_HTTP_HOST")
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
