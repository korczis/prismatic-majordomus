//! `majordomus peers`: who else is working in this repository, on the command line.
//!
//! The board was readable over MCP and HTTP only, so a worker whose client had dropped the
//! `majordomus_*` tools had nothing to ask with, and for a month eight sessions coordinated by
//! direct messages instead. This is the third projection of the same capability: the gather,
//! the stamping of each peer with its checkout and the judgement of which boards could be
//! read are `peers.list`'s, and nothing here repeats them. This module asks and renders.
//!
//! The answer is taken from the server holding this checkout's lease when there is one,
//! because a board is that process's memory; with no server here the capability still runs
//! in this process and still asks every sibling checkout's server, so the board of the rest
//! of the repository is not lost because this checkout serves nothing.
//!
//! When no board could be asked at all — no server answers for any checkout — the command
//! says so and exits [`EXIT_MISSING`] rather than 0: an empty board and no board are
//! different answers, and "nobody else is here" must never be what "I could not ask" prints.

use std::io::Write;

use serde_json::{json, Value};

use super::executions::{ask_server, cli_capability, map};
use crate::app::App;
use crate::cli::{OutputFormat, PeersArgs};
use crate::error::{Error, Result};
use crate::lease;

/// The exit code when no board could be asked: the missing-artifact code.
pub const EXIT_MISSING: u8 = 12;

/// Run `majordomus peers`.
pub fn run(args: PeersArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = cli_capability(ctx, &["peers"])?;
    let input = json!({ "checkouts": args.checkouts });
    let served = lease::serving(&app.repository);
    let answer = match &served {
        Some(url) => ask_server(ctx, url, id, &input)?,
        None => ctx.execute(id, input).map_err(map)?,
    };
    let asked = boards_asked(&answer, served.is_some());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut out, &answer)
                .map_err(|e| Error::Transport(e.into()))?;
            writeln!(out).map_err(Error::Transport)?;
        }
        OutputFormat::Text => render(&mut out, &answer, asked).map_err(Error::Transport)?,
    }
    if asked == 0 {
        // on stderr as well, so that a caller reading JSON on stdout is still told why
        eprintln!(
            "majordomus: no board could be asked: no server answers for any checkout this answer \
             covers (start one: majordomus serve ensure)"
        );
        return Ok(EXIT_MISSING);
    }
    Ok(0)
}

