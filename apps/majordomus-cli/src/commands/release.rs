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
        Some(ReleaseCommand::Debt { release, record }) => debt(&args, release, record),
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

/// Exit code when the debt refuses: a baseline grew, is undeclared or unreadable, or — judged
/// as a release — the total did not fall by the declared minimum.
pub const EXIT_DEBT: u8 = 10;

/// The debt, through the capability so that every surface counts one value.
///
/// Without `--release` this is the gate of a change: it exits [`EXIT_DEBT`] only when a
/// baseline grew or could not be counted. With it, it is the gate of a release, and a total
/// that did not fall by the minimum refuses too. `--record` prints the block a release
/// record carries and judges nothing, so that the writer of a record cannot be stopped by
/// the verdict it is recording. A declaration that cannot be read is [`EXIT_UNREADABLE`].
fn debt(args: &ReleaseArgs, as_release: bool, record: bool) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let value = app
        .context
        .execute("release.debt", serde_json::json!({}))
        .map_err(|e| Error::Refused {
            code: EXIT_UNREADABLE,
            reason: e.to_string(),
        })?;
    let root = std::path::PathBuf::from(&app.context.index.repository.root);
    debt_answer(
        value,
        args.format,
        as_release,
        record,
        Some(&root),
        &mut std::io::stdout().lock(),
    )
}

/// Write the debt report `value` to `out` in the form asked for, and say how to exit.
fn debt_answer(
    value: serde_json::Value,
    format: OutputFormat,
    as_release: bool,
    record: bool,
    root: Option<&std::path::Path>,
    out: &mut dyn Write,
) -> Result<u8> {
    let report: release::debt::DebtReport = typed(value.clone())?;
    if record {
        // The writer of a record is not stopped by a debt that is owed: that is the verdict
        // it records. It is stopped by counts that could not be taken, which it would
        // otherwise write down as numbers.
        if report.standing == release::debt::DebtStanding::Refused {
            return Ok(EXIT_DEBT);
        }
        write!(out, "{}", debt_record(&report)).map_err(Error::Transport)?;
        return Ok(0);
    }
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_default()
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            write!(out, "{}", debt_text(&report, as_release)).map_err(Error::Transport)?;
        }
    }
    let allowed = if as_release {
        report.release_allowed
    } else {
        report.change_allowed
    };
    // A release also asks each gate that can say so whether its baseline is above the truth.
    let (lines, tight) = match root.filter(|_| as_release) {
        Some(root) => debt_slack(root, &report),
        None => (Vec::new(), true),
    };
    if format == OutputFormat::Text {
        for line in &lines {
            writeln!(out, "{line}").map_err(Error::Transport)?;
        }
    }
    Ok(if allowed && tight { 0 } else { EXIT_DEBT })
}

/// Ask every baseline's gate that can state slack whether the baseline holds more than the
/// tree owes, and name the baselines whose gate cannot say.
///
/// The lines to print, and whether every baseline asked is tight. A gate that could not be
/// run is a refusal: "could not be asked" is not "has no slack".
fn debt_slack(root: &std::path::Path, report: &release::debt::DebtReport) -> (Vec<String>, bool) {
    let mut lines = Vec::new();
    let mut tight = true;
    let mut unmeasured = Vec::new();
    for baseline in &report.baselines {
        let Some(slack) = &baseline.slack else {
            unmeasured.push(baseline.id.as_str());
            continue;
        };
        let (code, text) = std::process::Command::new("sh")
            .arg("-c")
            .arg(&slack.command)
            .current_dir(root)
            .output()
            .map(|o| {
                (
                    o.status.code(),
                    String::from_utf8_lossy(&o.stdout).into_owned()
                        + &String::from_utf8_lossy(&o.stderr),
                )
            })
            .unwrap_or((None, String::new()));
        // a gate answers with 0 or 10; any other exit is a gate that could not judge
        if !matches!(code, Some(0 | 10)) {
            tight = false;
            lines.push(format!(
                "FAIL {}: its gate could not be asked whether the baseline is above the truth ({})",
                baseline.path, slack.command
            ));
        } else if text.contains(&slack.says) {
            tight = false;
            lines.push(format!(
                "FAIL {} declares {} and its own gate says it can be tightened ({}): write the baseline before the tag",
                baseline.path, baseline.count, slack.command
            ));
        }
    }
    if !unmeasured.is_empty() {
        lines.push(format!(
            "INFO slack not measured, the gate cannot state it: {}",
            unmeasured.join(", ")
        ));
    }
    (lines, tight)
}

