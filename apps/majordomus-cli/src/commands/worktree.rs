//! `majordomus worktree`: the command line's rendering of the worktree service.
//!
//! Every subcommand calls [`crate::worktree::WorktreeService`] directly — the same service
//! the capability registry's `worktree.*` handlers call — and prints the same typed
//! answers. It does not build the repository's index: a guard that runs on every commit
//! and a status that runs on every directory change cannot afford a two-second startup,
//! and nothing about the topology is in the index anyway. The one exception is
//! `create --issue`, which reads the issue's record from the index to name the branch.
//!
//! Nothing here decides anything: no path is derived, no standing is judged, no safety
//! check is made in this file. It reads arguments, calls the service, and prints. The human
//! form and the JSON form are two renderings of one typed answer.

use std::io::Write;

use serde::Serialize;
use serde_json::Value;

use crate::cli::{OutputFormat, WorktreeArgs, WorktreeCommand};
use crate::error::{Error, Result};
use crate::worktree::{
    migrate, BranchState, CreateRequest, Detail, DirtyState, MigrationAction, MigrationOptions,
    MigrationPlan, ReconcileEntry, ReconcileOptions, ReconcileOutcome, Reconciliation,
    RepositoryTopology, Severity, Standing, StatusReport, StepOutcome, TopologyDiagnostic,
    WorktreeService, WorktreeState, EXIT_MISSING, EXIT_REFUSED,
};

/// Run `majordomus worktree`.
pub fn run(args: WorktreeArgs) -> Result<u8> {
    let start = match &args.repo.repo {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let format = args.format;
    let svc = || WorktreeService::open(&start).map_err(refuse);
    match args.command {
        None | Some(WorktreeCommand::Status) => status(&svc()?, format, &mut out),
        Some(WorktreeCommand::List) => list(&svc()?, format, &mut out),
        Some(WorktreeCommand::Topology) => topology(&svc()?, format, &mut out),
        Some(WorktreeCommand::Root) => root(&svc()?, format, &mut out),
        Some(WorktreeCommand::Path { branch }) => path(&svc()?, &branch, &mut out),
        Some(WorktreeCommand::Inspect { branch }) => inspect(&svc()?, &branch, format, &mut out),
        Some(WorktreeCommand::Create {
            branch,
            base,
            issue,
        }) => {
            let branch = match (branch, issue) {
                (Some(b), _) => b,
                (None, Some(id)) => issue_branch(&args.repo, &id)?,
                (None, None) => {
                    return Err(Error::Refused {
                        code: crate::cli::EXIT_USAGE,
                        reason: "worktree create: a branch or --issue is required".into(),
                    })
                }
            };
            create(&svc()?, &branch, base, false, format, &mut out)
        }
        Some(WorktreeCommand::Ensure { branch, base }) => {
            create(&svc()?, &branch, base, true, format, &mut out)
        }
        Some(WorktreeCommand::Migrate {
            plan,
            dry_run,
            allow_copy,
            only,
            include_ephemeral,
        }) => migrate_cmd(
            &svc()?,
            !(plan || dry_run),
            MigrationOptions {
                allow_copy,
                only,
                include_ephemeral,
            },
            format,
            &mut out,
        ),
        Some(WorktreeCommand::Validate) => doctor(&svc()?, format, true, &mut out),
        Some(WorktreeCommand::Doctor) => doctor(&svc()?, format, false, &mut out),
        Some(WorktreeCommand::Guard { quiet }) => guard(&svc()?, format, quiet, &mut out),
        Some(WorktreeCommand::Repair { dry_run }) => repair(&svc()?, dry_run, format, &mut out),
        Some(WorktreeCommand::Remove { selector, force }) => {
            remove(&svc()?, &selector, force, format, &mut out)
        }
        Some(WorktreeCommand::Cleanup { remove }) => cleanup(&svc()?, format, remove, &mut out),
        Some(WorktreeCommand::Reconcile {
            selector,
            apply,
            include_scratch,
        }) => reconcile(
            &svc()?,
            selector.as_deref(),
            apply.then_some(ReconcileOptions { include_scratch }),
            format,
            &mut out,
        ),
        Some(WorktreeCommand::Branches { without_worktree }) => {
            branches(&svc()?, without_worktree, &mut out)
        }
    }
}

type Out<'a> = std::io::StdoutLock<'a>;

fn w(out: &mut Out<'_>, s: impl AsRef<str>) -> Result<()> {
    writeln!(out, "{}", s.as_ref()).map_err(Error::Transport)
}

fn json<T: Serialize>(out: &mut Out<'_>, v: &T) -> Result<()> {
    let value = serde_json::to_value(v).unwrap_or(Value::Null);
    w(
        out,
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
    )
}

/// A refusal from the service becomes this executable's error with the service's own words
/// and its own exit code. Nothing is rephrased and no debug struct reaches a person.
fn refuse(e: crate::worktree::WorktreeError) -> Error {
    Error::Refused {
        code: e.exit_code(),
        reason: e.to_string(),
    }
}

fn short(head: &Option<String>) -> String {
    head.as_deref()
        .map(|h| h[..12.min(h.len())].to_string())
        .unwrap_or_else(|| "-".into())
}

fn dirty_word(d: &Option<DirtyState>) -> String {
    match d {
        None => "-".into(),
        Some(d) => d.summary(),
    }
}

// ---------------------------------------------------------------- read-only

fn root(svc: &WorktreeService, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let t = svc.topology(Detail::Fast).map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &t.container)?,
        // One path and nothing else: this output is consumed by `cd "$(...)"`.
        OutputFormat::Text => w(out, &t.container.path)?,
    }
    Ok(0)
}