/// How many boards of the answer were read from a server that answers.
///
/// A board is a server's memory, so a checkout nothing serves has none: the capability
/// reports it with no peers and no reason, which is complete and is not a board that was
/// asked. This checkout's own board counts when the answer came from its server.
fn boards_asked(answer: &Value, served_here: bool) -> usize {
    let empty = Vec::new();
    answer["boards"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter(|b| {
            let answered = matches!(b["standing"].as_str(), Some("ready" | "outdated"));
            let here = b["this_checkout"].as_bool().unwrap_or(false);
            b.get("reason").is_none_or(Value::is_null) && answered && (!here || served_here)
        })
        .count()
}

/// The board for a person: one block per checkout, its peers and what each claimed.
fn render(out: &mut impl Write, v: &Value, asked: usize) -> std::io::Result<()> {
    let empty = Vec::new();
    let boards = v["boards"].as_array().unwrap_or(&empty);
    let peers = v["peers"].as_array().unwrap_or(&empty);
    writeln!(
        out,
        "peers      {} attached, {} of {} board(s) asked{}",
        v["count"].as_u64().unwrap_or(0),
        asked,
        boards.len(),
        if v["complete"].as_bool().unwrap_or(false) {
            ""
        } else {
            " — incomplete: a board could not be read"
        }
    )?;
    for b in boards {
        let worktree = b["worktree"].as_str().unwrap_or("?");
        writeln!(
            out,
            "\n{}{}  [{}]  server {}{}",
            worktree,
            if b["this_checkout"].as_bool().unwrap_or(false) {
                "  (this checkout)"
            } else {
                ""
            },
            b["branch"].as_str().unwrap_or("-"),
            b["standing"].as_str().unwrap_or("?"),
            b["url"]
                .as_str()
                .map(|u| format!("  {u}"))
                .unwrap_or_default(),
        )?;
        if let Some(why) = b["reason"].as_str() {
            writeln!(out, "  not asked  {why}")?;
            continue;
        }
        let here: Vec<&Value> = peers
            .iter()
            .filter(|p| p["checkout"]["worktree"].as_str() == Some(worktree))
            .collect();
        if here.is_empty() {
            writeln!(out, "  nobody")?;
        }
        for p in here {
            writeln!(
                out,
                "  {:<4} {} {}  {}  {}, seen {}s ago",
                p["id"].as_str().unwrap_or("?"),
                p["client"]["name"].as_str().unwrap_or("?"),
                p["client"]["version"].as_str().unwrap_or(""),
                p["transport"].as_str().unwrap_or("?"),
                if p["attached"].as_bool().unwrap_or(false) {
                    "attached"
                } else {
                    "gone"
                },
                p["last_seen_seconds_ago"].as_u64().unwrap_or(0),
            )?;
            for c in p["claims"].as_array().unwrap_or(&empty) {
                let scope: Vec<&str> = c["scope"]
                    .as_array()
                    .unwrap_or(&empty)
                    .iter()
                    .filter_map(Value::as_str)
                    .collect();
                writeln!(
                    out,
                    "       {}{}",
                    c["name"]
                        .as_str()
                        .map(|n| format!("{n}: "))
                        .unwrap_or_default(),
                    c["intent"].as_str().unwrap_or(""),
                )?;
                if !scope.is_empty() {
                    writeln!(out, "       paths  {}", scope.join(" "))?;
                }
            }
        }
    }
    let overlaps = v["overlaps"].as_array().unwrap_or(&empty);
    if !overlaps.is_empty() {
        writeln!(
            out,
            "\noverlaps   {} pair(s) of claims meet (majordomus peers --format json names them)",
            overlaps.len()
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(standing: &str, here: bool, reason: Option<&str>) -> Value {
        let mut b =
            json!({ "worktree": "/r", "standing": standing, "this_checkout": here, "attached": 0 });
        if let Some(r) = reason {
            b["reason"] = json!(r);
        }
        b
    }

    /// A checkout nothing serves has no board, and this process's empty one is not a
    /// server's: neither is a board that was asked.
    #[test]
    fn a_board_is_asked_only_of_a_server_that_answers() {
        let none = json!({ "boards": [board("absent", true, None), board("stale", false, None)] });
        assert_eq!(boards_asked(&none, false), 0);
        let sibling =
            json!({ "boards": [board("absent", true, None), board("ready", false, None)] });
        assert_eq!(boards_asked(&sibling, false), 1);
        let here =
            json!({ "boards": [board("ready", true, None), board("outdated", false, None)] });
        assert_eq!(boards_asked(&here, true), 2);
        // this checkout's lease names a server, and the answer did not come from it
        assert_eq!(boards_asked(&here, false), 1);
        let unread = json!({ "boards": [board("ready", false, Some("it answered 500"))] });
        assert_eq!(boards_asked(&unread, false), 0);
    }

    /// Every peer is printed under the checkout it is attached to, with what it claimed.
    #[test]
    fn the_text_names_each_peer_under_its_checkout() {
        let v = json!({
            "count": 1, "complete": false,
            "boards": [
                { "worktree": "/r", "branch": "master", "standing": "ready", "url": "http://127.0.0.1:1", "this_checkout": true, "attached": 1 },
                { "worktree": "/r-wt/x", "standing": "starting", "this_checkout": false, "attached": 0, "reason": "its server is starting" },
            ],
            "peers": [{
                "id": "p1", "client": { "name": "worker-alpha", "version": "0" }, "transport": "http",
                "attached": true, "last_seen_seconds_ago": 3,
                "claims": [{ "intent": "the ordering rule", "scope": ["apps/x", "docs"], "at": "t" }],
                "checkout": { "worktree": "/r" },
            }],
            "overlaps": [{}],
        });
        let mut out = Vec::new();
        render(&mut out, &v, 1).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(
            s.starts_with("peers      1 attached, 1 of 2 board(s) asked — incomplete"),
            "{s}"
        );
        assert!(
            s.contains("/r  (this checkout)  [master]  server ready  http://127.0.0.1:1"),
            "{s}"
        );
        assert!(
            s.contains("  p1   worker-alpha 0  http  attached, seen 3s ago"),
            "{s}"
        );
        assert!(
            s.contains("       the ordering rule\n       paths  apps/x docs"),
            "{s}"
        );
        assert!(
            s.contains("/r-wt/x  [-]  server starting\n  not asked  its server is starting"),
            "{s}"
        );
        assert!(s.contains("overlaps   1 pair(s)"), "{s}");
    }
}
