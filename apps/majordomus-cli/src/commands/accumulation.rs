//! `majordomus accumulation`: is anything piling up past its budget, and — on the policy's
//! consent — the reduction of what is provably spare (ADR 0150).
//!
//! The measurement and the verdict happen inside `accumulation.status`, so the terminal
//! report, the `--format json` document, the HTTP response and the MCP resource are four
//! renderings of one execution. What this command adds is the two things a shared server
//! does not do: `--brief`, the lines a provider's start event puts in front of a worker, and
//! `--reduce`, which removes what is spare in the repository it stands in.
//!
//! `--reduce` acts only when all of these hold, and says which did not when one does not:
//!
//! - the policy says `accumulation.act: reduce` — asking is not consenting;
//! - a quantity is over its budget — a watch reduces what crossed a line, not what it could;
//! - under `--if-due`, the cooldown since the last reduction has passed — every session of a
//!   machine fires the same events, and one of them acting is enough.
//!
//! What it then runs is `worktree cleanup --remove --branches`, in process: every candidate
//! is re-measured immediately before it goes, and a candidate that fails a reading is kept
//! and named.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::accumulation::{self, AccumulationAct, AccumulationReport, LastReduction};
use crate::app::App;
use crate::capability::Context;
use crate::cli::{AccumulationArgs, OutputFormat};
use crate::commands::worktree::Reduction;
use crate::error::{Error, Result};
use crate::worktree::WorktreeService;

/// The exit code when a quantity is over its budget. The code every unmet contract in this
/// executable uses.
pub const EXIT_OVER_BUDGET: u8 = 10;

/// Run `majordomus accumulation`.
///
/// The report goes through the capability. `--brief` and `--reduce` do not build the
/// repository's index: they run inside a provider's hook, on every start of every session,
/// and nothing they read is in the index — the policy, git's registry and the volume are
/// read directly, by the same functions the capability's handler calls.
pub fn run(args: AccumulationArgs) -> Result<u8> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    if !(args.brief || args.headline || args.reduce) {
        let app = App::load(&args.repo)?;
        let (value, report) = ask(&app.context)?;
        return answer(&value, &report, args.format, &mut out);
    }
    let start = match &args.repo.repo {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let (policy, local) = crate::capability::builtin::accumulation::declared(&start);
    let report = accumulation::measure(&start, &policy, &local).map_err(refused)?;
    if args.brief {
        return brief(&report, &mut out);
    }
    if args.headline {
        return headline(&report, &mut out);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let record = accumulation::record_path(std::path::Path::new(&report.repository), &local);
    reduce(&report, args.if_due, now, &record, &mut out, |branches| {
        let service = WorktreeService::open(&start).map_err(refused)?;
        crate::commands::worktree::reduce(&service, branches)
    })
}

fn refused(e: crate::worktree::WorktreeError) -> Error {
    Error::Refused {
        code: e.exit_code(),
        reason: e.to_string(),
    }
}

/// Ask `accumulation.status` of `ctx`: the answer as it was given, and typed.
fn ask(ctx: &Context) -> Result<(serde_json::Value, AccumulationReport)> {
    let id = ctx
        .registry
        .by_cli(&["accumulation".to_string()])
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: "no capability is exposed as `majordomus accumulation`".into(),
        })?;
    let value = ctx.execute(id, json!({})).map_err(|e| Error::Protocol {
        reason: format!("accumulation.status refused: {e}"),
    })?;
    let report: AccumulationReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: format!("accumulation.status answered something this command cannot read: {e}"),
        })?;
    Ok((value, report))
}

/// Write the answer in `format`, and say with the exit code whether every budget holds.
fn answer<W: Write>(
    value: &serde_json::Value,
    report: &AccumulationReport,
    format: OutputFormat,
    out: &mut W,
) -> Result<u8> {
    match format {
        OutputFormat::Json => writeln!(out, "{value:#}").map_err(Error::Transport)?,
        OutputFormat::Text => {
            for q in &report.quantities {
                writeln!(out, "{}", q.line()).map_err(Error::Transport)?;
            }
            if let Some(last) = &report.last_reduction {
                writeln!(out, "{}", last_line(last)).map_err(Error::Transport)?;
            }
            writeln!(out, "{}", report.summary()).map_err(Error::Transport)?;
        }
    }
    Ok(if report.within_budget {
        0
    } else {
        EXIT_OVER_BUDGET
    })
}