fn path(svc: &WorktreeService, branch: &str, out: &mut Out<'_>) -> Result<u8> {
    let p = svc.expected_path_of(branch).map_err(refuse)?;
    w(out, p.display().to_string())?;
    Ok(0)
}

fn render_worktree_line(w_: &WorktreeState) -> String {
    let standing = w_.standing.as_str().to_uppercase();
    let here = if w_.current { "  (you are here)" } else { "" };
    let mut line = format!("{standing:<10} {:<44} {}{here}", w_.label, w_.path);
    if let Some(e) = &w_.expected_path {
        if w_.standing == Standing::Misplaced {
            line.push_str(&format!("\n           -> belongs at {e}"));
        }
    }
    if let Some(d) = &w_.dirty {
        if !d.clean {
            line.push_str(&format!("\n           dirty: {}", d.summary()));
        }
    }
    if let Some(u) = &w_.upstream {
        if u.gone || u.ahead.unwrap_or(0) > 0 || u.behind.unwrap_or(0) > 0 {
            line.push_str(&format!(
                "\n           upstream {}: {}",
                u.name,
                if u.gone {
                    "gone".to_string()
                } else {
                    format!(
                        "ahead {}, behind {}",
                        u.ahead.unwrap_or(0),
                        u.behind.unwrap_or(0)
                    )
                }
            ));
        }
    }
    if let Some(i) = &w_.issue {
        line.push_str(&format!("\n           issue {i}"));
    }
    line
}

fn render_diagnostic(d: &TopologyDiagnostic) -> String {
    let mut s = format!(
        "{:<8} {}  {}",
        d.severity.as_str().to_uppercase(),
        d.code.as_str(),
        d.message
    );
    if let Some(p) = &d.path {
        s.push_str(&format!("\n         at {p}"));
    }
    if let Some(e) = &d.expected {
        s.push_str(&format!("\n         expected {e}"));
    }
    s.push_str(&format!("\n         [remedy: {}]", d.remedy));
    s
}

fn header(t: &RepositoryTopology, out: &mut Out<'_>) -> Result<()> {
    w(
        out,
        format!("repository  {}", t.repository.primary_worktree),
    )?;
    w(
        out,
        format!(
            "container   {}{}",
            t.container.path,
            if t.container.exists {
                ""
            } else {
                "  (not created yet)"
            }
        ),
    )?;
    w(
        out,
        format!(
            "trunk       {}  ({})",
            t.trunk.branch.as_deref().unwrap_or("(unknown)"),
            trunk_source_word(t.trunk.source)
        ),
    )?;
    Ok(())
}

fn trunk_source_word(source: crate::worktree::TrunkSource) -> &'static str {
    use crate::worktree::TrunkSource::*;
    match source {
        RemoteHead => "the remote's HEAD",
        DefaultBranchConfig => "init.defaultBranch",
        ConventionalName => "the conventional name",
        PrimaryCheckout => "the primary checkout's branch",
        Unknown => "unknown",
    }
}

fn list(svc: &WorktreeService, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let t = svc.topology(Detail::Full).map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &t.worktrees)?,
        OutputFormat::Text => {
            header(&t, out)?;
            w(out, "")?;
            for w_ in &t.worktrees {
                w(out, render_worktree_line(w_))?;
            }
            w(out, "")?;
            w(out, tallies_line(&t))?;
        }
    }
    Ok(if t.valid { 0 } else { EXIT_REFUSED })
}

fn tallies_line(t: &RepositoryTopology) -> String {
    let ta = &t.tallies;
    format!(
        "{} worktree(s): {} canonical, {} misplaced, {} detached, {} missing; {} branch(es), {} without a worktree, {} cleanup-eligible; {} error(s), {} warning(s)",
        ta.worktrees, ta.canonical, ta.misplaced, ta.detached, ta.missing, ta.branches,
        ta.branches_without_worktree, ta.cleanup_eligible, ta.errors, ta.warnings
    )
}

fn topology(svc: &WorktreeService, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let t = svc.topology(Detail::Full).map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &t)?,
        OutputFormat::Text => {
            header(&t, out)?;
            w(out, "")?;
            w(out, "worktrees")?;
            for w_ in &t.worktrees {
                w(
                    out,
                    format!("  {}", render_worktree_line(w_).replace('\n', "\n  ")),
                )?;
            }
            let without: Vec<&BranchState> =
                t.branches.iter().filter(|b| b.worktree.is_none()).collect();
            if !without.is_empty() {
                w(out, "")?;
                w(out, "branches without a worktree")?;
                for b in without {
                    w(
                        out,
                        format!(
                            "  {:<44} {}{}",
                            b.name,
                            short(&Some(b.head.clone())),
                            if b.cleanup_eligible {
                                "  merged, cleanup-eligible"
                            } else if b.merged_into_trunk == Some(true) {
                                "  merged"
                            } else {
                                ""
                            }
                        ),
                    )?;
                }
            }
            if !t.diagnostics.is_empty() {
                w(out, "")?;
                w(out, "diagnostics")?;
                for d in &t.diagnostics {
                    w(
                        out,
                        format!("  {}", render_diagnostic(d).replace('\n', "\n  ")),
                    )?;
                }
            }
            w(out, "")?;
            w(out, tallies_line(&t))?;
        }
    }
    Ok(if t.valid { 0 } else { EXIT_REFUSED })
}