/// A capability's answer as the type its declaration promises. An answer that is not that
/// type is a defect between two parts of one executable, reported as one and never rendered.
fn typed<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Error::Protocol {
        reason: e.to_string(),
    })
}

/// The block a release record carries: the total, and each baseline by its id.
fn debt_record(report: &release::debt::DebtReport) -> String {
    let recorded = report.recorded();
    let mut s = format!(
        "debt:\n  total: {}\n  payable: {}\n",
        recorded.total, report.payable
    );
    if !recorded.unpayable.is_empty() {
        s += "  unpayable:\n";
        for id in &recorded.unpayable {
            s += &format!("    - {id}\n");
        }
    }
    s += "  baselines:\n";
    for (id, count) in &recorded.baselines {
        s += &format!("    {id}: {count}\n");
    }
    s
}

/// The report, for a person: every baseline with what it was, then the verdict.
fn debt_text(report: &release::debt::DebtReport, as_release: bool) -> String {
    let against = report
        .previous_release
        .as_deref()
        .unwrap_or("no previous release");
    let mut s = format!("debt     against {against}\n");
    for b in &report.baselines {
        let was = b
            .previous
            .map_or_else(|| "-".to_string(), |n| n.to_string());
        let frozen = if b.payable { "" } else { "  (cannot be paid)" };
        s += &format!(
            "  {:<24} {:>6}  was {:>6}  {}{frozen}\n",
            b.id, b.count, was, b.path
        );
    }
    let was = report
        .previous_total
        .map_or_else(|| "-".to_string(), |n| n.to_string());
    s += &format!("  {:<24} {:>6}  was {:>6}\n", "total", report.total, was);
    if report.payable != report.total {
        let was = report
            .previous_payable
            .map_or_else(|| "-".to_string(), |n| n.to_string());
        s += &format!(
            "  {:<24} {:>6}  was {:>6}\n",
            "of which payable", report.payable, was
        );
    }
    // each finding is marked for what it does: a refusal of this verdict, or a statement
    for finding in &report.findings {
        let refuses = report.refusing.contains(finding)
            || (as_release && report.release_refusing.contains(finding));
        let word = if refuses { "FAIL" } else { "OK  " };
        s += &format!("{word} {finding}\n");
    }
    if as_release && report.standing == release::debt::DebtStanding::Owed {
        s += "     pay at least the minimum before the tag: remove a baseline entry by fixing what it names\n";
    }
    s
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

    let written = version::write(&root, &to).map_err(|e| Error::io(root.clone(), e))?;
    for f in &written {
        writeln!(out, "         {f} written").map_err(Error::Transport)?;
    }
    // Read both back. A half-applied bump is exactly the failure the one-writer rule exists
    // to prevent, so the writer proves its own work rather than leaving it to the build or
    // the release that finds out at its first step. The projection is not read: it is stale
    // now by design, and deriving it is the next step, not this one.
    let after_manifest = version::declared(&root).unwrap_or_default();
    let after_lock = version::locked(&root);
    let lock_disagrees = after_lock.as_deref().is_some_and(|v| v != to);
    if after_manifest != to || lock_disagrees {
        writeln!(
            out,
            "release: the bump did not take ({} states '{after_manifest}', {} states '{}')",
            version::MANIFEST,
            version::LOCK,
            after_lock.as_deref().unwrap_or("nothing")
        )
        .map_err(Error::Transport)?;
        return Ok(EXIT_DISAGREE);
    }
    writeln!(out, "         {} now declares {to}", version::MANIFEST).map_err(Error::Transport)?;
    writeln!(out, "         {}", derived_after(&to)).map_err(Error::Transport)?;
    Ok(0)
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

    /// The report a person reads: the stated version, the release it is measured from, the
    /// `next` with who decided it, and the commits as evidence — in that order, one write.
    /// Each severity prints as its own word, padded to one width so the messages after it
    /// line up; a note is not printed as a warning nor a warning as an error.
    #[test]
    fn every_severity_prints_as_its_own_word_at_one_width() {
        assert_eq!(severity_word(Severity::Error), "ERROR  ");
        assert_eq!(severity_word(Severity::Warning), "WARNING");
        assert_eq!(severity_word(Severity::Note), "NOTE   ");
    }

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

    fn debt_report(value: serde_json::Value) -> release::debt::DebtReport {
        serde_json::from_value(value).unwrap()
    }

    /// Two baselines, one of them frozen, against a release that carried the same.
    fn owed() -> release::debt::DebtReport {
        debt_report(serde_json::json!({
            "baselines": [
                {"id": "list", "path": ".ai/repo/list-baseline.txt", "gate": "list-check",
                 "form": "entries", "count": 5, "previous": 5, "payable": true, "because": ""},
                {"id": "frozen", "path": ".ai/repo/frozen-baseline.txt", "gate": "frozen-check",
                 "form": "counts", "count": 2, "previous": 2, "payable": false,
                 "because": "published commits in an append-only history"}
            ],
            "total": 7, "payable": 5,
            "previous_release": "v1.0.0", "previous_total": 7, "previous_payable": 5,
            "minimum_reduction": 1, "standing": "owed",
            "change_allowed": true, "release_allowed": false,
            "findings": [
                "the debt is 5 and was 5 at v1.0.0",
                ".ai/repo/frozen-baseline.txt is declared unpayable and holds 2: published commits in an append-only history"
            ],
            "release_refusing": ["the debt is 5 and was 5 at v1.0.0"]
        }))
    }

    #[test]
    fn the_debt_report_shows_every_baseline_with_what_it_was_and_the_verdict_asked_for() {
        let report = owed();

        // as a release: refused, with what to do
        let text = debt_text(&report, true);
        assert!(text.starts_with("debt     against v1.0.0\n"), "{text}");
        assert!(
            text.contains(
                "list                          5  was      5  .ai/repo/list-baseline.txt\n"
            ),
            "{text}"
        );
        assert!(
            text.contains(".ai/repo/frozen-baseline.txt  (cannot be paid)\n"),
            "{text}"
        );
        assert!(
            text.contains("total                         7  was      7\n"),
            "{text}"
        );
        assert!(
            text.contains("of which payable              5  was      5\n"),
            "{text}"
        );
        assert!(
            text.contains("FAIL the debt is 5 and was 5 at v1.0.0\n"),
            "{text}"
        );
        assert!(
            text.contains("pay at least the minimum before the tag")
                // a statement beside a refusal is not marked as one
                && text.contains("OK   .ai/repo/frozen-baseline.txt is declared unpayable and holds 2"),
            "{text}"
        );

        // as a change the same finding is not a refusal, and nothing is asked to be paid
        let text = debt_text(&report, false);
        assert!(
            text.contains("OK   the debt is 5 and was 5 at v1.0.0\n"),
            "{text}"
        );
        assert!(!text.contains("pay at least"), "{text}");
    }

    #[test]
    fn a_first_measurement_prints_no_number_it_was_not_given() {
        let report = debt_report(serde_json::json!({
            "baselines": [
                {"id": "list", "path": ".ai/repo/list-baseline.txt", "gate": "list-check",
                 "form": "entries", "count": 5, "previous": null, "payable": true, "because": ""}
            ],
            "total": 5, "payable": 5,
            "previous_release": null, "previous_total": null, "previous_payable": null,
            "minimum_reduction": 1, "standing": "unrecorded",
            "change_allowed": true, "release_allowed": true,
            "findings": ["recorded now and compared with nothing"]
        }));
        let text = debt_text(&report, true);
        assert!(
            text.starts_with("debt     against no previous release\n"),
            "{text}"
        );
        assert!(
            text.contains("list                          5  was      -"),
            "{text}"
        );
        assert!(
            text.contains("total                         5  was      -\n"),
            "{text}"
        );
        assert!(
            !text.contains("of which payable"),
            "all of it is payable: {text}"
        );
        assert!(
            text.contains("OK   recorded now and compared with nothing\n"),
            "{text}"
        );

        // the block a record carries names no unpayable baseline when there is none
        assert_eq!(
            debt_record(&report),
            "debt:\n  total: 5\n  payable: 5\n  baselines:\n    list: 5\n"
        );
    }

    #[test]
    fn an_answer_of_another_shape_is_a_protocol_error_and_is_not_rendered() {
        match typed::<release::debt::DebtReport>(serde_json::json!({"total": "seven"})) {
            Err(Error::Protocol { reason }) => assert!(!reason.is_empty()),
            other => panic!("a malformed answer was accepted: {}", other.is_ok()),
        }
    }

    #[test]
    fn debt_that_cannot_be_paid_has_no_earlier_number_at_a_first_measurement() {
        let mut report = owed();
        report.previous_payable = None;
        let text = debt_text(&report, false);
        assert!(
            text.contains("of which payable              5  was      -\n"),
            "{text}"
        );
    }

    /// A destination that takes nothing: a closed pipe.
    struct Closed;
    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn an_answer_that_cannot_be_written_or_read_is_an_error_and_never_an_exit_code() {
        let value = serde_json::to_value(owed()).unwrap();
        for (format, record) in [
            (OutputFormat::Text, true),
            (OutputFormat::Text, false),
            (OutputFormat::Json, false),
        ] {
            assert!(
                matches!(
                    debt_answer(value.clone(), format, true, record, None, &mut Closed),
                    Err(Error::Transport(_))
                ),
                "a report that reached nobody is not a verdict"
            );
        }
        assert!(matches!(
            debt_answer(
                serde_json::json!({}),
                OutputFormat::Text,
                true,
                false,
                None,
                &mut Vec::new()
            ),
            Err(Error::Protocol { .. })
        ));

        // written, the verdict is the exit: owed refuses a release and not a change
        let mut out = Vec::new();
        assert_eq!(
            debt_answer(
                value.clone(),
                OutputFormat::Text,
                true,
                false,
                None,
                &mut out
            )
            .unwrap(),
            EXIT_DEBT
        );
        assert!(String::from_utf8(out)
            .unwrap()
            .contains("FAIL the debt is 5"));
        assert_eq!(
            debt_answer(
                value,
                OutputFormat::Json,
                false,
                false,
                None,
                &mut Vec::new()
            )
            .unwrap(),
            0
        );
    }

    /// A report of one paid-down baseline whose gate is asked with `command`, and one whose
    /// gate cannot say.
    fn asked(command: &str) -> serde_json::Value {
        serde_json::json!({
            "baselines": [
                {"id": "list", "path": ".ai/repo/list-baseline.txt", "gate": "list-check",
                 "form": "entries", "count": 4, "previous": 5, "payable": true,
                 "slack": {"command": command, "says": "tighten the baseline"}},
                {"id": "numbers", "path": ".ai/repo/numbers-baseline.txt", "gate": "numbers-check",
                 "form": "counts", "count": 2, "previous": 2, "payable": true}
            ],
            "total": 6, "payable": 6,
            "previous_release": "v1.0.0", "previous_total": 7, "previous_payable": 7,
            "minimum_reduction": 1, "standing": "reduced",
            "change_allowed": true, "release_allowed": true,
            "findings": ["the debt fell from 7 to 6 since v1.0.0, by at least the minimum of 1"]
        })
    }

    #[test]
    fn a_baseline_its_own_gate_calls_tightenable_refuses_a_release_and_not_a_change() {
        let root = tempfile::tempdir().unwrap();
        let run = |command: &str, as_release: bool, format: OutputFormat| {
            let mut out = Vec::new();
            let code = debt_answer(
                asked(command),
                format,
                as_release,
                false,
                Some(root.path()),
                &mut out,
            )
            .unwrap();
            (code, String::from_utf8(out).unwrap())
        };

        // the gate says nothing of slack: the release stands, and the gate that cannot say
        // is named rather than passed over
        let (code, text) = run("echo 'no new debt'", true, OutputFormat::Text);
        assert_eq!(code, 0, "{text}");
        assert!(
            text.contains("INFO slack not measured, the gate cannot state it: numbers\n"),
            "{text}"
        );

        // the gate says the baseline is above the truth, on either stream and at any exit
        for command in [
            "echo 'FIXED scripts/x - tighten the baseline'",
            "echo 'tighten the baseline' >&2; exit 10",
        ] {
            let (code, text) = run(command, true, OutputFormat::Text);
            assert_eq!(code, EXIT_DEBT, "{text}");
            assert!(
                text.contains(&format!(
                    "FAIL .ai/repo/list-baseline.txt declares 4 and its own gate says it can be tightened ({command}): write the baseline before the tag"
                )),
                "{text}"
            );
        }

        // a gate that could not be run is not a gate with no slack
        let (code, text) = run("exit 127", true, OutputFormat::Text);
        assert_eq!(code, EXIT_DEBT);
        assert!(
            text.contains(
                "its gate could not be asked whether the baseline is above the truth (exit 127)"
            ),
            "{text}"
        );

        // a change is not asked, and a JSON answer is not followed by lines that are not JSON
        let (code, text) = run("echo 'tighten the baseline'", false, OutputFormat::Text);
        assert_eq!(code, 0);
        assert!(!text.contains("tightened"), "{text}");
        let (code, text) = run("echo 'tighten the baseline'", true, OutputFormat::Json);
        assert_eq!(code, EXIT_DEBT);
        assert!(
            serde_json::from_str::<serde_json::Value>(&text).is_ok(),
            "{text}"
        );

        // lines that cannot be written are an error, not an exit code
        assert!(matches!(
            debt_answer(
                asked("true"),
                OutputFormat::Text,
                true,
                false,
                Some(root.path()),
                &mut ClosedAfter(1)
            ),
            Err(Error::Transport(_))
        ));
    }

    #[test]
    fn when_every_gate_can_state_slack_nothing_is_named_as_not_measured() {
        let root = tempfile::tempdir().unwrap();
        let mut value = asked("echo 'no new debt'");
        value["baselines"].as_array_mut().unwrap().truncate(1);
        let (lines, tight) = debt_slack(root.path(), &typed(value).unwrap());
        assert!(tight);
        assert!(lines.is_empty(), "{lines:?}");
    }

    /// A destination that takes `n` writes and then closes.
    struct ClosedAfter(usize);
    impl Write for ClosedAfter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            self.0 -= 1;
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn the_debt_command_in_no_repository_is_an_error() {
        let nowhere = tempfile::tempdir().unwrap();
        assert!(debt(&debt_args(nowhere.path(), OutputFormat::Text), false, false).is_err());
    }

    #[test]
    fn a_refusal_of_any_change_is_marked_whichever_verdict_was_asked_for() {
        let mut report = owed();
        let grew = ".ai/repo/list-baseline.txt grew from 5 to 6 since v1.0.0".to_string();
        report.findings.insert(0, grew.clone());
        report.refusing = vec![grew];
        report.standing = release::debt::DebtStanding::Refused;
        let text = debt_text(&report, false);
        assert!(
            text.contains("FAIL .ai/repo/list-baseline.txt grew from 5 to 6"),
            "{text}"
        );
        assert!(text.contains("OK   the debt is 5 and was 5"), "{text}");

        // and counts that could not be taken are never written into a record
        let value = serde_json::to_value(&report).unwrap();
        let mut out = Vec::new();
        assert_eq!(
            debt_answer(value, OutputFormat::Text, true, true, None, &mut out).unwrap(),
            EXIT_DEBT
        );
        assert!(out.is_empty(), "a refused record prints no block");
    }

    /// A gate that says it could not judge: both declared gates exit 12 for that.
    #[test]
    fn a_gate_that_could_not_judge_is_not_a_gate_with_no_slack() {
        let root = tempfile::tempdir().unwrap();
        let command = "echo 'no verdict is available' >&2; exit 12";
        let mut out = Vec::new();
        let code = debt_answer(
            asked(command),
            OutputFormat::Text,
            true,
            false,
            Some(root.path()),
            &mut out,
        )
        .unwrap();
        assert_eq!(code, EXIT_DEBT);
        assert!(String::from_utf8(out).unwrap().contains(&format!(
            "its gate could not be asked whether the baseline is above the truth ({command})"
        )));
    }

    #[test]
    fn the_record_block_names_what_cannot_be_paid() {
        assert_eq!(
            debt_record(&owed()),
            "debt:\n  total: 7\n  payable: 5\n  unpayable:\n    - frozen\n  baselines:\n    frozen: 2\n    list: 5\n"
        );
    }

    fn debt_args(root: &std::path::Path, format: OutputFormat) -> ReleaseArgs {
        ReleaseArgs {
            repo: crate::cli::RepoArgs {
                repo: Some(root.to_path_buf()),
                discovery: crate::cli::DiscoveryMode::Filesystem,
                share: Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
                ..Default::default()
            },
            command: None,
            format,
        }
    }

    #[test]
    fn the_debt_command_exits_by_what_the_tree_carries() {
        let fixture = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = fixture.root();
        let text = debt_args(root, OutputFormat::Text);

        // no declaration: nothing is known to be debt, which is not a tree with none
        match debt(&text, false, false) {
            Err(Error::Refused { code, reason }) => {
                assert_eq!(code, EXIT_UNREADABLE);
                assert!(reason.contains("nothing is known to be debt"), "{reason}");
            }
            other => panic!(
                "an unreadable declaration answered {:?}",
                other.map_err(|e| e.to_string())
            ),
        }

        std::fs::create_dir_all(root.join(".ai/repo/ci")).unwrap();
        std::fs::write(root.join(".ai/repo/list-baseline.txt"), "one\ntwo\n").unwrap();
        std::fs::write(
            root.join(".ai/repo/ci/debt.yaml"),
            "version: 1\nminimum_reduction: 1\nbaselines:\n  - id: list\n    path: .ai/repo/list-baseline.txt\n    gate: list-check\n    form: entries\n",
        )
        .unwrap();
        // a first measurement refuses neither a change nor a release, in either format
        assert_eq!(debt(&text, false, false).unwrap(), 0);
        assert_eq!(debt(&text, true, false).unwrap(), 0);
        assert_eq!(
            debt(&debt_args(root, OutputFormat::Json), true, false).unwrap(),
            0
        );
        assert_eq!(debt(&text, false, true).unwrap(), 0);

        // a baseline the declaration does not count refuses both, and no record is written
        // over a tree that could not be counted
        std::fs::write(root.join(".ai/repo/other-baseline.txt"), "three\n").unwrap();
        assert_eq!(debt(&text, false, false).unwrap(), EXIT_DEBT);
        assert_eq!(debt(&text, true, false).unwrap(), EXIT_DEBT);
        assert_eq!(debt(&text, false, true).unwrap(), EXIT_DEBT);
    }
}
