//! Level B of the command line's documentation contract: every example the reference and
//! the website print is executed against the built executable, in a disposable repository.
//!
//! The argument vectors here are not written here. They are read from
//! [`majordomus_cli::cli::tree`], which is the same tree `docs/generated/cli.md`,
//! `docs/generated/cli.json` and every page under `/docs/cli/` render. An example that
//! stops working, an option that is renamed, a command that is removed: the run fails, and
//! the documentation cannot claim what the executable does not do.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};

use common::{dist_share, run_in, Fixture, BIN};
use majordomus_cli::cli::{CommandDoc, ExampleView, Expect};
use serde_json::{json, Value};

/// Every example of the command line, with the command it belongs to.
fn examples() -> Vec<(String, ExampleView)> {
    fn walk(c: &CommandDoc, out: &mut Vec<(String, ExampleView)>) {
        for e in &c.examples {
            out.push((c.command(), e.clone()));
        }
        for s in &c.subcommands {
            walk(s, out);
        }
    }
    let mut out = Vec::new();
    walk(&majordomus_cli::cli::tree(), &mut out);
    out
}

/// The expectation of one example, by its id, from the canonical declaration. The view the
/// projections carry holds the expectation as a phrase for a reader; the test needs the
/// typed value, which is the same declaration read directly.
fn expectation(id: &str) -> Expect {
    majordomus_cli::cli::EXAMPLES
        .iter()
        .flat_map(|s| s.examples.iter())
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("example {id} is in the tree and not in the declaration"))
        .expect
}

fn argv(e: &ExampleView) -> Vec<&str> {
    e.argv.iter().map(String::as_str).collect()
}

/// Run one example in its own repository: the setup commands first, then the example, each
/// asserted where it is documented to be asserted.
fn run_example(command: &str, e: &ExampleView) {
    let f = Fixture::new();
    for step in &e.setup {
        let args: Vec<&str> = step.argv.iter().map(String::as_str).collect();
        let (code, _, err) = run_in(&f.root(), &args, "");
        assert_eq!(
            code, 0,
            "example {} of `{command}`: the setup step `{}` failed:\n{err}",
            e.id, step.command
        );
    }
    match expectation(&e.id) {
        Expect::Success => {
            let (code, _, err) = run_in(&f.root(), &argv(e), "");
            assert_eq!(code, 0, "example {} (`{}`):\n{err}", e.id, e.command);
        }
        Expect::ExitCode(expected) => {
            let (code, _, err) = run_in(&f.root(), &argv(e), "");
            assert_eq!(
                code,
                i32::from(expected),
                "example {} (`{}`):\n{err}",
                e.id,
                e.command
            );
        }
        Expect::StdoutContains(fragments) => {
            let (code, out, err) = run_in(&f.root(), &argv(e), "");
            assert_eq!(code, 0, "example {} (`{}`):\n{err}", e.id, e.command);
            for fragment in fragments {
                assert!(
                    out.contains(fragment),
                    "example {} (`{}`) does not print {fragment:?}:\n{out}",
                    e.id,
                    e.command
                );
            }
        }
        Expect::Json(pointers) => {
            let (code, out, err) = run_in(&f.root(), &argv(e), "");
            assert_eq!(code, 0, "example {} (`{}`):\n{err}", e.id, e.command);
            let v: Value = serde_json::from_str(&out).unwrap_or_else(|why| {
                panic!(
                    "example {} (`{}`) is documented as JSON and printed {why}:\n{out}",
                    e.id, e.command
                )
            });
            for p in pointers {
                assert!(
                    v.pointer(p).is_some(),
                    "example {} (`{}`): the document has nothing at {p}",
                    e.id,
                    e.command
                );
            }
        }
        Expect::HttpReady(path) => http_example(&f, e, path),
        Expect::McpReady => mcp_example(&f, e),
    }
}

