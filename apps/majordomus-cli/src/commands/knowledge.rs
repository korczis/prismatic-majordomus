//! `majordomus knowledge`: what the knowledge deriver left for review, and whether it is
//! still writing, rendered for a terminal.
//!
//! Every subcommand runs a capability of the `knowledge_base` module through the one
//! executor, so the answer a person reads here and the answer a client reads over MCP or
//! HTTP are the same answer rendered twice, never two derivations that happen to agree.
//! This file renders and nothing else: which records are candidates, how a reference
//! resolves and whether the writer has stopped are decided in the capability, and
//! `--format json` prints its value verbatim.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{KnowledgeArgs, KnowledgeCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus knowledge`.
pub fn run(args: KnowledgeArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match args.command {
        KnowledgeCommand::Candidates => {
            let v = execute(ctx, &["knowledge", "candidates"], json!({}))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => candidates_text(&mut out, &v)?,
            }
            Ok(0)
        }
        KnowledgeCommand::Record { id } => {
            let v = execute(ctx, &["knowledge", "record"], json!({ "id": id }))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => record_text(&mut out, &v)?,
            }
            Ok(0)
        }
        KnowledgeCommand::Status => {
            let v = execute(ctx, &["knowledge", "status"], json!({}))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => status_text(&mut out, &v)?,
            }
            Ok(0)
        }
    }
}

// ---------------------------------------------------------------- text

fn w(out: &mut std::io::StdoutLock<'_>, s: String) -> Result<()> {
    writeln!(out, "{s}").map_err(Error::Transport)
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}

fn findings(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    for f in v["findings"].as_array().into_iter().flatten() {
        w(out, format!("finding      {}", f.as_str().unwrap_or("")))?;
    }
    Ok(())
}

fn candidates_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    let cap = match v["cap"].as_u64() {
        Some(c) => format!(", cap {c}"),
        None => String::new(),
    };
    w(
        out,
        format!(
            "candidates   {} awaiting review — {} on {}, {} whose episode is not known here{cap}",
            v["total"],
            v["on_this_branch"],
            s(v, "branch"),
            v["unattributed"]
        ),
    )?;
    for c in v["candidates"].as_array().into_iter().flatten() {
        w(
            out,
            format!(
                "{}  {}  {}  {}  {} ({})  {}",
                s(c, "id"),
                s(c, "class"),
                s(c, "date"),
                c["branch"].as_str().unwrap_or("-"),
                s(c, "freshness"),
                s(c, "freshness_reason"),
                s(c, "title"),
            ),
        )?;
    }
    findings(out, v)
}

fn resolution(r: &Value) -> String {
    let mut line = format!("{}  {}", s(r, "reference"), s(r, "resolution"));
    if let Some(t) = r["target"].as_str() {
        line.push_str(&format!("  -> {t}"));
    }
    if !s(r, "reason").is_empty() {
        line.push_str(&format!("  ({})", s(r, "reason")));
    }
    line
}

fn record_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    w(out, format!("id           {}", s(v, "id")))?;
    w(out, format!("path         {}", s(v, "path")))?;
    w(
        out,
        format!(
            "record       {} {} {} {}  {}",
            s(v, "class"),
            s(v, "status"),
            s(v, "epistemics"),
            s(v, "origin"),
            s(v, "date")
        ),
    )?;
    w(out, format!("title        {}", s(v, "title")))?;
    if let Some(by) = v["superseded_by"].as_str() {
        w(out, format!("superseded   by {by}"))?;
    }
    for r in v["derived_from"].as_array().into_iter().flatten() {
        w(out, format!("derived_from {}", resolution(r)))?;
    }
    for r in v["relations"].as_array().into_iter().flatten() {
        w(
            out,
            format!("relation     {}  {}", s(r, "relation_type"), resolution(r)),
        )?;
    }
    w(
        out,
        format!(
            "resolved     {}",
            if v["resolved"].as_bool().unwrap_or(false) {
                "every reference answers"
            } else {
                "a reference dangles; `majordomus knowledge check` names it"
            }
        ),
    )
}

fn mark(v: &Value) -> String {
    match v.as_object() {
        Some(m) => format!(
            "{} at {}",
            m.get("episode").and_then(Value::as_str).unwrap_or("-"),
            m.get("ts").and_then(Value::as_str).unwrap_or("-")
        ),
        None => "none".to_string(),
    }
}

fn status_text(out: &mut std::io::StdoutLock<'_>, v: &Value) -> Result<()> {
    w(
        out,
        format!("present      {}  ({})", v["present"], s(v, "branch")),
    )?;
    w(out, format!("last derived {}", mark(&v["last_derived"])))?;
    w(out, format!("last closed  {}", mark(&v["last_closed"])))?;
    w(
        out,
        format!(
            "underived    {} closed episode(s) no derivation names",
            v["closed_without_derivation"]
        ),
    )?;
    w(
        out,
        format!(
            "candidates   {} awaiting review, {} on this branch",
            v["candidates"], v["on_this_branch"]
        ),
    )?;
    w(
        out,
        format!(
            "freshness    {} — {}",
            s(v, "freshness"),
            s(v, "freshness_reason")
        ),
    )?;
    w(
        out,
        format!(
            "writer       {}",
            match (
                v["judged"].as_bool().unwrap_or(false),
                v["stopped_writer"].as_bool().unwrap_or(false)
            ) {
                (false, _) => "not judged",
                (true, true) => "STOPPED",
                (true, false) => "writing",
            }
        ),
    )?;
    w(
        out,
        format!(
            "switches     knowledge_on_end {}  knowledge_on_compact {}",
            v["knowledge_on_end"], v["knowledge_on_compact"]
        ),
    )?;
    findings(out, v)
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// Run one of this module's capabilities by the command line it declares, so that the
/// command line cannot reach a capability it does not claim.
fn execute(ctx: &crate::capability::Context, path: &[&str], input: Value) -> Result<Value> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(id, input).map_err(map)
}

fn map(e: CapabilityError) -> Error {
    match e {
        // A record the repository does not hold is a missing thing, not a malformed
        // request, and the exit code has to say which.
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Protocol { reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_renders_its_episode_and_time_and_absence_renders_as_none() {
        assert_eq!(
            mark(&json!({ "ts": "2026-09-12T10:00:00Z", "episode": "e1" })),
            "e1 at 2026-09-12T10:00:00Z"
        );
        assert_eq!(mark(&Value::Null), "none");
    }

    #[test]
    fn a_resolution_line_carries_the_target_or_the_reason() {
        assert_eq!(
            resolution(
                &json!({ "reference": "session:e1", "resolution": "object", "target": "majordomus://session/e1" })
            ),
            "session:e1  object  -> majordomus://session/e1"
        );
        assert_eq!(
            resolution(
                &json!({ "reference": "task:none", "resolution": "missing", "reason": "not a task" })
            ),
            "task:none  missing  (not a task)"
        );
    }
}
