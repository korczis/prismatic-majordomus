//! `majordomus worktree`: the command line's rendering of the worktree service.
//!
//! Every read-only subcommand executes the capability the registry exposes at its command
//! path, so the command line and MCP and HTTP answer from one execution path. The mutating
//! subcommands — create, remove, migrate, prune — call the same
//! [`crate::worktree::WorktreeService`] directly, because the capability registry does not
//! write and must not start.
//!
//! Nothing here decides anything: no path is derived, no policy is applied, no safety check
//! is made in this file. It reads arguments, calls the service, and prints. The human form
//! and the JSON form are two renderings of one typed answer, so a script never has to parse
//! the human one.

use std::io::Write;

use serde_json::{json, Value};

use crate::app::App;
use crate::capability::builtin::worktree::{policy_of, policy_path_of};
use crate::capability::CapabilityError;
use crate::cli::{OutputFormat, WorktreeArgs, WorktreeCommand};
use crate::error::{Error, Result};
use crate::worktree::{CreateRequest, MigrationPlan, PolicyStatus, WorktreeName, WorktreeService};

/// Run `majordomus worktree`.
pub fn run(args: WorktreeArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let format = args.format;
    match args.command {
        None | Some(WorktreeCommand::Status) => status(&app, format, &mut out),
        Some(WorktreeCommand::Root) => root(&app, format, &mut out),
        Some(WorktreeCommand::List { status, tsv }) => list(&app, status, tsv, format, &mut out),
        Some(WorktreeCommand::Path { selector }) => path(&app, &selector, &mut out),
        Some(WorktreeCommand::Create {
            name,
            branch,
            base,
            issue,
            detach,
        }) => create(&app, name, branch, base, issue, detach, format, &mut out),
        Some(WorktreeCommand::Remove { selector, force }) => {
            remove(&app, &selector, force, format, &mut out)
        }
        Some(WorktreeCommand::Migrate { plan, apply }) => {
            migrate(&app, apply && !plan, format, &mut out)
        }
        Some(WorktreeCommand::Prune { dry_run }) => prune(&app, dry_run, format, &mut out),
    }
}

type Out<'a> = std::io::StdoutLock<'a>;

fn w(out: &mut Out<'_>, s: impl AsRef<str>) -> Result<()> {
    writeln!(out, "{}", s.as_ref()).map_err(Error::Transport)
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// A refusal from the service becomes this executable's error with the service's own words
/// and its own exit code. Nothing is rephrased and no debug struct reaches a person.
fn refuse(e: crate::worktree::WorktreeError) -> Error {
    Error::Refused {
        code: e.exit_code(),
        reason: e.to_string(),
    }
}

fn map(e: CapabilityError) -> Error {
    Error::Protocol {
        reason: e.to_string(),
    }
}

/// Execute the capability exposed at a command path: the read-only half of this command
/// line is a projection of the registry, not a second implementation beside it.
fn call(app: &App, path: &[&str], input: Value) -> Result<Value> {
    let ctx = &app.context;
    let words: Vec<String> = path.iter().map(|w| (*w).to_string()).collect();
    let id = ctx
        .registry
        .by_cli(&words)
        .map(|c| c.id.as_str())
        .ok_or_else(|| Error::Protocol {
            reason: format!(
                "no capability is exposed as `majordomus {}`",
                path.join(" ")
            ),
        })?;
    ctx.execute(id, input).map_err(map)
}

/// The service, for the operations that change something. Built from the same policy and
/// the same repository root the capabilities use.
fn service(app: &App) -> Result<WorktreeService> {
    WorktreeService::open(
        std::path::Path::new(&app.index().repository.root),
        policy_of(app.index()),
        &policy_path_of(app.index()),
    )
    .map_err(refuse)
}

// ---------------------------------------------------------------- read-only

fn root(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["worktree", "root"], json!({}))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        // One path and nothing else: this output is consumed by `cd "$(...)"`, and a label
        // in front of it would make the most useful form of this command useless.
        OutputFormat::Text => w(out, v["worktree_root"].as_str().unwrap_or_default())?,
    }
    Ok(0)
}

