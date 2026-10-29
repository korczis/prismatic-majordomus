//! `majordomus design licence`: the never-publish gate, through the registry's
//! `design.licence` (I4336 (a)).
//!
//! The command renders the capability's answer and chooses the exit code from it; the
//! judgement is the capability's, so the terminal, the `--format json` document and MCP
//! cannot disagree about whether a public tree holds a licensed file.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::capability::handler::CapabilityError;
use crate::cli::{DesignArgs, DesignCommand, OutputFormat};
use crate::design::licence::{LicenceReport, MatchKind, Verdict, Visibility};
use crate::error::{Error, Result};

/// The exit code when a licensed file is in a public tree.
pub const EXIT_REFUSED: u8 = 10;
/// The exit code when a public tree owes a judgement this machine cannot give: not a pass.
pub const EXIT_NOT_JUDGED: u8 = 12;
/// The exit code when the evidence file cannot be read as kit-evidence/v1.
pub const EXIT_EVIDENCE: u8 = 2;

/// The capability the command renders; its command-line exposure is `design licence`.
const CAPABILITY: &str = "design.licence";

/// Run `majordomus design`.
pub fn run(args: DesignArgs) -> Result<u8> {
    run_to(args, &mut std::io::stdout().lock())
}

/// Run `majordomus design`, printing to `out`.
fn run_to(args: DesignArgs, out: &mut dyn Write) -> Result<u8> {
    let DesignCommand::Licence(licence) = args.command else {
        return Err(Error::Protocol {
            reason: "design licence was handed another design subcommand".to_string(),
        });
    };
    let app = App::load(&args.repo)?;
    let input = match &licence.evidence {
        Some(path) => json!({ "evidence": path.to_string_lossy() }),
        None => json!({}),
    };
    let value = app.context.execute(CAPABILITY, input).map_err(refusal)?;
    let report: LicenceReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("{CAPABILITY} answered with something this command cannot read: {e}"),
        })?;
    if licence.json || matches!(args.format, OutputFormat::Json) {
        writeln!(out, "{value:#}").map_err(Error::Transport)?;
    } else {
        render(out, &report)?;
    }
    Ok(exit_code(&report))
}

/// The exit code when no answer could be made at all: a tree git cannot list. Distinct
/// from every verdict's, so a caller never reads it as one.
pub const EXIT_UNANSWERED: u8 = 13;

/// A capability error as the exit code its cause earns: evidence that cannot be read is a
/// usage error, anything else is no answer. A `ui:` block that cannot be used is not an
/// error here: the capability answers it `not_judged`, naming the entry, so the document
/// and the exit (12) say why to every consumer.
fn refusal(e: CapabilityError) -> Error {
    let (code, reason) = match e {
        CapabilityError::InvalidInput(m) => (EXIT_EVIDENCE, format!("design licence: {m}")),
        other => (EXIT_UNANSWERED, format!("design licence: {other}")),
    };
    Error::Refused { code, reason }
}

/// The exit code a report earns.
fn exit_code(report: &LicenceReport) -> u8 {
    match report.verdict {
        Verdict::Active | Verdict::Inactive => 0,
        Verdict::Refused => EXIT_REFUSED,
        Verdict::NotJudged => EXIT_NOT_JUDGED,
    }
}

/// One line per match, then the verdict.
fn render(out: &mut dyn Write, report: &LicenceReport) -> Result<()> {
    for m in &report.matches {
        let kind = match m.kind {
            MatchKind::Exact => "exact",
            MatchKind::Normalised => "normalised",
        };
        let source = m
            .source
            .as_deref()
            .unwrap_or("a licensed file the evidence does not name");
        writeln!(out, "FAIL {} holds {source} ({kind} copy)", m.path).map_err(Error::Transport)?;
    }
    let kit = if report.kit.is_empty() {
        "-"
    } else {
        report.kit.as_str()
    };
    let line = match report.verdict {
        Verdict::Active => format!(
            "design licence: active: kit {kit}, {} tree, {} file(s) against {} fingerprint(s)",
            match report.visibility {
                Visibility::Public => "public",
                Visibility::Private => "private",
            },
            report.files,
            report.fingerprints
        ),
        Verdict::Inactive => format!(
            "design licence: inactive: {}",
            report.reason.as_deref().unwrap_or("nothing to judge")
        ),
        Verdict::NotJudged => format!(
            "design licence: not judged: {}",
            report.reason.as_deref().unwrap_or("no evidence")
        ),
        Verdict::Refused => format!(
            "design licence: refused: {} licensed file(s) in a public tree (kit {kit})",
            report.matches.len()
        ),
    };
    writeln!(out, "{line}").map_err(Error::Transport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::licence::Match;

    fn report(verdict: Verdict) -> LicenceReport {
        LicenceReport {
            verdict,
            fails: matches!(verdict, Verdict::Refused | Verdict::NotJudged),
            kit: "examplekit".into(),
            visibility: Visibility::Public,
            has_ui: true,
            reason: None,
            evidence: Vec::new(),
            fingerprints: 1,
            files: 3,
            matches: Vec::new(),
        }
    }

    #[test]
    fn the_exit_code_is_the_verdict() {
        assert_eq!(exit_code(&report(Verdict::Active)), 0);
        assert_eq!(exit_code(&report(Verdict::Inactive)), 0);
        assert_eq!(exit_code(&report(Verdict::Refused)), EXIT_REFUSED);
        assert_eq!(exit_code(&report(Verdict::NotJudged)), EXIT_NOT_JUDGED);
    }

    #[test]
    fn a_refusal_names_the_file_and_the_source() {
        let mut r = report(Verdict::Refused);
        r.matches.push(Match {
            path: "web/shell.html".into(),
            source: Some("blocks/shell.html".into()),
            kind: MatchKind::Normalised,
        });
        let mut out = Vec::new();
        render(&mut out, &r).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("FAIL web/shell.html holds blocks/shell.html (normalised copy)"));
        assert!(text.contains("refused: 1 licensed file(s)"), "{text}");
    }

    #[test]
    fn unreadable_evidence_is_a_usage_error() {
        let e = refusal(CapabilityError::InvalidInput("ev.tsv: line 1: bad".into()));
        assert_eq!(e.exit_code(), EXIT_EVIDENCE);
        let e = refusal(CapabilityError::Refused("anything".into()));
        assert_ne!(e.exit_code(), EXIT_REFUSED);
    }
}
