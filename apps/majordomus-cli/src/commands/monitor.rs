//! `majordomus monitor`: the terminal rendering of the `monitor` capability (ADR 0133).
//!
//! The stream is a fact of the running server's memory — the mesh journal lives there — so
//! the command finds this checkout's server and asks it over HTTP, like `majordomus mesh`
//! does. One page is printed with what is for the reader first; `--follow` asks again
//! with the page's cursor every `--interval` seconds and prints only what is new, so a
//! reconnect or a restarted server repeats nothing. `--format json` prints the page as the
//! capability answered it, or, while following, one signal per line (NDJSON).
//!
//! Exit codes: 0 answered (or a follow ended by `--count` or `--until`); 10 refused (a bad
//! cursor, an unknown source) or a follow that reached `--timeout` first.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::app::App;
use crate::cli::{MonitorArgs, OutputFormat};
use crate::error::{Error, Result};

/// One bounded request to the running server; the GitHub source alone may take seconds.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// The exit code of a refusal, or of a follow that timed out.
const EXIT_REFUSED: u8 = 10;

/// Run `majordomus monitor`.
pub fn run(args: MonitorArgs) -> Result<u8> {
    run_with(args, server_url)
}

/// [`run`], finding the server with `discover`: this checkout's ready server in earnest, a
/// test's own in a test.
fn run_with(
    args: MonitorArgs,
    discover: impl Fn(&MonitorArgs) -> Result<Option<String>>,
) -> Result<u8> {
    let color = wants_color(
        args.query.format,
        std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_none(),
    );
    let reader = args
        .reader
        .clone()
        .or_else(|| std::env::var("MAJORDOMUS_MONITOR_FOR").ok())
        .filter(|r| !r.trim().is_empty());
    let started = Instant::now();
    let mut after = args.after.clone();
    let mut shown: u64 = 0;
    let mut warned: Option<String> = None;
    let mut first = true;
    loop {
        let url = discover(&args)?;
        let page = match url {
            None => Err(Asked::NoServer),
            Some(url) => ask(&url, &target(&args, after.as_deref(), reader.as_deref())),
        };
        match page {
            Err(Asked::Refused(message)) => {
                eprintln!("monitor: {message}");
                return Ok(EXIT_REFUSED);
            }
            Err(Asked::NoServer) if !args.follow => {
                match args.query.format {
                    OutputFormat::Json => {
                        println!("{}", json!({"active": false, "reason": NO_SERVER}))
                    }
                    OutputFormat::Text => println!("monitor: {NO_SERVER}"),
                }
                return Ok(0);
            }
            Err(Asked::NoServer) => {
                if warned.as_deref() != Some(NO_SERVER) {
                    eprintln!(
                        "monitor: {NO_SERVER}; asking again every {}s",
                        args.interval
                    );
                    warned = Some(NO_SERVER.to_string());
                }
            }
            Err(Asked::Unreachable(reason)) | Err(Asked::Other(reason)) if !args.follow => {
                return Err(Error::Protocol { reason });
            }
            Err(Asked::Unreachable(reason)) | Err(Asked::Other(reason)) => {
                if warned.as_deref() != Some(reason.as_str()) {
                    eprintln!("monitor: {reason}; asking again every {}s", args.interval);
                    warned = Some(reason);
                }
            }
            Ok(page) => {
                warned = None;
                let signals: Vec<Value> = page["signals"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|s| matches(s, args.pattern.as_deref()))
                    .cloned()
                    .collect();
                if !args.follow {
                    match args.query.format {
                        OutputFormat::Json => println!(
                            "{}",
                            serde_json::to_string_pretty(&page).unwrap_or_default()
                        ),
                        OutputFormat::Text => print!("{}", render_page(&page, &signals, color)),
                    }
                    return Ok(0);
                }
                if first && args.query.format == OutputFormat::Text {
                    print!("{}", render_sources(&page, color));
                }
                first = false;
                for signal in &signals {
                    match args.query.format {
                        OutputFormat::Json => println!("{signal}"),
                        OutputFormat::Text => println!("{}", render_line(signal, color)),
                    }
                    shown += 1;
                    let done_count = args.count.is_some_and(|n| shown >= n);
                    let done_kind = args
                        .until
                        .as_deref()
                        .is_some_and(|k| signal["kind"].as_str() == Some(k));
                    if done_count || done_kind {
                        let _ = std::io::stdout().flush();
                        return Ok(0);
                    }
                }
                let _ = std::io::stdout().flush();
                if let Some(cursor) = page["cursor"].as_str() {
                    after = Some(cursor.to_string());
                }
                if page["more"] == json!(true) {
                    continue;
                }
            }
        }
        if args
            .timeout
            .is_some_and(|t| started.elapsed() >= Duration::from_secs(t))
        {
            eprintln!("monitor: --timeout reached before --count or --until was met");
            return Ok(EXIT_REFUSED);
        }
        std::thread::sleep(Duration::from_secs(args.interval.max(1)));
    }
}

