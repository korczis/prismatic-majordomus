//! `majordomus release`: the changelog, the version, and the one writer that raises it.
//!
//! The read half delegates to the capabilities, so the command line renders exactly what
//! HTTP and MCP answer with. `bump` does not: it writes tracked files, which no capability
//! may do, and the exposure policy keeps it off every machine surface for that reason.

use std::io::Write;

use crate::app::App;
use crate::cli::{OutputFormat, ReleaseArgs, ReleaseCommand};
use crate::error::{Error, Result};
use crate::release::compat::{Impact, Severity, Status, VersionPlan};
use crate::release::{self, changelog, version};

/// Exit code when the two writers of the version disagree, matching
/// `scripts/release-version --check`.
pub const EXIT_DISAGREE: u8 = 10;

/// Exit code when the declared version is smaller than the public contract requires.
///
/// The same code `scripts/ci/version-matches-surface` has always returned, kept so that the
/// gate reading it does not have to change its meaning when it becomes an adapter.
pub const EXIT_UNDER_VERSIONED: u8 = 10;

/// Exit code when the analysis could not be made at all: no release to compare with, a
/// baseline whose registry is not committed, a shallow clone. Distinct from a refusal,
/// because "I could not tell" and "the answer is no" are different facts.
pub const EXIT_UNREADABLE: u8 = 12;

/// Run `majordomus release`.
pub fn run(args: ReleaseArgs) -> Result<u8> {
    match args.command {
        None => render_changelog(&args, None),
        Some(ReleaseCommand::Changelog { ref version }) => {
            let v = version.clone();
            render_changelog(&args, v)
        }
        Some(ReleaseCommand::Version) => render_version(&args),
        Some(ReleaseCommand::Analyze { ref since, explain }) => {
            let since = since.clone();
            analyze(&args, since.as_deref(), explain)
        }
        Some(ReleaseCommand::Bump {
            ref level,
            ref exact,
            dry_run,
        }) => bump(&args, level.as_deref(), exact.as_deref(), dry_run),
    }
}

/// The changelog, through the capability so that every surface renders one value.
fn render_changelog(args: &ReleaseArgs, only: Option<String>) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let mut input = serde_json::Map::new();
    if let Some(v) = only {
        input.insert("version".into(), serde_json::Value::String(v));
    }
    let value = app
        .context
        .execute("release.changelog", serde_json::Value::Object(input))
        .map_err(|e| Error::Refused {
            code: 12,
            reason: e.to_string(),
        })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            let log: release::Changelog =
                serde_json::from_value(value).map_err(|e| Error::Protocol {
                    reason: e.to_string(),
                })?;
            write!(out, "{}", changelog::render(&log)).map_err(Error::Transport)?;
        }
    }
    Ok(0)
}

/// The version report. Exits 10 when the two writers disagree, which is the same verdict
/// `scripts/release-version --check` gives and the same code.
fn render_version(args: &ReleaseArgs) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let value = app
        .context
        .execute("release.version", serde_json::json!({}))
        .map_err(|e| Error::Refused {
            code: 12,
            reason: e.to_string(),
        })?;
    let report: release::model::VersionReport =
        serde_json::from_value(value.clone()).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            writeln!(out, "declared     {}", report.declared).map_err(Error::Transport)?;
            writeln!(out, "tool         {}", report.tool).map_err(Error::Transport)?;
            writeln!(
                out,
                "agree        {}",
                if report.agree { "yes" } else { "NO" }
            )
            .map_err(Error::Transport)?;
            writeln!(
                out,
                "last release {}",
                report.last_release.as_deref().unwrap_or("—")
            )
            .map_err(Error::Transport)?;
            writeln!(out, "commits      {} since it", report.changes.len())
                .map_err(Error::Transport)?;
            writeln!(out, "bump         {}", report.bump).map_err(Error::Transport)?;
            writeln!(
                out,
                "next         {}",
                report.next.as_deref().unwrap_or("—")
            )
            .map_err(Error::Transport)?;
        }
    }
    Ok(if report.agree { 0 } else { EXIT_DISAGREE })
}

