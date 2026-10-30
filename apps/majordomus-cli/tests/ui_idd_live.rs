//! The intent workspace follows the executions this server runs (I2783), over a real socket.
//!
//! The workspace declares that it follows (`data-follows-executions`), and the page names the
//! channel (`data-events`) and the one WebSocket client that holds it (the Cockpit's
//! `socket.js`). A move is an execution, so it ends on that channel; the screen read again
//! after it — what the script fetches when an execution ends — reads the status the record
//! now derives. Nothing on the channel is the plan: it only says when to read again.
//!
//! They prove the criterion `the-owner-directs-work-from-the-ui#the-workspace-follows-moves`.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

/// The unscoped channel the workspace's page names, read until a frame of one of `types` for
/// `execution` arrives. Just enough of RFC 6455 to read what this server writes.
struct Channel {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Channel {
    /// Open the channel and return once the server has said `stream.ready`: the frame written
    /// after the subscription exists, so an execution started after it is promised to it.
    fn open(address: &str, target: &str) -> Channel {
        let mut stream = TcpStream::connect(address).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .expect("a read timeout");
        let request = format!(
            "GET {target} HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).expect("write");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            assert_eq!(stream.read(&mut byte).expect("the handshake"), 1);
            head.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&head).into_owned();
        assert!(head.starts_with("HTTP/1.1 101"), "{head}");
        let mut channel = Channel {
            stream,
            buffer: Vec::new(),
        };
        let ready = channel.next().expect("the first frame");
        assert_eq!(ready["type"], "stream.ready", "{ready}");
        channel
    }

    /// The next text frame as JSON; `None` when the connection ended or went quiet.
    fn next(&mut self) -> Option<Value> {
        loop {
            if self.buffer.len() >= 2 {
                let opcode = self.buffer[0] & 0x0f;
                let (length, header) = match self.buffer[1] & 0x7f {
                    126 if self.buffer.len() >= 4 => (
                        u16::from_be_bytes([self.buffer[2], self.buffer[3]]) as usize,
                        4,
                    ),
                    127 if self.buffer.len() >= 10 => (
                        u64::from_be_bytes(self.buffer[2..10].try_into().unwrap()) as usize,
                        10,
                    ),
                    n if n < 126 => (n as usize, 2),
                    _ => (usize::MAX, 0),
                };
                if length != usize::MAX && self.buffer.len() >= header + length {
                    let payload: Vec<u8> =
                        self.buffer.drain(..header + length).skip(header).collect();
                    if opcode == 0x1 {
                        return Some(serde_json::from_slice(&payload).expect("a frame is JSON"));
                    }
                    continue; // a ping: not a message
                }
            }
            let mut chunk = [0u8; 8192];
            match self.stream.read(&mut chunk) {
                Ok(0) | Err(_) => return None,
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
            }
        }
    }

    /// Frames until one of `types` names `execution`, bounded.
    fn until(&mut self, execution: &str, types: &[&str]) -> Value {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            // the server writes a heartbeat every 20 s, so a read that times out is a dead
            // channel, not a quiet one
            let Some(frame) = self.next() else { break };
            let named =
                frame["data"]["execution_id"] == execution || frame["execution_id"] == execution;
            if named && types.iter().any(|t| frame["type"] == *t) {
                return frame;
            }
        }
        panic!("the channel never said that {execution} ended");
    }
}

/// The attribute `name`'s value on the page's `<body>`.
fn body_attr(page: &str, name: &str) -> String {
    page.split(&format!("{name}=\""))
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("the page names no {name}"))
        .replace("&amp;", "&")
}

fn row(page: &str, issue: &str) -> String {
    page.split(&format!("id=\"issue-{issue}\""))
        .nth(1)
        .and_then(|rest| rest.split("</tr>").next())
        .expect("the issue's row")
        .to_string()
}

#[test]
fn the_workspace_names_the_channel_it_follows_and_the_module_that_holds_it() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, page) = s.request("GET", "/ui/idd?intent=fixture-intent", None);
    assert_eq!(status, 200, "{page}");
    assert!(page.contains("data-follows-executions"), "{page}");
    assert_eq!(body_attr(&page, "data-events"), "/events");
    assert!(page.contains("data-live-state"), "{page}");
    let lib = body_attr(&page, "data-socket-lib");
    assert!(lib.starts_with("/cockpit/assets/socket.js"), "{lib}");
    let (status, _, module) = s.request("GET", &lib, None);
    assert_eq!(status, 200);
    assert!(module.contains("export function follow"), "{module}");

    // a screen that does not follow says nothing of the kind: the channel is the workspace's
    let (status, _, roadmap) = s.request("GET", "/ui/roadmap", None);
    assert_eq!(status, 200, "{roadmap}");
    assert!(!roadmap.contains("data-follows-executions"), "{roadmap}");
}

#[test]
fn a_move_ends_on_the_channel_the_workspace_follows_and_the_screen_read_again_shows_it() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let fragment = "/ui/idd?intent=fixture-intent&fragment";
    let (_, _, before) = s.request("GET", fragment, None);
    assert!(row(&before, "I0001").contains("ready"), "{before}");

    let mut channel = Channel::open(&s.address, "/events");
    let origin = format!("http://{}", s.address);
    let (status, _, body) = s.request_with(
        "POST",
        "/api/v1/executions/start",
        Some(
            &json!({ "capability": "plan.transition", "input": { "issue": "I0001", "transition": "start" } })
                .to_string(),
        ),
        &[("Origin", origin.as_str())],
    );
    assert_eq!(status, 200, "the move was not started: {body}");
    let id = serde_json::from_str::<Value>(&body).expect("JSON")["id"]
        .as_str()
        .expect("an execution id")
        .to_string();

    let ended = channel.until(
        &id,
        &[
            "execution.completed",
            "execution.failed",
            "execution.cancelled",
        ],
    );
    assert_eq!(ended["type"], "execution.completed", "{ended}");

    // what the script does when an execution ends: read the screen again, and again while a
    // source line says it is catching up with a repository that changed after its reading
    // (`catchUp` in ui.js), bounded. A screen that says it is current must show the move.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (status, _, after) = s.request("GET", fragment, None);
        assert_eq!(status, 200, "{after}");
        if row(&after, "I0001").contains("active") {
            break;
        }
        assert!(
            after.contains("data-catching-up"),
            "the screen says its readings are current and still reads the issue as before the move: {}",
            row(&after, "I0001")
        );
        assert!(
            Instant::now() < deadline,
            "the screen never caught up with the move: {}",
            row(&after, "I0001")
        );
        std::thread::sleep(Duration::from_millis(400));
    }
}
