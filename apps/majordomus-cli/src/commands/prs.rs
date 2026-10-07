//! `majordomus prs`: the command line's rendering of [`crate::integration`].
//!
//! Nothing is decided here. `status`, `plan`, `explain`, `events` and `brief` render the
//! queue the integration module builds from the last recorded observation — the same value
//! the HTTP route, the MCP tool and the Cockpit page (`/cockpit/integration`) render — and
//! never reach the network; neither does `repair` without `--apply`, which decides on the same
//! queue. `refresh`, `drain`, `repair --apply` and `cleanup` are the ones that do, and each says
//! so in its help.
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
    repair, IntegrationQueue, PullRequestAssessment, RelationToMaster,
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

/// The issue and milestone an assessment is part of, as one phrase; `None` when its branch
/// names no issue.
fn issue_line(a: &integration::PullRequestAssessment) -> Option<String> {
    let issue = a.issue.as_ref()?;
    Some(match &a.milestone {
        Some(m) => format!("{issue} · milestone {m}"),
        None => issue.clone(),
    })
}

/// Cleanup's second part: what merged pull requests left on origin, read now and recorded
/// ([`drain::report_left_branches`]), then printed.
fn cleanup_branches(root: &std::path::Path, out: &mut impl Write) -> Result<()> {
    let report = drain::report_left_branches(root).map_err(unusable)?;
    left_branches(&report.and_then(|r| r.branches), out)
}

/// The merged branches the forge left on origin, one line each, or why there is no list.
fn left_branches(left: &Option<Vec<drain::LeftBranch>>, out: &mut impl Write) -> Result<()> {
    match left {
        None => w(
            out,
            "merged branches: unread (origin or the merged pull requests could not be read)",
        ),
        Some(l) if l.is_empty() => w(
            out,
            "merged branches: none left on origin at the head that merged",
        ),
        Some(l) => {
            w(
                out,
                format!(
                    "merged branches left on origin ({}); the forge decides deletion, so none is deleted here:",
                    l.len()
                ),
            )?;
            for b in l {
                w(
                    out,
                    format!(
                        "  {:<48} #{:<5} {}  {}",
                        b.branch,
                        b.pr,
                        &b.tip[..b.tip.len().min(12)],
                        b.action
                    ),
                )?;
            }
            // the arm holds a list with at least one branch
            w(out, format!("  next: {}", l[0].next_step))
        }
    }
}

fn render_repair(r: &repair::RepairReport, out: &mut impl Write) -> Result<()> {
    let who = match (r.pr, &r.branch) {
        (Some(n), Some(b)) => format!("#{n} ({b})"),
        _ => r.target.clone(),
    };
    // the lines first, then one write: a report is printed whole or not at all
    let mut lines: Vec<String> = r
        .head_sha
        .iter()
        .map(|head| {
            format!(
                "{who}: head {} against {} {}{}",
                short(head),
                r.base,
                short(&r.master_sha),
                r.relation
                    .as_ref()
                    .map(|rel| format!(", {}", integration::classify::relation_word(rel)))
                    .unwrap_or_default()
            )
        })
        .collect();
    lines.extend(r.dry_run.then(|| {
        format!(
            "decided on the forge as observed at {}; prs refresh observes it again",
            r.observed_at
        )
    }));
    match &r.outcome {
        repair::RepairOutcome::NothingToRepair { why } => {
            lines.push(format!("nothing to repair: {why}"));
        }
        repair::RepairOutcome::WouldRepair => lines.push(format!(
            "dry run: would merge {} {} into {who}, derive, commit, and push it leased on {}; --apply does",
            r.base,
            short(&r.master_sha),
            r.head_sha.as_deref().map(short).unwrap_or("its head")
        )),
        repair::RepairOutcome::Repaired { head_after } => lines.push(format!(
            "repaired {who}: it carries {} {} and is at {}",
            r.base,
            short(&r.master_sha),
            short(head_after)
        )),
        repair::RepairOutcome::Refused { refusal } => {
            lines.push(format!("REFUSE {who}: {refusal}"));
            if let repair::RepairRefusal::AuthoredConflict { paths } = refusal {
                lines.extend(paths.iter().map(|p| format!("       {p}")));
                lines.push(format!(
                    "       merge {} in the branch's own worktree, decide what those files mean, then run this again",
                    r.base
                ));
            }
        }
    }
    w(out, lines.join("\n"))
}

/// The exit status of a repair: 12 when the trail could not record the act, 10 for any other
/// refusal and for a dry run over an observation nobody can vouch for, 0 otherwise.
fn repair_exit(r: &repair::RepairReport) -> u8 {
    match &r.outcome {
        repair::RepairOutcome::Refused {
            refusal: repair::RepairRefusal::TrailUnwritable { .. },
        } => UNUSABLE,
        repair::RepairOutcome::Refused { .. } => FINDING,
        // a dry run over a diagnosed observation is a reading of a queue nobody can vouch
        // for: it says what it would do and exits 10, as status and plan do
        _ if r.dry_run && !r.diagnostics.is_empty() => FINDING,
        _ => 0,
    }
}

/// The act of `prs repair --apply`: a push to the forge's remote, under the base branch's
/// lease for the whole of it, so a drain never races it. Every way it cannot be taken is
/// unusable (12).
fn repair_applied(
    root: &std::path::Path,
    target: &repair::RepairTarget,
) -> Result<repair::RepairReport> {
    executor_base(root)
        .and_then(|base| integration::exclusive::acquire(root, &base))
        .and_then(|lease| {
            let mut integrator = ForgeIntegrator {
                root,
                lease: Some(&lease),
            };
            repair::apply(root, &mut integrator, target)
        })
        .map_err(unusable)
}

/// The finding of a dry run that has no observation to decide from.
fn unobserved(reason: String) -> Error {
    Error::Refused {
        code: FINDING,
        reason,
    }
}