fn status(svc: &WorktreeService, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let s: StatusReport = svc.status().map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &s)?,
        OutputFormat::Text => {
            let wt = &s.worktree;
            w(out, format!("branch      {}", wt.label))?;
            w(
                out,
                format!(
                    "worktree    {} {}",
                    if s.canonical { "ok " } else { "!! " },
                    wt.standing.as_str()
                ),
            )?;
            w(out, format!("path        {}", wt.path))?;
            if let Some(e) = &wt.expected_path {
                if !s.canonical {
                    w(out, format!("expected    {e}"))?;
                }
            }
            w(out, format!("head        {}", short(&wt.head)))?;
            w(out, format!("dirty       {}", dirty_word(&wt.dirty)))?;
            if let Some(u) = &wt.upstream {
                w(
                    out,
                    format!(
                        "upstream    {} ({})",
                        u.name,
                        if u.gone {
                            "gone".into()
                        } else {
                            format!(
                                "ahead {}, behind {}",
                                u.ahead.unwrap_or(0),
                                u.behind.unwrap_or(0)
                            )
                        }
                    ),
                )?;
            }
            if let Some(i) = &wt.issue {
                w(out, format!("issue       {i}"))?;
            }
            w(out, format!("container   {}", s.container.path))?;
            w(
                out,
                format!(
                    "trunk       {} ({})",
                    s.trunk.branch.as_deref().unwrap_or("(unknown)"),
                    trunk_source_word(s.trunk.source)
                ),
            )?;
            for d in wt
                .diagnostics
                .iter()
                .filter(|d| d.severity != Severity::Info)
            {
                w(out, render_diagnostic(d))?;
            }
            if s.repository_errors > 0 {
                w(
                    out,
                    format!(
                        "repository  {} topology error(s) (majordomus worktree doctor)",
                        s.repository_errors
                    ),
                )?;
            }
        }
    }
    Ok(if s.canonical { 0 } else { EXIT_REFUSED })
}

fn inspect(
    svc: &WorktreeService,
    branch: &str,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let r = svc.inspect(branch).map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &r)?,
        OutputFormat::Text => {
            w(
                out,
                format!(
                    "branch      {}{}",
                    r.branch,
                    if r.branch_exists {
                        ""
                    } else {
                        "  (does not exist yet)"
                    }
                ),
            )?;
            w(out, format!("expected    {}", r.expected_path))?;
            match &r.worktree {
                Some(wt) => {
                    w(
                        out,
                        format!("worktree    {} ({})", wt.path, wt.standing.as_str()),
                    )?;
                    w(out, format!("head        {}", short(&wt.head)))?;
                    w(out, format!("dirty       {}", dirty_word(&wt.dirty)))?;
                }
                None => w(
                    out,
                    format!(
                        "worktree    none{}",
                        if r.destination_exists {
                            "  (something occupies the canonical path)"
                        } else {
                            ""
                        }
                    ),
                )?,
            }
            for d in r
                .diagnostics
                .iter()
                .filter(|d| d.severity != Severity::Info)
            {
                w(out, render_diagnostic(d))?;
            }
            if r.worktree.is_none() && !r.destination_exists {
                w(
                    out,
                    format!("create      majordomus worktree create {}", r.branch),
                )?;
            }
        }
    }
    Ok(0)
}

fn doctor(
    svc: &WorktreeService,
    format: OutputFormat,
    quiet: bool,
    out: &mut Out<'_>,
) -> Result<u8> {
    let t = svc.topology(Detail::Fast).map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &t.diagnostics)?,
        OutputFormat::Text => {
            if !quiet {
                header(&t, out)?;
                w(out, "")?;
            }
            for d in t
                .diagnostics
                .iter()
                .filter(|d| !quiet || d.severity == Severity::Error)
            {
                w(out, render_diagnostic(d))?;
            }
            w(
                out,
                format!(
                    "worktree topology: {} — {} error(s), {} warning(s)",
                    if t.valid { "valid" } else { "INVALID" },
                    t.tallies.errors,
                    t.tallies.warnings
                ),
            )?;
        }
    }
    Ok(if t.valid { 0 } else { EXIT_REFUSED })
}

fn guard(
    svc: &WorktreeService,
    format: OutputFormat,
    quiet: bool,
    out: &mut Out<'_>,
) -> Result<u8> {
    let v = svc.guard().map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &v)?,
        OutputFormat::Text => {
            if v.ok {
                if !quiet {
                    w(
                        out,
                        format!(
                            "worktree guard: ok — {} at {}{}",
                            v.branch.as_deref().unwrap_or("(detached)"),
                            v.path,
                            if v.exempt { " (exempt)" } else { "" }
                        ),
                    )?;
                }
            } else {
                w(out, "worktree guard: REFUSED")?;
                if let Some(r) = &v.reason {
                    w(out, render_diagnostic(r))?;
                }
            }
        }
    }
    Ok(if v.ok { 0 } else { EXIT_REFUSED })
}

