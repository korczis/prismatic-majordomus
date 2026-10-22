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

/// The capability the command renders; its command-line exposure is `layout check` (the
/// module's own example holds the two together).
const CAPABILITY: &str = "layout.check";

/// Run `majordomus layout`.
pub fn run(args: LayoutArgs) -> Result<u8> {
    run_to(args, &mut std::io::stdout().lock())
}

/// Run `majordomus layout`, printing to `out`.
fn run_to(args: LayoutArgs, out: &mut dyn Write) -> Result<u8> {
    let LayoutCommand::Check = args.command;
    let app = App::load(&args.repo)?;
    let (value, report) = app
        .context
        .execute(CAPABILITY, json!({}))
        .map_err(|e| e.to_string())
        .and_then(read)
        .map_err(|reason| Error::Protocol { reason })?;
    match args.format {
        OutputFormat::Json => writeln!(out, "{value:#}").map_err(Error::Transport)?,
        OutputFormat::Text => render(out, &report)?,
    }
    Ok(exit_code(&report))
}

/// The capability's answer as the report it renders, kept beside the answer itself.
fn read(
    value: serde_json::Value,
) -> std::result::Result<(serde_json::Value, LayoutReport), String> {
    serde_json::from_value(value.clone())
        .map(|report| (value, report))
        .map_err(unreadable)
}

/// Why an answer could not be read as a report.
fn unreadable(e: serde_json::Error) -> String {
    format!("{CAPABILITY} answered with something this command cannot read: {e}")
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
fn render(out: &mut dyn Write, report: &LayoutReport) -> Result<()> {
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
    use crate::cli::{DiscoveryMode, RepoArgs};
    use crate::layout::LayoutFinding;
    use crate::synthetic::{crate_share, SyntheticRepository};

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

    /// A writer that refuses every write, as a closed pipe does.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn args(root: &std::path::Path, format: OutputFormat) -> LayoutArgs {
        LayoutArgs {
            repo: RepoArgs {
                repo: Some(root.to_path_buf()),
                discovery: DiscoveryMode::Vcs,
                strict: false,
                share: Some(crate_share()),
            },
            command: LayoutCommand::Check,
            format,
        }
    }

    /// A synthetic layer whose tracked half is in git's index.
    fn layer() -> SyntheticRepository {
        let repo = SyntheticRepository::small().unwrap();
        for args in [&["init", "-q"][..], &["add", ".ai"][..]] {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(repo.root())
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {}", args.join(" "));
        }
        repo
    }

    /// A report nobody could measure says so, with its reason or the admission that none
    /// was given, and states no count that could read as a clean tree.
    #[test]
    fn an_unmeasured_report_says_why_and_counts_nothing() {
        let mut out = Vec::new();
        render(
            &mut out,
            &LayoutReport::of(false, Some("git could not start".into()), 0, Vec::new()),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "layout check: not measured: git could not start\n");

        let mut out = Vec::new();
        render(&mut out, &LayoutReport::of(false, None, 0, Vec::new())).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "layout check: not measured: no reason given\n");
    }

    /// An output that cannot be written is a transport error, whether the report was
    /// measured or not, rather than a verdict silently lost.
    #[test]
    fn an_output_that_cannot_be_written_is_a_transport_error() {
        let found = LayoutReport::of(true, None, 2, vec![finding()]);
        assert!(matches!(
            render(&mut Closed, &found),
            Err(Error::Transport(_))
        ));
        let unread = LayoutReport::of(false, None, 0, Vec::new());
        assert!(matches!(
            render(&mut Closed, &unread),
            Err(Error::Transport(_))
        ));
    }

    /// A directory that holds no layer is refused before anything is judged.
    #[test]
    fn a_directory_without_a_layer_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(run(args(dir.path(), OutputFormat::Text)).is_err());
    }

    /// The text rendering of a clean layer exits with the passing code, the verdict the
    /// capability gave.
    #[cfg(unix)]
    #[test]
    fn a_clean_layer_rendered_as_text_exits_zero() {
        let repo = layer();
        std::os::unix::fs::symlink(crate_share(), repo.root().join("share")).unwrap();
        assert_eq!(run(args(repo.root(), OutputFormat::Text)).unwrap(), 0);

        // the JSON form is the capability's answer whole, and a closed output is a transport
        // error in either form rather than a verdict silently lost
        let mut out = Vec::new();
        assert_eq!(
            run_to(args(repo.root(), OutputFormat::Json), &mut out).unwrap(),
            0
        );
        let answer: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(answer["passes"], true, "{answer}");
        for format in [OutputFormat::Json, OutputFormat::Text] {
            assert!(matches!(
                run_to(args(repo.root(), format), &mut Closed),
                Err(Error::Transport(_))
            ));
        }
    }

    /// An answer that is not a report is refused naming the capability that gave it.
    #[test]
    fn an_answer_that_is_not_a_report_is_refused_by_name() {
        let reason = read(json!({"passes": "maybe"})).unwrap_err();
        assert!(
            reason.starts_with("layout.check answered with something this command cannot read"),
            "{reason}"
        );
    }

    /// When the capability itself fails — here, the repository's own share holds kinds
    /// that do not parse — the command refuses with the capability's reason.
    #[test]
    fn a_failing_capability_is_refused_with_its_reason() {
        if std::env::var_os("MAJORDOMUS_SHARE").is_some() {
            return; // the capability would read that share, not the repository's own
        }
        let repo = layer();
        std::fs::create_dir_all(repo.root().join("share")).unwrap();
        std::fs::write(repo.root().join("share/kinds.yaml"), "kinds: [unclosed\n").unwrap();

        let err = run(args(repo.root(), OutputFormat::Text)).unwrap_err();
        assert!(
            matches!(&err, Error::Protocol { reason } if reason.contains("kinds could not be read")),
            "{err:?}"
        );
    }
}
