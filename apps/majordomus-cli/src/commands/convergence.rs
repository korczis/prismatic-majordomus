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
use crate::capability::Context;
use crate::cli::{ConvergenceArgs, OutputFormat};
use crate::convergence::ConvergenceReport;
use crate::error::{Error, Result};

/// The exit code when work is held where it can be lost. The code every unmet contract in
/// this executable uses, so a caller need not learn a second vocabulary for this one.
pub const EXIT_NOT_CONVERGED: u8 = 10;

/// Run `majordomus convergence`.
pub fn run(args: ConvergenceArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    answer(&app.context, args.format, args.all, &mut stdout.lock())
}

/// Ask `convergence.report` of `ctx`, write its answer to `out` in `format`, and say with the
/// exit code whether the repository converged.
fn answer<W: Write>(ctx: &Context, format: OutputFormat, all: bool, out: &mut W) -> Result<u8> {
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

    match format {
        // `{:#}` is serde_json's own pretty writer, the one `to_string_pretty` runs, so the
        // bytes are the same; serialising a `Value` cannot fail, so there is no fallback
        OutputFormat::Json => writeln!(out, "{value:#}").map_err(Error::Transport)?,
        OutputFormat::Text => render(out, &report, all)?,
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

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::capability::handler::CapabilityError;
    use crate::capability::model::{CliExposure, Exposure, Stability};
    use crate::capability::{builtin, CapabilityRegistry};
    use crate::cli::RepoArgs;
    use crate::convergence::{Disposition, Holding, HoldingKind};
    use crate::synthetic::SyntheticRepository;
    use crate::{capability, module};

    /// A sink that accepts `lines` complete lines and refuses every write after them, the
    /// way a closed pipe does.
    struct FailAfter {
        lines: usize,
        written: Vec<u8>,
    }

    impl Write for FailAfter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.lines == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "closed",
                ));
            }
            self.written.extend_from_slice(buf);
            self.lines -= buf.iter().filter(|b| **b == b'\n').count().min(self.lines);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .current_dir(dir)
            .args([
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?}");
    }

    fn holding(identity: &str, disposition: Disposition, at_risk: bool) -> Holding {
        Holding {
            kind: HoldingKind::Branch,
            identity: identity.into(),
            disposition,
            evidence: format!("evidence for {identity}"),
            remedy: if at_risk {
                format!("git push -u origin {identity}")
            } else {
                String::new()
            },
            at_risk,
        }
    }

    fn verdict() -> ConvergenceReport {
        ConvergenceReport {
            schema: crate::convergence::SCHEMA.into(),
            repository: "/r".into(),
            trunk: Some("master".into()),
            holdings: vec![
                holding("feature/local", Disposition::LocalOnly, true),
                holding("master", Disposition::Integrated, false),
            ],
            tallies: [("integrated".to_string(), 1), ("local_only".to_string(), 1)]
                .into_iter()
                .collect(),
            at_risk: 1,
            converged: false,
        }
    }

    /// What is at risk is printed with its remedy; what is reachable only under `--all`;
    /// then one line per disposition and the verdict's own summary.
    #[test]
    fn the_report_shows_what_is_at_risk_and_the_rest_only_when_asked() {
        let report = verdict();
        let mut out = Vec::new();
        render(&mut out, &report, false).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!(
                "RISK local_only {:<44} evidence for feature/local\n     remedy: git push -u origin feature/local\n    1  integrated\n    1  local_only\n{}\n",
                "feature/local",
                report.summary()
            )
        );

        let mut out = Vec::new();
        render(&mut out, &report, true).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains(&format!(
                "ok   integrated {:<44} evidence for master\n",
                "master"
            )),
            "{text}"
        );
        assert_eq!(
            text.matches("remedy:").count(),
            1,
            "only what is at risk has one"
        );
    }

    /// A sink that stops accepting is a transport error at whichever line it stopped, never
    /// a report cut short and called complete.
    #[test]
    fn a_sink_that_stops_accepting_is_an_error_at_every_line() {
        let report = verdict();
        // the at-risk row, its remedy, two tallies and the summary
        for lines in 0..5 {
            let mut out = FailAfter {
                lines,
                written: Vec::new(),
            };
            match render(&mut out, &report, false) {
                Err(Error::Transport(e)) => assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe),
                other => panic!("after {lines} line(s) the report answered {other:?}"),
            }
            assert_eq!(out.written.iter().filter(|b| **b == b'\n').count(), lines);
        }
        let mut out = FailAfter {
            lines: 5,
            written: Vec::new(),
        };
        render(&mut out, &report, false).expect("five lines fit");
    }

    /// Over a real git repository the exit code is the verdict: 0 once everything is
    /// committed on the trunk, 10 while a file is not, in either format; and a sink that
    /// refuses the answer is an error in either format.
    #[test]
    fn the_exit_code_is_the_verdict_and_both_formats_carry_it() {
        let repo = SyntheticRepository::small().unwrap();
        git(repo.root(), &["init", "-q", "-b", "master"]);
        git(repo.root(), &["add", "-A"]);
        git(repo.root(), &["commit", "-q", "-m", "base"]);
        let ctx = repo.context().unwrap();

        let mut out = Vec::new();
        assert_eq!(
            answer(&ctx, OutputFormat::Json, false, &mut out).unwrap(),
            0
        );
        let printed: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(printed["converged"], true);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!("{}\n", serde_json::to_string_pretty(&printed).unwrap()),
            "the JSON is serde_json's pretty rendering, byte for byte"
        );

        std::fs::write(repo.root().join("draft.txt"), "half written\n").unwrap();
        let mut out = Vec::new();
        assert_eq!(
            answer(&ctx, OutputFormat::Text, false, &mut out).unwrap(),
            EXIT_NOT_CONVERGED
        );
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("RISK uncommitted"), "{text}");
        assert!(
            text.trim_end().ends_with("exist on this disk only"),
            "{text}"
        );

        for format in [OutputFormat::Json, OutputFormat::Text] {
            let mut out = FailAfter {
                lines: 0,
                written: Vec::new(),
            };
            assert!(matches!(
                answer(&ctx, format, false, &mut out),
                Err(Error::Transport(_))
            ));
        }
    }

    /// A repository git cannot read is the capability's refusal, carried as a protocol
    /// error that names it; a path that is no repository at all fails before anything runs.
    #[test]
    fn a_refusal_is_carried_and_a_missing_repository_fails_first() {
        let repo = SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        match answer(&ctx, OutputFormat::Text, false, &mut Vec::new()) {
            Err(Error::Protocol { reason }) => {
                assert!(
                    reason.starts_with("convergence.report refused: "),
                    "{reason}"
                )
            }
            other => panic!("a repository with no git answered {other:?}"),
        }

        let nowhere = tempfile::tempdir().unwrap();
        let args = ConvergenceArgs {
            repo: RepoArgs {
                repo: Some(nowhere.path().to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Vcs,
                strict: false,
                share: None,
            },
            format: OutputFormat::Text,
            all: false,
        };
        assert!(run(args).is_err(), "no repository is not a verdict");
    }

    #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    struct Elsewhere {
        converged: String,
    }

    fn elsewhere(
        _: &crate::capability::Context,
        _: builtin::Empty,
    ) -> std::result::Result<Elsewhere, CapabilityError> {
        Ok(Elsewhere {
            converged: "perhaps".into(),
        })
    }

    /// A context whose registry holds `modules` over the synthetic repository's index.
    fn context_with(
        repo: &SyntheticRepository,
        modules: Vec<crate::capability::module::ModuleDescriptor>,
    ) -> crate::capability::Context {
        let index = repo.index().unwrap();
        let registry = CapabilityRegistry::builder()
            .with_modules(modules)
            .build()
            .expect("the registry builds");
        crate::capability::Context::new(Arc::new(index), Arc::new(registry))
    }

    /// The command line reaches only what a capability declares: a registry that exposes no
    /// `convergence` is named as such, and an answer of another shape is refused rather than
    /// rendered as a verdict it is not.
    #[test]
    fn an_absent_capability_and_an_unreadable_answer_are_protocol_errors() {
        let repo = SyntheticRepository::small().unwrap();

        let without: Vec<_> = builtin::modules()
            .into_iter()
            .filter(|m| m.id.as_str() != "convergence")
            .collect();
        match answer(
            &context_with(&repo, without),
            OutputFormat::Text,
            false,
            &mut Vec::new(),
        ) {
            Err(Error::Protocol { reason }) => assert_eq!(
                reason,
                "no capability is exposed as `majordomus convergence`"
            ),
            other => panic!("a registry with no convergence answered {other:?}"),
        }

        let impostor = module! {
            id: "convergence",
            title: "Convergence",
            description: "Answers in a shape the command cannot read.",
            stability: Stability::Experimental,
            capabilities: [
                capability! {
                    id: "convergence.report", title: "Elsewhere",
                    description: "A verdict that is not one.",
                    input: builtin::Empty, output: Elsewhere, stability: Stability::Experimental,
                    exposure: Exposure {
                        mcp: None,
                        http: None,
                        cli: Some(CliExposure { path: vec!["convergence".into()] }),
                    },
                    tags: [],
                    handler: elsewhere,
                },
            ],
        };
        match answer(
            &context_with(&repo, vec![impostor]),
            OutputFormat::Json,
            false,
            &mut Vec::new(),
        ) {
            Err(Error::Protocol { reason }) => assert!(
                reason
                    .starts_with("convergence.report answered something this command cannot read"),
                "{reason}"
            ),
            other => panic!("an unreadable answer was rendered: {other:?}"),
        }
    }
}