fn status(app: &App, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let v = call(app, &["worktree", "status"], json!({}))?;
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            let s = |k: &str| v[k].as_str().unwrap_or("-").to_string();
            w(out, format!("Repository       {}", s("repository")))?;
            w(out, format!("Current          {}", s("current")))?;
            w(out, format!("Type             {}", s("kind")))?;
            w(out, format!("Branch           {}", s("branch")))?;
            w(out, format!("Canonical root   {}", s("canonical_root")))?;
            w(
                out,
                format!("Policy           {}", s("policy").to_uppercase()),
            )?;
            if let Some(r) = v["policy_reason"].as_str() {
                w(out, format!("                 {r}"))?;
            }
            let changes = v["changes"].as_u64().unwrap_or(0);
            w(
                out,
                format!(
                    "Dirty            {}",
                    if v["dirty"].as_bool().unwrap_or(false) {
                        format!("yes ({changes} change(s))")
                    } else {
                        "no".to_string()
                    }
                ),
            )?;
            let violations = v["repository_violations"].as_u64().unwrap_or(0);
            if violations > 0 {
                w(
                    out,
                    format!(
                        "Repository       {violations} linked worktree(s) outside the canonical root \
                         (majordomus worktree migrate --plan)"
                    ),
                )?;
            }
        }
    }
    // The exit code answers the question the command asks. A worktree in the wrong place is
    // a contract failure, and a script that checks `worktree status` deserves to learn that
    // without parsing anything.
    Ok(if v["policy"].as_str() == Some("fail") {
        crate::worktree::EXIT_REFUSED
    } else {
        0
    })
}

fn list(
    app: &App,
    with_status: bool,
    tsv: bool,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let v = call(app, &["worktree", "list"], json!({ "status": with_status }))?;
    if tsv {
        // A third rendering of the same answer, for a shell with no JSON reader. Fixed
        // field order, one line per worktree, so `majordomus doctor` reads it with `read`
        // and carries no parser and no second derivation of any of these values.
        for r in v["worktrees"].as_array().unwrap_or(&Vec::new()) {
            let f = |k: &str| clean(r[k].as_str().unwrap_or(""));
            // An absent field is written as `-`, never as nothing: a shell reading this
            // with `IFS=$'\t' read` collapses runs of tabs, because a tab is IFS
            // whitespace, so an empty column would shift every column after it. No path,
            // branch, commit or reason is ever `-`, and git refuses `-` as a branch name.
            w(
                out,
                [
                    f("kind"),
                    f("policy"),
                    f("path"),
                    f("name"),
                    f("branch"),
                    f("head"),
                    f("proposed_path"),
                    f("policy_reason"),
                ]
                .join("\t"),
            )?;
        }
        let fails = v["enforcement"].as_str() == Some("error")
            && !v["violations"]
                .as_array()
                .map(Vec::is_empty)
                .unwrap_or(true);
        return Ok(if fails {
            crate::worktree::EXIT_REFUSED
        } else {
            0
        });
    }
    match format {
        OutputFormat::Json => w(out, pretty(&v))?,
        OutputFormat::Text => {
            let empty = Vec::new();
            let rows = v["worktrees"].as_array().unwrap_or(&empty);
            for (i, r) in rows.iter().enumerate() {
                if i > 0 {
                    w(out, "")?;
                }
                let kind = r["kind"].as_str().unwrap_or("linked").to_uppercase();
                let here = if r["current"].as_bool().unwrap_or(false) {
                    "  (you are here)"
                } else {
                    ""
                };
                w(out, format!("{kind}{here}"))?;
                w(out, format!("  {}", r["path"].as_str().unwrap_or("-")))?;
                match r["branch"].as_str() {
                    Some(b) => w(out, format!("  branch: {b}"))?,
                    None => w(out, "  branch: (detached)")?,
                }
                if let Some(head) = r["head"].as_str() {
                    w(out, format!("  head:   {}", &head[..head.len().min(12)]))?;
                }
                if let Some(d) = r["dirty"].as_bool() {
                    w(
                        out,
                        format!(
                            "  dirty:  {}",
                            if d {
                                format!("yes ({} change(s))", r["changes"].as_u64().unwrap_or(0))
                            } else {
                                "no".into()
                            }
                        ),
                    )?;
                }
                if r["locked"].is_string() {
                    w(out, "  locked: yes")?;
                }
                if r["prunable"].is_string() {
                    w(out, "  prunable: yes (the directory is gone)")?;
                }
                match r["policy"].as_str() {
                    Some("exempt") => w(out, "  policy: EXEMPT — the primary checkout")?,
                    Some("pass") => w(out, "  policy: PASS")?,
                    _ => {
                        w(
                            out,
                            format!(
                                "  policy: FAIL — {}",
                                r["policy_reason"]
                                    .as_str()
                                    .unwrap_or("outside the canonical root")
                            ),
                        )?;
                        if let Some(to) = r["proposed_path"].as_str() {
                            w(out, format!("          would move to {to}"))?;
                        }
                    }
                }
            }
            let t = &v["tallies"];
            w(out, "")?;
            w(
                out,
                format!(
                    "{} worktree(s): {} linked, {} compliant, {} outside {}",
                    t["worktrees"].as_u64().unwrap_or(0),
                    t["linked"].as_u64().unwrap_or(0),
                    t["compliant"].as_u64().unwrap_or(0),
                    t["violations"].as_u64().unwrap_or(0),
                    v["root"]["worktree_root"].as_str().unwrap_or("-"),
                ),
            )?;
        }
    }
    let fails = v["enforcement"].as_str() == Some("error")
        && !v["violations"]
            .as_array()
            .map(Vec::is_empty)
            .unwrap_or(true);
    Ok(if fails {
        crate::worktree::EXIT_REFUSED
    } else {
        0
    })
}

