//! The report contract (`share/brief/report.yaml`, schema `majordomus/brief-report/v1`) and
//! the one function that composes its three texts.
//!
//! # Why the wording is data
//!
//! On 2026-10-29 the owner read the interval prompt and called it a placeholder: it asked for
//! a fixed number of lines and named categories ("a board that could not be read, a superseded
//! server, ...") rather than anything measured, and the same contract was worded four
//! different ways — here, in the Claude Code mod, in docs/BRIEF.md and in ADR 0136. The
//! contract is now declared once, in a file people and adopters read, and this module holds
//! only its compiled copy, a parse of it, the conditions that fill its attention rows, and
//! [`render`]. A carrier relays what [`render`] wrote and states nothing of its own.
//!
//! # How the executable holds it
//!
//! As it holds the design declaration: `majordomus generate` writes the byte-for-byte copy
//! `report.yaml` beside this file behind a GENERATED banner, and this module includes it.
//! The fingerprint is the sha256 of the canonical bytes (the copy with its banner taken
//! off), cut to 12 hex digits; a unit test holds it equal to the share file's.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use super::{
    BriefAttention, BriefCiSection, BriefDeriveSection, BriefIntegrationSection,
    BriefMachineSection, BriefMeshSection, BriefPeersSection, BriefReleaseSection,
    BriefSectionState, BriefSessionSection, BriefVerdict, BriefWorkSection, SessionBrief,
};

/// The compiled copy of `share/brief/report.yaml`, banner included.
const COMPILED: &str = include_str!("report.yaml");

/// Where the canonical declaration lives, repository-relative.
pub const SOURCE: &str = "share/brief/report.yaml";
/// Where `majordomus generate` writes the compiled copy, repository-relative.
pub const COMPILED_PATH: &str = "apps/majordomus-cli/src/capability/builtin/brief/report.yaml";
/// The reader page `majordomus generate` renders from the declaration.
pub const READER_PAGE: &str = "docs/generated/brief-report.md";

/// One section of the report, in the order it is written.
#[derive(Debug, Clone, Deserialize)]
pub struct ReportSection {
    /// Its key, the same in every language.
    pub key: String,
    /// Its heading, in English; the report translates it.
    pub heading: String,
    /// What it holds.
    pub holds: String,
    /// What each of its lines carries as proof.
    #[serde(default)]
    pub evidence: String,
    /// How long it may be.
    pub cap: String,
    /// What an empty section is written as, when it may be empty.
    #[serde(default)]
    pub empty: Option<String>,
    /// The brief's sections it is written from: when none of them changed since the previous
    /// report, the interval prompt lists it as unchanged. Empty for the header, which always
    /// changes.
    #[serde(default)]
    pub from: Vec<String>,
}

/// One rule every line of the report obeys.
#[derive(Debug, Clone, Deserialize)]
pub struct ReportRule {
    /// Its id.
    pub id: String,
    /// Its text.
    pub text: String,
}

/// One attention row: the wording a condition of [`derive_attention`] fills.
#[derive(Debug, Clone, Deserialize)]
pub struct AttentionRow {
    /// Its id, the same as the condition's.
    pub id: String,
    /// The section of the brief it is about; `{section}` when the condition names it.
    pub section: String,
    /// The key of the report section the entry is written under; `{report_section}` when
    /// the condition names it.
    pub report_section: String,
    /// A status word filed in `share/design/tokens.yaml`.
    pub word: String,
    /// `session`, `person` or `seat:<name>`.
    pub actor: String,
    /// The fact, with `{name}` for each value the condition measured.
    pub fact: String,
    /// The command that refreshes or acts on it, with `{name}` likewise.
    pub remedy: String,
}

/// One fact the brief cannot hold current, and the command that is.
#[derive(Debug, Clone, Deserialize)]
pub struct LiveRead {
    /// The fact.
    pub fact: String,
    /// The command.
    pub command: String,
}

/// The parsed declaration.
#[derive(Debug, Clone, Deserialize)]
pub struct ReportContract {
    /// `majordomus/brief-report/v1`.
    pub schema: String,
    /// `brief-report`.
    pub id: String,
    /// Its version.
    pub version: u32,
    /// The first words of every interval prompt.
    pub marker: String,
    /// One sentence on what it is.
    pub about: String,
    /// The sentence that says the sections are written in order.
    pub order_note: String,
    /// What a session is told at its start, one sentence each, with `{n}` and `{cite}`.
    pub start: Vec<String>,
    /// The sections, in order.
    pub sections: Vec<ReportSection>,
    /// The rules.
    pub rules: Vec<ReportRule>,
    /// The attention rows.
    pub attention: Vec<AttentionRow>,
    /// The live reads.
    pub live_reads: Vec<LiveRead>,
}

impl ReportContract {
    /// The row of an attention id.
    pub fn row(&self, id: &str) -> Option<&AttentionRow> {
        self.attention.iter().find(|r| r.id == id)
    }
}

/// The canonical bytes: the compiled copy without the banner `majordomus generate` put on it.
pub fn canonical() -> &'static str {
    strip_banner(COMPILED)
}

