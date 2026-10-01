//! `majordomus prs`: the command line's rendering of [`crate::integration`].
//!
//! Nothing is decided here. `status`, `plan`, `explain`, `events` and `brief` render the
//! queue the integration module builds from the last recorded observation — the same value
//! the HTTP route, the MCP tool and the Cockpit page (`/cockpit/integration`) render — and
//! never reach the network. `refresh`, `drain` and `cleanup` are the three that do, and each
//! says so in its help.
//!
//! Exit codes: 0 when the answer is complete; 10 when it is a finding (a stale or absent
//! observation, a drain that stopped on a verification failure, a pull request that is not
//! open); 12 when it could not be answered (the forge, git or the lease refused).

use std::io::Write;
use std::path::PathBuf;

use serde::Serialize;

use crate::cli::{OutputFormat, PrsArgs, PrsCommand};
use crate::error::{Error, Result};
use crate::integration::{
    self,
    drain::{self, DrainStepOutcome, ForgeIntegrator, IntegrationLease},
    IntegrationQueue, PullRequestAssessment, RelationToMaster,
};

const FINDING: u8 = 10;
const UNUSABLE: u8 = 12;

fn unusable(reason: impl Into<String>) -> Error {
    Error::Refused {
        code: UNUSABLE,
        reason: reason.into(),
    }
}

fn root_of(args: &PrsArgs) -> Result<PathBuf> {
    let start = match &args.repo.repo {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&start)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| unusable(format!("git could not run: {e}")))?;
    if !out.status.success() {
        return Err(unusable(format!(
            "{} is not in a git repository",
            start.display()
        )));
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn json(out: &mut impl Write, value: &impl Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| unusable(e.to_string()))?;
    writeln!(out, "{text}").map_err(|e| Error::io("stdout", e))
}

