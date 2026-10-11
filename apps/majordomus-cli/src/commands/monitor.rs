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
    let color = args.query.format == OutputFormat::Text
        && std::io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none();
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
        let url = server_url(&args)?;
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
    let status = app
        .context
        .execute("server.status", json!({}))
        .map_err(|e| Error::Protocol {
            reason: format!("server.status: {e}"),
        })?;
    Ok(status["servers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|s| s["this_checkout"] == json!(true) && s["standing"] == json!("ready"))
        .and_then(|s| s["lease"]["url"].as_str())
        .map(str::to_string))
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

    #[test]
    fn a_clock_is_utc_hours_minutes_seconds() {
        assert_eq!(clock(0), "00:00:00");
        assert_eq!(clock(86_399), "23:59:59");
        assert_eq!(clock(1_791_661_234), "19:40:34");
    }
}
