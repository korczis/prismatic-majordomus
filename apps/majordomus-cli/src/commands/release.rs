//! `majordomus release`: the changelog, the version, and the one writer that raises it.
//!
//! The read half delegates to the capabilities, so the command line renders exactly what
//! HTTP and MCP answer with. `bump` and `advance` do not: they write tracked files, which no
//! capability may do, and the exposure policy keeps them off every machine surface for that
//! reason. They share one write ([`write_and_verify`]) and differ only in how the version is
//! chosen: `bump` from the contract or an explicit override, `advance` from the version
//! obligation (ADR 0106).

use std::io::Write;

use crate::app::App;
use crate::cli::{OutputFormat, ReleaseArgs, ReleaseCommand};
use crate::error::{Error, Result};
use crate::release::compat::{Impact, Severity, Status, VersionPlan};
use crate::release::obligation::{ObligationState, VersionObligation};
use crate::release::{self, changelog, version};

/// Exit code when the version is not stated the way it must be — the projection behind or
/// apart from the manifest, a version written by hand, a bump that did not take — matching
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
        Some(ReleaseCommand::Obligation { ref base }) => {
            let base = base.clone();
            render_obligation(&args, base.as_deref())
        }
        Some(ReleaseCommand::Advance { ref base, dry_run }) => {
            let base = base.clone();
            advance(&args, base.as_deref(), dry_run)
        }
        Some(ReleaseCommand::MergeVersion {
            ref base,
            ref ours,
            ref theirs,
            ref path,
        }) => merge_version(base, ours, theirs, path),
    }
}

/// The merge driver git runs for `merge=version` (`%O %A %B %P`): 0 when every hunk merged,
/// 1 when conflict markers were left for a person, as git requires of a driver, and 12 when
/// a file could not be read, which is no merge at all. It needs no repository: git runs it
/// inside a merge, with the three versions in temporary files.
fn merge_version(
    base: &std::path::Path,
    ours: &std::path::Path,
    theirs: &std::path::Path,
    path: &str,
) -> Result<u8> {
    match crate::release::reconcile::drive(base, ours, theirs, path) {
        Ok(true) => Ok(0),
        Ok(false) => Ok(1),
        Err(why) => {
            eprintln!("majordomus: release merge-version: {why}");
            Ok(EXIT_UNREADABLE)
        }
    }
}

/// The version obligation, the value HTTP and MCP answer with, rendered for a person or as
/// JSON. Exits with the verdict: 0 holds, 10 owed or behind, 12 unread.
fn render_obligation(args: &ReleaseArgs, base: Option<&str>) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let o = obligation_of(&app, base);
    report_obligation(&mut std::io::stdout().lock(), args.format, &o)
}

/// Write the obligation in a format, whole, and answer with its verdict's exit.
fn report_obligation(
    out: &mut impl Write,
    format: OutputFormat,
    o: &VersionObligation,
) -> Result<u8> {
    let body = match format {
        OutputFormat::Json => format!("{}\n", serde_json::to_string_pretty(o).unwrap_or_default()),
        OutputFormat::Text => obligation_text(o),
    };
    put(out, &body).map(|()| o.state.exit_code())
}

/// One write of a whole report, so a reader that has gone away is one transport failure,
/// reported as one, and never half a report followed by an exit that claims success.
fn put(out: &mut impl Write, text: &str) -> Result<()> {
    out.write_all(text.as_bytes()).map_err(Error::Transport)
}

/// The obligation: the same function the `release.obligation` capability answers with, over the
/// same repository, policy and registry — one decision, so the command line, HTTP and MCP
/// cannot render different values.
fn obligation_of(app: &App, base: Option<&str>) -> VersionObligation {
    let root = std::path::Path::new(&app.index().repository.root);
    let policy = release::obligation::policy_of(root);
    release::obligation::obligation(
        root,
        &app.context.registry,
        &app.index().objects,
        &policy,
        base,
    )
}

