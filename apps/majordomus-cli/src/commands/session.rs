//! `majordomus session`: the episode record, composed and read back through one derivation.
//!
//! Both subcommands are projections of registry capabilities and decide nothing themselves —
//! `session compose` of `lifecycle.compose`, `session show` of `lifecycle.record` — so the
//! record the shell lifecycle writes is byte-for-byte the record the API, MCP and the
//! Cockpit would describe. That is the whole point of the command existing: before it, the
//! shell derived a record's attribution itself and the runtime read the result, which is two
//! implementations of one semantic and exactly what `project.development-semantics-are-canonical`
//! refuses.
//!
//! `compose` takes its input on standard input rather than on flags. The boundary of an
//! episode is a dozen fields including two lists, and a command line that spelled each of
//! them as an option would be a second encoding of a type that already has one; the JSON the
//! capability declares is the encoding, and the shell writes it.

use std::io::{Read, Write};

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, SessionArgs, SessionCommand};
use crate::error::{Error, Result};

/// Run `majordomus session`.
pub fn run(args: SessionArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match args.command {
        SessionCommand::Compose => {
            let mut raw = String::new();
            std::io::stdin()
                .read_to_string(&mut raw)
                .map_err(Error::Transport)?;
            // An empty stdin is a caller that forgot to send the boundary, not an episode
            // with no boundary. Composing a record from `{}` would write an envelope with
            // no identity, which is worse than refusing.
            let input: Value = if raw.trim().is_empty() {
                return Err(Error::Protocol {
                    reason: "session compose: the episode's boundary is read from standard input as JSON"
                        .into(),
                });
            } else {
                serde_json::from_str(&raw)
                    .map_err(|e| Error::Protocol { reason: format!("session compose: {e}") })?
            };
            let v = ctx
                .execute("lifecycle.compose", input)
                .map_err(map)?;
            match args.format {
                // The default is the record itself, because the caller that matters is the
                // lifecycle, which writes these bytes into the layer.
                OutputFormat::Text => {
                    write!(out, "{}", v["record"].as_str().unwrap_or_default())
                        .map_err(Error::Transport)?
                }
                OutputFormat::Json => {
                    writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?
                }
            }
            Ok(0)
        }
        SessionCommand::Show { session_id } => {
            let v = ctx
                .execute(
                    "lifecycle.record",
                    json!({ "session_id": session_id.unwrap_or_default() }),
                )
                .map_err(map)?;
            match args.format {
                OutputFormat::Json => {
                    writeln!(out, "{}", pretty(&v)).map_err(Error::Transport)?
                }
                OutputFormat::Text => {
                    let f = &v["facts"];
                    writeln!(
                        out,
                        "session      {}",
                        f["boundary"]["session_id"].as_str().unwrap_or("?")
                    )
                    .map_err(Error::Transport)?;
                    writeln!(
                        out,
                        "outcome      {}",
                        f["boundary"]["outcome"].as_str().unwrap_or("?")
                    )
                    .map_err(Error::Transport)?;
                    writeln!(
                        out,
                        "completeness {}",
                        f["completeness"].as_str().unwrap_or("?")
                    )
                    .map_err(Error::Transport)?;
                    for k in [
                        "tasks",
                        "issues",
                        "milestones",
                        "checkpoints",
                        "handovers",
                        "decisions",
                        "questions",
                        "evidence",
                    ] {
                        for a in f[k].as_array().into_iter().flatten() {
                            writeln!(
                                out,
                                "{:<12} {}  ({})",
                                k,
                                a["id"].as_str().unwrap_or("?"),
                                a["source"].as_str().unwrap_or("?")
                            )
                            .map_err(Error::Transport)?;
                        }
                    }
                    for d in f["diagnostics"].as_array().into_iter().flatten() {
                        writeln!(out, "FAIL         {}", d["what"].as_str().unwrap_or("?"))
                            .map_err(Error::Transport)?;
                        writeln!(out, "             {}", d["remedy"].as_str().unwrap_or("?"))
                            .map_err(Error::Transport)?;
                    }
                    writeln!(out).map_err(Error::Transport)?;
                    write!(out, "{}", v["record"].as_str().unwrap_or_default())
                        .map_err(Error::Transport)?;
                }
            }
            // A record the repository can prove is incomplete is a verdict, and a verdict
            // that exits 0 is a footnote. 10 is the code this tool uses for a contract unmet.
            Ok(if v["facts"]["completeness"] == "incomplete" {
                crate::commands::scope::EXIT_OUT_OF_SCOPE
            } else {
                0
            })
        }
    }
}

/// A capability error as this command's error, the same mapping every other command makes.
fn map(e: CapabilityError) -> Error {
    match e {
        CapabilityError::InvalidInput(reason) => Error::Protocol { reason },
        other => Error::Protocol {
            reason: other.to_string(),
        },
    }
}

/// Pretty JSON, the shape every `--format json` prints.
fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}