fn render_proof(p: &integration::proof::DryRunProof, out: &mut impl Write) -> Result<()> {
    // said whole, in one write
    let mut lines: Vec<String> = Vec::new();
    for s in &p.steps {
        lines.push(format!("{:<16} {}", s.step, s.summary));
    }
    if !p.moved.is_empty() {
        lines.push("the non-mutating cycle moved something:".into());
        for m in &p.moved {
            let sign = if m.change == "added" { '+' } else { '-' };
            lines.push(format!("  {sign} {:<6} {}", m.section, m.line));
        }
    }
    for m in &p.mirrors {
        lines.push(format!("mirror: {m}"));
    }
    lines.push(format!("observed classification (base {}):", p.base));
    for c in &p.classification {
        lines.push(
            format!(
                "  #{}  {}  {}  {}",
                c.number,
                &c.head_sha[..c.head_sha.len().min(12)],
                c.disposition,
                c.next_action.as_deref().unwrap_or("")
            )
            .trim_end()
            .to_string(),
        );
    }
    if p.ok {
        lines.push("nothing moved (remote, forge, trail, lease, local refs)".into());
    }
    w(out, lines.join("\n"))
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
            diagnosed(&q, format)
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
            diagnosed(&q, format)
        }
        PrsCommand::Refresh => {
            // observed just now: what the queue learns from it is kept for the readers
            let obs = integration::refresh(&root)
                .and_then(|obs| integration::queue_and_record(&root).map(|_| obs))
                .map_err(unusable)?;
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
            resume_after_failure,
        } => {
            // the last stage of the rollout (ADR 0101 §13), unlocked by the trail's record of
            // verified merges and by nothing else: asked before the lease, the base or the forge
            if let Some(reason) = drain::continuous_refused(&drain::events(&root)) {
                return Err(Error::Refused {
                    code: FINDING,
                    reason,
                });
            }
            // the lease for the whole run: a second worker is refused here, before it acts, and
            // a failure is acknowledged only under it
            let lease = executor_base(&root)
                .and_then(|base| lease_for(&root, &base, resume_after_failure))
                .map_err(unusable)?;
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
                        for line in drain_lines(r) {
                            let _ = writeln!(out, "  {line}");
                        }
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
            resume_after_failure,
            ..
        } => {
            // a dry run changes nothing, and observers never contend with the executor
            let base = executor_base(&root).map_err(unusable)?;
            let lease = if dry_run {
                None
            } else {
                Some(lease_for(&root, &base, resume_after_failure).map_err(unusable)?)
            };
            let mut integrator = ForgeIntegrator {
                root: &root,
                lease: lease.as_ref(),
            };
            // a signal lets the step in progress finish and starts no other, as in continuous mode
            let stop = drain::stop_on_signals();
            let report =
                drain::drain_until(&root, &mut integrator, max, dry_run, refresh, Some(stop))
                    .map_err(unusable)?;
            let printed = if format == OutputFormat::Json {
                json(&mut out, &report)
            } else {
                w(&mut out, drain_lines(&report).join("\n"))
            };
            printed.map(|()| drain_exit(&report))
        }
        PrsCommand::Cleanup { apply, .. } => {
            let lease = if apply {
                let base = executor_base(&root).map_err(unusable)?;
                Some(integration::exclusive::acquire(&root, &base).map_err(unusable)?)
            } else {
                None
            };
            let mut integrator = ForgeIntegrator {
                root: &root,
                lease: lease.as_ref(),
            };
            let items = drain::cleanup(&root, &mut integrator, apply).map_err(unusable)?;
            if format == OutputFormat::Json {
                json(&mut out, &items)?;
                return Ok(0);
            }
            if items.is_empty() {
                w(
                    &mut out,
                    "nothing to clean up: no open pull request's work is on master already, and \
                     none was superseded by one that landed",
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
                            integration::reason_list(&i.reasons, ", ")
                        ),
                    )?;
                }
            }
            // what merged pull requests left on origin: reported, never deleted (owner
            // decision D4). Read here, on demand, and recorded for the offline surfaces;
            // never by refresh, which the executor runs before every decision.
            cleanup_branches(&root, &mut out).map(|()| 0)
        }
        PrsCommand::Repair { target, apply, .. } => {
            let target = repair::RepairTarget::parse(&target);
            // the default is the read `status` makes, from the last recorded observation — no
            // network, no lease, nothing written to the trail; `--apply` is the act
            let report = if apply {
                repair_applied(&root, &target)
            } else {
                repair::plan(&root, &target).map_err(unobserved)
            }?;
            let printed = if format == OutputFormat::Json {
                json(&mut out, &report)
            } else {
                report.diagnostics.iter().for_each(|d| eprintln!("! {d}"));
                render_repair(&report, &mut out)
            };
            printed.map(|()| repair_exit(&report))
        }
        PrsCommand::ProveDryRun => {
            let proof = integration::proof::prove_dry_run(&root).map_err(unusable)?;
            let written = if format == OutputFormat::Json {
                json(&mut out, &proof)
            } else {
                render_proof(&proof, &mut out)
            };
            written.map(|()| if proof.ok { 0 } else { FINDING })
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

/// The exit code of a bounded drain: 12 when an act was withheld because the trail could not
/// record it — the repository is not usable for integration until it can — 10 when a merge
/// could not be verified, 0 otherwise.
/// The base branch an executor takes the lease of: the one the forge was last observed to
/// name, observed now when this checkout never asked. Never a guessed `master`: a lease
/// taken on a name the repository does not use excludes nobody.
fn executor_base(root: &std::path::Path) -> std::result::Result<String, String> {
    match integration::load_observation(root)? {
        Some(o) => Ok(o.base),
        None => integration::refresh(root).map(|o| o.base),
    }
}

/// The base's lease, and under it, when the person asked, the acknowledgement of the merge an
/// earlier drain could not verify: `--resume-after-failure` never comes with `--dry-run`, so
/// nothing is acknowledged by an executor that does not hold the lease.
fn lease_for(
    root: &std::path::Path,
    base: &str,
    resume_after_failure: bool,
) -> std::result::Result<IntegrationLease, String> {
    integration::exclusive::acquire(root, base).and_then(|lease| {
        if resume_after_failure {
            drain::acknowledge_failure(root, RESUMED_BY).map(|_| lease)
        } else {
            Ok(lease)
        }
    })
}

/// What makes a queue less than a full answer, said the way `status` says it: on standard
/// error beside a person's text (the JSON carries them already), and as exit 10 for every
/// reading of the queue, so a plan or an explanation over a stale or partial queue is never
/// taken for a sound one.
fn diagnosed(q: &integration::IntegrationQueue, format: OutputFormat) -> Result<u8> {
    if format != OutputFormat::Json {
        for d in &q.diagnostics {
            eprintln!("! {d}");
        }
    }
    Ok(if q.diagnostics.is_empty() { 0 } else { FINDING })
}

/// Who the trail says acknowledged a merge that could not be verified.
const RESUMED_BY: &str = "a person, through prs drain --resume-after-failure";

fn drain_exit(report: &drain::DrainReport) -> u8 {
    let stopped_on = |f: fn(&DrainStepOutcome) -> bool| report.steps.iter().any(f);
    if stopped_on(|s| matches!(s, DrainStepOutcome::TrailUnwritable { .. })) {
        UNUSABLE
    } else if stopped_on(|s| {
        matches!(
            s,
            DrainStepOutcome::VerificationFailed { .. } | DrainStepOutcome::Halted { .. }
        )
    }) {
        FINDING
    } else {
        0
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
            (Some(h), false) => format!(
                "lease held by pid {} on {}, {}",
                h.pid,
                h.host,
                match &h.mesh_claim {
                    Some(key) => format!("across machines by mesh claim {key}"),
                    None => "per-clone guard only".to_string(),
                }
            ),
            (None, false) => "lease held (holder unreadable)".into(),
        },
        Ok(None) => "lease free".into(),
        Err(_) => "lease unknown".into(),
    });
    let trail = drain::events(root);
    if let Some(m) = trail
        .iter()
        .rev()
        .find(|e| e.action == drain::IntegrationAction::MergeSucceeded)
    {
        parts.push(format!(
            "last merge #{} {}",
            m.pr.unwrap_or_default(),
            ago(&m.at)
        ));
    }
    if let Some(e) = last_outcome(&trail) {
        parts.push(format!(
            "last {}{} {}",
            e.action.as_str(),
            e.pr.map(|n| format!(" #{n}")).unwrap_or_default(),
            ago(&e.at)
        ));
    }
    Some(parts.join("; "))
}