/// The obligation for a person at a terminal: the inputs first, the verdict, then why.
///
/// ```text
/// Majordomus version obligation
///   subject     feature/x
///   trunk       origin/master 0123456789ab declares 0.12.0
///   ...
/// ```
pub fn obligation_text(o: &VersionObligation) -> String {
    let mut s = String::from("Majordomus version obligation\n");
    let mut line = |k: &str, v: String| s.push_str(&format!("  {k:<12}{v}\n"));
    line("subject", o.subject.clone());
    line(
        "trunk",
        match &o.trunk {
            Some(t) => format!(
                "{} {} declares {}{}",
                t.reference,
                &t.commit[..t.commit.len().min(12)],
                t.version,
                if t.contained {
                    ""
                } else {
                    " (not contained in this tree)"
                }
            ),
            None => "unreadable".into(),
        },
    );
    line("declared", o.declared.clone());
    line(
        "carries",
        format!(
            "{} ({} authored, {} derived, {} release record(s), {} version advance)",
            o.carries.as_str(),
            o.paths.work,
            o.paths.derived,
            o.paths.release_evidence,
            o.paths.version_advance
        ),
    );
    line(
        "contract",
        match (&o.contract.unmeasured, &o.contract.baseline) {
            (Some(_), _) => "not measured".into(),
            (None, Some(b)) => format!(
                "{} since {b}{}",
                o.contract.required.as_str(),
                o.contract
                    .floor
                    .as_ref()
                    .map(|f| format!(", at least {f}"))
                    .unwrap_or_default()
            ),
            (None, None) => "none".into(),
        },
    );
    line(
        "cadence",
        format!(
            "{} of the policy's {}, at least {}",
            o.cadence.required.as_str(),
            o.cadence.policy.as_str(),
            o.cadence.floor
        ),
    );
    line("effective", o.effective.as_str().to_string());
    line("minimum", o.minimum.clone());
    line("state", o.state.as_str().to_string());
    line("id", o.id.clone());
    s.push_str("  why\n");
    for r in &o.reasons {
        s.push_str(&format!("    {r}\n"));
    }
    if !o.paths.work_examples.is_empty() {
        s.push_str("  work\n");
        for p in &o.paths.work_examples {
            s.push_str(&format!("    {p}\n"));
        }
        if o.paths.work > o.paths.work_examples.len() {
            s.push_str(&format!(
                "    ... and {} more\n",
                o.paths.work - o.paths.work_examples.len()
            ));
        }
    }
    if let Some(r) = &o.remedy {
        s.push_str(&format!("  remedy      {r}\n"));
    }
    s
}

/// `release advance`: satisfy the obligation through the one writer.
///
/// The obligation decides the version and [`write_and_verify`] writes it — the same write
/// `release bump` makes — so there is one writer and two ways of choosing what it writes.
/// It is idempotent by construction: the obligation is a predicate over the tree and the
/// trunk, so once the declared version reaches the minimum, asking again writes nothing.
fn advance(args: &ReleaseArgs, base: Option<&str>, dry_run: bool) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let root = std::path::Path::new(&app.index().repository.root).to_path_buf();
    let o = obligation_of(&app, base);
    advance_with(&mut std::io::stdout().lock(), &root, &o, dry_run)
}