fn path(app: &App, selector: &str, out: &mut Out<'_>) -> Result<u8> {
    let p = service(app)?.path_of(selector).map_err(refuse)?;
    w(out, p.display().to_string())?;
    Ok(0)
}

// ---------------------------------------------------------------- mutating

#[allow(clippy::too_many_arguments)]
fn create(
    app: &App,
    name: Option<String>,
    branch: Option<String>,
    base: Option<String>,
    issue: Option<String>,
    detach: bool,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    // An issue names the worktree from the issue record this repository already keeps: its
    // id and its slug, both authored there. Nothing is fetched, no number is invented, and
    // an id the repository does not have is an error rather than a new directory.
    let name = match (&issue, &name) {
        (Some(id), None) => Some(issue_label(app, id)?),
        _ => name,
    };
    let report = service(app)?
        .create(&CreateRequest {
            name,
            branch,
            base,
            detach,
        })
        .map_err(refuse)?;
    match format {
        OutputFormat::Json => w(
            out,
            pretty(&serde_json::to_value(&report).unwrap_or_default()),
        )?,
        OutputFormat::Text => {
            if report.root_created {
                w(
                    out,
                    format!("created the worktree container {}", report.worktree_root),
                )?;
            }
            w(out, format!("created {}", report.path))?;
            match (&report.branch, report.branch_created) {
                (Some(b), true) => w(out, format!("branch  {b} (new)"))?,
                (Some(b), false) => w(out, format!("branch  {b}"))?,
                (None, _) => w(out, "branch  (detached)")?,
            }
            w(
                out,
                format!("cd \"$(majordomus worktree path {})\"", report.name),
            )?;
        }
    }
    Ok(0)
}

/// `issue-<id>-<slug>` from the repository's own issue record. The form is the one this
/// repository's issues already carry — an id and an authored slug — so no second naming
/// convention is introduced here.
fn issue_label(app: &App, id: &str) -> Result<String> {
    let object = app
        .index()
        .objects
        .iter()
        .find(|o| o.kind == "issue" && o.identity == id)
        .ok_or_else(|| Error::Refused {
            code: crate::worktree::EXIT_MISSING,
            reason: format!(
                "no issue '{id}' in this repository's project model; \
                 `majordomus plan` lists them. Nothing was created and no issue number was invented"
            ),
        })?;
    let slug = object
        .metadata
        .get("slug")
        .and_then(Value::as_str)
        .or(object.title.as_deref())
        .unwrap_or(id);
    let label = format!("issue-{id}-{slug}");
    // Prove the derived label is one component here, where the issue can be named in the
    // message, rather than letting the service refuse it without that context.
    WorktreeName::derive(&label).map_err(refuse)?;
    Ok(label)
}

