//! `majordomus blackboard`: who works on what in this repository, on the command line
//! (ADR 0176).
//!
//! The capability joins; this renders. A board is its server's memory and the mesh runs in
//! that server, so the answer is asked of the server holding this checkout's lease; with no
//! server here the capability runs in this process, still asks every sibling checkout's server,
//! and says in its sources that this checkout's board and the mesh were not read.

use std::io::Write;

use serde_json::{json, Value};

use super::executions::{ask_server, cli_capability, map};
use crate::app::App;
use crate::cli::{BlackboardArgs, OutputFormat};
use crate::error::{Error, Result};
use crate::lease;

/// Run `majordomus blackboard`.
pub fn run(args: BlackboardArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = cli_capability(ctx, &["blackboard"])?;
    let mut input = json!({
        "checkouts": if args.this_checkout { "this" } else { "repository" },
    });
    if let Some(reader) = &args.reader {
        input["for"] = json!(reader);
    }
    if let Some(sources) = &args.sources {
        input["sources"] = json!(sources);
    }
    let answer = match lease::serving(&app.repository) {
        Some(url) => ask_server(ctx, &url, id, &input)?,
        None => ctx.execute(id, input).map_err(map)?,
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut out, &answer)
                .map_err(|e| Error::Transport(e.into()))?;
            writeln!(out).map_err(Error::Transport)?;
        }
        OutputFormat::Text => render(&mut out, &answer).map_err(Error::Transport)?,
    }
    Ok(0)
}

fn text<'v>(v: &'v Value, key: &str) -> &'v str {
    v[key].as_str().unwrap_or_default()
}

/// The answer as lines a person reads: the headline, the sources that are not `ok`, every
/// worker, then what meets and what waits.
fn render(out: &mut impl Write, v: &Value) -> std::io::Result<()> {
    let empty = Vec::new();
    let list = |key: &str| v[key].as_array().unwrap_or(&empty);
    writeln!(
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
    )?;
    for s in list("sources").iter().filter(|s| text(s, "state") != "ok") {
        writeln!(
            out,
            "source      {} {}: {}",
            text(s, "source"),
            text(s, "state"),
            text(s, "reason")
        )?;
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
        writeln!(
            out,
            "worker      {} {} {}{branch}{machine}: {intent}",
            text(w, "key"),
            text(w, "client"),
            text(w, "presence"),
        )?;
        for c in w["claims"].as_array().unwrap_or(&empty) {
            let paths: Vec<&str> = c["scope"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .filter_map(Value::as_str)
                .collect();
            writeln!(out, "  claim     {} {}", text(c, "mode"), paths.join(" "))?;
        }
    }
    for c in list("conflicts") {
        writeln!(
            out,
            "conflict    {} {} meets {}",
            text(c, "certainty"),
            c["first"]["worker"].as_str().unwrap_or_default(),
            c["second"]["worker"].as_str().unwrap_or_default(),
        )?;
    }
    let total = v["conflicts_total"].as_u64().unwrap_or(0) as usize;
    if total > list("conflicts").len() {
        writeln!(
            out,
            "conflict    … {} more",
            total - list("conflicts").len()
        )?;
    }
    for r in list("reviews") {
        writeln!(
            out,
            "review      {} {} asks {}: {}",
            text(r, "key"),
            text(r, "from"),
            r["reviewer"].as_str().unwrap_or("anyone"),
            text(r, "subject"),
        )?;
    }
    for h in list("handovers") {
        writeln!(
            out,
            "handover    {} from {} {}",
            text(h, "id"),
            text(h, "runtime"),
            h["branch"].as_str().unwrap_or_default(),
        )?;
    }
    Ok(())
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
                         "branch": "master", "claims": [{"mode": "advisory", "scope": ["docs"]}]}],
            "conflicts": [], "reviews": [], "handovers": []
        });
        let mut out = Vec::new();
        render(&mut out, &v).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(
            out.contains("1 present, 1 worker(s) on 1 board(s); INCOMPLETE"),
            "{out}"
        );
        assert!(out.contains("source      mesh absent: not active"), "{out}");
        assert!(!out.contains("source      board"), "{out}");
        assert!(
            out.contains("worker      board:c1/p1 codex attached @master: not announced"),
            "{out}"
        );
        assert!(out.contains("  claim     advisory docs"), "{out}");
    }
}
