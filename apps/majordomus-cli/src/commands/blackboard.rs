//! `majordomus blackboard`: who works on what in this repository, on the command line
//! (ADR 0176).
//!
//! The capability joins; this renders. A board is its server's memory and the mesh runs in
//! that server, so the answer is asked of the server holding this checkout's lease; with no
//! server here the capability runs in this process, still asks every sibling checkout's server,
//! and says in its sources that this checkout's board and the mesh were not read.

use std::fmt::Write as _;
use std::io::Write as _;

use serde_json::{json, Value};

use super::executions::{ask_server, cli_capability, map};
use crate::app::App;
use crate::cli::{BlackboardArgs, OutputFormat};
use crate::error::{Error, Result};
use crate::lease;

/// Run `majordomus blackboard`.
pub fn run(args: BlackboardArgs) -> Result<u8> {
    App::load(&args.repo)
        .and_then(|app| answer(&app, &args))
        .and_then(|answer| {
            std::io::stdout()
                .lock()
                .write_all(shown(args.format, &answer).as_bytes())
                .map(|()| 0)
                .map_err(Error::Transport)
        })
}

/// The blackboard, asked of the server holding this checkout's lease, or of this process
/// when there is none.
fn answer(app: &App, args: &BlackboardArgs) -> Result<Value> {
    let ctx = &app.context;
    let mut input = json!({
        "checkouts": if args.this_checkout { "this" } else { "repository" },
    });
    if let Some(reader) = &args.reader {
        input["for"] = json!(reader);
    }
    if let Some(sources) = &args.sources {
        input["sources"] = json!(sources);
    }
    cli_capability(ctx, &["blackboard"]).and_then(|id| match lease::serving(&app.repository) {
        Some(url) => ask_server(ctx, &url, id, &input),
        None => ctx.execute(id, input).map_err(map),
    })
}

/// The answer as `format` shows it.
fn shown(format: OutputFormat, answer: &Value) -> String {
    match format {
        OutputFormat::Json => format!(
            "{}\n",
            serde_json::to_string_pretty(answer).unwrap_or_default()
        ),
        OutputFormat::Text => render(answer),
    }
}

fn text<'v>(v: &'v Value, key: &str) -> &'v str {
    v[key].as_str().unwrap_or_default()
}