/// `text` without a leading GENERATED banner: the `# GENERATED`, `# Source:` and
/// `# Generator:` lines at its head.
pub(crate) fn strip_banner(text: &str) -> &str {
    let mut rest = text;
    for prefix in ["# GENERATED FILE", "# Source:", "# Generator:"] {
        if rest.starts_with(prefix) {
            rest = rest.split_once('\n').map(|(_, r)| r).unwrap_or("");
        }
    }
    rest
}

/// The contract, parsed once. A copy that does not parse is a build defect a unit test
/// catches before it ships, so this panics rather than composing a report from nothing.
pub fn contract() -> &'static ReportContract {
    static PARSED: OnceLock<ReportContract> = OnceLock::new();
    PARSED.get_or_init(|| {
        crate::metadata::yaml::parse_into(canonical())
            .unwrap_or_else(|e| panic!("{COMPILED_PATH} does not parse: {e}"))
    })
}

/// The contract's name as every text cites it: `brief-report/v1`.
pub fn contract_name() -> String {
    let c = contract();
    format!("{}/v{}", c.id, c.version)
}

/// The sha256 of the canonical bytes, 12 hex digits.
pub fn fingerprint() -> &'static str {
    static PRINT: OnceLock<String> = OnceLock::new();
    PRINT.get_or_init(|| fingerprint_of(canonical()))
}

/// The fingerprint of any text, as [`fingerprint`] computes it.
pub(crate) fn fingerprint_of(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// Which text [`render`] writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    /// `keep_awake`: what a session is told at its start.
    Start,
    /// `report.prompt`: what wakes a session on the interval.
    Interval,
    /// `report.reminder`: what rides on a prompt the person sends.
    Reminder,
}

/// The one composition of the contract's texts. Every text names the contract and its
/// fingerprint, carries the attention entries verbatim and once, and states no line count.
pub fn render(use_: Use, brief: &SessionBrief) -> String {
    let c = contract();
    let cite = format!("{} #{}", contract_name(), fingerprint());
    let at = when(brief);
    match use_ {
        Use::Reminder => {
            let mut s = format!("Majordomus brief {at} ({cite})\n\n");
            s.push_str(&attention_block(&brief.attention));
            s.push_str("\n\n");
            s.push_str(&fence(section_lines(&brief.lines)));
            s
        }
        Use::Interval => {
            let since = brief.report.since.as_deref();
            let mut s = format!("{}: the brief composed at {at} under {cite}.\n", c.marker);
            s.push_str(&changed_words(c, brief));
            s.push_str("\n\n");
            // the brief's lines without their attention block, which follows once, placed
            s.push_str(&fence(section_lines(&brief.lines)));
            s.push_str("\n\n");
            s.push_str(&placed_attention(c, &brief.attention, since));
            s.push_str("\n\n");
            s.push_str(&contract_block(c, since, repository_of(brief)));
            s
        }
        Use::Start => {
            let n = brief.interval_minutes.to_string();
            let mut s = c
                .start
                .iter()
                .map(|t| t.replace("{n}", &n).replace("{cite}", &cite))
                .collect::<Vec<_>>()
                .join(" ");
            s.push_str("\n\n");
            s.push_str(&placed_attention(c, &brief.attention, None));
            s.push_str("\n\n");
            s.push_str(&contract_block(c, None, repository_of(brief)));
            s
        }
    }
}

/// When the brief was composed: UTC, and the composing host's own clock when it was read.
fn when(brief: &SessionBrief) -> String {
    if brief.at_local.is_empty() {
        brief.at.clone()
    } else {
        format!("{} ({})", brief.at, brief.at_local)
    }
}

/// The brief's lines from the first section on: the attention block that leads them is
/// carried by the prompt once, placed under the report sections, and never a second time
/// inside the fence.
pub fn section_lines(lines: &[String]) -> &[String] {
    let first = lines
        .iter()
        .position(|l| !l.starts_with("attention") && !l.starts_with(' '))
        .unwrap_or(lines.len());
    &lines[first..]
}

/// What changed since the previous report, by the brief's own section fingerprints, and the
/// report sections none of whose sources moved: the caller passes the fingerprints of the
/// brief it was last told (`previous`), and [`super::changed_sections`] compares them.
fn changed_words(c: &ReportContract, brief: &SessionBrief) -> String {
    let since = brief
        .report
        .since
        .as_deref()
        .unwrap_or("the previous report");
    let Some(changed) = &brief.report.changed else {
        return format!(
            "Changed since {since}: not compared (the caller passed no fingerprints of the brief it was last told), so every section is written in full."
        );
    };
    let mut s = if changed.is_empty() {
        format!("Changed since {since}: no section of the brief.")
    } else {
        format!("Changed since {since}: {}.", changed.join(", "))
    };
    let placed: std::collections::BTreeSet<&str> = brief
        .attention
        .iter()
        .map(|a| a.report_section.as_str())
        .collect();
    let unchanged: Vec<&str> = c
        .sections
        .iter()
        .filter(|sec| !sec.from.is_empty())
        .filter(|sec| !sec.from.iter().any(|f| changed.contains(f)))
        .filter(|sec| !placed.contains(sec.key.as_str()))
        .map(|sec| sec.key.as_str())
        .collect();
    if !unchanged.is_empty() {
        s.push_str(&format!(
            " Unchanged since {since}, so written as the heading and [unchanged]: {}.",
            unchanged.join(", ")
        ));
    }
    s
}

