//! Only the holder removes its lease, and `serve stop` stops a server in order (I2128).
//!
//! Two defects lived in the same few lines. The signal handler unlinked the lease whenever
//! the process had held one and then died of the signal, so a `SIGTERM` that reached a server
//! in the half second between another process taking its lease over and its own reader
//! noticing removed the *successor's* lease: the checkout then had a running server and no
//! lease, and the next client started a second one. And because `serve stop` is a `SIGTERM`,
//! every stop was that kill: the episodes the server held were never closed, and in-flight
//! answers were cut.
//!
//! These tests drive real processes: a `majordomus serve` this test starts in a fixture of its
//! own, signalled by the pid this test spawned and by nothing else.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{dist_share, Fixture, BIN};
use majordomus_cli::lease::LeaseFile;
use serde_json::{json, Value};

/// Generous: the machine running this suite is often running nine others.
const WAIT: Duration = Duration::from_secs(40);

/// A `majordomus serve` with no pipe on stdin (it runs until it is stopped), its stderr kept.
struct Server {
    child: Child,
    url: String,
    log: Arc<Mutex<String>>,
}

impl Server {
    fn start(f: &Fixture) -> Self {
        let mut child = Command::new(BIN)
            .args(["serve", "--port", "0"])
            .current_dir(f.root())
            .env("MAJORDOMUS_LOG", "info")
            .env("MAJORDOMUS_SHARE", dist_share())
            .env("XDG_STATE_HOME", f.path(".xdg-state"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn majordomus serve");
        let log = Arc::new(Mutex::new(String::new()));
        let (tx, rx) = std::sync::mpsc::channel();
        let stderr = child.stderr.take().unwrap();
        {
            let log = Arc::clone(&log);
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if let Some(rest) = line.split("listening on http://").nth(1) {
                        let address = rest.split_whitespace().next().unwrap_or("").to_string();
                        let _ = tx.send(format!("http://{address}"));
                    }
                    let mut log = log.lock().unwrap();
                    log.push_str(&line);
                    log.push('\n');
                }
            });
        }
        let url = match rx.recv_timeout(WAIT) {
            Ok(url) => url,
            Err(_) => {
                let _ = child.kill();
                panic!(
                    "serve did not listen within {WAIT:?}:\n{}",
                    log.lock().unwrap()
                );
            }
        };
        Server { child, url, log }
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn log(&self) -> String {
        self.log.lock().unwrap().clone()
    }

    /// Wait for the process to end, within [`WAIT`]; a process that does not is killed by
    /// the pid this test spawned and the test fails with its log.
    fn wait_exit(&mut self) -> ExitStatus {
        let status = reap(&mut self.child);
        status.unwrap_or_else(|| panic!("serve did not end within {WAIT:?}:\n{}", self.log()))
    }

    /// Reap the process on a thread of its own from now on. A child this test has not reaped
    /// is a zombie, and a zombie still answers `kill(pid, 0)`: `serve stop`, which waits for
    /// the server's pid to go, would wait out its whole bound on a process that had ended.
    fn reap_in_background(mut self) -> std::thread::JoinHandle<(Option<ExitStatus>, String)> {
        std::thread::spawn(move || {
            let status = reap(&mut self.child);
            (status, self.log())
        })
    }
}