fn cleanup(
    svc: &WorktreeService,
    format: OutputFormat,
    remove: bool,
    out: &mut Out<'_>,
) -> Result<u8> {
    let t = svc.topology(Detail::Full).map_err(refuse)?;
    let eligible: Vec<&BranchState> = t.branches.iter().filter(|b| b.cleanup_eligible).collect();

    if remove {
        return reclaim(svc, &eligible, format, out);
    }
    match format {
        OutputFormat::Json => json(out, &eligible)?,
        OutputFormat::Text => {
            if eligible.is_empty() {
                w(out, "nothing is cleanup-eligible: no branch is both merged into the trunk and clean or unchecked-out")?;
            } else {
                w(out, "merged into the trunk, and clean or not checked out — nothing is deleted here:")?;
                for b in &eligible {
                    w(
                        out,
                        format!(
                            "  {:<44} {}",
                            b.name,
                            match &b.worktree {
                                Some(p) => format!(
                                    "majordomus worktree remove {}; then git branch -d {}   ({p})",
                                    b.name, b.name
                                ),
                                None => format!("git branch -d {}", b.name),
                            }
                        ),
                    )?;
                }
            }
        }
    }
    Ok(0)
}

/// Is anything working inside this directory right now?
///
/// `lsof -a -d cwd` asks the kernel which processes have their working directory there, which
/// is the only reading that is true at the moment it is taken. The mtime of a worktree is not:
/// two of the four swept on 2026-09-15 looked untouched since the 12th and had live processes
/// inside them. A missing `lsof` is not "nothing is running" — it is not knowing, and the
/// caller refuses on `None` rather than removing.
fn occupied(path: &str) -> Option<bool> {
    // a path that cannot be resolved is not knowing, and the caller refuses on `None`
    std::fs::canonicalize(path).ok()?;
    let dirs = crate::worktree::state::working_directories()?;
    Some(crate::worktree::state::is_occupied(
        std::path::Path::new(path),
        &dirs,
    ))
}

/// Remove the worktrees the listing offers, each re-measured immediately before it goes.
///
/// The listing and the removal are two moments, and everything that makes a worktree safe to
/// remove can change between them. So the listing only nominates: the branch's standing against
/// its remote, whether anything is running inside and the working tree's cleanliness are all
/// read again by [`refusal`], one worktree at a time, and a candidate that fails any of them is
/// named and skipped rather than removed.
///
/// Branches are never deleted. `git branch -d` refuses an unmerged branch on its own, but a
/// branch is the only durable name a piece of work has, and the disk is what was scarce.
fn reclaim(
    svc: &WorktreeService,
    eligible: &[&BranchState],
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let mut removed: Vec<String> = Vec::new();
    let mut refused: Vec<(String, String)> = Vec::new();

    for b in eligible {
        let Some(path) = b.worktree.clone() else {
            continue; // no worktree to reclaim; the branch alone costs nothing
        };
        if let Some(why) = refusal(svc, &b.name, &path) {
            refused.push((b.name.clone(), why));
            continue;
        }
        match svc.remove(&b.name, false) {
            Ok(_) => removed.push(b.name.clone()),
            Err(e) => refused.push((b.name.clone(), format!("{e}"))),
        }
    }

    match format {
        OutputFormat::Json => json(
            out,
            &serde_json::json!({
                "removed": removed,
                "refused": refused
                    .iter()
                    .map(|(n, r)| serde_json::json!({ "branch": n, "reason": r }))
                    .collect::<Vec<_>>(),
            }),
        )?,
        OutputFormat::Text => {
            for n in &removed {
                w(out, format!("removed   {n}"))?;
            }
            for (n, r) in &refused {
                w(out, format!("refused   {n:<44} {r}"))?;
            }
            w(
                out,
                format!(
                    "worktree cleanup: {} removed, {} refused; no branch was deleted",
                    removed.len(),
                    refused.len()
                ),
            )?;
        }
    }
    Ok(0)
}

/// Why this worktree must not be removed now, or `None` when every reading says it is spare.
///
/// Each reading is taken here, at the moment of removal, and none comes from the listing that
/// nominated the branch: a commit made, a push deleted or a file written after the listing is
/// seen.
fn refusal(svc: &WorktreeService, branch: &str, path: &str) -> Option<String> {
    let primary = &svc.identity().primary_worktree().path;
    // Commits that exist on one disk. `merged into the trunk` is about the branch's remote
    // history and says nothing about a local head that ran ahead of it.
    let now = match crate::worktree::state::branch(primary, branch) {
        Ok(Some(now)) => now,
        Ok(None) => return Some("the branch no longer exists".into()),
        Err(e) => return Some(format!("could not read the branch: {e}")),
    };
    match &now.upstream {
        None => return Some("no upstream: its commits are on this disk only".into()),
        Some(u) if u.gone => return Some(format!("its upstream {} is gone", u.name)),
        // `ahead` is unknown rather than zero when git could not compare the two, and an
        // unknown count is not a reason to remove anything: it is refused with the others.
        Some(u) if u.ahead.unwrap_or(1) > 0 => {
            return Some(match u.ahead {
                Some(n) => format!("{n} commit(s) ahead of {}: they reach no remote", u.name),
                None => format!("cannot tell how far it is ahead of {}", u.name),
            })
        }
        Some(_) => {}
    }
    match occupied(path) {
        None => return Some("cannot tell whether anything is working inside it (no lsof)".into()),
        Some(true) => return Some("a process has its working directory inside it".into()),
        Some(false) => {}
    }
    match crate::worktree::state::dirty_state(std::path::Path::new(path)) {
        Ok(d) if d.clean => None,
        Ok(d) => Some(format!("uncommitted work: {}", d.summary())),
        Err(e) => Some(format!("could not read its working tree: {e}")),
    }
}