/// The brief's lines in a fence.
fn fence(lines: &[String]) -> String {
    format!("```\n{}\n```", lines.join("\n"))
}

/// The attention entries, one per line as `[word] section: fact — remedy (actor)`, or the
/// one sentence that says none was measured.
pub fn attention_block(entries: &[BriefAttention]) -> String {
    if entries.is_empty() {
        return "Attention: none measured.".into();
    }
    entries
        .iter()
        .map(attention_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// The attention entries under the report section each is written in, in the contract's
/// order: where an entry goes is the contract's (`report_section`), never the writer's.
pub fn placed_attention(
    c: &ReportContract,
    entries: &[BriefAttention],
    since: Option<&str>,
) -> String {
    if entries.is_empty() {
        return "Attention: none measured.".into();
    }
    let mut s = String::from("Attention measured, by report section:");
    let mut written = 0;
    for sec in &c.sections {
        let here: Vec<&BriefAttention> = entries
            .iter()
            .filter(|a| a.report_section == sec.key)
            .collect();
        if here.is_empty() {
            continue;
        }
        s.push_str(&format!("\n- {} ({}):", heading_of(sec, since), sec.key));
        for a in here {
            s.push_str(&format!("\n  {}", attention_line(a)));
            written += 1;
        }
    }
    // an entry whose report section the contract does not name (an older server's) is still
    // carried, never dropped
    let stray: Vec<&BriefAttention> = entries
        .iter()
        .filter(|a| !c.sections.iter().any(|sec| sec.key == a.report_section))
        .collect();
    if !stray.is_empty() {
        s.push_str("\n- placed by no report section:");
        for a in stray {
            s.push_str(&format!("\n  {}", attention_line(a)));
            written += 1;
        }
    }
    debug_assert_eq!(written, entries.len());
    s
}

/// A section's heading with its slot filled: the previous report's instant, or words for it.
pub fn heading_of(sec: &ReportSection, since: Option<&str>) -> String {
    sec.heading
        .replace("{since}", since.unwrap_or("the previous report"))
}

/// One entry as every text writes it.
pub fn attention_line(a: &BriefAttention) -> String {
    format!(
        "[{}] {}: {} — {} ({})",
        a.word, a.section, a.fact, a.remedy, a.actor
    )
}

/// The repository a live read names: the slug the CI section read, when it read one, in place
/// of `{owner}/{repo}` — which `gh api` itself fills from the checkout, so a command the brief
/// could not complete is still one a person can run as written.
fn live_command(command: &str, repository: Option<&str>) -> String {
    match repository.filter(|r| r.split('/').count() == 2 && !r.contains(['{', '}', ' '])) {
        Some(r) => command.replace("{owner}/{repo}", r),
        None => command.to_string(),
    }
}

/// The slug a brief's CI section read, when it read one.
fn repository_of(brief: &SessionBrief) -> Option<&str> {
    Some(brief.ci.repository.as_str()).filter(|r| !r.is_empty())
}

/// The sections with their headings, holds, evidence and caps; the rules; the live reads.
fn contract_block(c: &ReportContract, since: Option<&str>, repository: Option<&str>) -> String {
    let mut s = format!("Write the report. {}\n", c.order_note);
    for (i, sec) in c.sections.iter().enumerate() {
        let heading = heading_of(sec, since);
        s.push_str(&format!(
            "{}. {} ({}) — {}. Evidence: {}. Cap: {}.",
            i + 1,
            heading,
            sec.key,
            sec.holds,
            sec.evidence,
            sec.cap
        ));
        if let Some(e) = &sec.empty {
            s.push_str(&format!(" Empty: {e}."));
        }
        s.push('\n');
    }
    s.push_str("\nRules:\n");
    for r in &c.rules {
        s.push_str(&format!("- {}: {}\n", r.id, r.text));
    }
    s.push_str("\nLive reads, where the brief is not current:\n");
    for l in &c.live_reads {
        s.push_str(&format!(
            "- {}: `{}`\n",
            l.fact,
            live_command(&l.command, repository)
        ));
    }
    s.trim_end().to_string()
}

// ---------------------------------------------------------------- attention

/// The sections [`derive_attention`] reads. Borrowed, so that the conditions see exactly
/// the typed fields the brief carries and never a summary string.
pub struct Measured<'a> {
    /// This session.
    pub session: &'a BriefSessionSection,
    /// Its own work.
    pub work: &'a BriefWorkSection,
    /// The boards.
    pub peers: &'a BriefPeersSection,
    /// The mesh.
    pub mesh: &'a BriefMeshSection,
    /// The integration queue.
    pub integration: &'a BriefIntegrationSection,
    /// The CI pool.
    pub ci: &'a BriefCiSection,
    /// The machine.
    pub machine: &'a BriefMachineSection,
    /// The release.
    pub release: &'a BriefReleaseSection,
    /// The derive queue.
    pub derive: &'a BriefDeriveSection,
    /// The reporting interval, in seconds: a fetch younger than it is not news.
    pub interval_seconds: u64,
}

/// Every attention id a condition below can emit: the contract's rows are this set, which
/// `every_attention_row_has_a_condition_and_every_condition_a_row` holds.
#[cfg(test)]
pub const CONDITIONS: &[&str] = &[
    "integration.stale",
    "integration.unfetched",
    "master.failing",
    "run.failing",
    "pr.dirty",
    "pr.blocked",
    "branch.no_pr",
    "ci.queued_idle",
    "ci.no_runner",
    "ci.unread_scope",
    "machine.below_floor",
    "machine.swap",
    "machine.builders_full",
    "peers.unread",
    "peers.superseded",
    "mesh.review",
    "mesh.handover",
    "mesh.overlap",
    "mesh.failing",
    "release.owed",
    "release.overdue",
    "derive.dead_holder",
    "session.no_task",
    "section.over_budget",
];

/// The rank an entry sorts by, by its consequence rather than its tone: what fails, blocks or
/// conflicts first; then what is stale, degraded, owed or not read; then what waits; and last
/// the brief's own cost, a section that ran past its budget, which says nothing about the
/// work. On 2026-10-30 "[unread] release: release took > 3000 ms" led the attention block
/// above "[stale] integration: queue observed 36 h ago ... (+93)", because `unread` and
/// `owed` were ranked with `failing`.
fn severity(a: &BriefAttention) -> u8 {
    if a.id == "section.over_budget" {
        return 4;
    }
    match a.word.as_str() {
        "failing" | "blocked" | "superseded" | "overdue" | "dirty" => 0,
        "stale" | "degraded" | "warn" | "partial" | "owed" | "unread" => 1,
        "queued" | "pending" | "unmerged" => 2,
        _ => 3,
    }
}

/// The report section an entry about a brief section is written under, for the rows whose
/// condition names the section (`section.over_budget`).
pub(crate) fn report_section_of(section: &str) -> &'static str {
    match section {
        "session" | "work" => "in_flight",
        "peers" | "mesh" => "mesh",
        "integration" | "release" => "integration",
        _ => "machine_ci",
    }
}

