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
    MigrationPlan, RepositoryTopology, Severity, Standing, StatusReport, StepOutcome,
    TopologyDiagnostic, WorktreeService, WorktreeState, EXIT_REFUSED,
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
        Some(WorktreeCommand::Cleanup) => cleanup(&svc()?, format, &mut out),
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

fn cleanup(svc: &WorktreeService, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let t = svc.topology(Detail::Full).map_err(refuse)?;
    let eligible: Vec<&BranchState> = t.branches.iter().filter(|b| b.cleanup_eligible).collect();
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
    use crate::worktree::{
        ContainerView, DiagnosticCode, RepositoryView, TopologyTallies, TrunkSource, TrunkView,
        UpstreamState, WorktreeKind,
    };

    fn state(label: &str, standing: Standing) -> WorktreeState {
        WorktreeState {
            path: "/a/foo-wt/feature/x".into(),
            kind: WorktreeKind::Linked,
            standing,
            branch: Some(label.into()),
            label: label.into(),
            head: Some("abcdef0123456789".into()),
            detached: false,
            expected_path: Some("/a/foo-wt/feature/x".into()),
            exists: true,
            current: false,
            locked: None,
            prunable: None,
            dirty: None,
            upstream: None,
            issue: None,
            diagnostics: Vec::new(),
        }
    }

    /// The text listing is what a person reads; every extra line under a worktree is there
    /// because it changes what they would do next. A destination is printed only when the
    /// worktree is misplaced — printing it beside a canonical worktree would say "belongs
    /// at" about the path it is already at, and a reader would go looking for a move that
    /// is not needed.
    #[test]
    fn the_destination_is_printed_only_for_a_worktree_that_has_to_move() {
        let canonical = state("feature/x", Standing::Canonical);
        let line = render_worktree_line(&canonical);
        assert!(line.starts_with("CANONICAL"), "{line}");
        assert!(
            line.contains("feature/x") && line.contains("/a/foo-wt/feature/x"),
            "{line}"
        );
        assert!(!line.contains("belongs at"), "{line}");

        let misplaced = state("feature/x", Standing::Misplaced);
        let line = render_worktree_line(&misplaced);
        assert!(line.starts_with("MISPLACED"), "{line}");
        assert!(line.contains("-> belongs at /a/foo-wt/feature/x"), "{line}");
    }

    /// "You are here" is the marker a person navigates by when several worktrees hold
    /// similar branches, and the dirty line is the warning before a migration. Both are
    /// suppressed when they do not apply: a clean worktree that printed "dirty: clean"
    /// would make every listing look alarming.
    #[test]
    fn the_current_marker_and_the_dirty_line_appear_only_when_they_mean_something() {
        let mut w_ = state("feature/x", Standing::Canonical);
        assert!(!render_worktree_line(&w_).contains("you are here"));
        w_.current = true;
        assert!(render_worktree_line(&w_).contains("(you are here)"));

        w_.dirty = Some(DirtyState {
            clean: true,
            ..Default::default()
        });
        assert!(
            !render_worktree_line(&w_).contains("dirty:"),
            "a clean tree says nothing"
        );
        w_.dirty = Some(DirtyState {
            unstaged: 2,
            untracked: 1,
            clean: false,
            ..Default::default()
        });
        assert!(
            render_worktree_line(&w_).contains("dirty: 2 unstaged, 1 untracked"),
            "{}",
            render_worktree_line(&w_)
        );
    }

    /// An upstream is worth a line only when the branch has moved away from it. Printing
    /// "ahead 0, behind 0" under every worktree would bury the one that is behind, and a
    /// gone upstream — the branch was merged and deleted on the remote — is the single most
    /// useful thing the listing can say about a stale worktree.
    #[test]
    fn the_upstream_line_appears_only_when_the_branch_and_its_upstream_have_moved_apart() {
        let mut w_ = state("feature/x", Standing::Canonical);
        w_.upstream = Some(UpstreamState {
            name: "origin/feature/x".into(),
            ahead: Some(0),
            behind: Some(0),
            gone: false,
        });
        assert!(
            !render_worktree_line(&w_).contains("upstream"),
            "an up-to-date branch is not news"
        );

        w_.upstream = Some(UpstreamState {
            name: "origin/feature/x".into(),
            ahead: Some(3),
            behind: Some(1),
            gone: false,
        });
        assert!(
            render_worktree_line(&w_).contains("upstream origin/feature/x: ahead 3, behind 1"),
            "{}",
            render_worktree_line(&w_)
        );

        w_.upstream = Some(UpstreamState {
            name: "origin/feature/x".into(),
            ahead: None,
            behind: None,
            gone: true,
        });
        let line = render_worktree_line(&w_);
        assert!(line.contains("upstream origin/feature/x: gone"), "{line}");
        assert!(
            !line.contains("ahead"),
            "a gone upstream has no distance to report: {line}"
        );

        w_.issue = Some("I0931".into());
        assert!(render_worktree_line(&w_).contains("issue I0931"));
    }

    /// A diagnostic without its remedy is a complaint. Every rendering carries the code a
    /// script matches on, the sentence a person reads, and the command that fixes it; the
    /// location lines are printed only when the diagnostic has them, because a
    /// repository-wide finding has no single path.
    #[test]
    fn a_rendered_diagnostic_always_carries_its_code_and_its_remedy() {
        let full = TopologyDiagnostic {
            code: DiagnosticCode::PathMismatch,
            severity: Severity::Error,
            path: Some("/tmp/scratch".into()),
            branch: Some("feature/x".into()),
            expected: Some("/a/foo-wt/feature/x".into()),
            message: "branch 'feature/x' belongs at /a/foo-wt/feature/x".into(),
            remedy: "majordomus worktree migrate".into(),
        };
        let s = render_diagnostic(&full);
        assert!(s.starts_with("ERROR"), "{s}");
        assert!(s.contains("worktree.path_mismatch"), "{s}");
        assert!(s.contains("at /tmp/scratch"), "{s}");
        assert!(s.contains("expected /a/foo-wt/feature/x"), "{s}");
        assert!(s.contains("[remedy: majordomus worktree migrate]"), "{s}");

        let bare = TopologyDiagnostic {
            path: None,
            expected: None,
            severity: Severity::Warning,
            ..full
        };
        let s = render_diagnostic(&bare);
        assert!(s.starts_with("WARNING"), "{s}");
        assert!(!s.contains("\n         at "), "{s}");
        assert!(!s.contains("expected"), "{s}");
        assert!(s.contains("[remedy:"), "the remedy is never optional: {s}");
    }

    /// The trunk is discovered, not configured, so the listing says *how* it was decided.
    /// A person seeing the wrong trunk needs to know whether to fix a remote's HEAD, a
    /// config value, or a branch name; `Unknown` has to say so rather than print nothing.
    #[test]
    fn every_way_the_trunk_can_be_decided_has_words_of_its_own() {
        let words: Vec<&str> = [
            TrunkSource::RemoteHead,
            TrunkSource::DefaultBranchConfig,
            TrunkSource::ConventionalName,
            TrunkSource::PrimaryCheckout,
            TrunkSource::Unknown,
        ]
        .into_iter()
        .map(trunk_source_word)
        .collect();
        assert_eq!(
            words,
            vec![
                "the remote's HEAD",
                "init.defaultBranch",
                "the conventional name",
                "the primary checkout's branch",
                "unknown",
            ]
        );
        assert_eq!(
            words
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            5,
            "two sources rendering the same words would make the answer unactionable"
        );
    }

    /// The tally line is the summary a person reads when the listing is too long to scan,
    /// and the counts must be the topology's own. A line that dropped `misplaced` would let
    /// a repository look healthy while the guard refuses every commit in it.
    #[test]
    fn the_tally_line_reports_the_counts_the_topology_holds() {
        let t = RepositoryTopology {
            schema: crate::worktree::SCHEMA.into(),
            repository: RepositoryView {
                primary_worktree: "/a/foo".into(),
                git_common_dir: "/a/foo/.git".into(),
                name: "foo".into(),
            },
            container: ContainerView {
                path: "/a/foo-wt".into(),
                suffix: "-wt".into(),
                exists: true,
            },
            trunk: TrunkView {
                branch: Some("master".into()),
                source: TrunkSource::ConventionalName,
                checked_out_at: Some("/a/foo".into()),
            },
            observed_from: "/a/foo".into(),
            worktrees: Vec::new(),
            branches: Vec::new(),
            diagnostics: Vec::new(),
            tallies: TopologyTallies {
                worktrees: 7,
                canonical: 4,
                misplaced: 2,
                detached: 1,
                ephemeral: 0,
                missing: 3,
                locked: 0,
                dirty: 0,
                branches: 9,
                branches_without_worktree: 5,
                cleanup_eligible: 2,
                errors: 6,
                warnings: 8,
            },
            valid: false,
        };
        assert_eq!(
            tallies_line(&t),
            "7 worktree(s): 4 canonical, 2 misplaced, 1 detached, 3 missing; \
             9 branch(es), 5 without a worktree, 2 cleanup-eligible; 6 error(s), 8 warning(s)"
        );
    }

    /// A commit is shown short enough to read and long enough to be unambiguous, and a
    /// worktree with no commit — an unborn repository — prints a dash rather than an empty
    /// column that would shift everything after it.
    #[test]
    fn a_commit_is_shortened_and_a_missing_one_is_a_dash() {
        assert_eq!(
            short(&Some("abcdef0123456789abcdef".into())),
            "abcdef012345"
        );
        assert_eq!(
            short(&Some("abc".into())),
            "abc",
            "a commit shorter than the window is not padded or panicked over"
        );
        assert_eq!(short(&None), "-");
        assert_eq!(
            dirty_word(&None),
            "-",
            "not asked for is not the same as clean"
        );
        assert_eq!(
            dirty_word(&Some(DirtyState {
                clean: true,
                ..Default::default()
            })),
            "clean"
        );
    }

    /// The service's refusal reaches the shell with the service's own exit code and its own
    /// words. Rephrasing it here would give one condition two wordings, and flattening the
    /// code would make every refusal look alike to a script.
    #[test]
    fn a_refusal_keeps_the_services_exit_code_and_its_own_sentence() {
        let e = crate::worktree::WorktreeError::NoSuchWorktree {
            selector: "nope".into(),
        };
        let sentence = e.to_string();
        match refuse(e) {
            Error::Refused { code, reason } => {
                assert_eq!(code, crate::worktree::EXIT_MISSING);
                assert_eq!(reason, sentence, "the words were rephrased on the way out");
            }
            other => panic!("a worktree refusal became {other:?}"),
        }

        let dirty = crate::worktree::WorktreeError::DirtyWorktree {
            path: "/a/foo-wt/feature/x".into(),
            summary: "1 untracked".into(),
            operation: "remove".into(),
        };
        match refuse(dirty) {
            Error::Refused { code, .. } => assert_eq!(code, EXIT_REFUSED),
            other => panic!("{other:?}"),
        }
    }
}
