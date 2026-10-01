//! `majordomus shell check`: the repository's shell automation, through the registry's
//! `shell.check`.
//!
//! The command renders the capability's answer and chooses the exit code from it; the
//! enumeration, the judgement and the verdict are the capability's, so the terminal, the
//! `--format json` document, MCP and HTTP cannot disagree about whether the gate passed.
//!
//! `--canonical` is the one thing this command does that the capability does not: it
//! rewrites the inventory with its records in canonical order. Writing is not something a
//! projection of the registry offers, so it is a flag a person passes, like
//! `quality report --write-baseline`.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::automation::{canonical_text, ShellReport, INVENTORY};
use crate::cli::{OutputFormat, ShellArgs, ShellCommand};
use crate::error::{Error, Result};

/// The exit code when a finding stands.
pub const EXIT_FINDINGS: u8 = 10;
/// The exit code when the tree could not be measured: not a pass.
pub const EXIT_UNMEASURED: u8 = 12;

/// Run `majordomus shell`.
pub fn run(args: ShellArgs) -> Result<u8> {
    let ShellCommand::Check { canonical } = args.command;
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    if canonical {
        rewrite(std::path::Path::new(&ctx.index.repository.root))?;
    }
    let words = ["shell".to_string(), "check".to_string()];
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus shell check`".into(),
        })?;
    let value = ctx.execute(id, json!({})).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })?;
    let report: ShellReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("shell.check answered with something this command cannot read: {e}"),
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
fn exit_code(report: &ShellReport) -> u8 {
    if !report.measured {
        EXIT_UNMEASURED
    } else if report.passes {
        0
    } else {
        EXIT_FINDINGS
    }
}

/// Rewrite the inventory in canonical order; a file with a line that is not a record is
/// left alone and the check that follows names the line.
fn rewrite(root: &std::path::Path) -> Result<()> {
    let path = root.join(INVENTORY);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    if let Some(ordered) = canonical_text(&text) {
        if ordered != text {
            std::fs::write(&path, ordered).map_err(Error::Transport)?;
        }
    }
    Ok(())
}

/// The terminal rendering of one report: every finding with its remedy, then the verdict
/// with the population it is about.
fn render<W: Write>(out: &mut W, report: &ShellReport) -> Result<()> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);
    if !report.measured {
        return w(
            out,
            format!(
                "shell check: not measured: {}",
                report.reason.as_deref().unwrap_or("no reason given")
            ),
        );
    }
    for f in &report.findings {
        let line = f.line.map(|l| format!(":{l}")).unwrap_or_default();
        w(
            out,
            format!("FAIL  {}  {}{line}  {}", f.code, f.path, f.message),
        )?;
        w(out, format!("      remedy: {}", f.remedy))?;
    }
    let by: Vec<String> = report
        .exemptions
        .iter()
        .map(|(d, n)| format!("{d} {n}"))
        .collect();
    w(
        out,
        format!(
            "shell units {} under {}; {} record(s) in {}; exemptions by disposition: {}",
            report.units.len(),
            report.governed.join(" "),
            report.records,
            report.inventory,
            if by.is_empty() {
                "none".to_string()
            } else {
                by.join(", ")
            }
        ),
    )?;
    if report.passes {
        w(out, "shell check: clean".into())
    } else {
        w(
            out,
            format!("shell check: {} finding(s)", report.findings.len()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::{ShellFinding, GOVERNED};

    fn report(findings: Vec<ShellFinding>, measured: bool) -> ShellReport {
        ShellReport {
            measured,
            reason: (!measured).then(|| "git is absent".to_string()),
            inventory: INVENTORY.into(),
            governed: GOVERNED.iter().map(|g| g.to_string()).collect(),
            units: Vec::new(),
            records: 0,
            exemptions: [("A".to_string(), 2)].into_iter().collect(),
            passes: measured && findings.is_empty(),
            findings,
        }
    }

    fn text(r: &ShellReport) -> String {
        let mut buf = Vec::new();
        render(&mut buf, r).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn the_rendering_names_the_file_the_remedy_and_the_verdict() {
        let finding = ShellFinding {
            code: "shell.undeclared".into(),
            path: "scripts/foo".into(),
            line: None,
            message: "scripts/foo is a bash unit the inventory does not declare".into(),
            remedy: "a scripted capability".into(),
        };
        let failing = report(vec![finding], true);
        let t = text(&failing);
        assert!(t.contains("FAIL  shell.undeclared  scripts/foo"), "{t}");
        assert!(t.contains("remedy: a scripted capability"), "{t}");
        assert!(t.contains("exemptions by disposition: A 2"), "{t}");
        assert!(t.ends_with("shell check: 1 finding(s)\n"), "{t}");
        assert_eq!(exit_code(&failing), EXIT_FINDINGS);

        let clean = report(Vec::new(), true);
        assert!(text(&clean).ends_with("shell check: clean\n"));
        assert_eq!(exit_code(&clean), 0);

        let unmeasured = report(Vec::new(), false);
        assert!(text(&unmeasured).contains("not measured: git is absent"));
        assert_eq!(
            exit_code(&unmeasured),
            EXIT_UNMEASURED,
            "unmeasured is not a pass"
        );
    }

    #[test]
    fn canonical_rewrites_only_a_readable_inventory() {
        let dir = tempfile::tempdir().unwrap();
        rewrite(dir.path()).expect("no inventory is nothing to rewrite");
        std::fs::create_dir_all(dir.path().join(".ai/repo/automation")).unwrap();
        let path = dir.path().join(INVENTORY);
        let b = r#"{"unit":"b","disposition":"A","reason":"r"}"#;
        let a = r#"{"unit":"a","disposition":"A","reason":"r"}"#;
        std::fs::write(&path, format!("{b}\n{a}\n")).unwrap();
        rewrite(dir.path()).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            format!("{a}\n{b}\n")
        );
        std::fs::write(&path, "{broken\n").unwrap();
        rewrite(dir.path()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{broken\n");
    }
}