/// The command that reads one section of the brief on its own: what a section past its
/// budget names, rather than the brief that just ran out of time on it.
pub(crate) fn direct_read(section: &str) -> &'static str {
    match section {
        "session" => "majordomus lifecycle episodes",
        "work" => "gh pr list --author @me",
        "peers" => "majordomus peers",
        "mesh" => "majordomus mesh state",
        "integration" => "majordomus prs brief",
        "ci" => "gh run list --limit 20",
        "machine" => "df -h /",
        "release" => "majordomus release next",
        _ => "majordomus doctor",
    }
}

/// One entry from its row and the values its condition measured.
fn entry(id: &str, values: &[(&str, String)]) -> Option<BriefAttention> {
    let row = contract().row(id)?;
    let fill = |template: &str| {
        let mut s = template.to_string();
        for (k, v) in values {
            s = s.replace(&format!("{{{k}}}"), v);
        }
        s
    };
    Some(BriefAttention {
        id: id.to_string(),
        section: fill(&row.section),
        report_section: fill(&row.report_section),
        word: row.word.clone(),
        fact: fill(&row.fact),
        remedy: fill(&row.remedy),
        actor: row.actor.clone(),
    })
}

/// What a template value says when the fact behind it was not read: the status word, in
/// brackets, as every other state of the report is written. On 2026-10-30 a brief at a load
/// average of 90 said "origin/master is 51b38035a7 (+?)": the commit count had not answered
/// inside its bound, and a question mark is a placeholder, not a fact.
pub(crate) const NOT_READ: &str = "[unread]";

/// How far origin/master moved past the observation: `+3 commits`, or that the count was not
/// read.
pub(crate) fn moved_words(moved: Option<u64>) -> String {
    match moved {
        Some(1) => "+1 commit".into(),
        Some(n) => format!("+{n} commits"),
        None => format!("moved, count {NOT_READ}"),
    }
}

/// A span of seconds as the report says it: `45 s`, `17 min`, `17 h`, `6 d`.
pub(crate) fn span(seconds: u64) -> String {
    match seconds {
        s @ 0..=119 => format!("{s} s"),
        s @ 120..=7199 => format!("{} min", s / 60),
        s @ 7200..=172_799 => format!("{} h", s / 3600),
        s => format!("{} d", s / 86_400),
    }
}

fn short(sha: &str) -> String {
    sha.chars().take(10).collect()
}