/// What `release advance` does with an obligation already computed: nothing when it holds, a
/// refusal when no version can be chosen, and the one write when it is owed.
fn advance_with(
    out: &mut impl Write,
    root: &std::path::Path,
    o: &VersionObligation,
    dry_run: bool,
) -> Result<u8> {
    let id = &o.id;
    match o.state {
        ObligationState::Satisfied | ObligationState::NotOwed => put(
            out,
            &format!(
                "release: the obligation {id} holds ({}): {} covers the minimum {}; nothing written\n",
                o.state.as_str(),
                o.declared,
                o.minimum
            ),
        )
        .map(|()| 0),
        ObligationState::Unverified | ObligationState::Behind => put(
            out,
            &format!(
                "release: REFUSED the obligation {id} is {}, so no version can be chosen for it\n{}         nothing was written\n",
                o.state.as_str(),
                obligation_text(o)
            ),
        )
        .map(|()| o.state.exit_code()),
        ObligationState::Owed => {
            let trunk = o.trunk.as_ref().map_or("the trunk", |t| t.version.as_str());
            let head = format!(
                "release: {} -> {} (obligation {id}: {} over {trunk}; contract {}, cadence {})\n",
                o.declared,
                o.minimum,
                o.effective.as_str(),
                o.contract.required.as_str(),
                o.cadence.required.as_str()
            );
            if dry_run {
                return put(
                    out,
                    &format!(
                        "{head}         {} (unwritten)\n         {} (unwritten)\n         {}\n",
                        version::MANIFEST,
                        version::LOCK,
                        derived_after(&o.minimum)
                    ),
                )
                .map(|()| 0);
            }
            write_and_verify(root, &o.minimum)
                .and_then(|(code, report)| put(out, &format!("{head}{report}")).map(|()| code))
        }
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

/// The version report, and the verdict on where the version is stated.
///
/// Exits 10 when the projection the shell tool reads is not current, or when a version is
/// written down by hand where the tool's own files live ([`version::diagnose`], the same
/// findings `release analyze` carries). That is the gate `version-authored-once` runs, and
/// the same code `scripts/release-version --check` gives. The findings are printed after
/// the report, or to stderr under `--format json`, whose stdout is the capability's value.
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
    let root = std::path::Path::new(&app.index().repository.root).to_path_buf();
    let findings = version::diagnose(&root);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match args.format {
        OutputFormat::Json => {
            writeln!(
                out,
                "{}",
                serde_json::to_string_pretty(&value).unwrap_or_default()
            )
            .map_err(Error::Transport)?;
            let mut err = std::io::stderr().lock();
            for d in &findings {
                writeln!(err, "{} {}: {}", severity_word(d.severity), d.id, d.message)
                    .map_err(Error::Transport)?;
            }
        }
        OutputFormat::Text => {
            // Rendered whole and handed to stdout at once, so a reader that has gone away
            // is one transport failure, reported — the exit says so, not a success.
            out.write_all(version_text(&report, &findings).as_bytes())
                .map_err(Error::Transport)?;
        }
    }
    Ok(if report.agree && findings.is_empty() {
        0
    } else {
        EXIT_DISAGREE
    })
}

/// What `release bump` says when it is asked to derive a version and the contract could not
/// be measured: that there is nothing to derive one from, why — a line per reason, as
/// `release version` gives them — and how to name the version deliberately instead.
fn unmeasured_refusal(why: &str) -> String {
    let mut lines = vec![
        "release: the public contract cannot be measured here, so there is no bump to derive"
            .to_string(),
    ];
    lines.extend(why.lines().map(|line| format!("         {line}")));
    lines.push(
        "         name the version deliberately: `majordomus release bump --level minor` or \
         `--exact <version>`"
            .to_string(),
    );
    lines.join("\n") + "\n"
}

/// The text rendering of `release version`: the version where it is stated, the release
/// it is measured from, the `next` that was decided and who decided it — with the reason,
/// line by line, when nobody could — and what the commits imply, labelled as the evidence
/// it is. The findings follow, each after a blank line.
fn version_text(
    report: &release::model::VersionReport,
    findings: &[release::compat::Diagnostic],
) -> String {
    use release::model::DecidedBy;
    let mut lines = vec![
        format!("declared     {}", report.declared),
        format!("tool         {}", report.tool),
        format!("agree        {}", if report.agree { "yes" } else { "NO" }),
        format!(
            "last release {}",
            report.last_release.as_deref().unwrap_or("—")
        ),
        format!("commits      {} since it", report.changes.len()),
        format!(
            "next         {} ({})",
            report.next.as_deref().unwrap_or("—"),
            report.decided_by.phrase()
        ),
    ];
    if let Some(why) = report.contract_unreadable.as_deref() {
        // One line per error, as `release bump` prints them when it refuses.
        lines.extend(
            why.lines()
                .map(|line| format!("             because {line}")),
        );
        lines.push(
            "             name it deliberately: `majordomus release bump --level <level>` \
             or `--exact <version>`"
                .to_string(),
        );
    }
    lines.push(format!("bump         {}", report.bump));
    lines.push(format!(
        "             commits imply {}{} ({})",
        report.bump,
        report
            .commits_imply
            .as_deref()
            .map(|v| format!(" -> {v}"))
            .unwrap_or_default(),
        match report.decided_by {
            DecidedBy::Contract => "evidence; the contract decides",
            DecidedBy::ContractAndCommits => {
                "evidence; the contract requires no release, so a patch carries them"
            }
            DecidedBy::Undecided => "evidence only; it does not answer in the contract's place",
        }
    ));
    for d in findings {
        lines.push(String::new());
        lines.push(format!(
            "{} {}: {}",
            severity_word(d.severity),
            d.id,
            d.message
        ));
    }
    lines.join("\n") + "\n"
}

/// How a diagnostic's severity is printed, the same word in every rendering.
fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "ERROR  ",
        Severity::Warning => "WARNING",
        Severity::Note => "NOTE   ",
    }
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
            "  tool          {} — {} IS NOT CURRENT; scripts/derive projects it",
            if plan.tool_version.is_empty() {
                "nothing"
            } else {
                plan.tool_version.as_str()
            },
            version::PROJECTION
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
        writeln!(out, "{} {}", severity_word(d.severity), d.message).map_err(Error::Transport)?;
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

