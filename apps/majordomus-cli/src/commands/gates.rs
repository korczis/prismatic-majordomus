//! `majordomus gates headroom`: whether every structure job fits in half its bound, through
//! the registry's `gates.headroom` (I4381).
//!
//! The command renders the capability's answer and chooses the exit code from it; the
//! judgement is the capability's, so the terminal, the `--format json` document, MCP and HTTP
//! cannot disagree about whether a job has headroom. The CI model runs it as the gate
//! `job-headroom`.

use std::io::Write;

use serde_json::json;

use crate::app::App;
use crate::cli::{GatesArgs, GatesCommand, OutputFormat};
use crate::error::{Error, Result};
use crate::gates::{HeadroomReport, HeadroomVerdict};

/// The exit code when a structure job is past half its bound.
pub const EXIT_EXCEEDS: u8 = 10;
/// The exit code when a job, its bound or the window could not be measured: not a pass.
pub const EXIT_UNMEASURED: u8 = 12;

/// The capability the command renders; its command-line exposure is `gates headroom`.
const CAPABILITY: &str = "gates.headroom";

/// Run `majordomus gates`.
pub fn run(args: GatesArgs) -> Result<u8> {
    run_to(args, &mut std::io::stdout().lock())
}

/// Run `majordomus gates`, printing to `out`.
fn run_to(args: GatesArgs, out: &mut dyn Write) -> Result<u8> {
    let GatesCommand::Headroom = args.command;
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
) -> std::result::Result<(serde_json::Value, HeadroomReport), String> {
    serde_json::from_value(value.clone())
        .map(|report| (value, report))
        .map_err(|e| format!("{CAPABILITY} answered with something this command cannot read: {e}"))
}

/// The exit code a report earns: a job past its share outranks one not measured, because it
/// names a measured regression.
fn exit_code(report: &HeadroomReport) -> u8 {
    if report.ok {
        0
    } else if report
        .jobs
        .iter()
        .any(|j| j.verdict == HeadroomVerdict::Exceeds)
    {
        EXIT_EXCEEDS
    } else {
        EXIT_UNMEASURED
    }
}