fn remove(
    app: &App,
    selector: &str,
    force: bool,
    format: OutputFormat,
    out: &mut Out<'_>,
) -> Result<u8> {
    let report = service(app)?.remove(selector, force).map_err(refuse)?;
    match format {
        OutputFormat::Json => w(
            out,
            pretty(&serde_json::to_value(&report).unwrap_or_default()),
        )?,
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

fn migrate(app: &App, apply: bool, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let svc = service(app)?;
    let plan = if apply {
        svc.migrate().map_err(refuse)?
    } else {
        svc.migration_plan().map_err(refuse)?
    };
    match format {
        OutputFormat::Json => w(
            out,
            pretty(&serde_json::to_value(&plan).unwrap_or_default()),
        )?,
        OutputFormat::Text => render_plan(&plan, out)?,
    }
    // A plan with nothing blocked is a success; a plan that could not carry out everything
    // it was asked to is not, and `--plan` never fails for having found something to do.
    Ok(if apply && plan.blocked > 0 {
        crate::worktree::EXIT_REFUSED
    } else {
        0
    })
}

fn render_plan(plan: &MigrationPlan, out: &mut Out<'_>) -> Result<()> {
    let verb = if plan.applied { "migrated" } else { "migrate" };
    if plan.steps.is_empty() {
        w(
            out,
            format!(
                "nothing to {verb}: every linked worktree is already under {}",
                plan.worktree_root
            ),
        )?;
        return Ok(());
    }
    for step in &plan.steps {
        w(out, "")?;
        w(out, step.from.clone())?;
        match (&step.to, step.applied, &step.blocked_by) {
            (Some(to), true, _) => w(out, format!("  -> {to}   (moved)"))?,
            (Some(to), false, _) => w(out, format!("  -> {to}"))?,
            (None, _, Some(why)) => w(out, format!("  BLOCKED — {why}"))?,
            (None, _, None) => w(out, "  BLOCKED")?,
        }
        if let Some(b) = &step.branch {
            w(out, format!("  branch: {b}"))?;
        }
    }
    w(out, "")?;
    w(
        out,
        format!(
            "{} step(s) to {verb} into {}: {} safe, {} blocked",
            plan.steps.len(),
            plan.worktree_root,
            plan.safe,
            plan.blocked
        ),
    )?;
    if !plan.applied && plan.safe > 0 {
        w(
            out,
            "nothing was changed; run `majordomus worktree migrate --apply`",
        )?;
    }
    Ok(())
}

fn prune(app: &App, dry_run: bool, format: OutputFormat, out: &mut Out<'_>) -> Result<u8> {
    let report = service(app)?.prune(dry_run).map_err(refuse)?;
    match format {
        OutputFormat::Json => w(
            out,
            pretty(&serde_json::to_value(&report).unwrap_or_default()),
        )?,
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
                        "{} record(s) {}",
                        report.pruned.len(),
                        if report.applied {
                            "dropped"
                        } else {
                            "would be dropped; nothing was changed"
                        }
                    ),
                )?;
            }
        }
    }
    Ok(0)
}

/// A field of the tab-separated form: no tab and no newline may survive in it, or a reader
/// splitting on tabs would see a row that is not the row that was written. A path can carry
/// both, so this is a correctness measure and not tidying.
fn clean(s: &str) -> String {
    let s = s.replace(['\t', '\n', '\r'], " ");
    if s.is_empty() {
        "-".to_string()
    } else {
        s
    }
}

/// Whether a report's policy status is a failure, for a caller that only wants the verdict.
pub fn is_failure(status: PolicyStatus) -> bool {
    status == PolicyStatus::Fail
}