/// Raise the version in the one place it is authored, to at least what the public contract
/// requires, and keep the lock's record of it in step; `scripts/derive` derives the rest.
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
/// release. A *derived* bump is refused, because there is nothing to derive it from — and
/// so is one whose baseline or declared version is not three numbers: the analysis could
/// only guess, and ADR 0051 refuses an unmeasurable baseline rather than guessing it.
///
/// # One selection
///
/// With no target named, the version written is [`version::default_target`] of
/// [`version::select`] — the selection `release version` reports — so the report's `next`
/// and what this writes are one answer, made once.
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
    // The one release selection, the same `release version` reports: the analysis and the
    // version decided from it. A plan that cannot be made is not an error here — it is the
    // absence of a floor, which the branches below handle explicitly and report.
    let selection = version::select(&root, &app.context.registry, &app.index().objects);
    let plan = selection.plan.as_ref().ok();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    // A plan that could not be trusted cannot authorise a write. The diagnostics say which
    // fact was unreadable, and they are printed rather than summarised away.
    if let Some(p) = plan {
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
    let floor = plan.and_then(|p| {
        version::Version::parse(&p.baseline.version).map(|base| base.raised_to(p.required))
    });

    let (to, source) = match (wanted_exact, wanted_level) {
        (Some(v), _) => (v, "explicit --exact"),
        (None, Some(impact)) => (current.raised_to(impact), "explicit --level"),
        // The selected version, which is the whole point: with no argument at all, the
        // contract decides — through the one selection `release version` reports, so the
        // writer raises to exactly the `next` the report states. With no measurable contract
        // no one decided, and there is nothing to raise to.
        (None, None) => match version::default_target(&selection.report, current) {
            Ok(to) => (
                to,
                match selection.report.decided_by {
                    release::model::DecidedBy::ContractAndCommits => {
                        "the public contract and the commits"
                    }
                    _ => "the public contract",
                },
            ),
            Err(why) => {
                out.write_all(unmeasured_refusal(&why).as_bytes())
                    .map_err(Error::Transport)?;
                return Ok(EXIT_UNREADABLE);
            }
        },
    };
    let explicit = wanted_exact.is_some() || wanted_level.is_some();

    // An override may go above the floor and never below it.
    if let (Some(f), Some(p)) = (floor, plan) {
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
    // The obligation's floor (ADR 0106): where the policy declares a cadence, a change set
    // carrying work owes it over the trunk's version, and no override may undershoot that
    // either. Without a cadence nothing is asked, so a repository that declares none is
    // judged by the contract alone, exactly as before.
    let policy = release::obligation::policy_of(&root);
    if policy.cadence != Impact::None {
        let o = release::obligation::obligation(
            &root,
            &app.context.registry,
            &app.index().objects,
            &policy,
            None,
        );
        if let Some(min) = version::Version::parse(&o.minimum) {
            if to < min && o.state != ObligationState::Unverified {
                return put(&mut out, &obligation_refusal(&to, &min, &o))
                    .map(|()| EXIT_UNDER_VERSIONED);
            }
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
        match plan {
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

    match plan {
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
    if explicit {
        if let Some(f) = floor {
            // Provenance: a version larger than the measurement is allowed and never silent.
            writeln!(
                out,
                "         required {f}, selected {to}; source: explicit override"
            )
            .map_err(Error::Transport)?;
        }
    }
    if let Some(p) = plan.filter(|p| p.understated) {
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
        writeln!(out, "         {} (unwritten)", version::LOCK).map_err(Error::Transport)?;
        writeln!(out, "         {}", derived_after(&to)).map_err(Error::Transport)?;
        return Ok(0);
    }

    write_and_verify(&root, &to).and_then(|(code, report)| put(&mut out, &report).map(|()| code))
}

/// The one write: the manifest's version line and the lock's record of it, then both read
/// back. `release bump` and `release advance` both end here, so there is one writer however
/// the version was chosen.
fn write_and_verify(root: &std::path::Path, to: &str) -> Result<(u8, String)> {
    let written = version::write(root, to).map_err(|e| Error::io(root.to_path_buf(), e))?;
    let mut report: String = written
        .iter()
        .map(|f| format!("         {f} written\n"))
        .collect();
    // Read both back. A half-applied bump is exactly the failure the one-writer rule exists
    // to prevent, so the writer proves its own work rather than leaving it to the build or
    // the release that finds out at its first step. The projection is not read: it is stale
    // now by design, and deriving it is the next step, not this one.
    let after_manifest = version::declared(root).unwrap_or_default();
    let after_lock = version::locked(root);
    let lock_disagrees = after_lock.as_deref().is_some_and(|v| v != to);
    if after_manifest != to || lock_disagrees {
        report.push_str(&format!(
            "release: the bump did not take ({} states '{after_manifest}', {} states '{}')\n",
            version::MANIFEST,
            version::LOCK,
            after_lock.as_deref().unwrap_or("nothing")
        ));
        return Ok((EXIT_DISAGREE, report));
    }
    report.push_str(&format!(
        "         {} now declares {to}\n         {}\n",
        version::MANIFEST,
        derived_after(to)
    ));
    Ok((0, report))
}

/// Why `release bump` refuses a version under the obligation's minimum: the minimum, every
/// reason the obligation gives for it, and the two commands that answer it.
fn obligation_refusal(
    to: &version::Version,
    min: &version::Version,
    o: &VersionObligation,
) -> String {
    let mut s = format!(
        "release: REFUSED {to} is below {min}, the minimum the version obligation {} computes\n",
        o.id
    );
    for r in &o.reasons {
        s.push_str(&format!("         {r}\n"));
    }
    s.push_str("         nothing was written; see `majordomus release obligation`, or run `majordomus release advance`\n");
    s
}

/// What a bump leaves for the derivation: the one sentence both the dry run and the write
/// end with, so the two cannot describe the next step differently.
fn derived_after(to: &str) -> String {
    format!(
        "{} and every generator stamp state {to} once scripts/derive has run: it builds, then \
         `majordomus generate` projects them. Never edit {} or bin/majordomus",
        version::PROJECTION,
        version::PROJECTION
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::compat::Diagnostic;
    use crate::release::model::{DecidedBy, VersionReport};

    fn report(decided_by: DecidedBy) -> VersionReport {
        VersionReport {
            declared: "0.4.0".into(),
            tool: "0.4.0".into(),
            agree: true,
            last_release: Some("0.4.0".into()),
            bump: "patch".into(),
            commits_imply: Some("0.4.1".into()),
            next: Some("0.5.0".into()),
            decided_by,
            contract_unreadable: None,
            changes: Vec::new(),
        }
    }

    /// An owed obligation of `feature/x` over a trunk at 1.8.0 this tree does not contain:
    /// the contract given, `work` authored paths (sampled at eight), a minor cadence, and a
    /// minimum of 2.0.0 — the value every test of the rendering and the advance starts from.
    fn obligation(
        contract: release::obligation::ContractRequirement,
        work: usize,
    ) -> VersionObligation {
        use release::obligation::*;
        VersionObligation {
            schema: OBLIGATION_SCHEMA.into(),
            id: "feature/x@1.8.0".into(),
            subject: "feature/x".into(),
            trunk: Some(Trunk {
                reference: "origin/master".into(),
                commit: "0123456789abcdef0123".into(),
                version: "1.8.0".into(),
                contained: false,
            }),
            declared: "1.8.0".into(),
            carries: Carries::Work,
            paths: PathCounts {
                work,
                derived: 2,
                release_evidence: 0,
                version_advance: 0,
                work_examples: (0..work.min(8)).map(|i| format!("lib/{i}.sh")).collect(),
            },
            contract,
            cadence: CadenceRequirement {
                policy: Impact::Minor,
                required: Impact::Minor,
                floor: "1.9.0".into(),
            },
            minimum: "2.0.0".into(),
            effective: Impact::Major,
            state: ObligationState::Owed,
            reasons: vec!["the public contract requires major since v1.8.0".into()],
            remedy: Some("majordomus release advance".into()),
        }
    }

    /// Every input the obligation was decided from is on the page, in the order a person
    /// checks them: the trunk (and that this tree is not on it), the contract, the cadence,
    /// the verdict, why, the work — sampled, with the rest counted — and the remedy.
    #[test]
    fn the_obligation_text_names_every_input_and_the_remedy() {
        use release::obligation::ContractRequirement;
        let measured = ContractRequirement {
            baseline: Some("v1.8.0".into()),
            required: Impact::Major,
            floor: Some("2.0.0".into()),
            unmeasured: None,
        };
        let text = obligation_text(&obligation(measured, 11));
        for line in [
            "trunk       origin/master 0123456789ab declares 1.8.0 (not contained in this tree)",
            "carries     work (11 authored, 2 derived, 0 release record(s), 0 version advance)",
            "contract    major since v1.8.0, at least 2.0.0",
            "cadence     minor of the policy's minor, at least 1.9.0",
            "effective   major",
            "minimum     2.0.0",
            "state       owed",
            "    lib/7.sh",
            "    ... and 3 more",
            "remedy      majordomus release advance",
        ] {
            assert!(text.contains(line), "missing {line:?} in:\n{text}");
        }

        let unmeasured = ContractRequirement {
            baseline: None,
            required: Impact::None,
            floor: None,
            unmeasured: Some("nothing published".into()),
        };
        assert!(obligation_text(&obligation(unmeasured, 1)).contains("contract    not measured"));

        let quiet = ContractRequirement {
            baseline: None,
            required: Impact::None,
            floor: None,
            unmeasured: None,
        };
        let mut o = obligation(quiet, 0);
        o.trunk = None;
        o.remedy = None;
        let text = obligation_text(&o);
        assert!(
            text.contains("contract    none") && text.contains("trunk       unreadable"),
            "{text}"
        );
        assert!(
            !text.contains("  work\n") && !text.contains("remedy"),
            "{text}"
        );
    }

    /// A writer whose reader has gone away.
    struct Gone;
    impl Write for Gone {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn manifest_at(version: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("apps/majordomus-cli")).unwrap();
        std::fs::write(
            dir.path().join(version::MANIFEST),
            format!("[package]\nversion = \"{version}\"\n"),
        )
        .unwrap();
        dir
    }

    fn quiet() -> release::obligation::ContractRequirement {
        release::obligation::ContractRequirement {
            baseline: None,
            required: Impact::None,
            floor: None,
            unmeasured: None,
        }
    }

    /// A report is written whole or not at all: a reader that went away is a transport
    /// failure, whatever the report was.
    #[test]
    fn a_report_to_a_reader_that_went_away_is_a_transport_failure() {
        assert!(matches!(put(&mut Gone, "x"), Err(Error::Transport(_))));
        let o = obligation(quiet(), 1);
        assert!(report_obligation(&mut Gone, OutputFormat::Text, &o).is_err());
        let dir = manifest_at("1.8.0");
        assert!(advance_with(&mut Gone, dir.path(), &o, false).is_err());
        assert_eq!(
            super::version::declared(dir.path()).as_deref(),
            Some("2.0.0"),
            "the write is made before the report"
        );
    }

    #[test]
    fn the_obligation_reports_in_either_format_and_exits_with_its_verdict() {
        let o = obligation(quiet(), 1);
        let mut text = Vec::new();
        assert_eq!(
            report_obligation(&mut text, OutputFormat::Text, &o).unwrap(),
            10
        );
        assert!(String::from_utf8(text)
            .unwrap()
            .starts_with("Majordomus version obligation"));
        let mut json = Vec::new();
        assert_eq!(
            report_obligation(&mut json, OutputFormat::Json, &o).unwrap(),
            10
        );
        let v: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(v["state"], "owed");
    }

    /// Each arm of `release advance`: nothing when it holds, a refusal when no version can
    /// be chosen, the plan on a dry run, and the one write when it is owed.
    #[test]
    fn the_advance_answers_each_verdict_its_own_way() {
        let dir = manifest_at("1.8.0");
        let mut o = obligation(quiet(), 1);

        o.state = ObligationState::Satisfied;
        let mut out = Vec::new();
        assert_eq!(advance_with(&mut out, dir.path(), &o, false).unwrap(), 0);
        assert!(String::from_utf8(out)
            .unwrap()
            .contains("holds (satisfied)"));

        o.state = ObligationState::Behind;
        let mut out = Vec::new();
        assert_eq!(advance_with(&mut out, dir.path(), &o, false).unwrap(), 10);
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("REFUSED the obligation feature/x@1.8.0 is behind")
                && text.contains("nothing was written")
        );

        o.state = ObligationState::Owed;
        let mut out = Vec::new();
        assert_eq!(advance_with(&mut out, dir.path(), &o, true).unwrap(), 0);
        assert!(String::from_utf8(out).unwrap().contains("(unwritten)"));
        assert_eq!(
            super::version::declared(dir.path()).as_deref(),
            Some("1.8.0"),
            "a dry run writes nothing"
        );

        let mut out = Vec::new();
        assert_eq!(advance_with(&mut out, dir.path(), &o, false).unwrap(), 0);
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("1.8.0 -> 2.0.0") && text.contains("now declares 2.0.0"),
            "{text}"
        );
        o.trunk = None;
        o.minimum = "2.1.0".into();
        let mut out = Vec::new();
        advance_with(&mut out, dir.path(), &o, true).unwrap();
        assert!(String::from_utf8(out).unwrap().contains("over the trunk"));
    }

    /// The writer reads its own work back: a manifest with no version line is a bump that
    /// did not take, and a manifest it cannot write is an error, not a report.
    #[test]
    fn the_write_is_read_back_and_a_failure_is_named() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("apps/majordomus-cli")).unwrap();
        std::fs::write(
            dir.path().join(version::MANIFEST),
            "[package]\nname = \"x\"\n",
        )
        .unwrap();
        let (code, report) = write_and_verify(dir.path(), "1.0.0").unwrap();
        assert_eq!(code, EXIT_DISAGREE);
        assert!(report.contains("the bump did not take"), "{report}");

        let dir = manifest_at("1.0.0");
        let manifest = dir.path().join(version::MANIFEST);
        std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o444)).unwrap();
        assert!(write_and_verify(dir.path(), "1.1.0").is_err());
        let o = obligation(quiet(), 1);
        assert!(advance_with(&mut Vec::new(), dir.path(), &o, false).is_err());
        std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn a_refused_override_names_the_minimum_every_reason_and_the_way_out() {
        let o = obligation(quiet(), 1);
        let v = |s| version::Version::parse(s).unwrap();
        let text = obligation_refusal(&v("1.8.1"), &v("2.0.0"), &o);
        assert!(text.starts_with("release: REFUSED 1.8.1 is below 2.0.0, the minimum the version obligation feature/x@1.8.0 computes"));
        assert!(text.contains("the public contract requires major since v1.8.0"));
        assert!(text.contains("majordomus release advance"));
    }

    /// Each severity prints as its own word, padded to one width so the messages after it
    /// line up; a note is not printed as a warning nor a warning as an error.
    #[test]
    fn every_severity_prints_as_its_own_word_at_one_width() {
        assert_eq!(severity_word(Severity::Error), "ERROR  ");
        assert_eq!(severity_word(Severity::Warning), "WARNING");
        assert_eq!(severity_word(Severity::Note), "NOTE   ");
    }

    /// The report a person reads: the stated version, the release it is measured from, the
    /// `next` with who decided it, and the commits as evidence — in that order, one write.
    #[test]
    fn the_version_report_names_who_decided_next_and_labels_the_commits_as_evidence() {
        assert_eq!(
            version_text(&report(DecidedBy::Contract), &[]),
            "declared     0.4.0\n\
             tool         0.4.0\n\
             agree        yes\n\
             last release 0.4.0\n\
             commits      0 since it\n\
             next         0.5.0 (decided by the contract)\n\
             bump         patch\n\
             \x20            commits imply patch -> 0.4.1 (evidence; the contract decides)\n"
        );

        let mut both = report(DecidedBy::ContractAndCommits);
        both.next = Some("0.4.1".into());
        let text = version_text(&both, &[]);
        assert!(
            text.contains("next         0.4.1 (decided by the contract and the commits)\n"),
            "{text}"
        );
        assert!(
            text.contains("(evidence; the contract requires no release, so a patch carries them)"),
            "{text}"
        );
        assert!(!text.contains("because"), "{text}");
    }

    /// Nobody decided: `next` is a dash, every reason is its own line, the way out is named,
    /// and the commits are evidence only — never an answer in the contract's place. A
    /// disagreement and the findings are not hidden behind a decision either.
    #[test]
    fn an_undecided_report_says_why_line_by_line_and_the_findings_follow() {
        let mut undecided = report(DecidedBy::Undecided);
        undecided.next = None;
        undecided.commits_imply = None;
        undecided.agree = false;
        undecided.last_release = None;
        undecided.contract_unreadable =
            Some("the tag v0.4.0 moved\nthe record names another commit".into());
        let findings = [Diagnostic {
            id: "projection-stale".into(),
            severity: Severity::Error,
            message: "bin/majordomus states 0.3.0".into(),
        }];
        let text = version_text(&undecided, &findings);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[2], "agree        NO");
        assert_eq!(lines[3], "last release —");
        assert_eq!(
            lines[5],
            "next         — (undecided: the contract could not be measured)"
        );
        assert_eq!(lines[6], "             because the tag v0.4.0 moved");
        assert_eq!(
            lines[7],
            "             because the record names another commit"
        );
        assert!(lines[8].contains("name it deliberately"), "{text}");
        assert_eq!(
            lines[10],
            "             commits imply patch (evidence only; it does not answer in the \
             contract's place)"
        );
        // each finding after a blank line, with the same severity word every rendering uses
        assert_eq!(lines[11], "");
        assert_eq!(
            lines[12],
            format!(
                "{} projection-stale: bin/majordomus states 0.3.0",
                severity_word(Severity::Error)
            )
        );
        assert_eq!(lines.len(), 13, "{text}");
        assert!(text.ends_with('\n'));
    }

    /// The writer's refusal to guess names every reason it was given, a line each, between
    /// what it refuses and how to name a version instead.
    #[test]
    fn the_refusal_to_derive_names_every_reason_and_the_way_out() {
        assert_eq!(
            unmeasured_refusal("nothing is published\nno tag names a release"),
            "release: the public contract cannot be measured here, so there is no bump to \
             derive\n         nothing is published\n         no tag names a release\n         \
             name the version deliberately: `majordomus release bump --level minor` or \
             `--exact <version>`\n"
        );
    }
}