/// `worktree reconcile`: the plan, one subject of it, or the act.
///
/// Generic over the sink so that what a person reads is asserted against a buffer. The exit
/// code is 0 for a reading and for an act, whatever they found; a selector that names no
/// subject is the missing-artifact code.
fn reconcile<W: Write>(
    svc: &WorktreeService,
    selector: Option<&str>,
    apply: Option<ReconcileOptions>,
    format: OutputFormat,
    out: &mut W,
) -> Result<u8> {
    if let Some(options) = apply {
        let outcome = svc.reconcile(&options).map_err(refuse)?;
        return render_outcome(&outcome, format, out).map(|()| 0);
    }
    let plan = svc.reconciliation().map_err(refuse)?;
    match selector {
        None => render_reconciliation(&plan, format, out).map(|()| 0),
        Some(selector) => {
            let Some(entry) = plan.entries.iter().find(|e| e.subject() == selector) else {
                return Err(Error::Refused {
                    code: EXIT_MISSING,
                    reason: format!(
                        "worktree reconcile: '{selector}' is neither a non-trunk branch nor the path of a detached worktree of this repository"
                    ),
                });
            };
            render_entry(entry, format, out).map(|()| 0)
        }
    }
}

fn line<W: Write>(out: &mut W, s: impl AsRef<str>) -> Result<()> {
    writeln!(out, "{}", s.as_ref()).map_err(Error::Transport)
}

fn document<W: Write, T: Serialize>(out: &mut W, v: &T) -> Result<()> {
    let value = serde_json::to_value(v).unwrap_or(Value::Null);
    line(out, format!("{value:#}"))
}

/// The plan: one line per subject, what `--apply` would take first, then the tallies and
/// the verdict.
fn render_reconciliation<W: Write>(
    plan: &Reconciliation,
    format: OutputFormat,
    out: &mut W,
) -> Result<()> {
    if format == OutputFormat::Json {
        return document(out, plan);
    }
    for e in &plan.entries {
        line(
            out,
            format!(
                "{:<5} {:<11} {:<34} {}",
                if e.automatic { "APPLY" } else { "" },
                e.state.as_str(),
                e.step.as_str(),
                e.subject()
            ),
        )?;
    }
    for (state, count) in &plan.tallies {
        line(out, format!("{count:>5}  {state}"))?;
    }
    line(out, plan.summary())
}

/// One subject: what it is, what it permits, and every reading that decided it.
fn render_entry<W: Write>(entry: &ReconcileEntry, format: OutputFormat, out: &mut W) -> Result<()> {
    if format == OutputFormat::Json {
        return document(out, entry);
    }
    line(out, format!("subject   {}", entry.subject()))?;
    if let Some(path) = &entry.worktree {
        line(out, format!("worktree  {path}"))?;
    }
    line(out, format!("head      {}", entry.head))?;
    line(out, format!("state     {}", entry.state.as_str()))?;
    line(
        out,
        format!(
            "step      {}{}",
            entry.step.as_str(),
            if entry.automatic {
                "   (reconcile --apply takes it)"
            } else {
                ""
            }
        ),
    )?;
    if let Some(behind) = entry.behind {
        line(out, format!("behind    {behind} trunk commit(s)"))?;
    }
    for reason in &entry.reasons {
        line(out, format!("because   {reason}"))?;
    }
    if !entry.command.is_empty() {
        line(out, format!("command   {}", entry.command))?;
    }
    Ok(())
}

/// The act: what went, how to bring a deleted branch back, and what was left with why.
fn render_outcome<W: Write>(
    outcome: &ReconcileOutcome,
    format: OutputFormat,
    out: &mut W,
) -> Result<()> {
    if format == OutputFormat::Json {
        return document(out, outcome);
    }
    for path in &outcome.removed {
        line(out, format!("removed   {path}"))?;
    }
    for branch in &outcome.deleted {
        line(
            out,
            format!(
                "deleted   {:<44} restore: {}",
                branch.branch,
                branch.restore()
            ),
        )?;
    }
    for refusal in &outcome.refused {
        line(
            out,
            format!("refused   {:<44} {}", refusal.subject, refusal.reason),
        )?;
    }
    line(out, outcome.summary())
}

fn branches(svc: &WorktreeService, without_worktree: bool, out: &mut Out<'_>) -> Result<u8> {
    let t = svc.topology(Detail::Fast).map_err(refuse)?;
    for b in t
        .branches
        .iter()
        .filter(|b| !without_worktree || b.worktree.is_none())
    {
        w(out, &b.name)?;
    }
    Ok(0)
}

// ---------------------------------------------------------------- mutating

fn create(
    svc: &WorktreeService,
    branch: &str,
    base: Option<String>,
    ensure: bool,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let request = CreateRequest {
        branch: branch.to_string(),
        base,
    };
    let report = if ensure {
        svc.ensure(&request)
    } else {
        svc.create(&request)
    }
    .map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &report)?,
        OutputFormat::Text => {
            if report.container_created {
                w(out, format!("created the container {}", report.container))?;
            }
            if report.existed {
                w(out, format!("exists  {}", report.path))?;
            } else {
                w(out, format!("created {}", report.path))?;
            }
            match (&report.base, report.branch_created) {
                (Some(b), true) => w(out, format!("branch  {} (new, from {b})", report.branch))?,
                _ => w(out, format!("branch  {}", report.branch))?,
            }
            w(out, report.envrc.describe())?;
            w(out, report.envrc_local.describe())?;
            w(
                out,
                format!("cd \"$(majordomus worktree path {})\"", report.branch),
            )?;
        }
    }
    Ok(0)
}