fn w(out: &mut impl Write, line: impl AsRef<str>) -> Result<()> {
    writeln!(out, "{}", line.as_ref()).map_err(|e| Error::io("stdout", e))
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

/// Run `majordomus prs`.
pub fn run(args: PrsArgs) -> Result<u8> {
    let root = root_of(&args)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let format = args.format;
    match args.command.unwrap_or(PrsCommand::Status) {
        PrsCommand::Status => {
            let q = integration::queue_of(&root).map_err(|e| Error::Refused {
                code: FINDING,
                reason: e,
            })?;
            status(&q, format, &mut out)?;
            Ok(if q.diagnostics.is_empty() { 0 } else { FINDING })
        }
        PrsCommand::Plan => {
            let q = integration::queue_of(&root).map_err(|e| Error::Refused {
                code: FINDING,
                reason: e,
            })?;
            plan(&q, format, &mut out)?;
            Ok(0)
        }
        PrsCommand::Explain { number } => {
            let q = integration::queue_of(&root).map_err(|e| Error::Refused {
                code: FINDING,
                reason: e,
            })?;
            let Some(a) = q.get(number) else {
                return Err(Error::Refused {
                    code: FINDING,
                    reason: format!(
                        "#{number} is not among the {} open pull request(s) observed at {}",
                        q.tallies.open, q.observed_at
                    ),
                });
            };
            let rank = q
                .assessments
                .iter()
                .position(|x| x.number == number)
                .map(|i| i + 1);
            explain(&q, a, rank.unwrap_or(0), format, &mut out)?;
            Ok(0)
        }
        PrsCommand::Refresh => {
            let obs = integration::refresh(&root).map_err(unusable)?;
            if format == OutputFormat::Json {
                json(&mut out, &obs)?;
            } else {
                w(
                    &mut out,
                    format!(
                        "observed {} open pull request(s) of {} at {}; {} is {}",
                        obs.pull_requests.len(),
                        obs.repository,
                        obs.observed_at,
                        obs.base,
                        short(&obs.base_sha)
                    ),
                )?;
            }
            Ok(0)
        }
        PrsCommand::Drain {
            max,
            dry_run: false,
            refresh,
            continuous: true,
            interval,
        } => {
            let base = integration::load_observation(&root)
                .ok()
                .flatten()
                .map(|o| o.base)
                .unwrap_or_else(|| "master".into());
            // the lease for the whole run: a second worker is refused here, before it acts
            let lease = IntegrationLease::acquire(&root, &base).map_err(unusable)?;
            let stop = drain::stop_on_signals();
            let mut integrator = ForgeIntegrator {
                root: &root,
                lease: Some(&lease),
            };
            let text = format != OutputFormat::Json;
            let report = drain::continuous(
                &root,
                &mut integrator,
                drain::ContinuousOptions {
                    max_per_cycle: max,
                    allow_refresh: refresh,
                    interval: std::time::Duration::from_secs(interval),
                    cycles: None,
                },
                stop,
                &mut std::thread::sleep,
                &mut |n, r| {
                    if text {
                        // as each cycle ends, not at the end of a run that may last days
                        let _ = writeln!(out, "cycle {n}:");
                        for s in &r.steps {
                            let _ = writeln!(out, "  {}", describe(s));
                        }
                        let _ = writeln!(out, "  stopped: {}", r.stopped);
                        let _ = out.flush();
                    }
                },
            );
            drop(lease);
            if text {
                w(
                    &mut out,
                    format!(
                        "continuous drain stopped after {} cycle(s), {} merge(s): {}",
                        report.cycles,
                        report.merged.len(),
                        report.stopped
                    ),
                )?;
                if let Some(f) = &report.failure {
                    w(&mut out, format!("! {f}"))?;
                }
            } else {
                json(&mut out, &report)?;
            }
            Ok(if report.failure.is_some() {
                UNUSABLE
            } else if report.stopped.contains("could not be verified") {
                FINDING
            } else {
                0
            })
        }
        PrsCommand::Drain {
            max,
            dry_run,
            refresh,
            ..
        } => {
            // a dry run changes nothing, and observers never contend with the executor
            let base = integration::load_observation(&root)
                .ok()
                .flatten()
                .map(|o| o.base)
                .unwrap_or_else(|| "master".into());
            let lease = if dry_run {
                None
            } else {
                Some(IntegrationLease::acquire(&root, &base).map_err(unusable)?)
            };
            let mut integrator = ForgeIntegrator {
                root: &root,
                lease: lease.as_ref(),
            };
            let report =
                drain::drain(&root, &mut integrator, max, dry_run, refresh).map_err(unusable)?;
            if format == OutputFormat::Json {
                json(&mut out, &report)?;
            } else {
                for s in &report.steps {
                    w(&mut out, describe(s))?;
                }
                w(&mut out, format!("stopped: {}", report.stopped))?;
            }
            let failed = report
                .steps
                .iter()
                .any(|s| matches!(s, DrainStepOutcome::VerificationFailed { .. }));
            Ok(if failed { FINDING } else { 0 })
        }
        PrsCommand::Cleanup { apply } => {
            let mut integrator = ForgeIntegrator {
                root: &root,
                lease: None,
            };
            let _lease = if apply {
                let base = integration::load_observation(&root)
                    .ok()
                    .flatten()
                    .map(|o| o.base)
                    .unwrap_or_else(|| "master".into());
                Some(IntegrationLease::acquire(&root, &base).map_err(unusable)?)
            } else {
                None
            };
            let items = drain::cleanup(&root, &mut integrator, apply).map_err(unusable)?;
            if format == OutputFormat::Json {
                json(&mut out, &items)?;
            } else if items.is_empty() {
                w(
                    &mut out,
                    "nothing to clean up: no open pull request's work is on master already",
                )?;
            } else {
                for i in &items {
                    w(
                        &mut out,
                        format!(
                            "#{:<5} {:<20} {:<18} {}",
                            i.pr,
                            i.disposition.as_str(),
                            i.action,
                            i.reasons.join(", ")
                        ),
                    )?;
                }
            }
            Ok(0)
        }
        PrsCommand::Brief => {
            if let Some(line) = brief(&root) {
                w(&mut out, line)?;
            }
            Ok(0)
        }
        PrsCommand::Events => {
            let events = drain::events(&root);
            if format == OutputFormat::Json {
                json(&mut out, &events)?;
            } else if events.is_empty() {
                w(
                    &mut out,
                    "no integration action is recorded in this checkout",
                )?;
            } else {
                for e in &events {
                    w(
                        &mut out,
                        format!(
                            "{}  {:<20} {}  {}",
                            e.at,
                            e.action,
                            e.pr.map(|n| format!("#{n}")).unwrap_or_else(|| "-".into()),
                            e.detail
                        ),
                    )?;
                }
            }
            Ok(0)
        }
    }
}

/// "12 min ago" for an RFC 3339 instant, or nothing when it does not parse.
fn ago(at: &str) -> String {
    let Some(then) = crate::peers::epoch_seconds(at) else {
        return String::new();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    match (now - then).max(0) {
        s @ 0..=119 => format!("{s} s ago"),
        s @ 120..=7199 => format!("{} min ago", s / 60),
        s @ 7200..=172_799 => format!("{} h ago", s / 3600),
        s => format!("{} d ago", s / 86_400),
    }
}

/// The briefing line: what the last queue built here said, who holds the lease, and the last
/// merge — from files alone. `None` when this checkout never observed the forge, so that a
/// briefing does not grow a section about nothing.
fn brief(root: &std::path::Path) -> Option<String> {
    let obs = integration::load_observation(root).ok().flatten()?;
    let mut parts = Vec::new();
    match integration::QueueSummary::load(root) {
        Some(s) => {
            let lanes: Vec<String> = s
                .tallies
                .by_lane
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect();
            parts.push(format!(
                "{} at {} observed {} ({}): {} open — {}",
                s.base,
                short(&s.master_sha),
                ago(&s.observed_at),
                s.observed_at,
                s.tallies.open,
                lanes.join(", ")
            ));
            parts.push(match s.next_merge {
                Some(n) => format!("next merge #{n}"),
                None => "nothing ready".into(),
            });
            if !s.starving.is_empty() {
                let list: Vec<String> = s.starving.iter().map(|n| format!("#{n}")).collect();
                parts.push(format!("starving {}", list.join(" ")));
            }
            if s.diagnostics > 0 {
                parts.push(format!(
                    "{} diagnostic(s): majordomus prs status",
                    s.diagnostics
                ));
            }
        }
        None => parts.push(format!(
            "observed {} at {}; no queue built since — majordomus prs status",
            obs.repository, obs.observed_at
        )),
    }
    parts.push(match IntegrationLease::read(root, &obs.base) {
        Ok(Some(l)) => match (&l.holder, l.stale) {
            (_, true) => format!(
                "lease stale (renewed {} s ago; the next executor takes it over)",
                l.renewed_seconds_ago
            ),
            (Some(h), false) => format!("lease held by pid {} on {}", h.pid, h.host),
            (None, false) => "lease held (holder unreadable)".into(),
        },
        Ok(None) => "lease free".into(),
        Err(_) => "lease unknown".into(),
    });
    if let Some(m) = drain::events(root)
        .iter()
        .rev()
        .find(|e| e.action == "merge_succeeded")
    {
        parts.push(format!(
            "last merge #{} {}",
            m.pr.unwrap_or_default(),
            ago(&m.at)
        ));
    }
    Some(parts.join("; "))
}

/// "since 2026-10-01T10:00:00Z (3 h ago), passed over 2× (last for #12)" for an actionable
/// pull request's wait.
fn waited(wait: &integration::ExecutorWait) -> String {
    let mut s = format!(
        "actionable since {} ({})",
        wait.actionable_since,
        ago(&wait.actionable_since)
    );
    if wait.passed_over > 0 {
        s.push_str(&format!(", passed over {}×", wait.passed_over));
        if let Some(p) = &wait.last_passed_over {
            s.push_str(&format!(" (last for #{} at {})", p.for_pr, p.at));
        }
    }
    s
}

fn describe(s: &DrainStepOutcome) -> String {
    match s {
        DrainStepOutcome::Idle { why } => format!("idle: {why}"),
        DrainStepOutcome::WouldMerge { pr } => format!("would merge #{pr}"),
        DrainStepOutcome::StaleDecision { pr, what } => {
            format!("#{pr}: decision stale ({what}); re-planning")
        }
        DrainStepOutcome::Merged {
            pr,
            master_before,
            master_after,
        } => {
            format!(
                "merged #{pr}: master {} -> {}",
                short(master_before),
                short(master_after)
            )
        }
        DrainStepOutcome::MergeRefused { pr, reason } => {
            format!("#{pr}: the forge refused the merge: {reason}")
        }
        DrainStepOutcome::VerificationFailed { pr, reason } => {
            format!("#{pr}: merged but not verified: {reason}")
        }
        DrainStepOutcome::WouldRefresh { pr } => format!("would bring master into #{pr}"),
        DrainStepOutcome::Refreshed {
            pr,
            head_before,
            head_after,
        } => {
            format!(
                "brought master into #{pr}: head {} -> {}",
                short(head_before),
                short(head_after)
            )
        }
        DrainStepOutcome::AwaitingChecks { pr } => {
            format!("#{pr} contains master; waiting for its required checks")
        }
        DrainStepOutcome::RefreshFailed { pr, reason } => {
            format!("#{pr}: bringing master in failed: {reason}")
        }
    }
}

fn header(q: &IntegrationQueue, out: &mut impl Write) -> Result<()> {
    w(
        out,
        format!(
            "{} · {} at {} · observed {} ({} open)",
            q.repository,
            q.base,
            short(&q.master_sha),
            q.observed_at,
            q.tallies.open
        ),
    )?;
    for d in &q.diagnostics {
        w(out, format!("! {d}"))?;
    }
    let lanes: Vec<String> = q
        .tallies
        .by_lane
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
    w(out, format!("lanes: {}", lanes.join(" · ")))
}

fn status(q: &IntegrationQueue, format: OutputFormat, out: &mut impl Write) -> Result<()> {
    if format == OutputFormat::Json {
        return json(out, q);
    }
    header(q, out)?;
    w(
        out,
        format!(
            "{:>4}  {:<6} {:<22} {:<6} {:<34} {}",
            "rank", "pr", "disposition", "risk", "reason", "title"
        ),
    )?;
    for (i, a) in q.assessments.iter().enumerate() {
        w(
            out,
            format!(
                "{:>4}  #{:<5} {:<22} {:<6} {:<34} {}",
                i + 1,
                a.number,
                a.disposition.as_str(),
                integration::classify::word(&a.risk),
                trunc(&a.reasons.join(","), 34),
                trunc(&a.title, 60)
            ),
        )?;
    }
    for n in &q.starving {
        if let Some(w_) = q.get(*n).and_then(|a| a.wait.as_ref()) {
            w(out, format!("starving: #{n} {}", waited(w_)))?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct PlanView<'a> {
    master_sha: &'a str,
    observed_at: &'a str,
    next_merge: Option<u64>,
    next_refresh: &'a [u64],
    repair: Vec<u64>,
    cleanup: Vec<u64>,
    held: Vec<u64>,
    diagnostics: &'a [String],
}

fn plan(q: &IntegrationQueue, format: OutputFormat, out: &mut impl Write) -> Result<()> {
    let lane = |l: integration::IntegrationLane| -> Vec<u64> {
        q.assessments
            .iter()
            .filter(|a| a.lane == l)
            .map(|a| a.number)
            .collect()
    };
    let view = PlanView {
        master_sha: &q.master_sha,
        observed_at: &q.observed_at,
        next_merge: q.next_merge,
        next_refresh: &q.next_refresh,
        repair: lane(integration::IntegrationLane::Repair),
        cleanup: lane(integration::IntegrationLane::Cleanup),
        held: lane(integration::IntegrationLane::Held),
        diagnostics: &q.diagnostics,
    };
    if format == OutputFormat::Json {
        return json(out, &view);
    }
    header(q, out)?;
    let list = |v: &[u64]| {
        if v.is_empty() {
            "none".to_string()
        } else {
            v.iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(" ")
        }
    };
    w(
        out,
        format!(
            "next merge:    {}",
            q.next_merge
                .map(|n| format!("#{n}"))
                .unwrap_or_else(|| "none is ready".into())
        ),
    )?;
    w(out, format!("refresh next:  {}", list(&q.next_refresh)))?;
    w(out, format!("repair:        {}", list(&view.repair)))?;
    w(out, format!("cleanup:       {}", list(&view.cleanup)))?;
    w(out, format!("held:          {}", list(&view.held)))?;
    w(
        out,
        "after any merge this plan is void: the executor observes again before its next step",
    )
}

#[derive(Serialize)]
struct ExplainView<'a> {
    rank: usize,
    assessment: &'a PullRequestAssessment,
}

fn explain(
    q: &IntegrationQueue,
    a: &PullRequestAssessment,
    rank: usize,
    format: OutputFormat,
    out: &mut impl Write,
) -> Result<()> {
    if format == OutputFormat::Json {
        return json(
            out,
            &ExplainView {
                rank,
                assessment: a,
            },
        );
    }
    w(out, format!("#{} {}", a.number, a.title))?;
    w(
        out,
        format!(
            "  disposition:  {} ({})",
            a.disposition.as_str(),
            integration::classify::word(&a.lane)
        ),
    )?;
    w(out, format!("  reasons:      {}", a.reasons.join(", ")))?;
    if let Some(n) = &a.next_action {
        w(out, format!("  next:         {n}"))?;
    }
    w(
        out,
        format!(
            "  against:      master {} · head {} (observed {})",
            short(&a.evaluated_against.master_sha),
            short(&a.evaluated_against.head_sha),
            q.observed_at
        ),
    )?;
    w(
        out,
        format!("  rank:         {rank} of {}", q.assessments.len()),
    )?;
    if let Some(wait) = &a.wait {
        w(out, format!("  waiting:      {}", waited(wait)))?;
    }
    w(
        out,
        format!(
            "  risk:         {} — {}",
            integration::classify::word(&a.risk),
            a.risk_factors.join("; ")
        ),
    )?;
    let relation = match &a.relation {
        RelationToMaster::Behind { behind, .. } => format!("behind master by {behind}"),
        RelationToMaster::Conflicting { paths } => format!("conflicts on {}", paths.join(", ")),
        r => integration::classify::word(r),
    };
    w(out, format!("  relation:     {relation}"))?;
    w(
        out,
        format!(
            "  checks:       {}",
            integration::classify::word(&a.required_checks)
        ),
    )?;
    w(
        out,
        format!("  review:       {}", integration::classify::word(&a.review)),
    )?;
    for d in &a.dependencies {
        w(
            out,
            format!(
                "  depends on:   #{} ({}, {})",
                d.number,
                integration::classify::word(&d.certainty),
                if d.satisfied { "landed" } else { "open" }
            ),
        )?;
    }
    if !a.overlaps.is_empty() {
        let o: Vec<String> = a
            .overlaps
            .iter()
            .map(|o| format!("#{} ({})", o.number, o.paths.len()))
            .collect();
        w(out, format!("  overlaps:     {}", o.join(" ")))?;
    }
    w(
        out,
        format!("  authored:     {} path(s)", a.authored_paths.len()),
    )?;
    w(out, "  evidence:")?;
    for e in &a.evidence {
        w(
            out,
            format!("    {:<20} {:<14} {}", e.kind, e.status, e.detail),
        )?;
    }
    Ok(())
}
