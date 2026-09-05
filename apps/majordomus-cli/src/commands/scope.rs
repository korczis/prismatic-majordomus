//! `majordomus scope`: the repository scope through the registry's own `scope.info` and
//! `scope.classify`. With no path it prints the declaration's origin and the tally; with
//! paths it judges each and, under `--check`, exits 10 when any is out.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, ScopeArgs};
use crate::error::{Error, Result};

/// The exit code when `--check` finds a path out of the scope.
pub const EXIT_OUT_OF_SCOPE: u8 = 10;

/// Run `majordomus scope`.
pub fn run(args: ScopeArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let w = |out: &mut std::io::StdoutLock<'_>, s: String| {
        writeln!(out, "{s}").map_err(Error::Transport)
    };
    if args.paths.is_empty() {
        let v = ctx
            .execute(cli_capability(ctx, &["scope"])?, json!({}))
            .map_err(map)?;
        match args.format {
            OutputFormat::Json => w(&mut out, pretty(&v))?,
            OutputFormat::Text => {
                w(
                    &mut out,
                    format!(
                        "scope        {} ({})",
                        v["path"].as_str().unwrap_or("?"),
                        v["origin"].as_str().unwrap_or("?")
                    ),
                )?;
                let n = |k: &str| v["declaration"][k].as_array().map_or(0, Vec::len);
                w(&mut out, format!("in           {} pathspec(s)", n("in")))?;
                let t = &v["tracked"];
                w(
                    &mut out,
                    format!(
                        "tracked      {} file(s): {} in, {} out",
                        t["files"], t["in"], t["out"]
                    ),
                )?;
                if let Some(by) = t["by_reason"].as_object() {
                    for (reason, count) in by {
                        w(&mut out, format!("  {reason:<20} {count}"))?;
                    }
                }
                for f in t["out_files"].as_array().into_iter().flatten() {
                    w(
                        &mut out,
                        format!(
                            "out  {:<20} {}{}",
                            f["reason"].as_str().unwrap_or("?"),
                            f["path"].as_str().unwrap_or("?"),
                            f["rule"]
                                .as_str()
                                .map(|r| format!("  ({r})"))
                                .unwrap_or_default()
                        ),
                    )?;
                }
            }
        }
        return Ok(0);
    }
    let id = cli_capability(ctx, &["scope", "classify"])?;
    let mut any_out = false;
    let mut results = Vec::with_capacity(args.paths.len());
    for path in &args.paths {
        let v = ctx.execute(id, json!({ "path": path })).map_err(map)?;
        if v["verdict"].as_str() == Some("out") {
            any_out = true;
        }
        results.push(v);
    }
    match args.format {
        OutputFormat::Json => w(&mut out, pretty(&Value::Array(results)))?,
        OutputFormat::Text => {
            for v in &results {
                let reason = v["reason"].as_str().unwrap_or("");
                let rule = v["rule"]
                    .as_str()
                    .map(|r| format!("  ({r})"))
                    .unwrap_or_default();
                let exists = if v["exists"].as_bool() == Some(true) {
                    ""
                } else {
                    "  [absent]"
                };
                w(
                    &mut out,
                    format!(
                        "{:<4} {:<20} {}{}{}",
                        v["verdict"].as_str().unwrap_or("?"),
                        reason,
                        v["path"].as_str().unwrap_or("?"),
                        rule,
                        exists
                    ),
                )?;
            }
        }
    }
    Ok(if args.check && any_out {
        EXIT_OUT_OF_SCOPE
    } else {
        0
    })
}

fn cli_capability<'a>(ctx: &'a crate::capability::Context, path: &[&str]) -> Result<&'a str> {
    let words: Vec<String> = path.iter().map(|w| w.to_string()).collect();
    ctx.registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })
}

fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::InvalidInput(reason) => Error::Protocol { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}
