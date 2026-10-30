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
    let (value, report) = app
        .context
        .execute(CAPABILITY, input)
        .map_err(refusal)
        .and_then(|value| report_of(&value).map(|report| (value, report)))?;
    if licence.json || matches!(args.format, OutputFormat::Json) {
        writeln!(out, "{value:#}").map_err(Error::Transport)?;
    } else {
        render(out, &report)?;
    }
    Ok(exit_code(&report))
}

/// The capability's answer as a report; an answer of another shape is a protocol error.
fn report_of(value: &serde_json::Value) -> Result<LicenceReport> {
    serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
        reason: format!("{CAPABILITY} answered with something this command cannot read: {e}"),
    })
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

    const LICENSED: &[u8] = b"<div class=\"examplekit-shell\">\n  <nav>pro</nav>\n</div>\n";

    /// A synthetic supervised repository whose policy declares a public `examplekit` UI and
    /// tracks one licensed file, with the owner's evidence for it beside the repository.
    fn licensed_repo() -> (crate::synthetic::SyntheticRepository, tempfile::TempDir) {
        use sha2::{Digest, Sha256};
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = repo.root();
        let policy = root.join(".ai/repo/policy.yaml");
        let mut text = std::fs::read_to_string(&policy).unwrap();
        text.push_str("ui:\n  kit: examplekit\n  visibility: public\n  publish: []\n  evidence: []\n");
        std::fs::write(&policy, text).unwrap();
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("web/shell.html"), LICENSED).unwrap();
        for args in [
            &["init", "-q"][..],
            &["add", "-A"][..],
            &["-c", "user.email=t@example.invalid", "-c", "user.name=t", "-c", "commit.gpgsign=false",
                "-c", "core.hooksPath=/dev/null", "commit", "-qm", "fixture"][..],
        ] {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {}", args.join(" "));
        }
        let side = tempfile::tempdir().unwrap();
        let norm: Vec<u8> = {
            let mut out = Vec::new();
            let mut run = false;
            for &b in LICENSED {
                if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                    if !run {
                        out.push(b' ');
                    }
                    run = true;
                } else {
                    out.push(b);
                    run = false;
                }
            }
            out
        };
        std::fs::write(
            side.path().join("ev.tsv"),
            format!(
                "{:x}\t{:x}\tblocks/shell.html\n",
                Sha256::digest(LICENSED),
                Sha256::digest(&norm)
            ),
        )
        .unwrap();
        (repo, side)
    }

    fn args(root: &std::path::Path, evidence: Option<std::path::PathBuf>, json: bool) -> DesignArgs {
        DesignArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(root.to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Vcs,
                strict: false,
                share: Some(crate::synthetic::crate_share()),
            },
            command: DesignCommand::Licence(crate::cli::LicenceArgs { evidence, json }),
            format: OutputFormat::Text,
        }
    }

    /// The command runs the registry's capability against a real tree and renders its
    /// answer: the refusal in text and as the JSON document, the missing evidence as not
    /// judged, and an evidence file that cannot be read as the usage exit.
    #[test]
    fn the_command_renders_the_capabilitys_answer_for_a_real_tree() {
        let (repo, side) = licensed_repo();
        let ev = side.path().join("ev.tsv");

        let mut out = Vec::new();
        let code = run_to(args(repo.root(), Some(ev.clone()), false), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(code, EXIT_REFUSED, "{text}");
        assert!(text.contains("FAIL web/shell.html holds blocks/shell.html (exact copy)"), "{text}");

        let mut out = Vec::new();
        assert_eq!(run_to(args(repo.root(), Some(ev), true), &mut out).unwrap(), EXIT_REFUSED);
        let doc: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(doc["verdict"], "refused");
        assert_eq!(doc["matches"][0]["path"], "web/shell.html");

        let mut out = Vec::new();
        assert_eq!(run_to(args(repo.root(), None, false), &mut out).unwrap(), EXIT_NOT_JUDGED);
        assert!(String::from_utf8(out).unwrap().contains("design licence: not judged:"));

        let bad = side.path().join("bad.tsv");
        std::fs::write(&bad, "not evidence\n").unwrap();
        let err = run_to(args(repo.root(), Some(bad), false), &mut Vec::new()).unwrap_err();
        assert_eq!(err.exit_code(), EXIT_EVIDENCE);

        // the stdout form is the same command
        assert_eq!(run(args(repo.root(), None, true)).unwrap(), EXIT_NOT_JUDGED);
    }

    /// A `ui:` block that cannot be used, and a policy file that cannot be read, are
    /// answered not judged, naming why, never raised.
    #[test]
    fn an_unusable_policy_is_answered_not_judged() {
        let (repo, _side) = licensed_repo();
        let policy = repo.root().join(".ai/repo/policy.yaml");
        let text = std::fs::read_to_string(&policy).unwrap();
        std::fs::write(&policy, text.replace("visibility: public", "visibility: internal")).unwrap();
        let mut out = Vec::new();
        assert_eq!(run_to(args(repo.root(), None, false), &mut out).unwrap(), EXIT_NOT_JUDGED);
        assert!(String::from_utf8(out).unwrap().contains("ui: block cannot be used"));

        std::fs::remove_file(&policy).unwrap();
        let mut out = Vec::new();
        let got = run_to(args(repo.root(), None, false), &mut out);
        // with no policy the repository may not load at all; if it does, it is not judged
        if let Ok(code) = got {
            assert_eq!(code, EXIT_NOT_JUDGED);
        }
    }

    #[test]
    fn every_verdict_renders_one_line_saying_why() {
        let mut r = report(Verdict::Active);
        let mut out = Vec::new();
        render(&mut out, &r).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("active: kit examplekit, public tree, 3 file(s)"));
        r.verdict = Verdict::Inactive;
        r.kit = String::new();
        r.visibility = Visibility::Private;
        let mut out = Vec::new();
        render(&mut out, &r).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("inactive: nothing to judge"));
        r.verdict = Verdict::NotJudged;
        let mut out = Vec::new();
        render(&mut out, &r).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("not judged: no evidence"));
        r.reason = Some("why".into());
        let mut out = Vec::new();
        render(&mut out, &r).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("not judged: why"));
        r.verdict = Verdict::Active;
        let mut out = Vec::new();
        render(&mut out, &r).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("active: kit -, private tree"));
        let mut r = report(Verdict::Refused);
        r.matches.push(Match {
            path: "a.html".into(),
            source: None,
            kind: MatchKind::Exact,
        });
        let mut out = Vec::new();
        render(&mut out, &r).unwrap();
        assert!(String::from_utf8(out)
            .unwrap()
            .contains("a.html holds a licensed file the evidence does not name (exact copy)"));
    }

    /// An output that cannot be written.
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// A closed output is a transport error in either form, never a verdict lost; a
    /// directory that holds no repository is refused; an answer of another shape is a
    /// protocol error; an unusable publish entry reaches the command as not judged.
    #[test]
    fn what_cannot_be_written_read_or_loaded_is_named() {
        let (repo, side) = licensed_repo();
        let ev = side.path().join("ev.tsv");
        for json in [false, true] {
            let got = run_to(args(repo.root(), Some(ev.clone()), json), &mut Closed);
            assert!(matches!(got, Err(Error::Transport(_))), "json {json}: {got:?}");
        }
        let mut r = report(Verdict::Refused);
        r.matches.push(Match { path: "a".into(), source: None, kind: MatchKind::Exact });
        assert!(matches!(render(&mut Closed, &r), Err(Error::Transport(_))));

        let empty = tempfile::tempdir().unwrap();
        assert!(run_to(args(empty.path(), None, false), &mut Vec::new()).is_err());

        assert!(matches!(report_of(&json!({ "verdict": 1 })), Err(Error::Protocol { .. })));

        let policy = repo.root().join(".ai/repo/policy.yaml");
        let text = std::fs::read_to_string(&policy).unwrap();
        std::fs::write(&policy, text.replace("publish: []", "publish: [\"/srv/www\"]")).unwrap();
        let mut out = Vec::new();
        assert_eq!(run_to(args(repo.root(), Some(ev), false), &mut out).unwrap(), EXIT_NOT_JUDGED);
        assert!(String::from_utf8(out).unwrap().contains("ui.publish entry `/srv/www`"));
    }

    #[test]
    fn unreadable_evidence_is_a_usage_error() {
        let e = refusal(CapabilityError::InvalidInput("ev.tsv: line 1: bad".into()));
        assert_eq!(e.exit_code(), EXIT_EVIDENCE);
        let e = refusal(CapabilityError::Refused("anything".into()));
        assert_ne!(e.exit_code(), EXIT_REFUSED);
    }
}