/// Bytes as GiB with one decimal, cut rather than rounded, so that free space is never
/// said to be more than was measured against a floor: the one wording of a disk or swap
/// figure in the attention entries, the machine line and the one line (2026-10-30: 19.6 GiB
/// was "19.6" in one, "20" in another and "19" in the third).
pub(crate) fn gib(bytes: u64) -> String {
    let tenths = bytes.saturating_mul(10) / (1u64 << 30);
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// Every condition the sections measured, as the contract words it: those a person must act
/// on first, then worst first. Read from typed fields only.
pub fn derive_attention(m: &Measured<'_>) -> Vec<BriefAttention> {
    let mut out: Vec<BriefAttention> = Vec::new();
    let mut push = |id: &str, values: &[(&str, String)]| {
        if let Some(e) = entry(id, values) {
            out.push(e);
        }
    };

    // --- every section that ran past its budget
    for (name, obs) in [
        ("session", &m.session.observed),
        ("work", &m.work.observed),
        ("peers", &m.peers.observed),
        ("mesh", &m.mesh.observed),
        ("integration", &m.integration.observed),
        ("ci", &m.ci.observed),
        ("machine", &m.machine.observed),
        ("release", &m.release.observed),
        ("derive", &m.derive.observed),
    ] {
        if obs.verdict == BriefVerdict::Unread
            && obs.budget_ms > 0
            && obs.elapsed_ms > obs.budget_ms
        {
            push(
                "section.over_budget",
                &[
                    ("section", name.into()),
                    ("report_section", report_section_of(name).into()),
                    ("budget", super::millis(obs.budget_ms)),
                    (
                        "remedy",
                        obs.remedy
                            .clone()
                            .filter(|r| !r.contains("majordomus brief"))
                            .unwrap_or_else(|| direct_read(name).into()),
                    ),
                ],
            );
        }
    }

    // --- integration
    let i = m.integration;
    if i.stale {
        push(
            "integration.stale",
            &[
                (
                    "age",
                    i.age_seconds.map(span).unwrap_or_else(|| NOT_READ.into()),
                ),
                (
                    "observed",
                    i.observed_master
                        .as_deref()
                        .map(short)
                        .unwrap_or_else(|| NOT_READ.into()),
                ),
                (
                    "now",
                    i.master_now
                        .as_deref()
                        .map(short)
                        .unwrap_or_else(|| NOT_READ.into()),
                ),
                ("moved", moved_words(i.moved_by)),
            ],
        );
    }
    // one cause, one entry: a stale queue's remedy already fetches, and a fetch younger than
    // the reporting interval is not news (an entry that comes and goes by the quarter hour
    // moved the fingerprint and woke the session for nothing)
    if let Some(age) = i
        .master_fetched_age_seconds
        .filter(|a| !i.stale && *a > m.interval_seconds.max(super::FETCH_STALE_AFTER.as_secs()))
    {
        push("integration.unfetched", &[("age", span(age))]);
    }
    if let Some(run) = i.master_ci.as_ref().filter(|r| r.is_failing()) {
        push(
            "master.failing",
            &[
                (
                    "sha",
                    i.master_now.as_deref().map(short).unwrap_or_default(),
                ),
                ("workflow", run.workflow.clone()),
                ("run", run.id.to_string()),
                (
                    "conclusion",
                    run.conclusion.clone().unwrap_or_else(|| run.status.clone()),
                ),
            ],
        );
    }

    // --- this session's own work
    for item in &m.work.items {
        if let Some(run) = item.run.as_ref().filter(|r| r.is_failing()) {
            push(
                "run.failing",
                &[
                    (
                        "branch",
                        item.train.clone().unwrap_or_else(|| item.branch.clone()),
                    ),
                    ("run", run.id.to_string()),
                    (
                        "done",
                        run.jobs_done
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| NOT_READ.into()),
                    ),
                    (
                        "total",
                        run.jobs_total
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| NOT_READ.into()),
                    ),
                    (
                        "jobs",
                        if run.failed_jobs.is_empty() {
                            "jobs not read".into()
                        } else {
                            run.failed_jobs.join(", ")
                        },
                    ),
                ],
            );
        }
        match (item.pr, item.merge_state.as_deref()) {
            (Some(pr), Some("DIRTY")) => push(
                "pr.dirty",
                &[
                    ("pr", pr.to_string()),
                    (
                        "worktree",
                        // a branch with no canonical worktree here is named by the command that
                        // prints where it belongs, never by a `<worktree>` to fill in
                        item.worktree
                            .as_deref()
                            .map(super::home_words)
                            .unwrap_or_else(|| {
                                format!("\"$(majordomus worktree path {})\"", item.branch)
                            }),
                    ),
                ],
            ),
            (Some(pr), Some("BLOCKED")) => push("pr.blocked", &[("pr", pr.to_string())]),
            // "no PR" is a fact only when the pull requests were read: a composition that is
            // not the leaseholder never asked the forge (2026-10-30, a linked worktree whose
            // server was absent said "pushed, no PR" of a branch it had not looked up)
            (None, _) if item.pushed && m.work.pull_requests_read && item.word == "unmerged" => {
                push(
                    "branch.no_pr",
                    &[
                        ("branch", item.branch.clone()),
                        ("head", item.head.as_deref().map(short).unwrap_or_default()),
                    ],
                )
            }
            _ => {}
        }
    }

    // --- the CI pool
    let ci = m.ci;
    if let Some(reason) = &ci.runners_unread {
        push("ci.unread_scope", &[("reason", reason.clone())]);
    }
    if let Some(queued) = ci.queued.filter(|q| *q > 0) {
        if ci.runners_unread.is_none() && !ci.runners.is_empty() {
            if ci.online == 0 {
                push(
                    "ci.no_runner",
                    &[
                        ("queued", queued.to_string()),
                        ("repo", ci.repository.clone()),
                    ],
                );
            } else if ci.busy < ci.online {
                push(
                    "ci.queued_idle",
                    &[
                        ("queued", queued.to_string()),
                        ("online", ci.online.to_string()),
                        ("busy", ci.busy.to_string()),
                        ("repo", ci.repository.clone()),
                    ],
                );
            }
        }
    }

    // --- the machine
    let mc = m.machine;
    if let (Some(free), Some(floor)) = (mc.free_bytes, mc.floor_gib) {
        if free < floor.saturating_mul(1 << 30) {
            push(
                "machine.below_floor",
                &[("free", gib(free)), ("floor", floor.to_string())],
            );
        }
    }
    if let (Some(used), Some(total)) = (mc.swap_used_bytes, mc.swap_total_bytes) {
        if let Some(pct) = used.saturating_mul(100).checked_div(total) {
            if pct > super::SWAP_DEGRADED_PERCENT {
                push(
                    "machine.swap",
                    &[
                        ("used", gib(used)),
                        ("total", gib(total)),
                        ("pct", pct.to_string()),
                    ],
                );
            }
        }
    }
    if let (Some(n), Some(max)) = (mc.builders, mc.max_builders) {
        if max > 0 && n >= max {
            push(
                "machine.builders_full",
                &[("n", n.to_string()), ("max", max.to_string())],
            );
        }
    }

    // --- the boards
    if !m.peers.unread.is_empty() {
        push("peers.unread", &[("boards", m.peers.unread.join("; "))]);
    }
    if let Some(leaseholder) = &m.peers.leaseholder {
        push("peers.superseded", &[("leaseholder", leaseholder.clone())]);
    }

    // --- the mesh
    let mesh = m.mesh;
    if mesh.state == BriefSectionState::Read {
        if mesh.reviews_for_you > 0 {
            push("mesh.review", &[("n", mesh.reviews_for_you.to_string())]);
        }
        if mesh.handovers_waiting > 0 {
            push(
                "mesh.handover",
                &[("n", mesh.handovers_waiting.to_string())],
            );
        }
        if mesh.overlaps_for_you > 0 {
            push("mesh.overlap", &[("n", mesh.overlaps_for_you.to_string())]);
        }
        if mesh.failing > 0 {
            push("mesh.failing", &[("n", mesh.failing.to_string())]);
        }
    }

    // --- the release
    use crate::release::currency::Stance;
    match (m.release.state, m.release.stance) {
        (BriefSectionState::Read, Some(Stance::Owed)) => {
            push("release.owed", &[("landed", m.release.landed.to_string())])
        }
        (BriefSectionState::Read, Some(Stance::Overdue)) => push(
            "release.overdue",
            &[("landed", m.release.landed.to_string())],
        ),
        _ => {}
    }

    // --- the derive lock
    if let (Some(pid), Some(false)) = (m.derive.pid, m.derive.alive) {
        push(
            "derive.dead_holder",
            &[
                ("pid", pid.to_string()),
                (
                    "held",
                    m.derive
                        .held_seconds
                        .map(span)
                        .unwrap_or_else(|| NOT_READ.into()),
                ),
            ],
        );
    }

    // --- the session
    // no task anywhere this session works: not in this checkout (whose record may be another
    // session's), and not in the canonical worktree of any of its own branches
    let task_in_a_worktree = m.work.items.iter().any(|i| i.task.is_some());
    if m.session.task_id.is_none()
        && m.session.state != BriefSectionState::Unread
        && !task_in_a_worktree
        && !m.session.task_record_foreign
    {
        push(
            "session.no_task",
            &[
                ("remedy", no_task_remedy(m)),
                // the session line's words for the same record: an outcome it does not state is
                // `unknown` (a filed status word), never `[none]`, which the session line two
                // rows below contradicted on 2026-10-30
                (
                    "task",
                    m.session
                        .last_task
                        .clone()
                        .unwrap_or_else(|| "none recorded".into()),
                ),
                (
                    "outcome",
                    m.session
                        .task_outcome
                        .clone()
                        .filter(|o| !o.is_empty())
                        .unwrap_or_else(|| "unknown".into()),
                ),
            ],
        );
    }

    crate::order::canonical(&mut out);
    out
}