/// The plan, through the capability so that the terminal renders what HTTP and MCP answer.
fn plan_of(app: &App, since: Option<&str>) -> Result<VersionPlan> {
    let mut input = serde_json::Map::new();
    if let Some(r) = since {
        input.insert("since".into(), serde_json::Value::String(r.to_string()));
    }
    let value = app
        .context
        .execute("release.analysis", serde_json::Value::Object(input))
        .map_err(|e| Error::Refused {
            code: EXIT_UNREADABLE,
            reason: e.to_string(),
        })?;
    serde_json::from_value(value).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })
}

/// `majordomus release analyze`.
///
/// Exits 0 when the declared version covers what the contract did, and
/// [`EXIT_UNDER_VERSIONED`] when it does not — which is what makes this command usable as
/// the gate rather than something the gate re-implements.
fn analyze(args: &ReleaseArgs, since: Option<&str>, explain: bool) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let plan = plan_of(&app, since)?;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => {
            // The canonical value, not a parse of the text below: every other surface
            // answers with exactly these bytes.
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&plan).unwrap_or_default()
            )
            .map_err(Error::Transport)?;
        }
        OutputFormat::Text => render_plan(&mut out, &plan, explain)?,
    }
    Ok(if plan.status == Status::Blocked {
        EXIT_UNDER_VERSIONED
    } else {
        0
    })
}