/// The answer as lines a person reads: the headline, the sources that are not `ok`, every
/// worker, then what meets and what waits. Written to a string, which cannot fail.
fn render(v: &Value) -> String {
    let mut out = String::new();
    let empty = Vec::new();
    let list = |key: &str| v[key].as_array().unwrap_or(&empty);
    let _ = writeln!(
        out,
        "blackboard  {} present, {} worker(s) on {} board(s); {}",
        v["present"].as_u64().unwrap_or(0),
        list("workers").len(),
        list("boards").len(),
        if v["complete"].as_bool() == Some(true) {
            "complete"
        } else {
            "INCOMPLETE"
        },
    );
    for s in list("sources").iter().filter(|s| text(s, "state") != "ok") {
        let _ = writeln!(
            out,
            "source      {} {}: {}",
            text(s, "source"),
            text(s, "state"),
            text(s, "reason")
        );
    }
    for w in list("workers") {
        let intent = w["intent"].as_str().unwrap_or("not announced");
        let branch = w["branch"]
            .as_str()
            .map(|b| format!(" @{b}"))
            .unwrap_or_default();
        let machine = w["machine"]
            .as_str()
            .map(|m| format!(" on {m}"))
            .unwrap_or_default();
        let _ = writeln!(
            out,
            "worker      {} {} {}{branch}{machine}: {intent}",
            text(w, "key"),
            text(w, "client"),
            text(w, "presence"),
        );
        for c in w["claims"].as_array().unwrap_or(&empty) {
            let paths: Vec<&str> = c["scope"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .filter_map(Value::as_str)
                .collect();
            let _ = writeln!(out, "  claim     {} {}", text(c, "mode"), paths.join(" "));
        }
    }
    for c in list("conflicts") {
        let _ = writeln!(
            out,
            "conflict    {} {} meets {}",
            text(c, "certainty"),
            c["first"]["worker"].as_str().unwrap_or_default(),
            c["second"]["worker"].as_str().unwrap_or_default(),
        );
    }
    let total = v["conflicts_total"].as_u64().unwrap_or(0) as usize;
    if total > list("conflicts").len() {
        let _ = writeln!(
            out,
            "conflict    … {} more",
            total - list("conflicts").len()
        );
    }
    for r in list("reviews") {
        let _ = writeln!(
            out,
            "review      {} {} asks {}: {}",
            text(r, "key"),
            text(r, "from"),
            r["reviewer"].as_str().unwrap_or("anyone"),
            text(r, "subject"),
        );
    }
    for h in list("handovers") {
        let _ = writeln!(
            out,
            "handover    {} from {} {}",
            text(h, "id"),
            text(h, "runtime"),
            h["branch"].as_str().unwrap_or_default(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a person reads names every worker, says whether the answer is complete and which
    /// source is not, and marks a worker that announced nothing as such rather than leaving a
    /// blank.
    #[test]
    fn the_text_names_every_worker_and_what_was_not_read() {
        let v = json!({
            "present": 1, "complete": false, "conflicts_total": 0,
            "boards": [{}],
            "sources": [
                {"source": "board", "state": "ok"},
                {"source": "mesh", "state": "absent", "reason": "not active"}
            ],
            "workers": [{"key": "board:c1/p1", "client": "codex", "presence": "attached",
                         "branch": "master", "machine": "lundra", "claims": [{"mode": "advisory", "scope": ["docs"]}]}],
            "conflicts": [], "reviews": [], "handovers": []
        });
        let out = render(&v);
        assert!(
            out.contains("1 present, 1 worker(s) on 1 board(s); INCOMPLETE"),
            "{out}"
        );
        assert!(out.contains("source      mesh absent: not active"), "{out}");
        assert!(!out.contains("source      board"), "{out}");
        assert!(
            out.contains("worker      board:c1/p1 codex attached @master on lundra: not announced"),
            "{out}"
        );
        assert!(out.contains("  claim     advisory docs"), "{out}");
    }

    /// What meets and what waits is a line each, with what the answer left out counted.
    #[test]
    fn conflicts_reviews_and_handovers_are_a_line_each() {
        let v = json!({
            "complete": true, "conflicts_total": 3,
            "conflicts": [{"certainty": "potential",
                           "first": {"worker": "a"}, "second": {"worker": "b"}}],
            "reviews": [{"key": "s/r-1", "from": "a", "reviewer": "1b", "subject": "read"},
                        {"key": "s/r-2", "from": "b", "subject": "anyone"}],
            "handovers": [{"id": "h-1", "runtime": "r", "branch": "side"}]
        });
        let out = render(&v);
        for line in [
            "; complete",
            "conflict    potential a meets b",
            "conflict    … 2 more",
            "review      s/r-1 a asks 1b: read",
            "review      s/r-2 b asks anyone: anyone",
            "handover    h-1 from r side",
        ] {
            assert!(out.contains(line), "{line:?} is not in {out}");
        }
    }

    fn args(repo: &std::path::Path, more: &[&str]) -> BlackboardArgs {
        use clap::Parser;
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let mut argv = vec![
            "majordomus".to_string(),
            "blackboard".into(),
            "--repo".into(),
            repo.display().to_string(),
            "--discovery".into(),
            "filesystem".into(),
            "--share".into(),
            share.display().to_string(),
        ];
        argv.extend(more.iter().map(|m| m.to_string()));
        match crate::cli::Cli::try_parse_from(argv).unwrap().command {
            crate::cli::Command::Blackboard(args) => args,
            _ => panic!("not the blackboard command"),
        }
    }

    /// With no server for the checkout the command answers in this process, in either format,
    /// and an input the capability refuses, or a directory that is not a repository, is an
    /// error rather than an empty board.
    #[test]
    fn the_command_answers_in_this_process_and_refuses_what_it_cannot_ask() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        assert_eq!(run(args(repo.root(), &[])).unwrap(), 0);
        let wide = args(
            repo.root(),
            &[
                "--this-checkout",
                "--for",
                "1b",
                "--sources",
                "mesh",
                "--format",
                "json",
            ],
        );
        let app = App::load(&wide.repo).unwrap();
        let answer = answer(&app, &wide).unwrap();
        assert!(answer["workers"].is_array(), "{answer}");
        assert!(shown(OutputFormat::Json, &answer).ends_with("}\n"));
        assert_eq!(run(wide).unwrap(), 0);
        assert!(run(args(repo.root(), &["--sources", "nowhere"])).is_err());
        let empty = tempfile::tempdir().unwrap();
        assert!(run(args(empty.path(), &[])).is_err());
    }

    /// A checkout whose lease names a server that answers for it is asked through that server.
    #[test]
    fn a_served_checkout_is_asked_through_its_server() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let wanted = args(repo.root(), &["--format", "json"]);
        let app = App::load(&wanted.repo).unwrap();
        let url = crate::monitor::testing::http_stub(
            crate::repository::identity(app.repository.root()),
            r#"{"workers": [], "complete": true, "served": "by the stub"}"#,
        );
        let lease = lease::lease_path(&app.repository);
        std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
        let doc = lease::LeaseDocument {
            schema: lease::SCHEMA.into(),
            pid: std::process::id(),
            token: "t".into(),
            root: app.repository.root().to_path_buf(),
            url: Some(url),
            started_at: "2026-10-18T00:00:00Z".into(),
            executable: None,
            version: Some(crate::VERSION.into()),
        };
        std::fs::write(&lease, serde_json::to_string(&doc).unwrap()).unwrap();
        let answer = answer(&app, &wanted).unwrap();
        assert_eq!(answer["served"], "by the stub", "{answer}");
    }
}
