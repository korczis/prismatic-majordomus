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
/// fingerprint, carries the attention entries verbatim, and states no line count.
pub fn render(use_: Use, brief: &SessionBrief) -> String {
    let c = contract();
    let cite = format!("{} #{}", contract_name(), fingerprint());
    match use_ {
        Use::Reminder => {
            let mut s = format!("Majordomus brief {} ({cite})\n\n", brief.at);
            s.push_str(&attention_block(&brief.attention));
            s.push_str("\n\n");
            s.push_str(&fence(&brief.lines));
            s
        }
        Use::Interval => {
            let mut s = format!(
                "{}: the brief composed at {} under {cite}.\n\n",
                c.marker, brief.at
            );
            s.push_str(&fence(&brief.lines));
            s.push_str("\n\n");
            s.push_str(&attention_block(&brief.attention));
            s.push_str("\n\n");
            s.push_str(&contract_block(c, brief.report.since.as_deref()));
            s
        }
        Use::Start => {
            let n = brief.interval_minutes;
            let mut s = format!(
                "Report on yourself every {n} minutes without being asked ({cite}, ADR 0136). \
                 Read the brief live each time, never from memory: `majordomus brief`, or \
                 `majordomus brief --report` for the report owed with the measured attention \
                 entries in it. Where your client schedules its own prompts and the \
                 majordomus-brief mod is not loaded, schedule `/loop {n}m /mj-brief` now; where \
                 the mod is loaded (Claude Code, `.claude/skills/majordomus-brief`), it shows \
                 the one-line brief as the status line and submits the report owed when a fact \
                 in the brief changed, at most once per interval, so a loop of your own would \
                 only repeat it. A brief in which only times and ages moved is owed no report.\n\n"
            );
            s.push_str("Attention measured now:\n");
            s.push_str(&attention_block(&brief.attention));
            s.push_str("\n\n");
            s.push_str(&contract_block(c, None));
            s
        }
    }
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

/// One attention entry as every text writes it.
pub fn attention_line(a: &BriefAttention) -> String {
    format!(
        "[{}] {}: {} — {} ({})",
        a.word, a.section, a.fact, a.remedy, a.actor
    )
}

/// The sections with their headings, holds, evidence and caps; the rules; the live reads.
fn contract_block(c: &ReportContract, since: Option<&str>) -> String {
    let mut s = format!("Write the report. {}\n", c.order_note);
    for (i, sec) in c.sections.iter().enumerate() {
        let heading = sec
            .heading
            .replace("{since}", since.unwrap_or("the previous report"));
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
        s.push_str(&format!("- {}: `{}`\n", l.fact, l.command));
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

/// The severity a word sorts by: a person's entries first, then bad, warn, info, neutral.
fn severity(word: &str) -> u8 {
    match word {
        "failing" | "blocked" | "unread" | "superseded" | "overdue" | "owed" | "dirty" => 0,
        "stale" | "degraded" | "warn" | "partial" => 1,
        "queued" | "pending" | "unmerged" => 2,
        _ => 3,
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
        word: row.word.clone(),
        fact: fill(&row.fact),
        remedy: fill(&row.remedy),
        actor: row.actor.clone(),
    })
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

fn gib(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / (1u64 << 30) as f64)
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
                    ("budget", obs.budget_ms.to_string()),
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
                ("age", i.age_seconds.map(span).unwrap_or_else(|| "?".into())),
                (
                    "observed",
                    i.observed_master
                        .as_deref()
                        .map(short)
                        .unwrap_or_else(|| "?".into()),
                ),
                (
                    "now",
                    i.master_now
                        .as_deref()
                        .map(short)
                        .unwrap_or_else(|| "?".into()),
                ),
                (
                    "moved",
                    i.moved_by
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "?".into()),
                ),
            ],
        );
    }
    if let Some(age) = i
        .master_fetched_age_seconds
        .filter(|a| *a > super::FETCH_STALE_AFTER.as_secs())
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
                            .unwrap_or_else(|| "?".into()),
                    ),
                    (
                        "total",
                        run.jobs_total
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "?".into()),
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
                        item.worktree.clone().unwrap_or_else(|| "<worktree>".into()),
                    ),
                ],
            ),
            (Some(pr), Some("BLOCKED")) => push("pr.blocked", &[("pr", pr.to_string())]),
            (None, _) if item.pushed => push(
                "branch.no_pr",
                &[
                    ("branch", item.branch.clone()),
                    ("head", item.head.as_deref().map(short).unwrap_or_default()),
                ],
            ),
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
                        .unwrap_or_else(|| "?".into()),
                ),
            ],
        );
    }

    // --- the session
    if m.session.task_id.is_none() && m.session.state != BriefSectionState::Unread {
        push(
            "session.no_task",
            &[
                (
                    "task",
                    m.session.last_task.clone().unwrap_or_else(|| "none".into()),
                ),
                (
                    "outcome",
                    m.session
                        .task_outcome
                        .clone()
                        .unwrap_or_else(|| "none".into()),
                ),
            ],
        );
    }

    crate::order::canonical(&mut out);
    out
}

/// The canonical order of attention entries: those a person must act on first, then worst
/// first, then by id.
impl crate::order::Ordered for BriefAttention {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let rank =
            i64::from(u8::from(self.actor != "person")) * 10 + i64::from(severity(&self.word));
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