/// The human rendering of a plan. One function, so `analyze` and `bump` cannot describe the
/// same verdict differently.
fn render_plan(out: &mut impl Write, plan: &VersionPlan, explain: bool) -> Result<()> {
    let (added, changed, removed) = plan.counts();
    writeln!(out, "Majordomus release analysis").map_err(Error::Transport)?;
    writeln!(out).map_err(Error::Transport)?;
    writeln!(out, "Baseline").map_err(Error::Transport)?;
    writeln!(out, "  release       {}", plan.baseline.reference).map_err(Error::Transport)?;
    writeln!(
        out,
        "  commit        {}",
        &plan.baseline.commit[..plan.baseline.commit.len().min(12)]
    )
    .map_err(Error::Transport)?;
    writeln!(out, "  surface       {} public atoms", plan.baseline.atoms)
        .map_err(Error::Transport)?;
    if !plan.baseline.recorded {
        writeln!(out, "  recorded      no — taken from git's tags alone")
            .map_err(Error::Transport)?;
    }
    writeln!(out).map_err(Error::Transport)?;
    writeln!(out, "Current").map_err(Error::Transport)?;
    writeln!(out, "  version       {}", plan.declared_version).map_err(Error::Transport)?;
    if !plan.writers_agree {
        writeln!(
            out,
            "  tool          {} — THE TWO WRITERS DISAGREE",
            plan.tool_version
        )
        .map_err(Error::Transport)?;
    }
    writeln!(out, "  surface       {} public atoms", plan.atoms).map_err(Error::Transport)?;
    writeln!(out).map_err(Error::Transport)?;
    writeln!(out, "Compatibility").map_err(Error::Transport)?;
    writeln!(out, "  implied       {}", plan.implied.as_str()).map_err(Error::Transport)?;
    writeln!(out, "  required      {}", plan.required.as_str()).map_err(Error::Transport)?;
    writeln!(out, "  declared      {}", plan.declared.as_str()).map_err(Error::Transport)?;
    writeln!(out, "  next minimum  {}", plan.required_version).map_err(Error::Transport)?;
    writeln!(out, "  policy        {}", plan.policy.statement()).map_err(Error::Transport)?;

    if !plan.changes.is_empty() {
        writeln!(out).map_err(Error::Transport)?;
        writeln!(
            out,
            "Changes       {added} added, {changed} changed, {removed} removed"
        )
        .map_err(Error::Transport)?;
        // Without --explain the list is capped: a plan with two hundred entries is not a
        // report a person reads, and the cap says how much it withheld rather than
        // truncating silently.
        let shown = if explain {
            plan.changes.len()
        } else {
            plan.changes.len().min(12)
        };
        for c in plan.changes.iter().take(shown) {
            writeln!(out, "  {c}").map_err(Error::Transport)?;
            if explain {
                writeln!(out, "      {} · {}", c.impact.as_str(), c.detail)
                    .map_err(Error::Transport)?;
            }
        }
        if shown < plan.changes.len() {
            writeln!(
                out,
                "  … {} more; run with --explain for every one and why it counts",
                plan.changes.len() - shown
            )
            .map_err(Error::Transport)?;
        }
    }

    if plan.breaking {
        writeln!(out).map_err(Error::Transport)?;
        writeln!(out, "BREAKING").map_err(Error::Transport)?;
        for c in plan.breaking_changes() {
            writeln!(out, "  {c}").map_err(Error::Transport)?;
        }
        writeln!(
            out,
            "  Each belongs in the release record and in the changelog as a breaking change."
        )
        .map_err(Error::Transport)?;
    }

    // The line this subsystem exists for: the label a person chose, measured.
    if plan.understated {
        writeln!(out).map_err(Error::Transport)?;
        writeln!(
            out,
            "WARNING the commits since {} classify themselves as {} and the contract moved by {}.",
            plan.baseline.reference,
            plan.commits.implied.as_str(),
            plan.implied.as_str()
        )
        .map_err(Error::Transport)?;
        writeln!(
            out,
            "        The contract decides; the commit subjects understate what happened."
        )
        .map_err(Error::Transport)?;
    }

    for d in &plan.diagnostics {
        writeln!(out).map_err(Error::Transport)?;
        writeln!(
            out,
            "{} {}",
            if d.severity == Severity::Error {
                "ERROR  "
            } else {
                "WARNING"
            },
            d.message
        )
        .map_err(Error::Transport)?;
    }

    writeln!(out).map_err(Error::Transport)?;
    match plan.status {
        Status::Ok => {
            writeln!(out, "Status").map_err(Error::Transport)?;
            if plan.required == Impact::None {
                writeln!(
                    out,
                    "  OK    the surface is unchanged since {}; no bump is owed",
                    plan.baseline.reference
                )
                .map_err(Error::Transport)?;
            } else {
                writeln!(
                    out,
                    "  OK    {} declared covers the {} the contract requires",
                    plan.declared.as_str(),
                    plan.required.as_str()
                )
                .map_err(Error::Transport)?;
            }
        }
        Status::Blocked => {
            writeln!(out, "Status").map_err(Error::Transport)?;
            writeln!(out, "  BLOCKED").map_err(Error::Transport)?;
            if plan.declared < plan.required {
                writeln!(
                    out,
                    "  the contract requires a {} release and the version declares {}",
                    plan.required.as_str(),
                    plan.declared.as_str()
                )
                .map_err(Error::Transport)?;
            }
            writeln!(out).map_err(Error::Transport)?;
            writeln!(out, "Fix").map_err(Error::Transport)?;
            writeln!(out, "  majordomus release bump").map_err(Error::Transport)?;
        }
    }
    Ok(())
}

