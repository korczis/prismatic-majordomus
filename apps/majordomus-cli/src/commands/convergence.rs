//! `majordomus convergence`: is any of this repository's work held where it can be lost?
//!
//! The command owns the rendering and nothing else. The measurement and the verdict happen
//! inside `convergence.report`, so the terminal report, the `--format json` document, the
//! HTTP response and the MCP resource are four renderings of one execution and cannot
//! disagree about whether the repository converged — which is the failure mode a verdict
//! something refuses on must not have.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::cli::{ConvergenceArgs, OutputFormat};
use crate::convergence::ConvergenceReport;
use crate::error::{Error, Result};

/// The exit code when work is held where it can be lost. The code every unmet contract in
/// this executable uses, so a caller need not learn a second vocabulary for this one.
pub const EXIT_NOT_CONVERGED: u8 = 10;

/// Run `majordomus convergence`.
pub fn run(args: ConvergenceArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = ctx
        .registry
        .by_cli(&["convergence".to_string()])
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus convergence`".into(),
        })?;
    let value = ctx.execute(id, json!({})).map_err(|e| Error::Protocol {
        reason: format!("convergence.report refused: {e}"),
    })?;
    let report: ConvergenceReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("convergence.report answered something this command cannot read: {e}"),
        })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => render(&mut out, &report, args.all)?,
    }
    Ok(if report.converged {
        0
    } else {
        EXIT_NOT_CONVERGED
    })
}

/// The terminal rendering of one verdict.
///
/// Generic over the sink so the shape a person reads can be asserted against a buffer
/// rather than only against a terminal. At-risk holdings are shown always; the reachable
/// ones only under `--all`, because a repository with three hundred branches answers the
/// question "is anything in danger" with a list nobody reads otherwise.
fn render<W: Write>(out: &mut W, report: &ConvergenceReport, all: bool) -> Result<()> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    for holding in report.holdings.iter().filter(|h| h.at_risk || all) {
        w(
            out,
            format!(
                "{:<4} {:<9} {:<44} {}",
                if holding.at_risk { "RISK" } else { "ok" },
                holding.disposition.as_str(),
                holding.identity,
                holding.evidence
            ),
        )?;
        if holding.at_risk {
            w(out, format!("     remedy: {}", holding.remedy))?;
        }
    }
    for (disposition, count) in &report.tallies {
        w(out, format!("{count:>5}  {disposition}"))?;
    }
    w(out, report.summary())
}
