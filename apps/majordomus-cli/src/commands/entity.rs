//! `majordomus entity`: every object of the layer as an addressable node.
//!
//! Both subcommands run a capability of the `entity` module through the one executor, so
//! what a person reads here, what the Cockpit renders, what the HTTP route answers and what
//! an MCP client is handed are one answer rendered four ways rather than four derivations
//! that happen to agree.
//!
//! The text rendering keeps the distinction the subsystem exists for. `resolved` is printed
//! as *what it names is in the tree* and never as a tick: whether any of it ever ran is a
//! different question, and the line that answers it points at the capability that can.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{EntityArgs, EntityCommand, OutputFormat};
use crate::error::{Error, Result};

/// Run `majordomus entity`.
pub fn run(args: EntityArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match args.command {
        EntityCommand::Kinds => {
            let v = execute(ctx, &["entity", "kinds"], json!({}))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => kinds_text(&mut out, &v)?,
            }
            // a collision means an object of this repository has no address of its own
            Ok(u8::from(
                !v["collisions"]
                    .as_array()
                    .map(Vec::is_empty)
                    .unwrap_or(true),
            ) * EXIT_COLLISION)
        }
        EntityCommand::Show { address } => {
            let v = execute(ctx, &["entity", "show"], input_for(&address))?;
            match args.format {
                OutputFormat::Json => writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?,
                OutputFormat::Text => show_text(&mut out, &v)?,
            }
            Ok(0)
        }
    }
}

/// The exit code when `entity kinds` finds a route collision.
pub const EXIT_COLLISION: u8 = 10;

/// An address on the command line is either the URI or the route's two segments. One
/// function so that both spellings reach the same capability input.
///
/// Anything else is passed through as a URI, so that the capability's own refusal is what a
/// reader sees rather than a second opinion from here. The cases are asserted in this
/// module's tests; the module is crate-private, so they are not a doctest.
pub fn input_for(address: &str) -> Value {
    if address.starts_with("majordomus://") {
        return json!({ "uri": address });
    }
    match address.split_once('/') {
        Some((kind, slug)) if !kind.is_empty() && !slug.is_empty() && !slug.contains('/') => {
            json!({ "kind": kind, "slug": slug })
        }
        _ => json!({ "uri": address }),
    }
}

fn kinds_text(out: &mut impl Write, v: &Value) -> Result<()> {
    let empty = Vec::new();
    for k in v["kinds"].as_array().unwrap_or(&empty) {
        writeln!(
            out,
            "{:<20} {:>5}  {}",
            k["kind"].as_str().unwrap_or_default(),
            k["count"].as_u64().unwrap_or_default(),
            k["route"].as_str().unwrap_or_default()
        )
        .map_err(Error::Transport)?;
    }
    writeln!(
        out,
        "\n{} kind(s); {} object(s) addressable",
        v["count"].as_u64().unwrap_or_default(),
        v["routable"].as_u64().unwrap_or_default()
    )
    .map_err(Error::Transport)?;
    for c in v["collisions"].as_array().unwrap_or(&empty) {
        writeln!(
            out,
            "collision  {}",
            c["correction"].as_str().unwrap_or_default()
        )
        .map_err(Error::Transport)?;
    }
    Ok(())
}

fn show_text(out: &mut impl Write, v: &Value) -> Result<()> {
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    writeln!(out, "{:<12}{}", "uri", s("uri")).map_err(Error::Transport)?;
    writeln!(out, "{:<12}{}", "kind", s("kind")).map_err(Error::Transport)?;
    writeln!(out, "{:<12}{}", "route", s("route")).map_err(Error::Transport)?;
    if !s("title").is_empty() {
        writeln!(out, "{:<12}{}", "title", s("title")).map_err(Error::Transport)?;
    }
    writeln!(
        out,
        "{:<12}{}",
        "source",
        v["provenance"]["path"].as_str().unwrap_or_default()
    )
    .map_err(Error::Transport)?;

    let empty = Vec::new();
    let relations = v["relations"].as_array().unwrap_or(&empty);
    let (out_edges, in_edges): (Vec<_>, Vec<_>) = relations
        .iter()
        .partition(|e| e["direction"].as_str() == Some("outgoing"));
    for (heading, set) in [("references", &out_edges), ("referenced by", &in_edges)] {
        if set.is_empty() {
            continue;
        }
        writeln!(out, "\n{heading}").map_err(Error::Transport)?;
        for e in set.iter() {
            writeln!(
                out,
                "  {:<16} {:<12} {}{}",
                e["edge"].as_str().unwrap_or_default(),
                e["kind"].as_str().unwrap_or_default(),
                e["label"].as_str().unwrap_or_default(),
                if e["external"].as_bool().unwrap_or(false) {
                    "  (outside the layer)"
                } else {
                    ""
                }
            )
            .map_err(Error::Transport)?;
        }
    }

    writeln!(out, "\nevidence").map_err(Error::Transport)?;
    writeln!(
        out,
        "  {:<16} {}",
        v["evidence"]["state"].as_str().unwrap_or_default(),
        v["evidence"]["meaning"].as_str().unwrap_or_default()
    )
    .map_err(Error::Transport)?;
    for a in v["evidence"]["artifacts"].as_array().unwrap_or(&empty) {
        writeln!(
            out,
            "  {:<16} {} ({})",
            if a["present"].as_bool().unwrap_or(false) {
                "in the tree"
            } else {
                "NOT in the tree"
            },
            a["path"].as_str().unwrap_or_default(),
            a["field"].as_str().unwrap_or_default()
        )
        .map_err(Error::Transport)?;
    }
    if let Some(p) = v["evidence"]["proof"].as_object() {
        writeln!(
            out,
            "  {:<16} {} — whether any of it ran",
            "ask",
            p.get("capability")
                .and_then(Value::as_str)
                .unwrap_or_default()
        )
        .map_err(Error::Transport)?;
    }

    writeln!(out, "\nserved at").map_err(Error::Transport)?;
    for s in v["surfaces"].as_array().unwrap_or(&empty) {
        writeln!(
            out,
            "  {:<10} {}",
            s["surface"].as_str().unwrap_or_default(),
            s["address"].as_str().unwrap_or_default()
        )
        .map_err(Error::Transport)?;
    }
    Ok(())
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

/// An entity the layer does not hold is a missing thing, not a malformed request: the exit
/// code has to say which, or a typo reads as "this entity has no relations".
fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::NotFound(reason) => Error::NotFound { reason },
        CapabilityError::InvalidInput(reason) | CapabilityError::Refused(reason) => {
            Error::Protocol { reason }
        }
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_read_as_a_uri_or_as_a_route() {
        assert_eq!(
            input_for("majordomus://rule/project.x@1"),
            json!({ "uri": "majordomus://rule/project.x@1" })
        );
        assert_eq!(
            input_for("rule/project-x-1"),
            json!({ "kind": "rule", "slug": "project-x-1" })
        );
        // a path with more than one separator is not a route, and is not guessed at
        assert_eq!(input_for("a/b/c"), json!({ "uri": "a/b/c" }));
        assert_eq!(input_for("/x"), json!({ "uri": "/x" }));
    }
}
