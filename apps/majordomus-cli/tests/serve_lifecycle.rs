//! The server's lifecycle as a person or a hook converges on it: `serve ensure` starts one
//! server and finds it the next time, three at once share one, a stale lease and a killed
//! server are both recovered, an idle server ends by itself, a taken port is not a failure,
//! `serve stop` ends the server this checkout's lease names and leaves another checkout's
//! alone.

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{dist_share, Fixture, BIN};
use majordomus_cli::lease::LeaseFile;
use serde_json::Value;

/// Run the executable in `cwd` with the distribution's share, capturing everything.
fn mj(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(BIN)
        .args(args)
        .current_dir(cwd)
        .env("MAJORDOMUS_SHARE", dist_share())
        .env("MAJORDOMUS_LOG", "info")
        .output()
        .expect("run majordomus");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn ensure(cwd: &Path, extra: &[&str]) -> (i32, Value, String) {
    let mut args = vec!["serve", "ensure", "--format", "json", "--wait", "40"];
    args.extend_from_slice(extra);
    let (code, out, err) = mj(cwd, &args);
    let v: Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("ensure printed no JSON ({e}): {out}\nstderr: {err}"));
    (code, v, err)
}

fn lease_path(f: &Fixture) -> std::path::PathBuf {
    f.path(".ai/local/state/mcp/server.json")
}