/// The branch an issue's work happens on: `feature/<id>-<slug>`, from the issue record
/// this repository keeps. The id is the provable link the topology reads back.
fn issue_branch(repo: &crate::cli::RepoArgs, id: &str) -> Result<String> {
    let app = crate::app::App::load(repo)?;
    let object = app
        .index()
        .objects
        .iter()
        .find(|o| o.kind == "issue" && o.identity == id)
        .ok_or_else(|| Error::Refused {
            code: crate::worktree::EXIT_MISSING,
            reason: format!(
                "no issue '{id}' in this repository's project model; nothing was created and no issue number was invented"
            ),
        })?;
    let slug = object
        .metadata
        .get("slug")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| id.to_lowercase());
    Ok(format!("feature/{id}-{slug}"))
}

fn migrate_cmd(
    svc: &WorktreeService,
    apply: bool,
    options: MigrationOptions,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let plan = if apply {
        migrate::apply(svc, &options).map_err(refuse)?
    } else {
        migrate::plan_with(svc, options.include_ephemeral).map_err(refuse)?
    };
    match format {
        OutputFormat::Json => json(out, &plan)?,
        OutputFormat::Text => render_plan(&plan, out)?,
    }
    // A plan never fails for having found something to do. An apply that left a step
    // blocked or failed says so through its exit code.
    Ok(if apply && (plan.blocked > 0 || plan.failed > 0) {
        EXIT_REFUSED
    } else {
        0
    })
}

fn render_plan(plan: &MigrationPlan, out: &mut Out<'_>) -> Result<()> {
    let verb = if plan.applied {
        "migrated"
    } else {
        "to migrate"
    };
    if plan.steps.is_empty() {
        w(
            out,
            format!(
                "nothing {verb}: every worktree with a branch is at its canonical path under {}",
                plan.container
            ),
        )?;
    }
    for step in &plan.steps {
        w(out, "")?;
        w(out, step.branch.as_str())?;
        w(out, format!("  from    {}", step.from))?;
        w(out, format!("  to      {}", step.to))?;
        w(out, format!("  head    {}", short(&step.head)))?;
        w(out, format!("  state   {}", step.dirty.summary()))?;
        let action = match step.action {
            MigrationAction::Move => "move",
            MigrationAction::MoveViaStaging => "move out of the container path, then into it",
            MigrationAction::CopyAndRepair => {
                "copy across filesystems, repair, verify, remove the original"
            }
            MigrationAction::None => "none",
        };
        match step.outcome {
            StepOutcome::Planned => w(out, format!("  action  {action}"))?,
            StepOutcome::Moved => {
                w(
                    out,
                    format!(
                        "  outcome moved and verified ({action}){}",
                        step.message
                            .as_deref()
                            .map(|m| format!(": {m}"))
                            .unwrap_or_default()
                    ),
                )?;
                if let Some(envrc) = &step.envrc {
                    w(out, format!("  {}", envrc.describe()))?;
                }
            }
            StepOutcome::Blocked => {
                w(out, "  outcome BLOCKED")?;
                for b in &step.blockers {
                    w(
                        out,
                        format!("    {}", render_diagnostic(b).replace('\n', "\n    ")),
                    )?;
                }
                if let Some(m) = &step.message {
                    w(out, format!("    {m}"))?;
                }
            }
            StepOutcome::Failed => {
                w(
                    out,
                    format!(
                        "  outcome FAILED: {}",
                        step.message.as_deref().unwrap_or("-")
                    ),
                )?;
                for d in &step.differences {
                    w(out, format!("    {d}"))?;
                }
            }
        }
    }
    if !plan.exceptions.is_empty() {
        w(out, "")?;
        w(out, "not migrated by design")?;
        for d in &plan.exceptions {
            w(
                out,
                format!("  {}", render_diagnostic(d).replace('\n', "\n  ")),
            )?;
        }
    }
    w(out, "")?;
    if plan.applied {
        w(
            out,
            format!(
                "{} step(s): {} moved and verified, {} blocked, {} failed",
                plan.steps.len(),
                plan.moved,
                plan.blocked,
                plan.failed
            ),
        )?;
        if let Some(p) = &plan.moved_current {
            w(out, format!("the worktree you were in moved: cd \"{p}\""))?;
        }
    } else {
        w(
            out,
            format!(
                "{} step(s): {} movable, {} blocked; nothing was changed. `majordomus worktree migrate` applies the movable ones",
                plan.steps.len(),
                plan.movable,
                plan.blocked
            ),
        )?;
    }
    Ok(())
}

fn repair(
    svc: &WorktreeService,
    dry_run: bool,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let report = svc.repair(dry_run).map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &report)?,
        OutputFormat::Text => {
            if report.pruned.is_empty() {
                w(
                    out,
                    "nothing to prune: every registered worktree still exists",
                )?;
            } else {
                for line in &report.pruned {
                    w(out, line)?;
                }
                w(
                    out,
                    format!(
                        "{} registration(s) {}",
                        report.pruned.len(),
                        if report.applied {
                            "dropped"
                        } else {
                            "would be dropped; nothing was changed"
                        }
                    ),
                )?;
            }
            for line in &report.repaired {
                w(out, line)?;
            }
            if report.applied {
                w(
                    out,
                    "administrative links repaired where git found them stale",
                )?;
            }
        }
    }
    Ok(0)
}

