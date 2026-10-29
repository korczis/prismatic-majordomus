//! An open live channel keeps the process in use: a person watching an execution is using
//! the server for as long as the socket stays open, whether or not they send anything.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use common::Fixture;
use majordomus_cli::http::{server, Router};

#[test]
fn an_open_live_channel_keeps_the_process_in_use_without_a_request() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let address = bound.address().to_string();
    let running = bound.start(Router::new(app.context.clone(), "test"));

    let mut stream = TcpStream::connect(&address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
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
    // the first frame is written by the channel's own thread, once it is open
    assert_eq!(
        stream.read(&mut byte).unwrap(),
        1,
        "the channel's first frame"
    );

    // the request that opened it is in the past now: only the open channel says "in use"
    std::thread::sleep(Duration::from_millis(5));
    assert!(
        server::used_within(Duration::ZERO),
        "an open channel is use, with no request inside the window"
    );

    drop(stream);
    running.stop();
}