/// The last event that says how the executor's work last went — a refresh it pushed, a merge
/// that failed or could not be verified, a decision that went stale — for a person resuming
/// it. A successful merge is said beside it already; the rest of the trail is detail.
fn last_outcome(trail: &[drain::IntegrationEvent]) -> Option<&drain::IntegrationEvent> {
    use drain::IntegrationAction as A;
    trail.iter().rev().find(|e| {
        matches!(
            e.action,
            A::Refreshed | A::VerificationFailed | A::MergeFailed | A::StaleDecision
        )
    })
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

/// One sentence per drain outcome, every outcome named: what the command line prints and the
/// dry-run proof records.
/// A drain as lines: one sentence per step, then why it stopped when the last step has not
/// already said so ([`drain::DrainReport::stop_unsaid`]).
pub(crate) fn drain_lines(r: &drain::DrainReport) -> Vec<String> {
    r.steps
        .iter()
        .map(describe)
        .chain(r.stop_unsaid().map(|why| format!("stopped: {why}")))
        .collect()
}

pub(crate) fn describe(s: &DrainStepOutcome) -> String {
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
        DrainStepOutcome::MergeRefused { pr, reason, class } => {
            format!("#{pr}: the forge refused the merge ({class}): {reason}")
        }
        DrainStepOutcome::VerificationFailed { pr, reason } => {
            format!("#{pr}: merged but not verified: {reason}")
        }
        DrainStepOutcome::Halted { pr, reason } => format!(
            "halted: {} was merged but not verified ({reason}); look, then run `prs drain --resume-after-failure`",
            pr.map_or_else(|| "a pull request".to_string(), |n| format!("#{n}"))
        ),
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
        DrainStepOutcome::RefreshFailed { pr, reason, class } => {
            format!("#{pr}: bringing master in failed ({class}): {reason}")
        }
        DrainStepOutcome::TrailUnwritable {
            pr,
            unrecorded,
            reason,
        } => format!(
            "#{pr}: nothing was done, because the trail could not record {unrecorded}: {reason}"
        ),
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
                trunc(&integration::reason_list(&a.reasons, ","), 34),
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
    // said whole, in one write
    let mut lines: Vec<String> = vec![
        format!("#{} {}", a.number, a.title),
        format!(
            "  disposition:  {} ({})",
            a.disposition.as_str(),
            integration::classify::word(&a.lane)
        ),
    ];
    if let Some(by) = a.superseded_by {
        lines.push(format!("  superseded:   by #{by}, which landed"));
    }
    lines.push(format!(
        "  reasons:      {}",
        integration::reason_list(&a.reasons, ", ")
    ));
    // every gate in policy order: the first failed one decided the disposition
    let gates: Vec<String> = a
        .gates
        .iter()
        .map(|g| {
            format!(
                "{} {}",
                g.gate.as_str(),
                if g.passed { "passed" } else { "failed" }
            )
        })
        .collect();
    lines.push(format!("  gates:        {}", gates.join(", ")));
    if let Some(n) = &a.next_action {
        lines.push(format!("  next:         {n}"));
    }
    if let Some(line) = issue_line(a) {
        lines.push(format!("  issue:        {line}"));
    }
    lines.push(format!(
        "  against:      master {} · head {} (observed {})",
        short(&a.evaluated_against.master_sha),
        short(&a.evaluated_against.head_sha),
        q.observed_at
    ));
    lines.push(format!("  rank:         {rank} of {}", q.assessments.len()));
    if let Some(f) = &a.rank_factors {
        // the components the order compares, in its order: the first that differs decides
        lines.push(format!(
                "  rank factors: lane {}, {}, risk {}, contention {}, dependents {}, paths {}, opened {}",
                integration::classify::word(&f.lane),
                f.disposition.as_str(),
                integration::classify::word(&f.risk),
                f.contention,
                f.dependents,
                f.authored_paths,
                f.created_at
            ));
    }
    if let Some(wait) = &a.wait {
        lines.push(format!("  waiting:      {}", waited(wait)));
    }
    lines.push(format!(
        "  risk:         {} — {}",
        integration::classify::word(&a.risk),
        a.risk_factors.join("; ")
    ));
    let relation = match &a.relation {
        RelationToMaster::Behind { behind, .. } => format!("behind master by {behind}"),
        RelationToMaster::Conflicting { paths } => format!("conflicts on {}", paths.join(", ")),
        r => integration::classify::word(r),
    };
    lines.push(format!("  relation:     {relation}"));
    lines.push(format!(
        "  checks:       {}",
        integration::classify::word(&a.required_checks)
    ));
    lines.push(format!(
        "  review:       {}",
        integration::classify::word(&a.review)
    ));
    for d in &a.dependencies {
        lines.push(format!(
            "  depends on:   #{} ({}, {})",
            d.number,
            integration::classify::word(&d.certainty),
            match d.state {
                integration::DependencyState::Merged => "landed",
                integration::DependencyState::Open => "open",
                integration::DependencyState::ClosedUnmerged => "closed without a merge",
                integration::DependencyState::Unread => "unread",
            }
        ));
    }
    if !a.overlaps.is_empty() {
        let o: Vec<String> = a
            .overlaps
            .iter()
            .map(|o| match o.kind {
                integration::OverlapKind::Authored => format!("#{} ({})", o.number, o.paths.len()),
                kind => format!(
                    "#{} ({}, {})",
                    o.number,
                    o.paths.len(),
                    integration::classify::word(&kind)
                ),
            })
            .collect();
        lines.push(format!("  overlaps:     {}", o.join(" ")));
    }
    lines.push(format!(
        "  authored:     {} path(s)",
        a.authored_paths.len()
    ));
    lines.push("  evidence:".into());
    for e in &a.evidence {
        lines.push(format!("    {:<20} {:<14} {}", e.kind, e.status, e.detail));
    }
    w(out, lines.join("\n"))
}

#[cfg(test)]
mod tests {
    //! The renderings of `majordomus prs`, over a queue built the way the command builds
    //! it: a recorded observation and git's answer about each head, in a scratch clone.

    use super::*;
    use crate::integration::{
        drain::{IntegrationAction, IntegrationEvent},
        store_observation, wait, CheckObservation, CheckRunState, ForgeObservation,
        PullRequestObservation, OBSERVATION_SCHEMA,
    };

    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn git(root: &std::path::Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .expect("git");
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn pr(n: u64, sha: &str, state: CheckRunState) -> PullRequestObservation {
        PullRequestObservation {
            number: n,
            title: format!("change number {n} with a title long enough to be cut short somewhere"),
            author: "someone".into(),
            head_ref: format!("feature/{n}"),
            head_sha: sha.into(),
            base_ref: "master".into(),
            draft: false,
            labels: vec![],
            created_at: format!("2026-09-0{n}T00:00:00Z"),
            updated_at: format!("2026-09-0{n}T00:00:00Z"),
            body: String::new(),
            checks: vec![CheckObservation {
                name: "ci".into(),
                state,
                ..Default::default()
            }],
            review_decision: String::new(),
            auto_merge: false,
            cross_repository: false,
            latest_reviews: Vec::new(),
            review_requests: Vec::new(),
        }
    }

    /// A clone with an origin/master, an observation of five pull requests in five
    /// dispositions, and a trail in which #1 has been passed over until it is starving.
    fn world() -> (Scratch, IntegrationQueue) {
        // tests run on parallel threads: the clock alone named two scratch clones alike
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "mj-prs-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "master"]);
        git(&root, &["commit", "-q", "--allow-empty", "-m", "base"]);
        let sha = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["update-ref", "refs/remotes/origin/master", &sha]);

        // every pull request's head is a commit of its own on top of master: a head master
        // already contains would be superseded, not open work
        let head = |n: u64| {
            git(
                &root,
                &["checkout", "-q", "-B", &format!("feature/{n}"), &sha],
            );
            std::fs::write(root.join(format!("{n}.md")), format!("change {n}\n")).unwrap();
            git(&root, &["add", &format!("{n}.md")]);
            git(&root, &["commit", "-q", "-m", &format!("change {n}")]);
            git(&root, &["rev-parse", "HEAD"])
        };
        let heads: Vec<String> = (1..=6).map(head).collect();
        git(&root, &["checkout", "-q", "master"]);
        let mut draft = pr(4, &heads[3], CheckRunState::Passed);
        draft.draft = true;
        let mut held = pr(5, &heads[4], CheckRunState::Passed);
        held.labels = vec!["blocked".into()];
        let mut dependent = pr(6, &heads[5], CheckRunState::Passed);
        dependent.body = "Depends on #2.".into();
        store_observation(
            &root,
            &ForgeObservation {
                schema: OBSERVATION_SCHEMA,
                repository: "owner/repo".into(),
                base: "master".into(),
                base_sha: sha.clone(),
                observed_at: "2026-10-01T00:00:00Z".into(),
                required_checks: Some(vec!["ci".into()]),
                review_policy: Some(Default::default()),
                up_to_date_required: Some(true),
                merge_methods: vec!["merge".into()],
                pull_requests: vec![
                    pr(1, &heads[0], CheckRunState::Passed),
                    pr(2, &heads[1], CheckRunState::Failed),
                    pr(3, &heads[2], CheckRunState::Pending),
                    draft,
                    held,
                    dependent,
                ],
                resolved: Default::default(),
                delete_branch_on_merge: None,
            },
        )
        .unwrap();
        let event = |action: IntegrationAction, pr: u64, over: Vec<u64>| IntegrationEvent {
            at: "2026-10-01T00:00:00Z".into(),
            actor: "test".into(),
            pr: Some(pr),
            master_before: Some(sha.clone()),
            head_sha: Some(sha.clone()),
            passed_over: over,
            ..IntegrationEvent::of(action)
        };
        drain::record(&root, event(wait::BECAME_ACTIONABLE, 1, vec![])).unwrap();
        for _ in 0..wait::STARVING_AFTER {
            drain::record(&root, event(IntegrationAction::Selected, 9, vec![1])).unwrap();
        }
        let q = integration::queue_and_record(&root).expect("a queue");
        (Scratch(root), q)
    }

    fn text(f: impl FnOnce(&mut Vec<u8>) -> Result<()>) -> String {
        let mut out = Vec::new();
        f(&mut out).expect("rendered");
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn status_lists_every_pull_request_in_rank_order_and_names_the_starving() {
        let (_s, q) = world();
        let t = text(|o| status(&q, OutputFormat::Text, o));
        assert!(t.starts_with("owner/repo · master at "), "{t}");
        assert!(t.contains("lanes: "), "{t}");
        for n in 1..=6 {
            assert!(t.contains(&format!("#{n} ")), "#{n} missing:\n{t}");
        }
        assert!(t.contains("starving: #1 actionable since"), "{t}");
        assert!(t.contains("passed over 3×"), "{t}");
        assert!(t.contains('…') || t.lines().all(|l| l.chars().count() < 160));
        let j: serde_json::Value =
            serde_json::from_str(&text(|o| status(&q, OutputFormat::Json, o))).unwrap();
        assert_eq!(j["assessments"].as_array().unwrap().len(), 6);
        assert_eq!(j["starving"], serde_json::json!([1]));
    }

    #[test]
    fn the_plan_names_the_next_merge_and_every_lane() {
        let (_s, q) = world();
        let t = text(|o| plan(&q, OutputFormat::Text, o));
        assert!(t.contains("#1"), "{t}");
        let j: serde_json::Value =
            serde_json::from_str(&text(|o| plan(&q, OutputFormat::Json, o))).unwrap();
        assert_eq!(j["next_merge"], 1, "{j}");
        assert!(j["repair"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!(2)));
    }

    #[test]
    fn explain_says_why_each_pull_request_is_where_it_is() {
        let (_s, q) = world();
        for (rank, a) in q.assessments.iter().enumerate() {
            let t = text(|o| explain(&q, a, rank + 1, OutputFormat::Text, o));
            assert!(t.starts_with(&format!("#{} ", a.number)), "{t}");
            assert!(t.contains(a.disposition.as_str()), "{t}");
            let j: serde_json::Value =
                serde_json::from_str(&text(|o| explain(&q, a, rank + 1, OutputFormat::Json, o)))
                    .unwrap();
            assert_eq!(j["rank"], rank + 1);
            assert_eq!(j["assessment"]["number"], a.number);
        }
    }

    #[test]
    fn explain_names_the_successor_that_landed() {
        let (_s, q) = world();
        use crate::integration::{
            DependencyCertainty, DependencyState, OverlapKind, PathOverlap, PullRequestDependency,
        };
        let mut a = q.assessments[0].clone();
        a.superseded_by = Some(9);
        a.next_action = None;
        a.dependencies = [
            (5, DependencyState::Open),
            (4, DependencyState::Merged),
            (6, DependencyState::ClosedUnmerged),
            (7, DependencyState::Unread),
        ]
        .into_iter()
        .map(|(number, state)| PullRequestDependency {
            number,
            certainty: DependencyCertainty::Confirmed,
            satisfied: state == DependencyState::Merged,
            state,
        })
        .collect();
        a.overlaps = vec![
            PathOverlap {
                number: 3,
                paths: vec!["src/a.rs".into()],
                kind: OverlapKind::Authored,
            },
            PathOverlap {
                number: 8,
                paths: vec!["apps/majordomus-cli/Cargo.toml".into()],
                kind: OverlapKind::VersionBump,
            },
        ];
        let t = text(|o| explain(&q, &a, 1, OutputFormat::Text, o));
        assert!(t.contains("superseded:   by #9, which landed"), "{t}");
        assert!(!t.contains("  next:"), "{t}");
        assert!(t.contains("depends on:   #5 (confirmed, open)"), "{t}");
        assert!(t.contains("depends on:   #4 (confirmed, landed)"), "{t}");
        assert!(
            t.contains("depends on:   #6 (confirmed, closed without a merge)"),
            "{t}"
        );
        assert!(t.contains("depends on:   #7 (confirmed, unread)"), "{t}");
        assert!(
            t.contains("overlaps:     #3 (1) #8 (1, version_bump)"),
            "{t}"
        );
    }

    #[test]
    fn a_dry_run_proof_says_each_step_and_what_moved() {
        use crate::integration::proof::{
            DryRunProof, Moved, ObservedClassification, ProofStep, Snapshot,
        };
        let empty = Snapshot {
            sections: Vec::new(),
        };
        let mut proof = DryRunProof {
            ok: true,
            base: "master".into(),
            steps: vec![ProofStep {
                step: "refresh".into(),
                summary: "observed 1".into(),
            }],
            moved: Vec::new(),
            mirrors: Vec::new(),
            before: empty.clone(),
            after: empty,
            classification: vec![ObservedClassification {
                number: 1,
                head_sha: "a".repeat(40),
                disposition: "ready".into(),
                next_action: None,
            }],
        };
        let t = text(|o| render_proof(&proof, o));
        assert!(t.starts_with("refresh          observed 1\n"), "{t}");
        assert!(t.contains("  #1  aaaaaaaaaaaa  ready\n"), "{t}");
        assert!(
            t.ends_with("nothing moved (remote, forge, trail, lease, local refs)\n"),
            "{t}"
        );
        proof.ok = false;
        proof.moved = ["added", "removed"]
            .into_iter()
            .map(|change| Moved {
                section: "trail".into(),
                change: change.into(),
                line: "x".into(),
            })
            .collect();
        proof.mirrors = vec!["refs/x is y, origin serves z".into()];
        let t = text(|o| render_proof(&proof, o));
        assert!(
            t.contains("moved something:\n  + trail  x\n  - trail  x\n"),
            "{t}"
        );
        assert!(t.contains("mirror: refs/x is y"), "{t}");
        assert!(!t.contains("nothing moved"), "{t}");
    }

    #[test]
    fn a_halt_that_names_no_pull_request_says_a_pull_request() {
        let said = describe(&DrainStepOutcome::Halted {
            pr: None,
            reason: "unproved".into(),
        });
        assert!(
            said.starts_with("halted: a pull request was merged"),
            "{said}"
        );
    }

    #[test]
    fn the_brief_is_one_line_from_files_and_absent_without_an_observation() {
        let (s, _q) = world();
        let b = brief(&s.0).expect("a brief");
        assert!(b.contains("master at "), "{b}");
        assert!(b.contains("next merge #1"), "{b}");
        assert!(b.contains("starving #1"), "{b}");
        assert!(!b.contains("last merge") && !b.contains("; last "), "{b}");

        // the trail's last merge, and the last step that says how the work went: the newest of
        // a refresh, a failed or unverified merge, a stale decision — a merge is said already
        let event = |action, pr: Option<u64>| drain::IntegrationEvent {
            at: "2026-10-01T00:00:00Z".into(),
            pr,
            ..drain::IntegrationEvent::of(action)
        };
        use drain::IntegrationAction as A;
        for e in [
            event(A::MergeFailed, Some(4)),
            event(A::MergeSucceeded, Some(5)),
            event(A::StaleDecision, Some(6)),
            event(A::LeaseReleased, None),
        ] {
            drain::record(&s.0, e).expect("recorded");
        }
        let lease = IntegrationLease::acquire(&s.0, "master").expect("the lease");
        let b = brief(&s.0).expect("a brief");
        assert!(b.contains("last merge #5"), "{b}");
        assert!(b.contains("last stale_decision #6"), "{b}");
        assert!(
            !b.contains("merge_failed"),
            "only the newest outcome is said: {b}"
        );
        assert!(b.contains("per-clone guard only"), "{b}");
        drop(lease);
        drain::record(&s.0, event(A::Refreshed, None)).expect("recorded");
        let b = brief(&s.0).expect("a brief");
        assert!(
            b.contains("last refreshed "),
            "an outcome without a pull request: {b}"
        );

        let empty = std::env::temp_dir().join(format!("mj-prs-empty-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        assert!(
            brief(&empty).is_none(),
            "a checkout that never observed says nothing"
        );
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[test]
    fn every_drain_outcome_has_its_own_sentence() {
        let outcomes = [
            DrainStepOutcome::Idle {
                why: "nothing".into(),
            },
            DrainStepOutcome::WouldMerge { pr: 1 },
            DrainStepOutcome::StaleDecision {
                pr: 2,
                what: "master moved".into(),
            },
            DrainStepOutcome::Merged {
                pr: 3,
                master_before: "a".repeat(40),
                master_after: "b".repeat(40),
            },
            DrainStepOutcome::MergeRefused {
                pr: 4,
                reason: "protected".into(),
                class: drain::FailureClass::PolicyViolation,
            },
            DrainStepOutcome::VerificationFailed {
                pr: 5,
                reason: "absent".into(),
            },
            DrainStepOutcome::WouldRefresh { pr: 6 },
            DrainStepOutcome::Refreshed {
                pr: 7,
                head_before: "c".repeat(40),
                head_after: "d".repeat(40),
            },
            DrainStepOutcome::AwaitingChecks { pr: 8 },
            DrainStepOutcome::RefreshFailed {
                pr: 9,
                reason: "conflict".into(),
                class: drain::FailureClass::Conflict,
            },
            DrainStepOutcome::TrailUnwritable {
                pr: 10,
                unrecorded: IntegrationAction::MergeAttempted,
                reason: "read-only".into(),
            },
        ];
        let said: std::collections::BTreeSet<String> = outcomes.iter().map(describe).collect();
        assert_eq!(
            said.len(),
            outcomes.len(),
            "two outcomes read alike: {said:?}"
        );
        assert!(describe(&outcomes[3]).contains("aaaaaaaaaa -> bbbbbbbbbb"));
        assert!(describe(&outcomes[10]).contains("merge_attempted"));
    }

    #[test]
    fn a_drain_that_could_not_record_an_act_exits_unusable() {
        let report = |steps: Vec<DrainStepOutcome>| drain::DrainReport {
            dry_run: false,
            steps,
            merged: Vec::new(),
            stopped: String::new(),
        };
        let unrecorded = DrainStepOutcome::TrailUnwritable {
            pr: 1,
            unrecorded: IntegrationAction::MergeAttempted,
            reason: "read-only".into(),
        };
        let unverified = DrainStepOutcome::VerificationFailed {
            pr: 2,
            reason: "absent".into(),
        };
        assert_eq!(drain_exit(&report(vec![unrecorded])), UNUSABLE);
        assert_eq!(drain_exit(&report(vec![unverified])), FINDING);
        assert_eq!(
            drain_exit(&report(vec![DrainStepOutcome::WouldMerge { pr: 3 }])),
            0
        );
    }

    #[test]
    fn a_moment_is_said_as_how_long_ago() {
        assert_eq!(ago("not a time"), "", "an unreadable moment says nothing");
        let at = |secs: u64| {
            crate::peers::rfc3339(
                std::time::SystemTime::now() - std::time::Duration::from_secs(secs),
            )
        };
        assert!(ago(&at(5)).ends_with(" s ago"), "{}", ago(&at(5)));
        assert!(ago(&at(600)).ends_with(" min ago"), "{}", ago(&at(600)));
        assert!(
            ago(&at(5 * 3600)).ends_with(" h ago"),
            "{}",
            ago(&at(5 * 3600))
        );
        assert!(
            ago(&at(3 * 86_400)).ends_with(" d ago"),
            "{}",
            ago(&at(3 * 86_400))
        );
    }

    #[test]
    fn a_drain_says_why_it_stopped_once() {
        let report = |steps: Vec<DrainStepOutcome>, stopped: &str| drain::DrainReport {
            dry_run: true,
            steps,
            merged: vec![],
            stopped: stopped.into(),
        };
        let why = "nothing is ready; 7 pull request(s) need master brought in first: #1";
        // an idle step is the reason itself: said once, not again as `stopped:`
        let idle = drain_lines(&report(
            vec![DrainStepOutcome::Idle { why: why.into() }],
            why,
        ));
        assert_eq!(idle, [format!("idle: {why}")]);
        assert_eq!(idle.join("; ").matches(why).count(), 1, "{idle:?}");
        // a halt says why nothing began, once
        let halted = drain_lines(&report(
            vec![DrainStepOutcome::Halted {
                pr: Some(4),
                reason: "the merge commit is not on master".into(),
            }],
            "#4 could not be verified after merging (the merge commit is not on master)",
        ));
        assert_eq!(halted.len(), 1, "{halted:?}");
        assert!(halted[0].starts_with("halted: #4"), "{halted:?}");
        // a step that does not end the drain by itself is followed by why the drain ended
        let bounded = drain_lines(&report(
            vec![DrainStepOutcome::WouldMerge { pr: 3 }],
            "dry run: #3 would be merged; nothing after it is planned",
        ));
        assert_eq!(
            bounded,
            [
                "would merge #3".to_string(),
                "stopped: dry run: #3 would be merged; nothing after it is planned".to_string()
            ]
        );
        // and a drain that took no step and gave no reason says nothing at all
        assert!(drain_lines(&report(vec![], "")).is_empty());
    }

    #[test]
    fn every_repair_outcome_has_its_own_sentence_and_a_refusal_names_the_files() {
        let report = |dry_run: bool, outcome: repair::RepairOutcome| repair::RepairReport {
            dry_run,
            target: "#7".into(),
            pr: Some(7),
            branch: Some("feature/7".into()),
            base: "master".into(),
            master_sha: "m".repeat(40),
            head_sha: Some("h".repeat(40)),
            observed_at: "2026-10-01T00:00:00Z".into(),
            relation: Some(RelationToMaster::Behind {
                behind: 1,
                authored: vec!["src/a.rs".into()],
            }),
            outcome,
            diagnostics: vec![],
        };
        let say = |r: repair::RepairReport| text(|o| render_repair(&r, o));
        let would = say(report(true, repair::RepairOutcome::WouldRepair));
        assert!(
            would.contains("#7 (feature/7): head hhhhhhhhhh against master mmmmmmmmmm, behind"),
            "{would}"
        );
        assert!(
            would.contains("observed at 2026-10-01T00:00:00Z"),
            "{would}"
        );
        assert!(
            would.contains("dry run: would merge master mmmmmmmmmm into #7 (feature/7)"),
            "{would}"
        );
        assert!(
            would.contains("leased on hhhhhhhhhh; --apply does"),
            "{would}"
        );
        let done = say(report(
            false,
            repair::RepairOutcome::Repaired {
                head_after: "n".repeat(40),
            },
        ));
        assert!(
            done.contains(
                "repaired #7 (feature/7): it carries master mmmmmmmmmm and is at nnnnnnnnnn"
            ),
            "{done}"
        );
        assert!(
            !done.contains("observed at"),
            "an act is not a dry run: {done}"
        );
        let nothing = say(report(
            true,
            repair::RepairOutcome::NothingToRepair {
                why: "its head already contains master".into(),
            },
        ));
        assert!(
            nothing.contains("nothing to repair: its head already contains master"),
            "{nothing}"
        );
        let refused = say(report(
            true,
            repair::RepairOutcome::Refused {
                refusal: repair::RepairRefusal::AuthoredConflict {
                    paths: vec!["src/a.rs".into(), "src/b.rs".into()],
                },
            },
        ));
        assert!(
            refused
                .contains("REFUSE #7 (feature/7): merging master conflicts on 2 authored file(s)"),
            "{refused}"
        );
        assert!(
            refused.contains("\n       src/a.rs\n       src/b.rs\n"),
            "{refused}"
        );
        assert!(
            refused.contains("merge master in the branch's own worktree"),
            "{refused}"
        );
        // what is not open is named as it was given
        let mut absent = report(
            true,
            repair::RepairOutcome::Refused {
                refusal: repair::RepairRefusal::NotOpen {
                    target: "#7".into(),
                },
            },
        );
        absent.pr = None;
        absent.branch = None;
        absent.head_sha = None;
        let absent = say(absent);
        assert!(absent.starts_with("decided on the forge"), "{absent}");
        assert!(
            absent.contains("REFUSE #7: #7 is not an open pull request"),
            "{absent}"
        );
    }

    #[test]
    fn a_repair_exits_by_what_it_could_vouch_for() {
        let report = |dry_run: bool, outcome: repair::RepairOutcome, diagnostics: &[&str]| {
            repair::RepairReport {
                dry_run,
                target: "#7".into(),
                pr: Some(7),
                branch: Some("feature/7".into()),
                base: "master".into(),
                master_sha: "m".repeat(40),
                head_sha: Some("h".repeat(40)),
                observed_at: "2026-10-01T00:00:00Z".into(),
                relation: None,
                outcome,
                diagnostics: diagnostics.iter().map(|d| d.to_string()).collect(),
            }
        };
        let refused = |refusal| repair::RepairOutcome::Refused { refusal };
        let unwritable = refused(repair::RepairRefusal::TrailUnwritable {
            reason: "disk full".into(),
        });
        // an act the trail could not announce was not taken: the tool is unusable here
        assert_eq!(repair_exit(&report(false, unwritable, &[])), UNUSABLE);
        let forked = refused(repair::RepairRefusal::ForkHead);
        assert_eq!(repair_exit(&report(false, forked, &[])), FINDING);
        let would = || repair::RepairOutcome::WouldRepair;
        assert_eq!(repair_exit(&report(true, would(), &[])), 0);
        assert_eq!(
            repair_exit(&report(true, would(), &["stale observation"])),
            FINDING,
            "a dry run over a diagnosed observation vouches for nothing"
        );
        let repaired = repair::RepairOutcome::Repaired {
            head_after: "n".repeat(40),
        };
        assert_eq!(
            repair_exit(&report(false, repaired, &["stale observation"])),
            0,
            "an act decides on its own fresh observation"
        );
    }
}

#[cfg(test)]
mod issue_line_tests {
    use super::*;

    #[test]
    fn the_issue_line_names_the_milestone_when_there_is_one() {
        let q = crate::integration::issue_test_queue(&["feature/I0810-x"]);
        let mut a = q.assessments[0].clone();
        assert_eq!(issue_line(&a), None);
        a.issue = Some("I0810".into());
        assert_eq!(issue_line(&a).as_deref(), Some("I0810"));
        a.milestone = Some("M003".into());
        assert_eq!(issue_line(&a).as_deref(), Some("I0810 · milestone M003"));
    }

    #[test]
    fn explain_says_the_issue_and_milestone() {
        let mut q = crate::integration::issue_test_queue(&["feature/I0810-x"]);
        q.assessments[0].issue = Some("I0810".into());
        q.assessments[0].milestone = Some("M003".into());
        let a = q.assessments[0].clone();
        let mut out = Vec::new();
        explain(&q, &a, 1, OutputFormat::Text, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("issue:        I0810 · milestone M003"),
            "{text}"
        );
    }
}

#[cfg(test)]
mod issue_write_tests {
    use super::*;

    /// A writer that refuses the write carrying `needle`.
    struct RefuseOn(&'static str);

    impl Write for RefuseOn {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if String::from_utf8_lossy(buf).contains(self.0) {
                Err(std::io::Error::other("refused"))
            } else {
                Ok(buf.len())
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_refused_issue_line_is_an_error() {
        let mut q = crate::integration::issue_test_queue(&["feature/I0810-x"]);
        q.assessments[0].issue = Some("I0810".into());
        let a = q.assessments[0].clone();
        assert!(explain(&q, &a, 1, OutputFormat::Text, &mut RefuseOn("issue:")).is_err());
    }
}

#[cfg(test)]
mod left_branch_render_tests {
    use super::*;

    fn rendered(left: &Option<Vec<drain::LeftBranch>>) -> String {
        let mut out = Vec::new();
        left_branches(left, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn every_answer_of_the_report_is_said() {
        assert!(rendered(&None).contains("merged branches: unread"));
        assert!(rendered(&Some(Vec::new())).contains("none left on origin"));
        let b = drain::LeftBranch {
            branch: "fix/a".into(),
            tip: "0123456789abcdef".into(),
            pr: 7,
            merged_at: "t".into(),
            action: "left_for_a_person".into(),
            next_step: "delete it".into(),
        };
        let text = rendered(&Some(vec![b]));
        assert!(text.contains("left on origin (1)"), "{text}");
        assert!(
            text.contains("fix/a") && text.contains("#7") && text.contains("0123456789ab"),
            "{text}"
        );
        assert!(text.contains("next: delete it"), "{text}");
    }

    /// A writer that refuses the write carrying `needle`, so each line's refusal is reached.
    struct RefuseOn(&'static str);

    impl Write for RefuseOn {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if String::from_utf8_lossy(buf).contains(self.0) {
                Err(std::io::Error::other("refused"))
            } else {
                Ok(buf.len())
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_refused_write_is_an_error_on_every_line() {
        let b = drain::LeftBranch {
            branch: "fix/a".into(),
            tip: "0123456789abcdef".into(),
            pr: 7,
            merged_at: "t".into(),
            action: "left_for_a_person".into(),
            next_step: "delete it".into(),
        };
        for needle in ["left on origin", "fix/a", "next:"] {
            assert!(
                left_branches(&Some(vec![b.clone()]), &mut RefuseOn(needle)).is_err(),
                "{needle}"
            );
        }
    }

    #[test]
    fn an_unreadable_observation_fails_the_branch_report() {
        let dir = tempfile::tempdir().unwrap();
        let path = crate::integration::state_path(dir.path(), crate::integration::OBSERVATION_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "not an observation").unwrap();
        assert!(cleanup_branches(dir.path(), &mut Vec::new()).is_err());
    }

    #[test]
    fn cleanup_with_nothing_observed_reports_unread() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Vec::new();
        cleanup_branches(dir.path(), &mut out).unwrap();
        assert!(String::from_utf8(out)
            .unwrap()
            .contains("merged branches: unread"));
    }
}
