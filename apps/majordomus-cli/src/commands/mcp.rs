//! `majordomus mcp`: load the application, and either serve the registry over stdio or,
//! with `--inspect`, print it and stop.

use std::io::Write;

use crate::app::App;
use crate::cli::{McpArgs, OutputFormat, Transport};
use crate::error::{Error, Result};
use crate::mcp::{stdio, Server, Surface};
use crate::model::Severity;

/// Run `majordomus mcp`.
pub fn run(args: McpArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let surface = Surface::new(app.context.clone());
    if args.inspect {
        return inspect(&surface, args.format);
    }
    let mut server = Server::new(surface, crate::VERSION);
    match args.transport {
        Transport::Stdio => {
            let stdin = std::io::stdin();
            let stdout = std::io::stdout();
            stdio::serve(&mut server, stdin.lock(), stdout.lock())?;
        }
    }
    Ok(0)
}

fn inspect(surface: &Surface, format: OutputFormat) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let index = surface.index();
    match format {
        OutputFormat::Json => {
            let v = serde_json::json!({
                "repository": surface.repository_info().map_err(|e| Error::Protocol { reason: e.to_string() })?,
                "resources": surface.resources(),
                "tools": surface.tools(),
            });
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&v).map_err(|e| Error::Protocol {
                    reason: e.to_string()
                })?
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => {
            let w = |out: &mut std::io::StdoutLock<'_>, line: String| {
                writeln!(out, "{line}").map_err(Error::Transport)
            };
            w(&mut out, format!("repository  {}", index.repository.root))?;
            w(
                &mut out,
                format!("discovery   {}", index.repository.discovery),
            )?;
            w(
                &mut out,
                format!(
                    "state       {}",
                    match index.state {
                        crate::index::State::Ok => "ok",
                        crate::index::State::Degraded => "degraded",
                    }
                ),
            )?;
            for (kind, n) in index.kinds() {
                w(&mut out, format!("kind        {kind:<12} {n}"))?;
            }
            let summary = surface.registry().summary();
            w(
                &mut out,
                format!(
                    "capabilities {} ({} builtin, {} declarative)",
                    summary.total, summary.builtin, summary.declarative
                ),
            )?;
            for r in surface.resources() {
                w(&mut out, format!("resource    {}", r.uri))?;
            }
            for t in surface.tools() {
                w(&mut out, format!("tool        {}", t.name))?;
            }
            for d in &index.diagnostics {
                let level = match d.severity {
                    Severity::Error => "FAIL",
                    Severity::Warning => "WARN",
                    Severity::Info => "INFO",
                };
                w(
                    &mut out,
                    format!(
                        "{level:<4} {:<22} {} — {}",
                        d.code,
                        d.path.as_deref().unwrap_or("-"),
                        d.message
                    ),
                )?;
            }
        }
    }
    Ok(if index.errors() > 0 { 10 } else { 0 })
}