/// Raise the version in every place that states it, to at least what the public contract
/// requires.
///
/// # The authority this no longer has
///
/// This used to compute the bump itself, from the conventional-commit types of the commits
/// since the last release: `feat:` was a minor, anything else a patch, and a capability
/// deleted under a `refactor:` heading was a patch. It does not compute anything now. It
/// asks [`crate::release::compat::analyze`] — the same value `release analyze`, the HTTP
/// route, the MCP tool and the CI gate get — and its whole job is to apply it:
///
/// ```text
///   plan ─ validate ─ apply(plan)
/// ```
///
/// `--level` and `--exact` name a *higher* version than the measured minimum, never a lower
/// one. An override that could go under the requirement would not be an override; it would
/// be the hole that makes the whole measurement decorative.
///
/// # When the contract cannot be measured
///
/// A repository that has published nothing, or whose last release predates the committed
/// registry, has no baseline — so there is no floor, and the honest thing is to say so
/// rather than to invent one. A version named explicitly is still written: refusing would
/// make the command unusable in exactly the repositories that most need to cut a first
/// release. A *derived* bump is refused, because there is nothing to derive it from.
fn bump(args: &ReleaseArgs, level: Option<&str>, exact: Option<&str>, dry_run: bool) -> Result<u8> {
    // Arguments are validated before anything is read, so a typo is an exit 2 whatever the
    // state of the repository around it.
    let wanted_exact = match exact {
        Some(v) => Some(version::Version::parse(v).ok_or_else(|| Error::Refused {
            code: 2,
            reason: format!("'{v}' is not a version; it must be three numbers, `1.2.3`"),
        })?),
        None => None,
    };
    let wanted_level = match level {
        Some(word) => Some(Impact::parse(word).ok_or_else(|| Error::Refused {
            code: 2,
            reason: format!("'{word}' is not a level; it is one of major, minor, patch, none"),
        })?),
        None => None,
    };

    let app = App::load(&args.repo)?;
    let root = std::path::Path::new(&app.index().repository.root).to_path_buf();
    // A plan that cannot be made is not an error here — it is the absence of a floor, which
    // the branches below handle explicitly and report.
    let plan = plan_of(&app, None).ok();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    // A plan that could not be trusted cannot authorise a write. The diagnostics say which
    // fact was unreadable, and they are printed rather than summarised away.
    if let Some(p) = &plan {
        if p.has_errors() {
            writeln!(
                out,
                "release: the version cannot be raised from a plan that is not sound"
            )
            .map_err(Error::Transport)?;
            for d in p
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
            {
                writeln!(out, "         {}", d.message).map_err(Error::Transport)?;
            }
            return Ok(EXIT_UNREADABLE);
        }
    }

    let current = version::declared(&root)
        .and_then(|v| version::Version::parse(&v))
        .ok_or_else(|| Error::Refused {
            code: EXIT_UNREADABLE,
            reason: format!(
                "{} declares no version that is three numbers, so nothing can be raised",
                version::MANIFEST
            ),
        })?;

    // The floor: the baseline raised by what the contract requires. `None` when no baseline
    // could be read — which is a different thing from a floor of zero, and is said so.
    let floor = plan.as_ref().and_then(|p| {
        version::Version::parse(&p.baseline.version).map(|base| base.raised_to(p.required))
    });

    let (to, source) = match (wanted_exact, wanted_level) {
        (Some(v), _) => (v, "explicit --exact"),
        (None, Some(impact)) => (current.raised_to(impact), "explicit --level"),
        // The measured minimum, which is the whole point: with no argument at all, the
        // contract decides. With no measurable contract there is nothing to decide from.
        (None, None) => match floor {
            Some(f) => (
                if current >= f { current } else { f },
                "the public contract",
            ),
            None => {
                writeln!(
                    out,
                    "release: the public contract cannot be measured here, so there is no bump to derive"
                )
                .map_err(Error::Transport)?;
                writeln!(
                    out,
                    "         {}",
                    plan_refusal(&app).unwrap_or_else(|| "no baseline could be read".into())
                )
                .map_err(Error::Transport)?;
                writeln!(
                    out,
                    "         name the version deliberately: `majordomus release bump --level minor` or `--exact <version>`"
                )
                .map_err(Error::Transport)?;
                return Ok(EXIT_UNREADABLE);
            }
        },
    };

    // An override may go above the floor and never below it.
    if let (Some(f), Some(p)) = (floor, plan.as_ref()) {
        if to < f {
            writeln!(
                out,
                "release: REFUSED {to} is below {f}, which is the smallest version this tree may declare"
            )
            .map_err(Error::Transport)?;
            writeln!(
                out,
                "         the contract requires a {} release since {} and {to} was asked for",
                p.required.as_str(),
                p.baseline.reference
            )
            .map_err(Error::Transport)?;
            for c in p.changes.iter().take(8) {
                writeln!(out, "         {c}").map_err(Error::Transport)?;
            }
            writeln!(
                out,
                "         nothing was written; see `majordomus release analyze --explain`"
            )
            .map_err(Error::Transport)?;
            return Ok(EXIT_UNDER_VERSIONED);
        }
    }
    if to < current {
        writeln!(
            out,
            "release: REFUSED {to} is below the {current} this tree already declares; a version does not go down"
        )
        .map_err(Error::Transport)?;
        return Ok(EXIT_UNDER_VERSIONED);
    }

    let to = to.to_string();
    let declared = current.to_string();
    if to == declared {
        match plan.as_ref() {
            Some(p) => writeln!(
                out,
                "release: the version is already {to}, which covers the {} the contract requires since {}; nothing written",
                p.required.as_str(),
                p.baseline.reference
            ),
            None => writeln!(out, "release: the version is already {to}; nothing written"),
        }
        .map_err(Error::Transport)?;
        return Ok(0);
    }

    match plan.as_ref() {
        Some(p) => writeln!(
            out,
            "release: {declared} -> {to} ({} required since {}, from {source})",
            p.required.as_str(),
            p.baseline.reference
        ),
        None => writeln!(
            out,
            "release: would raise {declared} -> {to} (from {source}; the contract is not measurable here)"
        ),
    }
    .map_err(Error::Transport)?;
    if source != "the public contract" {
        if let Some(f) = floor {
            // Provenance: a version larger than the measurement is allowed and never silent.
            writeln!(
                out,
                "         required {f}, selected {to}; source: explicit override"
            )
            .map_err(Error::Transport)?;
        }
    }
    if plan.as_ref().is_some_and(|p| p.understated) {
        let p = plan.as_ref().expect("checked just above");
        writeln!(
            out,
            "         note: the commit subjects classify this window as {} and the contract moved by {}",
            p.commits.implied.as_str(),
            p.implied.as_str()
        )
        .map_err(Error::Transport)?;
    }

    if dry_run {
        writeln!(out, "         {} (unwritten)", version::MANIFEST).map_err(Error::Transport)?;
        writeln!(out, "         {} (unwritten)", version::ENTRY).map_err(Error::Transport)?;
        return Ok(0);
    }

    let written = version::write(&root, &to).map_err(|e| Error::io(root.clone(), e))?;
    for f in &written {
        writeln!(out, "         {f} written").map_err(Error::Transport)?;
    }
    // Read every site back. A half-applied bump is exactly the failure the one-writer rule
    // exists to prevent, so the writer proves its own work rather than leaving it to the
    // release that finds out at its first step.
    let after_manifest = version::declared(&root).unwrap_or_default();
    let after_tool = version::tool(&root).unwrap_or_default();
    let after_lock = version::locked(&root);
    let lock_disagrees = after_lock.as_deref().is_some_and(|v| v != to);
    if after_manifest != to || after_tool != to || lock_disagrees {
        writeln!(
            out,
            "release: the bump did not take in every place ({} states '{after_manifest}', {} states '{after_tool}', {} states '{}')",
            version::MANIFEST,
            version::ENTRY,
            version::LOCK,
            after_lock.as_deref().unwrap_or("nothing")
        )
        .map_err(Error::Transport)?;
        return Ok(EXIT_DISAGREE);
    }
    writeln!(
        out,
        "         both writers agree; every site states {to}. `majordomus generate changelog` renders the new section"
    )
    .map_err(Error::Transport)?;
    Ok(0)
}

/// Why the plan could not be made, for the message that says a floor is unmeasurable.
fn plan_refusal(app: &App) -> Option<String> {
    match plan_of(app, None) {
        Ok(_) => None,
        Err(Error::Refused { reason, .. }) => Some(reason),
        Err(e) => Some(e.to_string()),
    }
}