/// Wait for `child` within [`WAIT`]; one that has not ended by then is killed (by the pid this
/// test spawned) and `None` is answered.
fn reap(child: &mut Child) -> Option<ExitStatus> {
    let deadline = Instant::now() + WAIT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn lease_path(f: &Fixture) -> PathBuf {
    f.path(".ai/local/state/mcp/server.json")
}

fn terminate(pid: u32) {
    // SAFETY: kill(2) with the pid of a child this test spawned and has not reaped.
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    assert_eq!(rc, 0, "kill -TERM {pid}");
}

/// Replace the lease atomically with one another process holds, the way a successful
/// takeover leaves it: the old owner then reads either its own lease or the successor's,
/// never half of one.
fn install_successor(path: &Path, root: &Path) -> String {
    let token = "a-successor-took-this-lease".to_string();
    let doc = json!({
        "schema": "majordomus-mcp-lease/v1",
        // a live process: this test's own, so nothing judges the successor dead
        "pid": std::process::id(),
        "token": token,
        "root": root,
        "url": "http://127.0.0.1:1",
        "started_at": "2026-10-09T00:00:00Z",
        "version": majordomus_cli::VERSION,
    });
    let tmp = path.with_file_name("successor.tmp");
    std::fs::write(&tmp, doc.to_string()).unwrap();
    std::fs::rename(&tmp, path).unwrap();
    token
}

/// A `SIGTERM` that reaches a server in the window after its lease was taken over, before its
/// own reader has noticed, must leave the successor's lease where it is.
#[test]
fn sigterm_spares_a_successor_lease() {
    let f = Fixture::new();
    let mut server = Server::start(&f);
    let path = lease_path(&f);
    assert!(matches!(LeaseFile::read(&path), LeaseFile::Document(_)));

    // the takeover and the signal in the same instant: well inside the server's 500 ms tick
    let successor = install_successor(&path, &f.root());
    terminate(server.pid());
    server.wait_exit();

    match LeaseFile::read(&path) {
        LeaseFile::Document(d) => assert_eq!(
            d.token,
            successor,
            "the stopped server removed or rewrote a lease that was not its own:\n{}",
            server.log()
        ),
        other => panic!(
            "the successor's lease is gone ({other:?}): the stopped server removed a lease \
             it no longer held\n{}",
            server.log()
        ),
    }
}

/// One HTTP/1.1 request; (status, lowercased headers, body).
fn http(
    url: &str,
    method: &str,
    target: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (u16, Vec<(String, String)>, String) {
    let host = url.trim_start_matches("http://");
    let mut stream = TcpStream::connect(host).expect("connect");
    stream.set_read_timeout(Some(WAIT)).unwrap();
    let body = body.unwrap_or("");
    let extra: String = headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}\r\n"))
        .collect();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\n\
         {extra}Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").expect("a header/body split");
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .expect("a status line");
    let headers = lines
        .filter_map(|l| {
            l.split_once(": ")
                .map(|(k, v)| (k.to_lowercase(), v.to_string()))
        })
        .collect();
    (status, headers, body.to_string())
}

/// A stand-in for the repository's own tool, which the server's episode driver runs as
/// `bin/majordomus capture session --event start|end`: it writes down every call and the
/// payload it was handed, so the test can read which episodes were opened and closed.
fn install_capture_recorder(f: &Fixture) -> PathBuf {
    let record = f.path("capture.log");
    let script = format!(
        "#!/bin/sh\npayload=$(cat)\nprintf '%s %s\\n' \"$*\" \"$payload\" >> '{}'\n",
        record.display()
    );
    f.write("bin/majordomus", &script);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            f.path("bin/majordomus"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    record
}

/// `serve stop` is the ordered stop: the server closes the episodes it holds, releases its
/// lease, and ends by returning from `main` — exit status 0, not death by signal 15.
#[test]
fn serve_stop_ends_the_server_in_order_and_closes_its_episodes() {
    let f = Fixture::new();
    let record = install_capture_recorder(&f);
    let server = Server::start(&f);
    let url = server.url.clone();
    let log = Arc::clone(&server.log);
    let server_log = move || log.lock().unwrap().clone();

    // a peer that carries a piece of work: an episode the server holds
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18", "capabilities": {},
            "clientInfo": { "name": "lease-holder-test", "version": "0" }
        }
    })
    .to_string();
    let (status, headers, body) = http(&url, "POST", "/mcp", &[], Some(&init));
    assert_eq!(status, 200, "{body}");
    let sid = headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.clone())
        .expect("a session id");
    let attach = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": "majordomus_session_attach",
                    "arguments": { "external_id": "work-in-progress-1" } }
    })
    .to_string();
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &sid)],
        Some(&attach),
    );
    assert_eq!(status, 200, "{body}");
    let answer: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    assert!(answer["result"]["isError"] != json!(true), "{body}");
    let recorded = std::fs::read_to_string(&record).unwrap_or_default();
    assert!(
        recorded.contains("--event start") && recorded.contains("work-in-progress-1"),
        "the episode was opened through the repository's tool: {recorded}"
    );

    // the person's command, exactly as they type it
    let ended = server.reap_in_background();
    let out = Command::new(BIN)
        .args(["serve", "stop"])
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", dist_share())
        .env("XDG_STATE_HOME", f.path(".xdg-state"))
        .output()
        .expect("run serve stop");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "serve stop: {stdout}{}\nserver log:\n{}",
        String::from_utf8_lossy(&out.stderr),
        server_log()
    );
    assert!(stdout.starts_with("stopped "), "{stdout}");

    let (status, final_log) = ended.join().unwrap();
    let status = status.unwrap_or_else(|| panic!("serve did not end:\n{final_log}"));
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            status.signal(),
            None,
            "the server died of a signal instead of stopping in order:\n{}",
            final_log
        );
    }
    assert_eq!(status.code(), Some(0), "{}", final_log);

    let recorded = std::fs::read_to_string(&record).unwrap_or_default();
    let closed = recorded
        .lines()
        .find(|l| l.contains("--event end") && l.contains("work-in-progress-1"));
    assert!(
        closed.is_some_and(|l| l.contains("shutdown")),
        "the stopped server closed its episode as a shutdown, through the repository's tool; \
         recorded:\n{recorded}\nserver log:\n{}",
        final_log
    );
    assert!(final_log.contains("shared server stopped"), "{}", final_log);
    assert_eq!(LeaseFile::read(&lease_path(&f)), LeaseFile::Absent);
}
