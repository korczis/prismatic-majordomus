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
use crate::capability::Context;
use crate::cli::{OutputFormat, PeersArgs};
use crate::error::{Error, Result};
use crate::lease;

/// The exit code when no board could be asked: the missing-artifact code.
pub const EXIT_MISSING: u8 = 12;

/// Run `majordomus peers`.
pub fn run(args: PeersArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let served = lease::serving(&app.repository);
    let stdout = std::io::stdout();
    answer(&app.context, served.as_deref(), &args, &mut stdout.lock())
}

/// Ask the capability exposed as `majordomus peers` — of the server at `served` when this
/// checkout has one, of this process otherwise — and write its answer to `out`.
fn answer(
    ctx: &Context,
    served: Option<&str>,
    args: &PeersArgs,
    out: &mut impl Write,
) -> Result<u8> {
    let id = cli_capability(ctx, &["peers"])?;
    let input = json!({ "checkouts": args.checkouts });
    let answer = match served {
        Some(url) => ask_server(ctx, url, id, &input)?,
        None => ctx.execute(id, input).map_err(map)?,
    };
    let asked = boards_asked(&answer, served.is_some());
    match args.format {
        // a JSON value always serialises: the only failure left is the writer's
        OutputFormat::Json => writeln!(out, "{answer:#}"),
        OutputFormat::Text => render(out, &answer, asked),
    }
    .map_err(Error::Transport)?;
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
    // the capability's default summary counts the checkouts that hold no board instead of
    // listing them: say how many, so a short listing is not read as the whole repository
    if let Some(omitted) = v["omitted"].as_object().filter(|o| !o.is_empty()) {
        let by_standing: Vec<String> = omitted
            .iter()
            .map(|(standing, n)| format!("{standing} {}", n.as_u64().unwrap_or(0)))
            .collect();
        writeln!(
            out,
            "omitted    {} checkout(s) with no server and nobody attached ({})",
            omitted.values().filter_map(Value::as_u64).sum::<u64>(),
            by_standing.join(", "),
        )?;
    }
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
    use crate::capability::CapabilityError;

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

    /// Every peer state the text knows: a complete answer, a checkout nobody is attached
    /// to, a peer that has gone, and a claim with a name and no paths.
    #[test]
    fn the_text_says_who_has_gone_and_where_nobody_is() {
        let v = json!({
            "count": 0, "complete": true,
            "boards": [
                { "worktree": "/r", "standing": "ready", "this_checkout": false, "attached": 0 },
                { "worktree": "/r-wt/y", "standing": "ready", "this_checkout": false, "attached": 0 },
            ],
            "peers": [{
                "id": "p2", "client": { "name": "worker-beta" }, "transport": "stdio",
                "attached": false, "last_seen_seconds_ago": 40,
                "claims": [{ "name": "ordering", "intent": "the ordering rule" }],
                "checkout": { "worktree": "/r-wt/y" },
            }],
        });
        let mut out = Vec::new();
        render(&mut out, &v, 2).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(
            s.starts_with("peers      0 attached, 2 of 2 board(s) asked\n"),
            "{s}"
        );
        assert!(s.contains("/r  [-]  server ready\n  nobody\n"), "{s}");
        assert!(
            s.contains("  p2   worker-beta   stdio  gone, seen 40s ago"),
            "{s}"
        );
        assert!(s.ends_with("       ordering: the ordering rule\n"), "{s}");
        assert!(
            !s.contains("paths"),
            "a claim without paths prints none: {s}"
        );
        assert!(!s.contains("overlaps"), "no overlap, no line: {s}");
        assert!(!s.contains("omitted"), "nothing omitted, no line: {s}");
    }

    /// The checkouts the summary counts rather than lists are said, by standing, under the
    /// header: a short listing is never the whole repository.
    #[test]
    fn the_text_counts_the_checkouts_the_summary_omits() {
        let v = json!({
            "count": 0, "complete": true, "covered": 4, "peers": [],
            "boards": [{ "worktree": "/r", "standing": "absent", "this_checkout": true, "attached": 0 }],
            "omitted": { "absent": 2, "stale": 1 },
        });
        let mut out = Vec::new();
        render(&mut out, &v, 0).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(
            s.contains(
                "board(s) asked\nomitted    3 checkout(s) with no server and nobody attached \
                 (absent 2, stale 1)\n"
            ),
            "{s}"
        );
        // an omitted checkout holds no board, so it never makes a board asked
        assert_eq!(boards_asked(&v, false), 0);
    }

    /// A writer that keeps its first `n` writes and refuses the rest: a reader that went
    /// away part of the way through.
    struct ClosesAfter(usize, Vec<u8>);

    impl Write for ClosesAfter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "the reader went away",
                ));
            }
            self.0 -= 1;
            self.1.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Wherever the reader goes away, the board says so rather than ending as if it were
    /// whole, and what was written is the start of the whole board.
    #[test]
    fn a_board_cut_at_any_line_is_an_error() {
        let v = json!({
            "count": 2, "complete": true,
            "boards": [
                { "worktree": "/r", "branch": "master", "standing": "ready", "url": "http://127.0.0.1:1", "this_checkout": true },
                { "worktree": "/r-wt/x", "standing": "starting", "reason": "its server is starting" },
                { "worktree": "/r-wt/y", "standing": "ready" },
            ],
            "peers": [
                {
                    "id": "p1", "client": { "name": "worker-alpha", "version": "0" },
                    "transport": "http", "attached": true, "last_seen_seconds_ago": 3,
                    "claims": [{ "name": "order", "intent": "the ordering rule", "scope": ["docs"] }],
                    "checkout": { "worktree": "/r" },
                },
                {
                    "id": "p2", "client": { "name": "worker-beta" }, "transport": "stdio",
                    "attached": false, "last_seen_seconds_ago": 40, "claims": [],
                    "checkout": { "worktree": "/r" },
                },
            ],
            "overlaps": [{}],
        });
        let mut whole = Vec::new();
        render(&mut whole, &v, 2).unwrap();
        let mut cuts = 0;
        loop {
            let mut out = ClosesAfter(cuts, Vec::new());
            match render(&mut out, &v, 2) {
                Ok(()) => {
                    assert_eq!(out.1, whole, "an uncut board is the whole board");
                    break;
                }
                Err(e) => {
                    assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "cut at {cuts}");
                    assert!(
                        whole.starts_with(&out.1),
                        "cut at {cuts} wrote out of order"
                    );
                }
            }
            cuts += 1;
        }
        assert!(cuts > 10, "every line of the board was cut: {cuts}");
    }

    fn args(argv: &[&str]) -> PeersArgs {
        use clap::Parser;
        let mut full = vec!["majordomus", "peers"];
        full.extend_from_slice(argv);
        match crate::cli::Cli::try_parse_from(full)
            .expect("parses")
            .command
        {
            crate::cli::Command::Peers(a) => a,
            _ => panic!("not the peers command"),
        }
    }

    #[test]
    fn a_directory_that_is_no_repository_is_refused_before_anything_is_asked() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let path = dir.path().to_string_lossy().to_string();
        assert!(run(args(&["--repo", &path])).is_err());
    }

    /// `peers`, run whole, where nothing serves: no board could be asked, in either shape,
    /// and that is the missing-artifact exit and not an empty board's 0.
    #[test]
    fn peers_says_no_board_when_no_server_answers() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let root = repo.root().to_string_lossy().to_string();
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let share = share.to_string_lossy().to_string();
        for format in ["text", "json"] {
            let code = run(args(&[
                "--repo",
                &root,
                "--discovery",
                "filesystem",
                "--share",
                &share,
                "--format",
                format,
            ]))
            .expect("an answer, not an error");
            assert_eq!(code, EXIT_MISSING, "{format}");
        }
    }

    /// A lease that names a server which does not answer is an error, never an empty board.
    #[test]
    fn a_server_that_does_not_answer_is_an_error_and_not_a_board() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let mut out = Vec::new();
        // nothing listens on port 1 of the loopback address
        let e = answer(&ctx, Some("http://127.0.0.1:1"), &args(&[]), &mut out)
            .expect_err("nobody answered");
        assert!(matches!(e, Error::Http { .. }), "{e:?}");
        assert!(out.is_empty(), "an unanswered question printed something");
    }

    #[test]
    fn a_registry_without_the_peers_command_says_so() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let bare = Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(
                crate::capability::CapabilityRegistry::builder()
                    .build()
                    .expect("an empty registry"),
            ),
        );
        match answer(&bare, None, &args(&[]), &mut Vec::new()) {
            Err(Error::Protocol { reason }) => {
                assert_eq!(reason, "no capability is exposed as `majordomus peers`")
            }
            other => panic!("expected a protocol error, got {other:?}"),
        }
    }

    #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
    struct Asked {
        #[serde(default)]
        checkouts: Option<String>,
    }

    impl crate::capability::BenchmarkCases for Asked {
        fn benchmark_cases(
            _: &crate::capability::CaseContext<'_>,
        ) -> Vec<crate::capability::NamedCase<Self>> {
            vec![crate::capability::NamedCase::new(
                "default",
                Asked { checkouts: None },
            )]
        }
    }

    /// A sibling's board, read from a server that answered: one board asked.
    fn one_board(_: &Context, _: Asked) -> std::result::Result<Value, CapabilityError> {
        Ok(json!({
            "count": 0, "complete": true, "peers": [], "overlaps": [],
            "boards": [{ "worktree": "/r-wt/x", "standing": "ready", "this_checkout": false }],
        }))
    }

    fn refuses(_: &Context, _: Asked) -> std::result::Result<Value, CapabilityError> {
        Err(CapabilityError::Refused("no board today".into()))
    }

    /// A context whose `majordomus peers` is `handler` and nothing else.
    fn exposing(
        repo: &crate::synthetic::SyntheticRepository,
        handler: fn(&Context, Asked) -> std::result::Result<Value, CapabilityError>,
    ) -> Context {
        use crate::capability::model::{CliExposure, Exposure, Stability};
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_builtin(vec![crate::capability! {
                id: "test.peers",
                title: "A board of one",
                description: "Answers one board, or refuses.",
                input: Asked,
                output: Value,
                stability: Stability::Experimental,
                exposure: Exposure {
                    cli: Some(CliExposure { path: vec!["peers".into()] }),
                    ..Exposure::default()
                },
                tags: [],
                handler: handler,
            }])
            .build()
            .expect("a registry of one");
        Context::new(
            std::sync::Arc::new(repo.index().expect("an index")),
            std::sync::Arc::new(registry),
        )
    }

    /// A board that was asked is an answer: exit 0, in either shape, and the JSON is the
    /// capability's answer unchanged.
    #[test]
    fn a_board_that_was_asked_ends_on_zero() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = exposing(&repo, one_board);
        let mut out = Vec::new();
        let code = answer(&ctx, None, &args(&["--format", "json"]), &mut out).expect("answers");
        assert_eq!(code, 0);
        let back: Value = serde_json::from_slice(&out).expect("one JSON document");
        assert_eq!(back, one_board(&ctx, Asked { checkouts: None }).unwrap());
        assert!(out.ends_with(b"}\n"), "one document, one line end");
        let mut out = Vec::new();
        assert_eq!(
            answer(&ctx, None, &args(&[]), &mut out).expect("answers"),
            0
        );
        let s = String::from_utf8(out).unwrap();
        assert!(
            s.starts_with("peers      0 attached, 1 of 1 board(s) asked\n"),
            "{s}"
        );
    }

    /// The capability's own refusal reaches the person, and nothing is printed.
    #[test]
    fn a_board_that_is_refused_is_reported() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = exposing(&repo, refuses);
        let mut out = Vec::new();
        match answer(&ctx, None, &args(&[]), &mut out) {
            Err(Error::Protocol { reason }) => {
                assert!(reason.contains("no board today"), "{reason}")
            }
            other => panic!("expected a protocol error, got {other:?}"),
        }
        assert!(out.is_empty(), "a refusal printed something");
    }

    /// A reader that went away is an error in either shape, not a board printed to nobody.
    #[test]
    fn a_closed_output_is_an_error_in_either_shape() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = exposing(&repo, one_board);
        for argv in [&[][..], &["--format", "json"][..]] {
            let e = answer(&ctx, None, &args(argv), &mut ClosesAfter(0, Vec::new()))
                .expect_err("nothing can be written");
            assert!(
                matches!(&e, Error::Transport(io) if io.kind() == std::io::ErrorKind::BrokenPipe),
                "{argv:?}: {e:?}"
            );
        }
    }
}