/// One line per job, one per problem, then the verdict.
fn render(out: &mut dyn Write, report: &HeadroomReport) -> Result<()> {
    for job in &report.jobs {
        let word = match job.verdict {
            HeadroomVerdict::Fits => "fits",
            HeadroomVerdict::Exceeds => "EXCEEDS",
            HeadroomVerdict::Unmeasured => "UNMEASURED",
        };
        writeln!(out, "{word:<10} {}", job.reason).map_err(Error::Transport)?;
    }
    for problem in &report.problems {
        writeln!(out, "problem    {problem}").map_err(Error::Transport)?;
    }
    writeln!(
        out,
        "gates headroom: {} structure job(s), each p90 at most {}% of its bound: {}",
        report.jobs.len(),
        report.percent,
        if report.ok { "ok" } else { "refused" }
    )
    .map_err(Error::Transport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{DiscoveryMode, RepoArgs};
    use crate::gates::{GateCost, JobHeadroom};
    use crate::synthetic::{crate_share, SyntheticRepository};

    fn job(verdict: HeadroomVerdict) -> JobHeadroom {
        JobHeadroom {
            job: "structure".into(),
            bound_seconds: Some(3300),
            limit_seconds: Some(1650),
            runs: 3,
            p90_seconds: Some(1683),
            used_percent: Some(51),
            verdict,
            reason: "job structure: p90 1683 s over 3 run(s) is 51% of its 3300 s bound".into(),
            costliest_gates: vec![GateCost {
                gate: "shell-lint".into(),
                seconds: 1388,
            }],
        }
    }

    fn report(ok: bool, jobs: Vec<JobHeadroom>, problems: Vec<String>) -> HeadroomReport {
        HeadroomReport {
            ok,
            percent: 50,
            window: ".ai/repo/ci/structure-job-durations.tsv".into(),
            jobs,
            problems,
        }
    }

    #[test]
    fn the_exit_code_is_the_verdict() {
        assert_eq!(
            exit_code(&report(true, vec![job(HeadroomVerdict::Fits)], vec![])),
            0
        );
        assert_eq!(
            exit_code(&report(
                false,
                vec![
                    job(HeadroomVerdict::Exceeds),
                    job(HeadroomVerdict::Unmeasured)
                ],
                vec![]
            )),
            EXIT_EXCEEDS
        );
        assert_eq!(
            exit_code(&report(
                false,
                vec![job(HeadroomVerdict::Unmeasured)],
                vec![]
            )),
            EXIT_UNMEASURED
        );
        assert_eq!(
            exit_code(&report(false, vec![], vec!["no window".into()])),
            EXIT_UNMEASURED
        );
    }

    #[test]
    fn the_text_names_each_job_each_problem_and_the_verdict() {
        let mut out = Vec::new();
        render(
            &mut out,
            &report(
                false,
                vec![job(HeadroomVerdict::Exceeds)],
                vec!["a line".into()],
            ),
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.starts_with("EXCEEDS    job structure: p90 1683 s"),
            "{text}"
        );
        assert!(text.contains("problem    a line"), "{text}");
        assert!(
            text.ends_with("each p90 at most 50% of its bound: refused\n"),
            "{text}"
        );
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

    #[test]
    fn an_output_that_cannot_be_written_is_a_transport_error() {
        for r in [
            report(true, vec![job(HeadroomVerdict::Fits)], vec![]),
            report(false, vec![], vec!["p".into()]),
        ] {
            assert!(matches!(render(&mut Closed, &r), Err(Error::Transport(_))));
        }
    }

    #[test]
    fn an_answer_that_is_not_a_report_is_refused_by_name() {
        let reason = read(json!({"ok": "maybe"})).unwrap_err();
        assert!(
            reason.starts_with("gates.headroom answered with something this command cannot read"),
            "{reason}"
        );
    }

    fn args(root: &std::path::Path, format: OutputFormat) -> GatesArgs {
        GatesArgs {
            repo: RepoArgs {
                repo: Some(root.to_path_buf()),
                // the synthetic layer is not a git repository: walk it
                discovery: DiscoveryMode::Filesystem,
                strict: false,
                share: Some(crate_share()),
            },
            command: GatesCommand::Headroom,
            format,
        }
    }

    #[test]
    fn a_directory_without_a_layer_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(run(args(dir.path(), OutputFormat::Text)).is_err());
    }

    /// Over a synthetic layer: a window inside half the bound exits 0 in both forms, one past
    /// it exits 10, and a removed window exits 12.
    #[test]
    fn the_command_exits_with_the_capabilitys_verdict() {
        let repo = SyntheticRepository::small().unwrap();
        let root = repo.root().to_path_buf();
        std::fs::create_dir_all(root.join(".ai/repo/ci")).unwrap();
        std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
        std::fs::write(
            root.join(".ai/repo/ci/gates.yaml"),
            "version: 1\ngates:\n  - id: lint\n    job: structure\n    runs: \"true\"\n    \
             summary: lints\nclasses: []\n",
        )
        .unwrap();
        std::fs::write(
            root.join(".github/workflows/validate.yml"),
            "jobs:\n  structure:\n    timeout-minutes: 55\n",
        )
        .unwrap();
        let window = root.join(".ai/repo/ci/structure-job-durations.tsv");

        std::fs::write(&window, "1\tstructure\t1617\n").unwrap();
        assert_eq!(run(args(&root, OutputFormat::Text)).unwrap(), 0);
        let mut out = Vec::new();
        assert_eq!(
            run_to(args(&root, OutputFormat::Json), &mut out).unwrap(),
            0
        );
        let answer: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(answer["ok"], true, "{answer}");
        assert!(matches!(
            run_to(args(&root, OutputFormat::Json), &mut Closed),
            Err(Error::Transport(_))
        ));

        std::fs::write(&window, "1\tstructure\t1683\n").unwrap();
        assert_eq!(run(args(&root, OutputFormat::Text)).unwrap(), EXIT_EXCEEDS);

        std::fs::remove_file(&window).unwrap();
        assert_eq!(
            run(args(&root, OutputFormat::Text)).unwrap(),
            EXIT_UNMEASURED
        );
    }
}