/// What the watch last did, in one line.
fn last_line(last: &LastReduction) -> String {
    format!(
        "last reduction at {}: {} worktree(s) removed, {} branch name(s) deleted, {} kept ({} over budget)",
        last.at,
        last.removed.len(),
        last.deleted.len(),
        last.refused,
        last.because.join(", ")
    )
}

/// What a briefing carries, and nothing when every budget holds or the policy has the watch
/// off. Always exit 0: this runs inside a provider's hook, where a non-zero exit is noise
/// in front of a person.
fn brief<W: Write>(report: &AccumulationReport, out: &mut W) -> Result<u8> {
    if !report.watched {
        return Ok(0);
    }
    for line in report.brief() {
        writeln!(out, "{line}").map_err(Error::Transport)?;
    }
    Ok(0)
}

/// What a mesh claim carries, and nothing unless the watch is on, the policy has it
/// announced and a budget is crossed. Always exit 0, for the reason [`brief`] does.
fn headline<W: Write>(report: &AccumulationReport, out: &mut W) -> Result<u8> {
    if report.watched && report.announce_on_mesh {
        if let Some(line) = report.headline() {
            writeln!(out, "{line}").map_err(Error::Transport)?;
        }
    }
    Ok(0)
}

/// Reduce what is spare, when the policy consents, a budget is crossed and — under `if_due`
/// — the cooldown has passed. `act` is what removes: it takes whether branch names may go.
fn reduce<W: Write>(
    report: &AccumulationReport,
    if_due: bool,
    now: u64,
    record: &std::path::Path,
    out: &mut W,
    act: impl FnOnce(bool) -> Result<Reduction>,
) -> Result<u8> {
    let mut say = |s: String| writeln!(out, "{s}").map_err(Error::Transport);
    let because = report.over_ids();
    if because.is_empty() {
        say("accumulation: nothing is over budget, so nothing is reduced".into())?;
        return Ok(0);
    }
    if report.act != AccumulationAct::Reduce {
        say(format!(
            "accumulation: {} over budget and accumulation.act is {}: nothing is removed here — {}",
            because.join(", "),
            report.act.as_str(),
            accumulation::REDUCE_COMMAND
        ))?;
        return Ok(EXIT_OVER_BUDGET);
    }
    let last = accumulation::last_reduction(record).map(|l| l.at);
    if if_due && !accumulation::is_due(last, report.cooldown_minutes, now) {
        say(format!(
            "accumulation: a reduction ran less than {} minute(s) ago; not due",
            report.cooldown_minutes
        ))?;
        return Ok(0);
    }
    let mut done = LastReduction {
        at: now,
        because,
        removed: Vec::new(),
        deleted: Vec::new(),
        refused: 0,
    };
    // before acting: a second session's watch must find the cooldown already taken
    accumulation::record(record, &done).map_err(|e| Error::io(record, e))?;
    let reduction = act(true)?;
    for line in reduction.lines() {
        say(line)?;
    }
    done.removed = reduction.removed;
    done.deleted = reduction.deleted;
    done.refused = reduction.refused.len();
    accumulation::record(record, &done).map_err(|e| Error::io(record, e))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::accumulation::{judge, AccumulationPolicy, Readings};
    use crate::commands::worktree::Kept;
    use crate::synthetic::SyntheticRepository;

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

    fn report(act: AccumulationAct, worktrees: u64) -> AccumulationReport {
        let policy = AccumulationPolicy {
            watch: true,
            act,
            worktrees_max: Some(2),
            cooldown_minutes: Some(10),
            ..Default::default()
        };
        let readings = Readings {
            worktrees: Ok(worktrees),
            merged_branches: Ok(0),
            disk_free_mb: Ok(1),
        };
        AccumulationReport::of("/r", &policy, judge(&readings, &policy))
    }

    fn text(out: Vec<u8>) -> String {
        String::from_utf8(out).unwrap()
    }

    /// The report lists every quantity and ends with the verdict; the exit code is the
    /// verdict in either format, and the JSON is the capability's answer byte for byte.
    #[test]
    fn the_exit_code_is_the_verdict_and_both_formats_carry_it() {
        let mut over = report(AccumulationAct::Report, 5);
        over.last_reduction = Some(LastReduction {
            at: 7,
            because: vec!["worktrees".into()],
            removed: vec!["a".into()],
            deleted: vec!["a".into(), "b".into()],
            refused: 3,
        });
        let value = serde_json::to_value(&over).unwrap();
        let mut out = Vec::new();
        assert_eq!(
            answer(&value, &over, OutputFormat::Text, &mut out).unwrap(),
            EXIT_OVER_BUDGET
        );
        let printed = text(out);
        assert_eq!(printed.lines().count(), 5, "{printed}");
        assert!(printed.starts_with("over        worktrees"), "{printed}");
        assert!(
            printed.contains("last reduction at 7: 1 worktree(s) removed, 2 branch name(s) deleted, 3 kept (worktrees over budget)"),
            "{printed}"
        );
        assert!(printed.trim_end().ends_with(&over.summary()), "{printed}");

        let fine = report(AccumulationAct::Report, 1);
        let value = serde_json::to_value(&fine).unwrap();
        let mut out = Vec::new();
        assert_eq!(
            answer(&value, &fine, OutputFormat::Json, &mut out).unwrap(),
            0
        );
        assert_eq!(
            text(out),
            format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
        );
    }

    /// A briefing gets the lines that ask something, nothing when every budget holds, and
    /// nothing at all when the policy has the watch off — and never a failing exit.
    #[test]
    fn a_briefing_is_silent_unless_a_watched_budget_is_crossed() {
        let mut out = Vec::new();
        assert_eq!(
            brief(&report(AccumulationAct::Report, 5), &mut out).unwrap(),
            0
        );
        assert!(text(out).starts_with("over        worktrees"));

        let mut out = Vec::new();
        assert_eq!(
            brief(&report(AccumulationAct::Report, 1), &mut out).unwrap(),
            0
        );
        assert!(out.is_empty());

        let mut unwatched = report(AccumulationAct::Report, 5);
        unwatched.watched = false;
        let mut out = Vec::new();
        assert_eq!(brief(&unwatched, &mut out).unwrap(), 0);
        assert!(out.is_empty(), "off is off");
    }

    /// The mesh hears one line, and only when the policy has it announced, the watch is on
    /// and a budget is crossed.
    #[test]
    fn the_mesh_hears_a_crossed_budget_only_when_the_policy_announces_it() {
        let mut over = report(AccumulationAct::Report, 5);
        let mut out = Vec::new();
        assert_eq!(headline(&over, &mut out).unwrap(), 0);
        assert!(out.is_empty(), "announce_on_mesh is not set");

        over.announce_on_mesh = true;
        let mut out = Vec::new();
        headline(&over, &mut out).unwrap();
        assert_eq!(text(out), "worktrees 5 (at most 2)\n");

        over.watched = false;
        let mut out = Vec::new();
        headline(&over, &mut out).unwrap();
        assert!(out.is_empty(), "off is off");

        let mut fine = report(AccumulationAct::Report, 1);
        fine.announce_on_mesh = true;
        let mut out = Vec::new();
        headline(&fine, &mut out).unwrap();
        assert!(out.is_empty(), "nothing is crossed");
    }

    /// Nothing over budget, a policy that only reports, and a cooldown not yet passed each
    /// stop a reduction before anything is touched, and each says so.
    #[test]
    fn a_reduction_needs_a_crossed_budget_the_policys_consent_and_its_turn() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("state/accumulation/last.json");
        let never = |_: bool| -> Result<Reduction> { panic!("nothing may be removed") };

        let mut out = Vec::new();
        assert_eq!(
            reduce(
                &report(AccumulationAct::Reduce, 1),
                false,
                100,
                &record,
                &mut out,
                never
            )
            .unwrap(),
            0
        );
        assert!(text(out).contains("nothing is over budget"));

        let mut out = Vec::new();
        assert_eq!(
            reduce(
                &report(AccumulationAct::Report, 5),
                false,
                100,
                &record,
                &mut out,
                never
            )
            .unwrap(),
            EXIT_OVER_BUDGET
        );
        let said = text(out);
        assert!(
            said.contains("accumulation.act is report: nothing is removed here"),
            "{said}"
        );
        assert!(said.contains(accumulation::REDUCE_COMMAND), "{said}");
        assert!(!record.exists(), "a refusal leaves no record");

        accumulation::record(
            &record,
            &LastReduction {
                at: 1_000,
                because: vec![],
                removed: vec![],
                deleted: vec![],
                refused: 0,
            },
        )
        .unwrap();
        let mut out = Vec::new();
        assert_eq!(
            reduce(
                &report(AccumulationAct::Reduce, 5),
                true,
                1_100,
                &record,
                &mut out,
                never
            )
            .unwrap(),
            0
        );
        assert!(text(out).contains("less than 10 minute(s) ago; not due"));
    }

    /// When it is due it acts with the names included, prints what was done, and leaves the
    /// record the next measurement reads; without `--if-due` the cooldown is not consulted.
    #[test]
    fn a_due_reduction_acts_and_records_what_it_did() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join("state/accumulation/last.json");
        let acted = |branches: bool| -> Result<Reduction> {
            assert!(branches, "the watch asks for the names as well");
            Ok(Reduction {
                removed: vec!["feature/a".into()],
                deleted: vec!["feature/a".into(), "fix/b".into()],
                refused: vec![Kept {
                    branch: "feature/c".into(),
                    reason: "uncommitted work".into(),
                }],
                branches: true,
            })
        };
        let mut out = Vec::new();
        assert_eq!(
            reduce(
                &report(AccumulationAct::Reduce, 5),
                true,
                5_000,
                &record,
                &mut out,
                acted
            )
            .unwrap(),
            0
        );
        let said = text(out);
        assert!(said.contains("removed   feature/a"), "{said}");
        assert!(said.contains("deleted   fix/b"), "{said}");
        assert!(said.contains("refused   feature/c"), "{said}");
        assert!(
            said.contains("worktree cleanup: 1 removed, 1 refused; 2 branch name(s) deleted"),
            "{said}"
        );
        let last = accumulation::last_reduction(&record).unwrap();
        assert_eq!(
            (
                last.at,
                last.removed.len(),
                last.deleted.len(),
                last.refused
            ),
            (5_000, 1, 2, 1)
        );
        assert_eq!(last.because, ["worktrees"]);

        // a person who asks outright is not told to wait
        let mut out = Vec::new();
        reduce(
            &report(AccumulationAct::Reduce, 5),
            false,
            5_001,
            &record,
            &mut out,
            acted,
        )
        .unwrap();
        assert_eq!(accumulation::last_reduction(&record).unwrap().at, 5_001);

        // a reduction that fails leaves the cooldown taken and reports the failure
        let fails = |_: bool| -> Result<Reduction> {
            Err(Error::Protocol {
                reason: "git went away".into(),
            })
        };
        let mut out = Vec::new();
        assert!(reduce(
            &report(AccumulationAct::Reduce, 5),
            false,
            9_000,
            &record,
            &mut out,
            fails
        )
        .is_err());
        assert_eq!(accumulation::last_reduction(&record).unwrap().at, 9_000);
    }

    /// The command reaches the capability through the registry, over a real repository.
    #[test]
    fn the_command_asks_the_capability_the_registry_exposes() {
        let repo = SyntheticRepository::small().unwrap();
        git(repo.root(), &["init", "-q", "-b", "master"]);
        git(repo.root(), &["add", "-A"]);
        git(repo.root(), &["commit", "-q", "-m", "base"]);
        let ctx = repo.context().unwrap();
        let (value, typed) = ask(&ctx).unwrap();
        assert_eq!(value["schema"], accumulation::SCHEMA);
        assert_eq!(typed.quantities.len(), 3);
        assert!(
            typed.within_budget,
            "no budget is declared, so none is crossed"
        );

        let args = AccumulationArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Vcs,
                strict: false,
                share: None,
            },
            format: OutputFormat::Text,
            brief: false,
            headline: false,
            reduce: false,
            if_due: false,
        };
        assert!(run(args).is_err(), "no repository is not a verdict");
        for (brief, reduce) in [(true, false), (false, true)] {
            let here = AccumulationArgs {
                repo: crate::cli::RepoArgs {
                    repo: Some(repo.root().to_path_buf()),
                    discovery: crate::cli::DiscoveryMode::Vcs,
                    strict: false,
                    share: None,
                },
                format: OutputFormat::Text,
                brief,
                headline: false,
                reduce,
                if_due: false,
            };
            assert_eq!(
                run(here).unwrap(),
                0,
                "nothing is budgeted, so nothing is asked"
            );
            let nowhere = AccumulationArgs {
                repo: crate::cli::RepoArgs {
                    repo: Some(tempfile::tempdir().unwrap().path().to_path_buf()),
                    discovery: crate::cli::DiscoveryMode::Vcs,
                    strict: false,
                    share: None,
                },
                format: OutputFormat::Text,
                brief,
                headline: false,
                reduce,
                if_due: false,
            };
            assert!(run(nowhere).is_err(), "a hook that cannot measure says so");
        }
        let carried = refused(crate::worktree::WorktreeError::GitUnavailable {
            reason: "gone".into(),
        });
        assert!(carried.to_string().contains("gone"), "{carried}");
    }
}