/// What a session with no task runs: `majordomus start` with the issue and scope of its own
/// announced claim when one names an issue, which runs as written; else the command that
/// names the issue to take. Never a template with `<task>` and `<paths>` to fill in, which
/// a shell reads as redirections.
fn no_task_remedy(m: &Measured<'_>) -> String {
    let claim = m
        .work
        .claims
        .iter()
        // the claim of a branch: its scope names the work's paths, not a machine or a seat
        .find(|c| {
            c.issue.is_some()
                && c.scope.iter().any(|p| p.starts_with("refs/heads/"))
                && c.scope.iter().any(|p| !p.starts_with("refs/"))
        });
    match claim {
        Some(c) => {
            let issue = c.issue.as_deref().unwrap_or_default();
            let intent = clip_words(c.intent.as_deref().unwrap_or(issue), 60);
            let title = if intent.starts_with(issue) {
                intent
            } else {
                format!("{issue}: {intent}")
            };
            let scope: Vec<String> = c
                .scope
                .iter()
                .filter(|p| !p.starts_with("refs/"))
                .map(|p| shell_word(p))
                .collect();
            format!(
                "majordomus start {} --scope {}",
                shell_word(&title),
                scope.join(" ")
            )
        }
        None => "majordomus plan next".into(),
    }
}

/// A word as a POSIX shell reads it back: bare when it is plain, else single-quoted.
fn shell_word(w: &str) -> String {
    let plain = !w.is_empty()
        && w.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:@%+=,".contains(c));
    if plain {
        w.to_string()
    } else {
        format!("'{}'", w.replace('\'', "'\\''"))
    }
}