fn get(url: &str, target: &str) -> Option<(u16, String)> {
    let address = url.strip_prefix("http://")?;
    let mut stream =
        TcpStream::connect_timeout(&address.parse().ok()?, Duration::from_secs(2)).ok()?;
    stream
        .write_all(
            format!("GET {target} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .ok()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).ok()?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n")?;
    let status: u16 = head.split_whitespace().nth(1)?.parse().ok()?;
    Some((status, body.to_string()))
}

fn wait_until(what: &str, timeout: Duration, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("{what} did not happen within {timeout:?}");
}

#[test]
fn ensure_starts_one_server_finds_it_the_next_time_and_stop_ends_it() {
    let f = Fixture::new();
    let (code, a, err) = ensure(&f.root(), &["--idle", "120"]);
    assert_eq!(code, 0, "{a}\n{err}");
    assert_eq!(a["standing"], "ready", "{a}");
    assert_eq!(a["started"], true, "the first call is what started it");
    let url = a["url"].as_str().expect("an address").to_string();
    let pid = a["pid"].as_u64().expect("a pid");
    assert_eq!(a["version"], majordomus_cli::VERSION);
    match LeaseFile::read(&lease_path(&f)) {
        LeaseFile::Document(d) => {
            assert_eq!(d.url.as_deref(), Some(url.as_str()));
            assert_eq!(u64::from(d.pid), pid);
        }
        other => panic!("no lease after ensure: {other:?}"),
    }
    assert!(
        f.path(".ai/local/state/mcp/server.log").is_file(),
        "the started server logs beside the lease"
    );

    // the second call starts nothing and answers the same server
    let (code, b, _) = ensure(&f.root(), &["--idle", "120"]);
    assert_eq!(code, 0);
    assert_eq!(b["standing"], "ready");
    assert_eq!(b["started"], false, "nothing to start: {b}");
    assert_eq!(b["url"], url);
    assert_eq!(b["pid"], pid);

    // the server answers its own status with the lease it holds
    let (status, body) = get(&url, "/api/v1/server").expect("the server answers");
    assert_eq!(status, 200);
    let s: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(s["standing"], "ready");
    assert_eq!(s["this_process"]["pid"], pid);
    assert_eq!(s["this_process"]["url"], url);

    // the command line agrees, from outside the server
    let (code, out, err) = mj(&f.root(), &["serve", "status"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("standing   ready"), "{out}");
    assert!(out.contains("<- this checkout"), "{out}");

    // stop: the lease goes and the port closes
    let (code, out, err) = mj(&f.root(), &["serve", "stop"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.starts_with("stopped "), "{out}");
    assert_eq!(LeaseFile::read(&lease_path(&f)), LeaseFile::Absent);
    wait_until("the port closes", Duration::from_secs(5), || {
        get(&url, "/").is_none()
    });
    let (code, out, _) = mj(&f.root(), &["serve", "stop"]);
    assert_eq!(code, 0);
    assert!(out.contains("nothing to stop"), "{out}");
}

#[test]
fn three_ensures_at_once_share_one_server() {
    let f = Fixture::new();
    let root = f.root();
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let root = root.clone();
            std::thread::spawn(move || ensure(&root, &["--idle", "120"]))
        })
        .collect();
    let answers: Vec<(i32, Value, String)> =
        handles.into_iter().map(|h| h.join().unwrap()).collect();
    for (code, v, err) in &answers {
        assert_eq!(*code, 0, "{v}\n{err}");
        assert_eq!(v["standing"], "ready", "{v}");
    }
    let urls: std::collections::BTreeSet<&str> = answers
        .iter()
        .map(|(_, v, _)| v["url"].as_str().unwrap())
        .collect();
    assert_eq!(urls.len(), 1, "one server, whoever started it: {urls:?}");
    let pids: std::collections::BTreeSet<u64> = answers
        .iter()
        .map(|(_, v, _)| v["pid"].as_u64().unwrap())
        .collect();
    assert_eq!(pids.len(), 1, "one pid: {pids:?}");
    let (_, s) = get(urls.iter().next().unwrap(), "/api/v1/server").unwrap();
    let s: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(s["servers"].as_array().unwrap().len(), 1);
    // what stop said is the diagnosis: "still binding" means a second server took the lease
    let (code, out, err) = mj(&root, &["serve", "stop"]);
    assert_eq!(code, 0, "serve stop: {out}{err}");
}

#[test]
fn a_stale_lease_and_a_killed_server_are_both_recovered() {
    let f = Fixture::new();
    let lease = lease_path(&f);
    std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
    std::fs::write(
        &lease,
        format!(
            r#"{{"schema":"majordomus-mcp-lease/v1","pid":1,"token":"gone","root":"{}","url":"http://127.0.0.1:1","started_at":"2026-09-10T00:00:00Z","version":"{}"}}"#,
            f.root().display(),
            majordomus_cli::VERSION
        ),
    )
    .unwrap();
    let (code, a, err) = ensure(&f.root(), &["--idle", "120"]);
    assert_eq!(code, 0, "{a}\n{err}");
    assert_eq!(a["standing"], "ready");
    assert_eq!(a["started"], true, "the stale lease was taken over: {a}");
    let pid = a["pid"].as_u64().unwrap();
    assert_ne!(pid, 1);
    let url = a["url"].as_str().unwrap().to_string();

    // the server dies without cleaning up: the lease is left behind, stale
    let killed = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    wait_until(
        "the killed server stops answering",
        Duration::from_secs(5),
        || get(&url, "/").is_none(),
    );
    assert!(
        matches!(LeaseFile::read(&lease), LeaseFile::Document(_)),
        "SIGKILL leaves the lease behind"
    );
    let (code, out, _) = mj(&f.root(), &["serve", "status"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("standing   stale"), "{out}");

    // the next ensure recovers: a new server, a new pid, the lease rewritten
    let (code, b, err) = ensure(&f.root(), &["--idle", "120"]);
    assert_eq!(code, 0, "{b}\n{err}");
    assert_eq!(b["standing"], "ready");
    assert_eq!(b["started"], true);
    assert_ne!(b["pid"].as_u64().unwrap(), pid);
    let (code, _, _) = mj(&f.root(), &["serve", "stop"]);
    assert_eq!(code, 0);
}

#[test]
fn a_live_owner_that_answers_late_keeps_its_lease() {
    // The race that failed CI: a probe timing out against a live but slow owner must not
    // hand its lease to a second server. This owner is slow for its first two requests —
    // past one probe's timeout — and answers at once after that, the way a server under
    // load catches up; the patience a live owner gets must outlast that.
    let f = Fixture::new();
    let slow = TcpListener::bind("127.0.0.1:0").unwrap();
    let slow_url = format!("http://{}", slow.local_addr().unwrap());
    let body = serde_json::json!({
        "name": "majordomus",
        "repository_id": majordomus_cli::repository::identity(&f.root()),
    })
    .to_string();
    let served = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in slow.incoming() {
            let Ok(mut s) = stream else { continue };
            let body = body.clone();
            let nth = served.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut request = [0u8; 4096];
                let _ = s.read(&mut request);
                if nth < 2 {
                    std::thread::sleep(Duration::from_secs(3));
                }
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
            });
        }
    });
    let lease = lease_path(&f);
    std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
    std::fs::write(
        &lease,
        format!(
            r#"{{"schema":"majordomus-mcp-lease/v1","pid":{},"token":"late","root":"{}","url":"{}","started_at":"2026-09-10T00:00:00Z","version":"{}"}}"#,
            std::process::id(),
            f.root().display(),
            slow_url,
            majordomus_cli::VERSION
        ),
    )
    .unwrap();
    let (code, a, err) = ensure(&f.root(), &["--idle", "120"]);
    assert_eq!(code, 0, "{a}\n{err}");
    assert_eq!(a["standing"], "ready", "{a}");
    assert_eq!(
        a["started"], false,
        "a live owner that answers late is not replaced: {a}"
    );
    assert_eq!(a["url"].as_str().unwrap(), slow_url, "{a}");
    assert!(
        matches!(LeaseFile::read(&lease), LeaseFile::Document(d) if d.url.as_deref() == Some(slow_url.as_str())),
        "the lease still names the slow owner: nothing took it over"
    );
    // never `serve stop` here: the lease names this test process, and stop signals its pid
    std::fs::remove_file(&lease).unwrap();
}

