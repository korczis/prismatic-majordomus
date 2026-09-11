//! `majordomus landing`: what is preventing this repository from being landed, through the
//! registry's `landing.closure`.
//!
//! The command renders and nothing else. Every stage, its verdict, the evidence behind it
//! and the remedy are decided inside the capability, out of other capabilities' own
//! answers, so this report, the `--format json` document, the HTTP response, the MCP
//! resource and the Cockpit page are renderings of one execution. A verdict that differed
//! by surface would be exactly the thing a landing closure exists to make impossible.
//!
//! The exit code is the verdict: `0` when the repository is landed, [`EXIT_NOT_LANDED`]
//! when any stage refuses or goes unanswered. That is what lets a script ask the question
//! and what makes "not landed" an outcome a caller cannot ignore.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::capability::builtin::landing::LandingClosure;
use crate::capability::CapabilityError;
use crate::cli::{LandingArgs, OutputFormat};
use crate::error::{Error, Result};
use crate::gates::GateStatus;

/// The exit code when the repository is not landed: the code every unmet contract in this
/// executable uses.
pub const EXIT_NOT_LANDED: u8 = 10;

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}

/// Run `majordomus landing`.
pub fn run(args: LandingArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = ctx
        .registry
        .by_cli(&["landing".to_string()])
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus landing`".into(),
        })?;
    let value = ctx.execute(id, json!({})).map_err(map)?;
    let closure: LandingClosure =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("landing.closure answered something this command cannot read: {e}"),
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
        OutputFormat::Text => render(&mut out, &closure, args.all)?,
    }
    Ok(if closure.landed { 0 } else { EXIT_NOT_LANDED })
}

/// The word a reader scans the left margin for. Six letters at most, so the stage names
/// line up whatever the verdict is.
fn word(status: GateStatus) -> &'static str {
    match status {
        GateStatus::Pass => "OK",
        GateStatus::Fail => "FAIL",
        GateStatus::Stale => "STALE",
        GateStatus::Blocked => "BLOCK",
        GateStatus::Queued => "OWED",
        GateStatus::Exempt => "n/a",
        GateStatus::Unknown => "?",
    }
}

/// The terminal rendering of one closure.
///
/// Generic over the sink, so the shape a person reads is asserted against a buffer rather
/// than only against a terminal.
fn render<W: Write>(out: &mut W, c: &LandingClosure, all: bool) -> Result<()> {
    let w = |out: &mut W, s: String| writeln!(out, "{s}").map_err(Error::Transport);

    w(out, String::new())?;
    w(
        out,
        format!(
            "  head      {}{}",
            c.head.as_deref().unwrap_or("(no commit)"),
            c.branch
                .as_deref()
                .map(|b| format!(" on {b}"))
                .unwrap_or_default()
        ),
    )?;
    w(
        out,
        format!("  trunk     {}", c.trunk.as_deref().unwrap_or("(unknown)")),
    )?;
    w(out, format!("  verdict   {}", c.verdict))?;
    w(out, String::new())?;

    for s in &c.stages {
        w(
            out,
            format!("  {:<5} {:<15} {}", word(s.status), s.id, s.evidence),
        )?;
        // the findings are the point of a report that must not read as a generic refusal:
        // a passing stage has nothing to show and a refusing one shows what it read
        let shown = if all {
            s.findings.len()
        } else {
            5.min(s.findings.len())
        };
        for f in s.findings.iter().take(shown) {
            w(out, format!("          - {f}"))?;
        }
        if s.findings.len() > shown {
            w(
                out,
                format!(
                    "          - … {} more; --all shows them",
                    s.findings.len() - shown
                ),
            )?;
        }
        if s.status.refuses() || s.status.unverified() {
            w(out, format!("          → {}", s.remediation))?;
            w(out, format!("            decided by {}", s.source))?;
        }
    }
    w(out, String::new())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin::landing::LandingStage;
    use std::collections::BTreeMap;

    fn closure(stages: Vec<LandingStage>, landed: bool) -> LandingClosure {
        LandingClosure {
            schema: "majordomus/landing-closure/v1".into(),
            head: Some("abc1234".into()),
            branch: Some("master".into()),
            trunk: Some("master".into()),
            landed,
            verdict: if landed {
                "LANDED".into()
            } else {
                "NOT LANDED".into()
            },
            stages,
            tallies: BTreeMap::new(),
            blocking: Vec::new(),
            unverified: Vec::new(),
            at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn stage(id: &str, status: GateStatus, findings: Vec<&str>) -> LandingStage {
        LandingStage {
            id: id.into(),
            question: "?".into(),
            status,
            evidence: format!("{id} evidence"),
            source: format!("{id}.source"),
            remediation: format!("run {id}"),
            findings: findings.into_iter().map(String::from).collect(),
        }
    }

    fn rendered(c: &LandingClosure, all: bool) -> String {
        let mut buf = Vec::new();
        render(&mut buf, c, all).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn a_refusing_stage_prints_what_it_read_and_what_to_do() {
        let out = rendered(
            &closure(
                vec![stage("commit", GateStatus::Fail, vec!["/x — 3 unstaged"])],
                false,
            ),
            false,
        );
        assert!(out.contains("FAIL"), "{out}");
        assert!(
            out.contains("/x — 3 unstaged"),
            "the finding, not a summary: {out}"
        );
        assert!(out.contains("→ run commit"), "the remedy is named: {out}");
        assert!(
            out.contains("decided by commit.source"),
            "the owner is named: {out}"
        );
    }

    #[test]
    fn a_passing_stage_is_one_line_and_asks_for_nothing() {
        let out = rendered(
            &closure(vec![stage("ci", GateStatus::Pass, vec![])], true),
            false,
        );
        assert!(out.contains("OK    ci"), "{out}");
        assert!(
            !out.contains("→"),
            "nothing to do, so nothing is suggested: {out}"
        );
    }

    #[test]
    fn the_findings_are_capped_and_the_cap_says_so() {
        let many: Vec<&str> = vec!["a", "b", "c", "d", "e", "f", "g"];
        let c = closure(vec![stage("push", GateStatus::Fail, many)], false);
        let short = rendered(&c, false);
        assert!(short.contains("2 more; --all shows them"), "{short}");
        let long = rendered(&c, true);
        assert!(
            !long.contains("more; --all"),
            "--all shows them all: {long}"
        );
        assert!(long.contains("- g"), "{long}");
    }
}
