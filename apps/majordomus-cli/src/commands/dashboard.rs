//! `majordomus dashboard`: the Dashboard Suite on the command line.
//!
//! Every subcommand runs a capability of the `dashboard` module through the one executor,
//! so what a person reads here, what the Cockpit's overview renders, what the HTTP route
//! answers and what an MCP client is handed are one answer rendered four ways. Nothing is
//! computed here: the text rendering prints each card's value as the capability carried it,
//! beside the source capability and pointer it was read from.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::cli::{DashboardArgs, DashboardCommand, OutputFormat};
use crate::error::{Error, Result};

/// The exit code when the overview is `fail` or `unknown`: a verdict not satisfied, the
/// code `quality report` and `shell check` use for the same thing.
pub const EXIT_UNSATISFIED: u8 = 10;

/// Run `majordomus dashboard`.
pub fn run(args: DashboardArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match args.command {
        DashboardCommand::Overview => {
            let id = ctx
                .registry
                .by_cli(&["dashboard".to_string(), "overview".to_string()])
                .map(|c| c.id.as_str().to_string())
                .ok_or_else(|| Error::Protocol {
                    reason: "no capability is exposed as `majordomus dashboard overview`".into(),
                })?;
            let v = ctx.execute(&id, json!({})).map_err(|e| Error::Protocol {
                reason: e.to_string(),
            })?;
            match args.format {
                OutputFormat::Json => writeln!(
                    out,
                    "{}",
                    serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
                )
                .map_err(Error::Transport)?,
                OutputFormat::Text => overview_text(&mut out, &v)?,
            }
            Ok(exit_for(&v))
        }
    }
}

/// The exit code the overview's own status word implies.
fn exit_for(v: &Value) -> u8 {
    match v["status"].as_str() {
        Some("ok") | Some("warn") => 0,
        _ => EXIT_UNSATISFIED,
    }
}

/// A card's value as a person reads it: a string bare, `none` for null, anything else as
/// JSON. Only the rendering differs; the value is the capability's.
fn shown(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "none".into(),
        other => other.to_string(),
    }
}

fn overview_text(out: &mut impl Write, v: &Value) -> Result<()> {
    let empty = Vec::new();
    for q in v["questions"].as_array().unwrap_or(&empty) {
        writeln!(
            out,
            "{}  [{}]",
            q["title"].as_str().unwrap_or_default(),
            q["status"].as_str().unwrap_or_default()
        )
        .map_err(Error::Transport)?;
        for c in q["cards"].as_array().unwrap_or(&empty) {
            writeln!(
                out,
                "  {:<8} {:<34} {:<12} {}{}",
                c["status"].as_str().unwrap_or_default(),
                c["title"].as_str().unwrap_or_default(),
                shown(&c["value"]),
                c["source"]["capability"].as_str().unwrap_or_default(),
                c["source"]["pointer"].as_str().unwrap_or_default(),
            )
            .map_err(Error::Transport)?;
        }
        writeln!(out).map_err(Error::Transport)?;
    }
    writeln!(
        out,
        "overview: {}",
        v["status"].as_str().unwrap_or_default()
    )
    .map_err(Error::Transport)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exit_code_is_the_overviews_own_status() {
        assert_eq!(exit_for(&json!({ "status": "ok" })), 0);
        assert_eq!(exit_for(&json!({ "status": "warn" })), 0);
        assert_eq!(exit_for(&json!({ "status": "fail" })), EXIT_UNSATISFIED);
        // an undecided overview is not a satisfied one
        assert_eq!(exit_for(&json!({ "status": "unknown" })), EXIT_UNSATISFIED);
        assert_eq!(exit_for(&json!({})), EXIT_UNSATISFIED);
    }

    #[test]
    fn a_value_is_shown_as_the_capability_carried_it() {
        assert_eq!(shown(&json!("0.10.0")), "0.10.0");
        assert_eq!(shown(&json!(3)), "3");
        assert_eq!(shown(&json!(true)), "true");
        assert_eq!(shown(&Value::Null), "none");
    }
}
