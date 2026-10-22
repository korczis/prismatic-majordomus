//! `majordomus layout check`: the boundary between the layer's two halves, through the
//! registry's `layout.check` (I2871).
//!
//! The command renders the capability's answer and chooses the exit code from it; the
//! judgement and the verdict are the capability's, so the terminal, the `--format json`
//! document, MCP and HTTP cannot disagree about whether the boundary holds.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::cli::{LayoutArgs, LayoutCommand, OutputFormat};
use crate::error::{Error, Result};
use crate::layout::LayoutReport;

/// The exit code when a finding stands.
pub const EXIT_FINDINGS: u8 = 10;
/// The exit code when the halves could not be read: not a pass.
pub const EXIT_UNMEASURED: u8 = 12;

/// Run `majordomus layout`.
pub fn run(args: LayoutArgs) -> Result<u8> {
    let LayoutCommand::Check = args.command;
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let words = ["layout".to_string(), "check".to_string()];
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus layout check`".into(),
        })?;
    let value = ctx.execute(id, json!({})).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let report: LayoutReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("layout.check answered with something this command cannot read: {e}"),
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
        OutputFormat::Text => render(&mut out, &report)?,
    }
    Ok(exit_code(&report))
}

/// The exit code a report earns.
fn exit_code(report: &LayoutReport) -> u8 {
    if !report.measured {
        EXIT_UNMEASURED
    } else if report.passes {
        0
    } else {
        EXIT_FINDINGS
    }
}

/// One line per finding, then the verdict.
fn render(out: &mut impl Write, report: &LayoutReport) -> Result<()> {
    if !report.measured {
        writeln!(
            out,
            "layout check: not measured: {}",
            report.reason.as_deref().unwrap_or("no reason given")
        )
        .map_err(Error::Transport)?;
        return Ok(());
    }
    for f in &report.findings {
        writeln!(
            out,
            "FAIL {} violates {}: excludes {}; {}",
            f.path, f.contract, f.excludes, f.reason
        )
        .map_err(Error::Transport)?;
    }
    writeln!(
        out,
        "layout check: {} path(s) judged, {} finding(s)",
        report.checked,
        report.findings.len()
    )
    .map_err(Error::Transport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutFinding;

    fn finding() -> LayoutFinding {
        LayoutFinding {
            path: ".ai/local/rules/copied.md".into(),
            contract: "manifest local".into(),
            excludes: "canonical".into(),
            reason: "a copy of .ai/repo/rules/copied.md".into(),
        }
    }

    #[test]
    fn the_exit_code_is_the_verdict() {
        assert_eq!(exit_code(&LayoutReport::of(true, None, 2, Vec::new())), 0);
        assert_eq!(
            exit_code(&LayoutReport::of(true, None, 2, vec![finding()])),
            EXIT_FINDINGS
        );
        assert_eq!(
            exit_code(&LayoutReport::of(
                false,
                Some("no git".into()),
                0,
                Vec::new()
            )),
            EXIT_UNMEASURED
        );
    }

    #[test]
    fn a_finding_names_the_path_and_the_contract() {
        let mut out = Vec::new();
        render(&mut out, &LayoutReport::of(true, None, 2, vec![finding()])).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains(".ai/local/rules/copied.md violates manifest local: excludes canonical"),
            "{text}"
        );
        assert!(text.contains("2 path(s) judged, 1 finding(s)"), "{text}");
    }
}