/// A server example: spawn it, wait for the address it logs with a deadline, make one real
/// request, then stop it the documented way and prove it ended cleanly. Nothing is left
/// running: the guard waits for the child whatever the assertions did.
fn http_example(f: &Fixture, e: &ExampleView, path: &str) {
    let mut child = Command::new(BIN)
        .args(argv(e))
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "info")
        .env("MAJORDOMUS_SHARE", dist_share())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the server");
    let stderr = child.stderr.take().expect("stderr");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut lines = BufReader::new(stderr).lines();
        for line in lines.by_ref() {
            let Ok(line) = line else { break };
            if let Some(rest) = line.split("listening on http://").nth(1) {
                let _ = tx.send(rest.split_whitespace().next().unwrap_or("").to_string());
                break;
            }
        }
        for _ in lines {}
    });
    let address = rx
        .recv_timeout(std::time::Duration::from_secs(30))
        .unwrap_or_else(|_| {
            let _ = child.kill();
            panic!(
                "example {} (`{}`) did not log an address within 30s",
                e.id, e.command
            )
        });
    let mut stream = TcpStream::connect(&address).expect("connect to the server");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .expect("write the request");
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read the response");
    let text = String::from_utf8_lossy(&raw);
    let status: u16 = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    // the documented way to stop it: close stdin
    drop(child.stdin.take());
    let ended = child.wait().expect("wait for the server");
    assert!(
        (200..300).contains(&status),
        "example {} (`{}`): GET {path} answered {status}",
        e.id,
        e.command
    );
    assert_eq!(
        ended.code(),
        Some(0),
        "example {} (`{}`) did not exit 0 when stopped",
        e.id,
        e.command
    );
}

/// An MCP example: spawn it, speak the real protocol on stdin, and prove that stdout
/// carried protocol frames and nothing else. End of input ends the session.
fn mcp_example(f: &Fixture, e: &ExampleView) {
    let mut child = Command::new(BIN)
        .args(argv(e))
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "info")
        .env("MAJORDOMUS_SHARE", dist_share())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the MCP server");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for frame in [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"cli-examples","version":"0"}}}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        ] {
            writeln!(stdin, "{frame}").expect("write a frame");
        }
    }
    let out = child.wait_with_output().expect("wait for the MCP server");
    let stdout = String::from_utf8(out.stdout).expect("stdout is UTF-8");
    let frames: Vec<Value> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|why| {
                panic!(
                    "example {} (`{}`) wrote a non-protocol line on stdout ({why}): {l}",
                    e.id, e.command
                )
            })
        })
        .collect();
    assert_eq!(
        out.status.code(),
        Some(0),
        "example {} (`{}`) did not exit 0 at end of input:\n{}",
        e.id,
        e.command,
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        frames.len(),
        2,
        "example {} (`{}`) answered {} frames, not one per request:\n{stdout}",
        e.id,
        e.command,
        frames.len()
    );
    assert!(
        frames[0]["result"]["serverInfo"].is_object(),
        "example {} (`{}`): initialize did not answer with a serverInfo:\n{stdout}",
        e.id,
        e.command
    );
    assert!(
        frames[1]["result"]["tools"]
            .as_array()
            .is_some_and(|t| !t.is_empty()),
        "example {} (`{}`): tools/list answered no tool:\n{stdout}",
        e.id,
        e.command
    );
}

#[test]
fn every_documented_example_runs() {
    let all = examples();
    assert!(
        !all.is_empty(),
        "the command line documents no example at all"
    );
    for (command, e) in &all {
        run_example(command, e);
    }
}

#[test]
fn every_runnable_command_is_covered_by_the_run() {
    // The coverage of this suite is the tree's own: the set of commands the run touched is
    // the set of runnable commands. No list of commands is kept here.
    let tree = majordomus_cli::cli::tree();
    let runnable: Vec<String> = tree
        .flatten()
        .into_iter()
        .filter(|c| c.executable)
        .map(|c| c.command())
        .collect();
    let exercised: std::collections::BTreeSet<String> =
        examples().into_iter().map(|(c, _)| c).collect();
    let missing: Vec<&String> = runnable
        .iter()
        .filter(|c| !exercised.contains(*c))
        .collect();
    assert!(
        missing.is_empty(),
        "commands a person can run that no example exercises: {missing:?}"
    );
}