/// Whether the text is painted: text, to a terminal, and `NO_COLOR` unset.
fn wants_color(format: OutputFormat, terminal: bool, no_color_unset: bool) -> bool {
    format == OutputFormat::Text && terminal && no_color_unset
}

/// What a reader is told when no server serves this checkout.
const NO_SERVER: &str = "no running server for this checkout: the stream lives inside the shared server (majordomus serve ensure starts one)";

/// What asking the server came to.
enum Asked {
    /// No server serves this checkout.
    NoServer,
    /// The capability refused the input: printed, exit 10.
    Refused(String),
    /// The server could not be reached.
    Unreachable(String),
    /// The server answered something that is not a page.
    Other(String),
}

/// This checkout's ready server, when there is one.
fn server_url(args: &MonitorArgs) -> Result<Option<String>> {
    let app = App::load(&args.query.repo)?;
    app.context
        .execute("server.status", json!({}))
        .map(|status| ready_url(&status))
        .map_err(status_failed)
}

/// A `server.status` that could not be asked, as the command's error.
fn status_failed(e: impl std::fmt::Display) -> Error {
    Error::Protocol {
        reason: format!("server.status: {e}"),
    }
}

/// The address of this checkout's ready server in a `server.status` answer.
fn ready_url(status: &Value) -> Option<String> {
    status["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["this_checkout"] == json!(true) && s["standing"] == json!("ready"))
        .and_then(|s| s["lease"]["url"].as_str())
        .map(str::to_string)
}

/// The request target of one page.
pub(crate) fn target(args: &MonitorArgs, after: Option<&str>, reader: Option<&str>) -> String {
    let mut pairs = Vec::new();
    if let Some(after) = after {
        pairs.push(format!("after={}", encode(after)));
    }
    if let Some(limit) = args.limit {
        pairs.push(format!("limit={limit}"));
    }
    if let Some(sources) = &args.sources {
        pairs.push(format!("sources={}", encode(sources)));
    }
    if let Some(reader) = reader {
        pairs.push(format!("for={}", encode(reader)));
    }
    if let Some(since) = args.since {
        pairs.push(format!("since={since}"));
    }
    if pairs.is_empty() {
        "/api/v1/monitor".into()
    } else {
        format!("/api/v1/monitor?{}", pairs.join("&"))
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn ask(url: &str, target: &str) -> std::result::Result<Value, Asked> {
    let reply = crate::mcp::bridge::request(url, "GET", target, &[], None, REQUEST_TIMEOUT)
        .map_err(|e| Asked::Unreachable(format!("{url}{target}: {e}")))?;
    let value: Value = serde_json::from_str(&reply.body).map_err(|e| {
        Asked::Other(format!(
            "{url}{target}: status {}, not JSON: {e}",
            reply.status
        ))
    })?;
    if reply.status == 200 {
        return Ok(value);
    }
    match value.get("error") {
        Some(error) => Err(Asked::Refused(format!(
            "{}: {}",
            error["code"].as_str().unwrap_or("error"),
            error["message"].as_str().unwrap_or("")
        ))),
        None => Err(Asked::Other(format!(
            "{url}{target}: status {}",
            reply.status
        ))),
    }
}

/// Whether a signal's summary contains `pattern`, ignoring case; no pattern keeps everything.
pub(crate) fn matches(signal: &Value, pattern: Option<&str>) -> bool {
    let Some(pattern) = pattern.filter(|p| !p.is_empty()) else {
        return true;
    };
    signal["summary"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase()
        .contains(&pattern.to_lowercase())
}

fn paint(text: &str, code: &str, color: bool) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// `HH:MM:SS` of a Unix time, in UTC.
pub(crate) fn clock(at: u64) -> String {
    let day = at % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, day % 3600 / 60, day % 60)
}

/// One signal as one line: time, source, machine, a mark for its severity and for you, and
/// the summary.
pub(crate) fn render_line(signal: &Value, color: bool) -> String {
    let at = clock(signal["at"].as_u64().unwrap_or(0));
    let source = signal["source"].as_str().unwrap_or("?");
    let machine = signal["machine"].as_str().unwrap_or("");
    let summary = signal["summary"].as_str().unwrap_or("");
    let (mark, code) = match signal["severity"].as_str() {
        Some("action") => ("!!", "1;31"),
        Some("attention") => ("! ", "33"),
        _ => ("  ", "0"),
    };
    let you = if signal["for_you"] == json!(true) {
        paint("> ", "1;36", color)
    } else {
        "  ".to_string()
    };
    let tag = format!("{source:<8} {machine:<10.10}");
    format!(
        "{} {} {}{} {}",
        paint(&at, "2", color),
        paint(&tag, "2", color),
        you,
        paint(mark, code, color),
        paint(summary, code, color && code != "0")
    )
}

/// The sources line: every source's state, with the reason of any that was not read.
pub(crate) fn render_sources(page: &Value, color: bool) -> String {
    let parts: Vec<String> = page["sources"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            let name = s["source"].as_str().unwrap_or("?");
            match s["state"].as_str() {
                Some("ok") => format!("{name} ok"),
                Some(state) => {
                    let reason = s["reason"].as_str().unwrap_or("");
                    paint(
                        &format!("{name} {state} ({reason})"),
                        "33",
                        color && state != "skipped",
                    )
                }
                None => name.to_string(),
            }
        })
        .collect();
    format!("sources  {}\n", parts.join(" · "))
}

/// A page for a person: what is for the reader first, then everything, then how much was
/// folded away, the sources, and how to go on.
pub(crate) fn render_page(page: &Value, signals: &[Value], color: bool) -> String {
    let mut out = String::new();
    let mine: Vec<&Value> = signals
        .iter()
        .filter(|s| s["for_you"] == json!(true))
        .collect();
    if !mine.is_empty() {
        out.push_str(&paint(&format!("For you ({})", mine.len()), "1;36", color));
        out.push('\n');
        for s in &mine {
            out.push_str(&render_line(s, color));
            out.push('\n');
        }
        out.push('\n');
    }
    if signals.is_empty() {
        out.push_str("nothing happened in this window\n");
    }
    for s in signals {
        out.push_str(&render_line(s, color));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&format!(
        "{} signal(s), {} folded away as noise\n",
        signals.len(),
        page["suppressed"].as_u64().unwrap_or(0)
    ));
    out.push_str(&render_sources(page, color));
    if let Some(cursor) = page["cursor"].as_str().filter(|c| !c.is_empty()) {
        out.push_str(&format!(
            "next     majordomus monitor --after '{cursor}'   (or --follow)\n"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn args(argv: &[&str]) -> MonitorArgs {
        let mut full = vec!["majordomus", "monitor"];
        full.extend_from_slice(argv);
        match crate::cli::Cli::try_parse_from(full).unwrap().command {
            crate::cli::Command::Monitor(m) => m,
            _ => panic!("expected monitor"),
        }
    }

    fn page() -> Value {
        json!({
            "signals": [
                {"at": 3_723, "source": "mesh", "kind": "review_requested", "severity": "action",
                 "machine": "lundra", "summary": "3d asks 1b to review (r-1): PR #885", "for_you": true},
                {"at": 3_724, "source": "executor", "kind": "merged", "severity": "info",
                 "summary": "executor merged #868", "for_you": false},
                {"at": 3_725, "source": "github", "kind": "run_failed", "severity": "attention",
                 "summary": "ci failure on fix/x", "for_you": false}
            ],
            "for_you": 1,
            "suppressed": 954,
            "sources": [
                {"source": "mesh", "state": "ok", "signals": 1, "suppressed": 954},
                {"source": "executor", "state": "ok", "signals": 1, "suppressed": 0},
                {"source": "github", "state": "absent", "reason": "gh is not installed",
                 "signals": 0, "suppressed": 0}
            ],
            "cursor": "m9.1.s~x4",
            "more": false
        })
    }

    /// A server that answers the requests it gets with `replies`, in order, the last one
    /// for ever after; it lives as long as the test process.
    fn scripted(replies: Vec<(u16, String)>) -> String {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        std::thread::spawn(move || {
            let mut replies = replies.into_iter().peekable();
            for request in server.incoming_requests() {
                let (status, body) = match replies.next() {
                    Some(r) if replies.peek().is_some() => r,
                    Some(last) => {
                        replies = vec![last.clone()].into_iter().peekable();
                        last
                    }
                    None => (500, String::new()),
                };
                let _ = request
                    .respond(tiny_http::Response::from_string(body).with_status_code(status));
            }
        });
        url
    }

    fn signal(at: u64, kind: &str, summary: &str) -> Value {
        json!({"at": at, "source": "mesh", "kind": kind, "severity": "info",
               "summary": summary, "for_you": false})
    }

    fn a_page(signals: Vec<Value>, cursor: &str, more: bool) -> String {
        json!({"signals": signals, "for_you": 0, "suppressed": 0,
               "sources": [{"source": "mesh", "state": "ok", "signals": 1, "suppressed": 0}],
               "cursor": cursor, "more": more})
        .to_string()
    }

    fn at(url: String) -> impl Fn(&MonitorArgs) -> Result<Option<String>> {
        move |_| Ok(Some(url.clone()))
    }

    #[test]
    fn one_page_is_printed_and_the_command_ends() {
        let body = a_page(vec![signal(1, "session_joined", "3d joined")], "c1", false);
        for format in ["text", "json"] {
            let url = scripted(vec![(200, body.clone())]);
            let code = run_with(args(&["--format", format, "--match", "3d"]), at(url)).unwrap();
            assert_eq!(code, 0, "{format}");
        }
    }

    #[test]
    fn a_follow_goes_on_past_a_full_page_and_ends_at_its_count_or_kind() {
        let first = a_page(vec![signal(1, "session_joined", "3d joined")], "c1", true);
        let second = a_page(
            vec![
                signal(2, "claimed", "3d claims"),
                signal(3, "merged", "#885 merged"),
            ],
            "c2",
            false,
        );
        let url = scripted(vec![(200, first.clone()), (200, second.clone())]);
        assert_eq!(
            run_with(args(&["--follow", "--count", "3"]), at(url)).unwrap(),
            0
        );
        let url = scripted(vec![(200, first), (200, second)]);
        let code = run_with(
            args(&["--follow", "--until", "merged", "--format", "json"]),
            at(url),
        )
        .unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn a_follow_says_once_what_went_wrong_and_goes_on_when_it_answers_again() {
        let good = a_page(vec![signal(4, "merged", "#885 merged")], "c4", false);
        let url = scripted(vec![
            (502, "not json".into()),
            (502, "not json".into()),
            (503, json!({"busy": true}).to_string()),
            (200, good),
        ]);
        let code = run_with(
            args(&[
                "--follow",
                "--count",
                "1",
                "--interval",
                "1",
                "--timeout",
                "60",
            ]),
            at(url),
        )
        .unwrap();
        assert_eq!(code, 0);
    }

    #[test]
    fn without_follow_a_bad_answer_is_an_error_and_a_refusal_exits_ten() {
        let url = scripted(vec![(502, "not json".into())]);
        let e = run_with(args(&[]), at(url)).unwrap_err();
        assert!(e.to_string().contains("status 502, not JSON"), "{e}");
        let url = scripted(vec![(503, json!({"busy": true}).to_string())]);
        let e = run_with(args(&[]), at(url)).unwrap_err();
        assert!(e.to_string().ends_with("status 503"), "{e}");
        let refusal = json!({"error": {"code": "invalid_input", "message": "bad cursor"}});
        let url = scripted(vec![(400, refusal.to_string())]);
        assert_eq!(
            run_with(args(&["--after", "x"]), at(url)).unwrap(),
            EXIT_REFUSED
        );
    }

    #[test]
    fn no_server_is_an_answer_and_a_follow_waits_for_one_until_its_timeout() {
        let none = |_: &MonitorArgs| Ok(None);
        assert_eq!(run_with(args(&[]), none).unwrap(), 0);
        assert_eq!(run_with(args(&["--format", "json"]), none).unwrap(), 0);
        let code = run_with(
            args(&["--follow", "--interval", "1", "--timeout", "2"]),
            none,
        )
        .unwrap();
        assert_eq!(code, EXIT_REFUSED, "a follow that reaches its timeout");
        let refused = run_with(args(&[]), |_: &MonitorArgs| {
            Err(Error::Protocol {
                reason: "server.status: down".into(),
            })
        });
        assert!(
            refused.is_err(),
            "a discovery that fails is the command's failure"
        );
    }

    #[test]
    fn a_server_that_cannot_be_reached_is_unreachable() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        match ask(&url, "/api/v1/monitor") {
            Err(Asked::Unreachable(reason)) => assert!(reason.starts_with(&url), "{reason}"),
            _ => panic!("a closed port is unreachable"),
        }
    }

    #[test]
    fn the_server_of_a_repository_without_one_is_none() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.root())
            .status()
            .expect("git");
        let root = repo.root().to_str().unwrap().to_string();
        let share = crate::synthetic::crate_share();
        let share = share.to_str().unwrap();
        assert_eq!(
            server_url(&args(&["--repo", &root, "--share", share])).unwrap(),
            None
        );
    }

    #[test]
    fn a_page_shows_what_is_for_you_first_and_how_to_go_on() {
        let p = page();
        let signals: Vec<Value> = p["signals"].as_array().unwrap().clone();
        let text = render_page(&p, &signals, false);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "For you (1)");
        assert!(
            lines[1].contains("> !! 3d asks 1b to review"),
            "{}",
            lines[1]
        );
        assert!(
            lines[1].starts_with("01:02:03 mesh     lundra"),
            "{}",
            lines[1]
        );
        assert!(text.contains("3 signal(s), 954 folded away as noise"));
        assert!(
            text.contains("sources  mesh ok · executor ok · github absent (gh is not installed)")
        );
        assert!(text.contains("next     majordomus monitor --after 'm9.1.s~x4'"));
        assert!(!text.contains('\x1b'), "no colour unless asked");
        assert!(render_page(&p, &[], true).contains("nothing happened in this window"));
        assert!(render_line(&signals[0], true).contains("\x1b[1;31m!!"));
    }

    #[test]
    fn a_pattern_keeps_only_what_it_names() {
        let p = page();
        let signals = p["signals"].as_array().unwrap();
        let kept: Vec<&Value> = signals
            .iter()
            .filter(|s| matches(s, Some("#885")))
            .collect();
        assert_eq!(kept.len(), 1);
        assert!(matches(&signals[2], Some("FIX/X")));
        assert!(matches(&signals[2], None));
        assert!(matches(&signals[2], Some("")));
    }

    #[test]
    fn the_target_carries_every_option_encoded() {
        let a = args(&["--sources", "mesh,github", "--limit", "5", "--since", "2h"]);
        assert_eq!(
            target(&a, Some("m1.2.s~x3"), Some("1b, cd")),
            "/api/v1/monitor?after=m1.2.s~x3&limit=5&sources=mesh%2Cgithub&for=1b%2C%20cd&since=7200"
        );
        assert_eq!(target(&args(&[]), None, None), "/api/v1/monitor");
    }

    /// A closed port, as a server address nobody answers at.
    fn closed() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        url
    }

    #[test]
    fn a_server_nobody_answers_at_is_an_error_once_and_a_follow_waits_out_its_timeout() {
        let e = run_with(args(&[]), at(closed())).unwrap_err();
        assert!(e.to_string().contains("/api/v1/monitor"), "{e}");
        let code = run_with(
            args(&["--follow", "--interval", "1", "--timeout", "1"]),
            at(closed()),
        )
        .unwrap();
        assert_eq!(code, EXIT_REFUSED, "a follow that reaches its timeout");
    }

    #[test]
    fn a_follow_over_a_page_without_a_cursor_keeps_its_place_until_its_timeout() {
        let page = json!({"signals": [signal(1, "merged", "#1 merged")], "for_you": 0,
                          "suppressed": 0, "sources": [], "more": false})
        .to_string();
        let url = scripted(vec![(200, page)]);
        let code = run_with(
            args(&[
                "--follow",
                "--count",
                "5",
                "--interval",
                "1",
                "--timeout",
                "1",
            ]),
            at(url),
        )
        .unwrap();
        assert_eq!(code, EXIT_REFUSED, "the count was never met");
    }

    #[test]
    fn colour_is_for_text_on_a_terminal_without_no_color() {
        assert!(wants_color(OutputFormat::Text, true, true));
        assert!(!wants_color(OutputFormat::Text, false, true));
        assert!(!wants_color(OutputFormat::Text, true, false));
        assert!(!wants_color(OutputFormat::Json, true, true));
    }

    #[test]
    fn the_ready_server_of_this_checkout_is_the_one_asked() {
        let status = json!({"servers": [
            {"this_checkout": false, "standing": "ready", "lease": {"url": "http://other"}},
            {"this_checkout": true, "standing": "starting", "lease": {"url": "http://young"}},
            {"this_checkout": true, "standing": "ready", "lease": {"url": "http://here"}}
        ]});
        assert_eq!(ready_url(&status).as_deref(), Some("http://here"));
        let no_url = json!({"servers": [{"this_checkout": true, "standing": "ready"}]});
        assert_eq!(ready_url(&no_url), None);
        assert_eq!(ready_url(&json!({})), None);
        let e = status_failed("down");
        assert!(e.to_string().contains("server.status: down"), "{e}");
    }

    #[test]
    fn a_path_that_is_no_repository_has_no_server_to_ask() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        let missing = missing.to_str().unwrap();
        assert!(server_url(&args(&["--repo", missing])).is_err());
    }

    #[test]
    fn a_source_without_a_state_is_its_name_and_an_empty_cursor_is_no_way_on() {
        let page = json!({"sources": [{"source": "mesh"},
                                      {"source": "github", "state": "skipped"}],
                          "cursor": ""});
        assert_eq!(
            render_sources(&page, true),
            "sources  mesh · github skipped ()\n"
        );
        let text = render_page(&page, &[], false);
        assert!(!text.contains("next "), "{text}");
        let text = render_page(&json!({}), &[], false);
        assert!(
            text.contains("0 signal(s), 0 folded away as noise"),
            "{text}"
        );
    }

    #[test]
    fn a_clock_is_utc_hours_minutes_seconds() {
        assert_eq!(clock(0), "00:00:00");
        assert_eq!(clock(86_399), "23:59:59");
        assert_eq!(clock(1_791_661_234), "19:40:34");
    }
}