/// At most `n` characters of a text, cut at a word.
fn clip_words(s: &str, n: usize) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() <= n {
        return s;
    }
    let cut: String = s.chars().take(n).collect();
    match cut.rfind(' ') {
        Some(i) if i > n / 2 => cut[..i].to_string(),
        _ => cut,
    }
}

/// The canonical order of attention entries: those a person must act on first, then by
/// consequence ([`severity`]), then by id.
impl crate::order::Ordered for BriefAttention {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let rank = i64::from(u8::from(self.actor != "person")) * 10 + i64::from(severity(self));
        crate::order::OrderKey::plain(&self.id, &self.id).ranked(rank)
    }
}

/// The attention ids by section, for the mod's per-section fingerprint and the Cockpit.
pub fn by_section(entries: &[BriefAttention]) -> BTreeMap<String, Vec<&BriefAttention>> {
    let mut map: BTreeMap<String, Vec<&BriefAttention>> = BTreeMap::new();
    for e in entries {
        map.entry(e.section.clone()).or_default().push(e);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn a_live_read_names_the_repository_or_lets_gh_fill_it() {
        // the contract once said `repos/<owner>/<repo>`: a placeholder a person had to fill in
        let runners = contract()
            .live_reads
            .iter()
            .find(|l| l.fact == "the CI pool")
            .expect("the CI pool's live read");
        // the command is the contract's alone (case 4401): here only its slot is pinned
        assert!(
            runners.command.contains("repos/{owner}/{repo}/"),
            "{}",
            runners.command
        );
        assert!(
            live_command(&runners.command, Some("korczis/prismatic-majordomus"))
                .contains("repos/korczis/prismatic-majordomus/"),
        );
        // unread, or the fallback a failed read leaves: gh fills `{owner}/{repo}` itself
        for unknown in [None, Some(""), Some("{owner}/{repo}"), Some("not a slug")] {
            assert_eq!(
                live_command(&runners.command, unknown),
                runners.command,
                "{unknown:?}"
            );
        }
        for l in &contract().live_reads {
            assert!(
                !l.command.contains("<owner>") && !l.command.contains("<repo>"),
                "{}",
                l.command
            );
        }
    }

    #[test]
    fn the_compiled_contract_is_the_share_file() {
        let share = std::fs::read_to_string(repo().join(SOURCE)).expect("the share file");
        assert_eq!(
            fingerprint(),
            fingerprint_of(&share),
            "{COMPILED_PATH} is not {SOURCE}: run majordomus generate"
        );
        assert_eq!(canonical(), share);
        assert!(
            COMPILED.starts_with("# GENERATED FILE"),
            "the compiled copy carries its banner"
        );
    }

    #[test]
    fn the_contract_parses_and_its_sections_are_in_order() {
        let c = contract();
        assert_eq!(c.schema, "majordomus/brief-report/v1");
        let keys: Vec<&str> = c.sections.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            keys,
            [
                "header",
                "blocked",
                "delivered",
                "in_flight",
                "integration",
                "mesh",
                "machine_ci",
                "next"
            ]
        );
        assert!(!c.marker.is_empty() && !c.rules.is_empty() && !c.live_reads.is_empty());
    }

    #[test]
    fn the_contract_is_valid_against_its_schema() {
        let schema_path =
            repo().join("share/schemas/majordomus/brief-report/brief-report.v1.schema.json");
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&schema_path).expect("the schema"))
                .expect("JSON");
        let doc = serde_json::Value::Object(
            crate::metadata::yaml::parse_mapping(canonical()).expect("the subset parses it"),
        );
        let validator = jsonschema::validator_for(&schema).expect("a valid schema");
        let errors: Vec<String> = validator.iter_errors(&doc).map(|e| e.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?}");
        // and a row whose actor is nobody's is refused
        let mut bad = doc.clone();
        bad["attention"][0]["actor"] = serde_json::json!("somebody");
        assert!(!validator.is_valid(&bad));
    }

    #[test]
    fn every_row_is_placed_and_every_remedy_runs_as_written() {
        let c = contract();
        let keys: Vec<&str> = c.sections.iter().map(|s| s.key.as_str()).collect();
        for r in &c.attention {
            assert!(
                r.report_section == "{report_section}" || keys.contains(&r.report_section.as_str()),
                "{}: report section {} is no section of the report",
                r.id,
                r.report_section
            );
            // a person's entry is reported under "Blocked on you"
            if r.actor == "person" {
                assert_eq!(r.report_section, "blocked", "{}", r.id);
            }
            // no template for a person to fill in, and no question mark for a fact
            for text in [&r.remedy, &r.fact] {
                assert!(!text.contains('<'), "{}: {text}", r.id);
                assert!(!text.split(' ').any(|w| w == "?"), "{}: {text}", r.id);
            }
            // the brief that just ran out of time is no remedy
            assert!(
                !r.remedy.contains("majordomus brief"),
                "{}: {}",
                r.id,
                r.remedy
            );
        }
        for sec in &c.sections {
            for f in &sec.from {
                assert!(
                    super::super::BUDGETS.iter().any(|(n, _)| *n == f.as_str()),
                    "{} is written from {f}, which is no section of the brief",
                    sec.key
                );
            }
        }
        for name in [
            "session",
            "work",
            "peers",
            "mesh",
            "integration",
            "ci",
            "machine",
            "release",
            "derive",
        ] {
            assert!(keys.contains(&report_section_of(name)), "{name}");
            assert!(!direct_read(name).contains("majordomus brief"), "{name}");
        }
    }

    #[test]
    fn the_contract_speaks_one_vocabulary() {
        let c = contract();
        let tokens = std::fs::read_to_string(repo().join("share/design/tokens.yaml"))
            .expect("the design declaration");
        let filed: Vec<String> = tokens
            .lines()
            .skip_while(|l| !l.starts_with("  states:"))
            .skip(1)
            .take_while(|l| l.starts_with("    "))
            .flat_map(|l| {
                l.split_once('[')
                    .and_then(|(_, r)| r.split_once(']'))
                    .map(|(w, _)| {
                        w.split(',')
                            .map(|x| x.trim().to_string())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();
        let rule = &c
            .rules
            .iter()
            .find(|r| r.id == "status-words")
            .expect("the status-words rule")
            .text;
        let banned = rule
            .split_once("No other verdict adjective (")
            .and_then(|(_, r)| r.split_once(')'))
            .map(|(b, _)| b)
            .expect("the rule names what it bans");
        for w in banned.split(',').map(str::trim) {
            assert!(
                !filed.iter().any(|f| f == w),
                "the rule bans '{w}', which tokens.yaml files as a status word"
            );
        }
        // every bracketed word the contract writes is filed
        for text in c
            .sections
            .iter()
            .flat_map(|s| [&s.holds, &s.evidence])
            .chain(c.rules.iter().map(|r| &r.text))
        {
            for (at, _) in text.match_indices('[') {
                let Some(end) = text[at + 1..].find(']') else {
                    continue;
                };
                let w = &text[at + 1..at + 1 + end];
                if w.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                    assert!(
                        filed.iter().any(|f| f == w),
                        "[{w}] is not a filed status word"
                    );
                }
            }
            for capital in ["VERIFIED", "PARTIAL", "BLOCKED", "DONE"] {
                assert!(!text.contains(capital), "a verdict in capitals: {text}");
            }
        }
        // the start text says what the fingerprints leave out, not only times and ages
        let start = c.start.join(" ");
        assert!(!start.contains("only times and ages"), "{start}");
        assert!(start.contains("{n}") && start.contains("{cite}"), "{start}");
    }

    #[test]
    fn an_entry_is_placed_once_under_its_report_section() {
        let c = contract();
        let e = |id: &str, values: &[(&str, String)]| entry(id, values).expect(id);
        let entries = vec![
            e("ci.unread_scope", &[("reason", "403".into())]),
            e("integration.unfetched", &[("age", "2 h".into())]),
            e(
                "section.over_budget",
                &[
                    ("section", "release".into()),
                    ("report_section", report_section_of("release").into()),
                    ("budget", "3.0 s".into()),
                    ("remedy", direct_read("release").into()),
                ],
            ),
        ];
        let placed = placed_attention(c, &entries, Some("2026-10-30T05:33:00Z"));
        for a in &entries {
            assert_eq!(placed.matches(&attention_line(a)).count(), 1, "{placed}");
        }
        assert!(
            placed.starts_with("Attention measured, by report section:"),
            "{placed}"
        );
        assert!(placed.contains("- Blocked on you (blocked):"), "{placed}");
        assert!(placed.contains("- Integration (integration):"), "{placed}");
        assert_eq!(
            entries[2].fact, "took > 3.0 s",
            "the budget is said as the answered-by line says it"
        );
        assert_eq!(entries[2].remedy, "majordomus release next");
        // the fenced lines carry no attention block: the prompt carries each entry once
        let lines: Vec<String> = [
            "attention    [stale] integration: queue observed 36 h ago",
            "             — git fetch origin master (session)",
            "session      [current] p2",
            "             on master",
            "work         [current] 1 branch(es)",
        ]
        .iter()
        .map(|l| l.to_string())
        .collect();
        assert_eq!(section_lines(&lines), &lines[2..]);
        assert_eq!(
            heading_of(&c.sections[2], None),
            "Delivered since the previous report"
        );
    }

    #[test]
    fn a_figure_of_gib_is_cut_never_rounded_up() {
        assert_eq!(gib((19 << 30) + (7 << 30) / 10), "19.6");
        assert_eq!(gib((19 << 30) + (99 << 30) / 100), "19.9");
        assert_eq!(gib(35 << 30), "35.0");
        assert_eq!(moved_words(Some(93)), "+93 commits");
        assert_eq!(moved_words(Some(1)), "+1 commit");
    }

    #[test]
    fn a_banner_is_taken_off_and_nothing_else() {
        assert_eq!(
            strip_banner("# GENERATED FILE x\n# Source: y\n# Generator: z\nbody\n"),
            "body\n"
        );
        assert_eq!(strip_banner("# a comment\nbody\n"), "# a comment\nbody\n");
        assert_eq!(span(59), "59 s");
        assert_eq!(span(17 * 3600), "17 h");
        assert_eq!(span(6 * 86_400), "6 d");
    }
}