fn remove(
    svc: &WorktreeService,
    selector: &str,
    force: bool,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    // Say what is about to go, and what it holds, before it goes: a removal that prints its
    // subject only afterwards has taught its lesson too late. The service refuses dirty
    // work without `force` regardless; this line is for the person who typed `--force`.
    if format == OutputFormat::Text {
        let record = svc.resolve(selector).map_err(refuse)?;
        let held = record
            .branch
            .clone()
            .unwrap_or_else(|| "a detached HEAD".into());
        let dirty = if record.path.is_dir() {
            crate::worktree::state::dirty_state(&record.path)
                .map(|d| d.summary())
                .unwrap_or_else(|_| "unknown".into())
        } else {
            "directory missing".into()
        };
        w(
            out,
            format!(
                "removing {} (holding {held}; {dirty}){}",
                record.path.display(),
                if force { "; forced" } else { "" }
            ),
        )?;
    }
    let report = svc.remove(selector, force).map_err(refuse)?;
    match format {
        OutputFormat::Json => json(out, &report)?,
        OutputFormat::Text => {
            w(out, format!("removed {}", report.path))?;
            if let Some(b) = &report.branch {
                w(
                    out,
                    format!("branch  {b} still exists (git branch -d {b} removes it)"),
                )?;
            }
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn git(cwd: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args([
                "-c",
                "user.email=t@t",
                "-c",
                "user.name=t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "init.defaultBranch=master",
            ])
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A repository with its own remote and one worktree on `spare`: merged into the trunk,
    /// pushed, clean, and therefore nominated by the listing.
    fn nominated() -> (tempfile::TempDir, WorktreeService, BranchState) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().canonicalize().expect("canonical tempdir");
        git(&root, &["init", "-q", "--bare", "origin.git"]);
        git(&root, &["init", "-q", "repo"]);
        let repo = root.join("repo");
        // absolute, so a push from inside the worktree reaches the same remote
        let origin = root.join("origin.git").to_string_lossy().to_string();
        git(&repo, &["remote", "add", "origin", origin.as_str()]);
        std::fs::write(repo.join("one.txt"), "one\n").expect("write");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "trunk"]);
        git(&repo, &["push", "-q", "-u", "origin", "master"]);
        git(&repo, &["switch", "-q", "-c", "spare"]);
        std::fs::write(repo.join("spare.txt"), "spare\n").expect("write");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-qm", "spare"]);
        git(&repo, &["push", "-q", "-u", "origin", "spare"]);
        git(&repo, &["switch", "-q", "master"]);
        git(
            &repo,
            &["merge", "-q", "--no-ff", "-m", "merge spare", "spare"],
        );
        git(&repo, &["push", "-q", "origin", "master"]);
        let svc = WorktreeService::open(&repo).expect("open the service");
        let at = svc.expected_path_of("spare").expect("the path of spare");
        let at = at.to_string_lossy().to_string();
        git(&repo, &["worktree", "add", "-q", at.as_str(), "spare"]);
        let t = svc.topology(Detail::Full).expect("topology");
        let b = t
            .branches
            .into_iter()
            .find(|b| b.name == "spare" && b.cleanup_eligible)
            .expect("the listing nominates spare");
        (tmp, svc, b)
    }

    #[test]
    fn a_commit_made_after_the_listing_is_seen_by_the_reclaim() {
        let (_tmp, svc, b) = nominated();
        let path = b.worktree.clone().expect("a worktree");
        git(
            Path::new(&path),
            &["commit", "-q", "--allow-empty", "-m", "on one disk only"],
        );
        // the listing still says zero ahead; the reading taken now does not
        assert_eq!(b.upstream.as_ref().and_then(|u| u.ahead), Some(0));
        let why = refusal(&svc, &b.name, &path).expect("refused");
        assert!(why.contains("1 commit(s) ahead"), "{why}");
    }

    #[test]
    fn an_upstream_deleted_after_the_listing_is_seen_by_the_reclaim() {
        let (_tmp, svc, b) = nominated();
        let path = b.worktree.clone().expect("a worktree");
        git(
            Path::new(&path),
            &["push", "-q", "origin", "--delete", "spare"],
        );
        git(Path::new(&path), &["fetch", "-q", "--prune", "origin"]);
        let why = refusal(&svc, &b.name, &path).expect("refused");
        assert!(why.contains("is gone"), "{why}");
    }

    #[test]
    fn work_written_after_the_listing_is_seen_by_the_reclaim() {
        let (_tmp, svc, b) = nominated();
        let path = b.worktree.clone().expect("a worktree");
        if occupied(&path).is_none() {
            eprintln!("no lsof: the occupancy reading refuses first, which is its own test");
            return;
        }
        assert_eq!(
            refusal(&svc, &b.name, &path),
            None,
            "nothing refuses a spare worktree"
        );
        std::fs::write(
            Path::new(&path).join("spare.txt"),
            "written after the listing\n",
        )
        .expect("write");
        let why = refusal(&svc, &b.name, &path).expect("refused");
        assert!(why.contains("uncommitted work"), "{why}");
    }

    /// What a reconcile wrote to a buffer, with its exit code.
    fn reconciled(
        svc: &WorktreeService,
        selector: Option<&str>,
        apply: Option<ReconcileOptions>,
        format: OutputFormat,
    ) -> (u8, String) {
        let mut out: Vec<u8> = Vec::new();
        let code = reconcile(svc, selector, apply, format, &mut out).expect("reconcile");
        (code, String::from_utf8(out).expect("utf-8"))
    }

    /// The plan a person reads: what `--apply` would take is marked, every state is tallied,
    /// and the last line is the verdict.
    #[test]
    fn the_reconciliation_marks_what_the_act_would_take() {
        let (_tmp, svc, b) = nominated();
        let (code, text) = reconciled(&svc, None, None, OutputFormat::Text);
        assert_eq!(code, 0);
        let first = text.lines().next().expect("a line");
        assert!(first.starts_with("APPLY merged"), "{text}");
        assert!(first.contains("remove_worktree_and_delete_branch") && first.ends_with("spare"));
        assert!(text.contains("    1  merged"), "{text}");
        assert!(
            text.trim_end().ends_with(
                "1 of 1 subject(s) removable on proof: majordomus worktree reconcile --apply"
            ),
            "{text}"
        );

        let (_, document) = reconciled(&svc, None, None, OutputFormat::Json);
        let plan: Reconciliation = serde_json::from_str(&document).expect("the plan as JSON");
        assert_eq!(plan.entries[0].branch.as_deref(), Some("spare"));
        assert_eq!(plan.entries[0].head, b.head);
        assert!(plan.entries[0].automatic && !plan.settled);
    }

    /// One subject says why it stands where it does; a name that is no subject is the
    /// missing-artifact code, not an empty answer.
    #[test]
    fn one_subject_says_why_it_stands_where_it_does() {
        let (_tmp, svc, b) = nominated();
        let (code, text) = reconciled(&svc, Some("spare"), None, OutputFormat::Text);
        assert_eq!(code, 0);
        for wanted in [
            "subject   spare",
            "worktree  ",
            "state     merged",
            "step      remove_worktree_and_delete_branch   (reconcile --apply takes it)",
            "because   the trunk reaches the branch, and it was published",
            "command   majordomus worktree reconcile --apply",
        ] {
            assert!(text.contains(wanted), "no '{wanted}' in:\n{text}");
        }
        assert!(text.contains(&format!("head      {}", b.head)));

        let (_, document) = reconciled(&svc, Some("spare"), None, OutputFormat::Json);
        let entry: ReconcileEntry = serde_json::from_str(&document).expect("the entry as JSON");
        assert_eq!(entry.subject(), "spare");

        let mut out: Vec<u8> = Vec::new();
        match reconcile(
            &svc,
            Some("no-such-branch"),
            None,
            OutputFormat::Text,
            &mut out,
        ) {
            Err(Error::Refused { code, reason }) => {
                assert_eq!(code, EXIT_MISSING);
                assert!(reason.contains("'no-such-branch'"), "{reason}");
            }
            other => panic!("an unknown subject was answered: {other:?}"),
        }
        assert!(out.is_empty());

        // a subject that is kept prints no marker and no command, and one that merges
        // cleanly says how far behind it is
        let held = ReconcileEntry {
            branch: Some("feature/later".into()),
            worktree: None,
            head: "0123abc".into(),
            state: crate::worktree::WorkState::Stale,
            step: crate::worktree::ReconcileStep::Keep,
            automatic: false,
            scratch: false,
            here: false,
            relation: Some(crate::worktree::TrunkRelation::Mergeable),
            behind: Some(2),
            reasons: vec!["the trunk has moved since it left".into()],
            command: String::new(),
        };
        let mut out: Vec<u8> = Vec::new();
        render_entry(&held, OutputFormat::Text, &mut out).expect("render");
        let text = String::from_utf8(out).expect("utf-8");
        assert!(text.contains("step      keep\n"), "{text}");
        assert!(text.contains("behind    2 trunk commit(s)"));
        assert!(!text.contains("command") && !text.contains("worktree  "));
    }

    /// The act says what went and how to bring a deleted branch back; run again it says
    /// nothing went, in the same shape.
    #[test]
    fn the_act_names_what_went_and_does_nothing_the_second_time() {
        let (_tmp, svc, b) = nominated();
        let path = b.worktree.clone().expect("spare is checked out");
        let apply = Some(ReconcileOptions::default());
        let (code, text) = reconciled(&svc, None, apply, OutputFormat::Text);
        assert_eq!(code, 0);
        assert!(text.contains(&format!("removed   {path}")), "{text}");
        assert!(
            text.contains(&format!("restore: git branch spare {}", b.head)),
            "{text}"
        );
        assert!(text.contains("1 worktree(s) removed, 1 branch(es) deleted, 0 refused"));
        assert!(!std::path::Path::new(&path).exists());

        let (_, document) = reconciled(&svc, None, apply, OutputFormat::Json);
        let outcome: ReconcileOutcome = serde_json::from_str(&document).expect("the outcome");
        assert_eq!(outcome, ReconcileOutcome::default());
        let (_, text) = reconciled(&svc, None, None, OutputFormat::Text);
        assert!(text.contains("nothing to reconcile"), "{text}");

        // a refusal is printed with its subject and its reason
        let refused = ReconcileOutcome {
            refused: vec![crate::worktree::ReconcileRefusal {
                subject: "feature/x".into(),
                reason: "changed since it was listed: now dirty".into(),
            }],
            ..ReconcileOutcome::default()
        };
        let mut out: Vec<u8> = Vec::new();
        render_outcome(&refused, OutputFormat::Text, &mut out).expect("render");
        let text = String::from_utf8(out).expect("utf-8");
        assert!(text.contains("refused   feature/x"), "{text}");
        assert!(text.contains("changed since it was listed: now dirty"));
        assert!(text.contains("0 worktree(s) removed, 0 branch(es) deleted, 1 refused"));
    }
}