#[test]
fn a_live_owner_that_never_answers_is_still_taken_over() {
    // The patience a slow owner gets is bounded: a lease naming a live process whose address
    // accepts a connection and never answers is a ghost, and the next ensure must replace it
    // rather than wait on it forever.
    let f = Fixture::new();
    let wedged = TcpListener::bind("127.0.0.1:0").unwrap();
    let wedged_url = format!("http://{}", wedged.local_addr().unwrap());
    let lease = lease_path(&f);
    std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
    std::fs::write(
        &lease,
        format!(
            r#"{{"schema":"majordomus-mcp-lease/v1","pid":{},"token":"wedged","root":"{}","url":"{}","started_at":"2026-09-10T00:00:00Z","version":"{}"}}"#,
            std::process::id(),
            f.root().display(),
            wedged_url,
            majordomus_cli::VERSION
        ),
    )
    .unwrap();
    let t0 = Instant::now();
    let (code, a, err) = ensure(&f.root(), &["--idle", "120"]);
    let took = t0.elapsed();
    assert_eq!(code, 0, "{a}\n{err}");
    assert_eq!(a["standing"], "ready", "{a}");
    assert_eq!(a["started"], true, "the wedged owner was taken over: {a}");
    assert_ne!(a["url"].as_str().unwrap(), wedged_url, "{a}");
    assert_ne!(
        a["pid"].as_u64().unwrap(),
        u64::from(std::process::id()),
        "{a}"
    );
    assert!(
        took < Duration::from_secs(40),
        "taken over within ensure's wait, not after it: {took:?}"
    );
    let (code, out, err) = mj(&f.root(), &["serve", "stop"]);
    assert_eq!(code, 0, "serve stop: {out}{err}");
    drop(wedged);
}

#[test]
fn an_idle_server_ends_by_itself() {
    let f = Fixture::new();
    let (code, a, err) = ensure(&f.root(), &["--idle", "1"]);
    assert_eq!(code, 0, "{a}\n{err}");
    let url = a["url"].as_str().unwrap().to_string();
    wait_until("the idle server ends", Duration::from_secs(15), || {
        LeaseFile::read(&lease_path(&f)) == LeaseFile::Absent
    });
    wait_until("its port closes", Duration::from_secs(5), || {
        get(&url, "/").is_none()
    });
    let (code, out, _) = mj(&f.root(), &["serve", "status"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("standing   absent"), "{out}");
}

#[test]
fn ensure_takes_a_free_port_when_the_one_it_asks_for_is_taken() {
    let f = Fixture::new();
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let (code, a, err) = ensure(&f.root(), &["--idle", "120", "--port", &port.to_string()]);
    assert_eq!(code, 0, "{a}\n{err}");
    assert_eq!(a["standing"], "ready");
    let url = a["url"].as_str().unwrap();
    assert!(
        !url.ends_with(&format!(":{port}")),
        "the taken port was not bound: {url}"
    );
    let (code, _, _) = mj(&f.root(), &["serve", "stop"]);
    assert_eq!(code, 0);
    drop(taken);
}

#[test]
fn stop_leaves_a_server_of_another_checkout_alone() {
    let mine = Fixture::new();
    let other = Fixture::new();
    let (code, a, err) = ensure(&mine.root(), &["--idle", "120"]);
    assert_eq!(code, 0, "{a}\n{err}");
    let url = a["url"].as_str().unwrap().to_string();
    let pid = a["pid"].as_u64().unwrap();
    // the other checkout's lease names my server: it answers, but not for that root
    let lease = lease_path(&other);
    std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
    std::fs::write(
        &lease,
        format!(
            r#"{{"schema":"majordomus-mcp-lease/v1","pid":{pid},"token":"x","root":"{}","url":"{url}","started_at":"2026-09-10T00:00:00Z","version":"{}"}}"#,
            other.root().display(),
            majordomus_cli::VERSION
        ),
    )
    .unwrap();
    let (code, out, _) = mj(&other.root(), &["serve", "stop"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("does not answer for this checkout"), "{out}");
    assert!(
        get(&url, "/").is_some(),
        "my server is still there after the other checkout's stop"
    );
    let (code, out, _) = mj(&mine.root(), &["serve", "stop"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.starts_with("stopped "), "{out}");
}
