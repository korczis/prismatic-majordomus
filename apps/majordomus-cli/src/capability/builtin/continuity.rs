//! The `continuity` module: what this checkout's lifecycle is holding right now — the open
//! episode, the record the next worker would resume from, the newest progress note, what is
//! blocking acceptance, and whether the provider's events actually reach the tool.
//!
//! Everything else in this executable reads the shared half of the layer: objects that are
//! tracked, identical in every clone, and safe to project onto a public site. This module
//! reads the other half, `.ai/local/state/`, and the difference decides the whole design.
//!
//! **It is read, never written.** The lifecycle is the shell tool's; this process is one of
//! its readers. A second writer for the same records would be a second account of events the
//! ledger already holds, which is the thing that design refuses.
//!
//! **It is served, never published.** These records name this machine — a worktree path, the
//! checkpoint that happened to be newest on this disk — and a fact about a disk is not a
//! fact about the repository (ADR 0014). So the exposure is the loopback server and MCP,
//! both of which serve the worker sitting in front of this checkout, and there is no
//! projection into `docs/generated/` or `site/`. The static site documents the mechanism and
//! carries none of its content. A generator that published a session's briefing would
//! publish the one thing in the layer that no other clone can reproduce.
//!
//! **Absence is an answer.** Every field here can be legitimately empty: a fresh clone has
//! no ledger, a worker outside a task has no task, and a branch with no prior episode has no
//! handover. Each of those is reported as itself rather than as a failure, and the resolver
//! below never widens its search to make one go away — a record from another worktree
//! silently becoming your context is worse than having none, because you cannot tell it is
//! wrong until you have acted on it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::git::{self, GitState};
use crate::metadata::frontmatter;
use crate::metadata::yaml;
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which `continuity.state` is read as an MCP resource.
pub const CONTINUITY_URI: &str = "majordomus://continuity";

/// The local half of the layer, relative to the repository root. Not configurable here: the
/// shell tool decides where its state lives, and a second opinion about the path would be a
/// second source of truth for the one thing both halves must agree on.
const STATE_DIR: &str = ".ai/local/state";

/// How far a record's recorded commit is from the commit this checkout is on.
///
/// The four words are the shell tool's, deliberately. A reader that met `advanced` from one
/// surface and `stale` from another would have to learn the same four facts twice.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Divergence {
    /// Written at this commit. Trust it.
    Exact,
    /// Git has moved forward since. Trust it, and expect some of it to be done.
    Advanced,
    /// The recorded commit is not an ancestor: history was rewritten. Trust git, not this.
    Diverged,
    /// Another branch or another worktree. This record is not about your work.
    DifferentContext,
    /// Git could not answer, so this process says so rather than guessing `exact`.
    Unknown,
}

impl Divergence {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::Divergence;
    /// assert_eq!(Divergence::Advanced.as_str(), "advanced");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Divergence::Exact => "exact",
            Divergence::Advanced => "advanced",
            Divergence::Diverged => "diverged",
            Divergence::DifferentContext => "different_context",
            Divergence::Unknown => "unknown",
        }
    }

    /// Whether a record carrying this label may be read as current knowledge. `diverged`
    /// and `different_context` may not: the first describes a history that no longer
    /// exists, the second describes somebody else's work.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::Divergence;
    /// assert!(Divergence::Advanced.trustworthy());
    /// assert!(!Divergence::Diverged.trustworthy());
    /// ```
    pub fn trustworthy(self) -> bool {
        matches!(self, Divergence::Exact | Divergence::Advanced)
    }
}

/// How old a record is, judged against `session.freshness` in the policy.
///
/// This is the second of two independent judgements every record carries, and it exists
/// because the first one cannot answer this question. [`Divergence`] says where a record's
/// commit sits relative to HEAD; `advanced` means that commit is an ancestor of this one,
/// which sounds like agreement and is identically true on the day the record is written and
/// a month later. Between 2026-09-05 and 2026-09-11 this repository served a handover that
/// was `advanced` and six days dead, and every surface that read this model — MCP, the HTTP
/// API, the Cockpit — presented its `Next Action` as the thing to do (ADR 0041).
///
/// The thresholds are policy and are read, never restated here: a constant in this file
/// would be a second source of truth for a number the policy owns, and a repository whose
/// policy predates the key is reported as [`Freshness::Unknown`] naming the missing key
/// rather than judged against a default nobody declared.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Inside `session.freshness.fresh_minutes`. Current; act on it.
    Fresh,
    /// Between the two thresholds. Still current, but its age is worth stating.
    Aging,
    /// At or beyond `session.freshness.stale_minutes`. History, never current.
    Stale,
    /// Nothing to judge it by: no timestamp, or no thresholds declared.
    Unknown,
    /// A timestamp that does not parse, or one in the future.
    Invalid,
}

impl Freshness {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::Freshness;
    /// assert_eq!(Freshness::Stale.as_str(), "stale");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Freshness::Fresh => "fresh",
            Freshness::Aging => "aging",
            Freshness::Stale => "stale",
            Freshness::Unknown => "unknown",
            Freshness::Invalid => "invalid",
        }
    }

    /// Whether a record this old must be shown as history rather than as the current
    /// instruction. `unknown` is not history: a record with no timestamp is not thereby old.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::Freshness;
    /// assert!(Freshness::Stale.history());
    /// assert!(!Freshness::Unknown.history());
    /// ```
    pub fn history(self) -> bool {
        matches!(self, Freshness::Stale | Freshness::Invalid)
    }
}

/// Seconds since the Unix epoch for an RFC 3339 instant in UTC (`2026-09-05T12:34:56Z`), or
/// `None` when the string is not one. The inverse of [`crate::peers::rfc3339`], and written
/// beside it for the same reason: no date crate, and the layer's timestamps are one shape.
///
/// ```
/// use majordomus_cli::capability::builtin::continuity::epoch_seconds;
/// assert_eq!(epoch_seconds("1970-01-01T00:00:00Z"), Some(0));
/// assert_eq!(epoch_seconds("2026-08-29T10:40:00Z"), Some(1_788_000_000));
/// assert_eq!(epoch_seconds("not a timestamp"), None);
/// ```
pub fn epoch_seconds(ts: &str) -> Option<i64> {
    let b = ts.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    let n = |a: usize, z: usize| ts.get(a..z)?.parse::<i64>().ok();
    let (y, mth, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, sec) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mth) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    // days-from-civil, Howard Hinnant's algorithm: the inverse of the one in peers.rs.
    let y = if mth <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if mth > 2 { mth - 3 } else { mth + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + sec)
}

/// The thresholds a record is judged against, as the policy declares them.
#[derive(Debug, Clone, Copy, Default)]
pub struct Thresholds {
    /// `session.freshness.fresh_minutes`.
    pub fresh_minutes: Option<i64>,
    /// `session.freshness.stale_minutes`.
    pub stale_minutes: Option<i64>,
}

impl Thresholds {
    /// Judge one record's `created_at` against these thresholds, at `now` (Unix seconds).
    /// Returns the verdict, the age in whole minutes when there is one, and the reason —
    /// which is never empty, because a verdict a reader cannot act on is the failure this
    /// whole subsystem is being corrected for.
    pub fn judge(self, created_at: &str, now: i64) -> (Freshness, Option<i64>, String) {
        if created_at.is_empty() {
            return (Freshness::Unknown, None, "no timestamp".into());
        }
        let Some(then) = epoch_seconds(created_at) else {
            return (
                Freshness::Invalid,
                None,
                format!("timestamp does not parse: {created_at}"),
            );
        };
        let minutes = (now - then) / 60;
        if minutes < 0 {
            return (
                Freshness::Invalid,
                Some(minutes),
                format!("timestamp is {} minute(s) in the future", -minutes),
            );
        }
        let (Some(fresh), Some(stale)) = (self.fresh_minutes, self.stale_minutes) else {
            return (
                Freshness::Unknown,
                Some(minutes),
                "the policy declares no session.freshness thresholds".into(),
            );
        };
        if minutes >= stale {
            (
                Freshness::Stale,
                Some(minutes),
                format!(
                    "{} old, past the {} this repository calls stale",
                    span(minutes),
                    span(stale)
                ),
            )
        } else if minutes >= fresh {
            (
                Freshness::Aging,
                Some(minutes),
                format!(
                    "{} old, past the {} this repository calls fresh",
                    span(minutes),
                    span(fresh)
                ),
            )
        } else {
            (Freshness::Fresh, Some(minutes), format!("{} old", span(minutes)))
        }
    }
}

/// Whole minutes as the shell tool renders a span: `18m`, `25h`, `6d`.
///
/// ```
/// use majordomus_cli::capability::builtin::continuity::span;
/// assert_eq!(span(18), "18m");
/// assert_eq!(span(1500), "25h");
/// assert_eq!(span(8640), "6d");
/// ```
pub fn span(minutes: i64) -> String {
    if minutes < 90 {
        format!("{minutes}m")
    } else if minutes < 2880 {
        format!("{}h", minutes / 60)
    } else {
        format!("{}d", minutes / 1440)
    }
}

/// Which tier of the resolution rule matched. There is no third tier on purpose: a record
/// from an unrelated worktree or branch is never offered.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Match {
    /// Same repository, same worktree, same branch.
    SameWorktreeSameBranch,
    /// Same repository, same branch, another worktree of it.
    SameBranch,
}

/// What the resolution rule did with one file it looked at.
///
/// Three outcomes and no fourth, because the rule has three: a file is not a record at all,
/// or it is a record that lost, or it is the one that won.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// This is the record the rule chose.
    Selected,
    /// It matched a tier and lost — to a nearer tier, or to a later timestamp.
    Superseded,
    /// It never matched a tier: it is about another branch or another worktree, or it
    /// could not be read as a record at all.
    Rejected,
}

impl Standing {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::Standing;
    /// assert_eq!(Standing::Superseded.as_str(), "superseded");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Standing::Selected => "selected",
            Standing::Superseded => "superseded",
            Standing::Rejected => "rejected",
        }
    }
}

/// One file the resolution rule looked at, and what it decided about it.
///
/// This is the part of the rule that was always computed and never reported. The selected
/// record has been visible on every surface for months; the four it beat, and the reason
/// each lost, have been visible nowhere — so a worker handed the wrong record could not
/// tell a deliberate refusal ("that one is another branch's") from a defect ("that one has
/// no front matter") from a tie-break ("that one is nine minutes older").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Candidate {
    /// Repository-relative path of the file.
    pub path: String,
    /// What the rule did with it.
    pub outcome: Standing,
    /// Why, in one sentence, naming the field that decided it. Never empty.
    pub reason: String,
}

// ------------------------------------------------------------------ provenance
//
// Everything above answers *what* the lifecycle is holding. Everything below answers *why*,
// and it exists because between 2026-09-05 and 2026-09-11 the first answer was confidently
// wrong for six days. `continuity.state` reported a handover labelled `advanced` — a true
// statement about git topology that says nothing about age — and every surface presented its
// `Next Action` as the thing to do. The record was finished work. A worker who could have
// asked *why is this the record you are showing me* would have been told "tier 0, the newest
// of four, asserted six days ago" and would have stopped reading at the third clause.
//
// The rule for what may appear here is the one this subsystem was failing: **every line is
// derived from a fact this process can point at**, and where it cannot, it says so. There is
// no inference dressed as a reading, no default substituted for a missing declaration, and
// no sentence whose evidence list is empty. A plausible explanation is worse than none,
// because it is the same confident wrongness one level down.

/// What kind of thing an explanation is pointing at.
///
/// The list is short on purpose: these are the only four places a fact about this subsystem
/// can come from, and a fifth would mean the subsystem had grown a source nobody declared.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A file, named by its repository-relative path.
    File,
    /// A symbolic link, and what it points at. Distinguished from a file because for the
    /// episode pointer the link *is* the fact: the target name is the answer.
    Symlink,
    /// One line of `.ai/local/state/ledger.jsonl`, named by its one-based line number.
    LedgerLine,
    /// A key of the policy, named in dotted form with the file that declares it.
    PolicyKey,
    /// Something that is not there. An absence is evidence — it is the evidence for most of
    /// the answers this subsystem cannot give — and recording it as a kind rather than as
    /// prose keeps "we looked and found nothing" distinguishable from "we did not look".
    Absent,
}

impl EvidenceKind {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::EvidenceKind;
    /// assert_eq!(EvidenceKind::LedgerLine.as_str(), "ledger_line");
    /// assert_eq!(EvidenceKind::Absent.as_str(), "absent");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::File => "file",
            EvidenceKind::Symlink => "symlink",
            EvidenceKind::LedgerLine => "ledger_line",
            EvidenceKind::PolicyKey => "policy_key",
            EvidenceKind::Absent => "absent",
        }
    }
}

/// One thing a reader can go and check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    /// What kind of thing it is.
    pub kind: EvidenceKind,
    /// Where it is: a repository-relative path, `.ai/local/state/ledger.jsonl:412`, or a
    /// dotted policy key. Always enough to find the thing by hand.
    pub locator: String,
    /// What it says, quoted or summarised — never interpreted.
    pub detail: String,
}

impl Evidence {
    /// Evidence that is a file at a repository-relative path.
    pub fn file(locator: impl Into<String>, detail: impl Into<String>) -> Self {
        Evidence {
            kind: EvidenceKind::File,
            locator: locator.into(),
            detail: detail.into(),
        }
    }

    /// Evidence that a thing is not there. The locator still names where it was looked for,
    /// because "no file at X" is checkable and "nothing found" is not.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::continuity::{Evidence, EvidenceKind};
    /// let e = Evidence::absent(".ai/local/prompts", "the directory does not exist");
    /// assert_eq!(e.kind, EvidenceKind::Absent);
    /// ```
    pub fn absent(locator: impl Into<String>, detail: impl Into<String>) -> Self {
        Evidence {
            kind: EvidenceKind::Absent,
            locator: locator.into(),
            detail: detail.into(),
        }
    }

    /// Evidence that is one line of the ledger, numbered from one as an editor numbers it.
    pub fn ledger(line: usize, detail: impl Into<String>) -> Self {
        Evidence {
            kind: EvidenceKind::LedgerLine,
            locator: format!("{STATE_DIR}/ledger.jsonl:{line}"),
            detail: detail.into(),
        }
    }

    /// Evidence that is a policy key, named with the file that declares it.
    pub fn policy(path: &str, key: &str, detail: impl Into<String>) -> Self {
        Evidence {
            kind: EvidenceKind::PolicyKey,
            locator: format!("{path}#{key}"),
            detail: detail.into(),
        }
    }
}

/// One question about this subsystem, and the answer with its workings.
///
/// `known` is the field that makes this honest. Every other field can be filled with
/// something plausible; this one records whether the process actually established the
/// answer, and when it is false `answer` is empty and `reason` says what stopped it. A
/// legacy prompt record with no episode field, an episode with no closed record, a policy
/// that predates `session.freshness` — each of those produces an unknown that names its own
/// cause, rather than a likely-looking answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Answer {
    /// The question, in dotted form: `episode.current`, `handover.freshness`.
    pub question: String,
    /// The answer in one line. Empty exactly when `known` is false.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub answer: String,
    /// Whether this process established the answer at all.
    pub known: bool,
    /// Why the answer is what it is, or — when `known` is false — why there is none. Never
    /// empty: a verdict a reader cannot check is the thing being corrected for.
    pub reason: String,
    /// What a reader can go and look at. Never empty; an absence is recorded as
    /// [`EvidenceKind::Absent`] rather than as an empty list, so that a sentence with no
    /// evidence at all is a defect that shows.
    pub evidence: Vec<Evidence>,
    /// What else was in the running, and why it is not the answer. Empty when the question
    /// had no alternatives to weigh.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub considered: Vec<Candidate>,
}

impl Answer {
    /// An answer this process established.
    fn known(
        question: &str,
        answer: impl Into<String>,
        reason: impl Into<String>,
        evidence: Vec<Evidence>,
    ) -> Self {
        Answer {
            question: question.into(),
            answer: answer.into(),
            known: true,
            reason: reason.into(),
            evidence,
            considered: Vec::new(),
        }
    }

    /// An answer this process could not establish, with the reason it could not.
    ///
    /// The one constructor that matters for the standard this module is held to. Absence is
    /// an answer; invention is not.
    fn unknown(question: &str, reason: impl Into<String>, evidence: Vec<Evidence>) -> Self {
        Answer {
            question: question.into(),
            answer: String::new(),
            known: false,
            reason: reason.into(),
            evidence,
            considered: Vec::new(),
        }
    }

    /// Attach what else was weighed.
    fn weighing(mut self, considered: Vec<Candidate>) -> Self {
        self.considered = considered;
        self
    }
}

/// The episode an explanation is about, and how this process came to be talking about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Subject {
    /// The episode's id, when one was resolved.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub episode: String,
    /// The provider session that owns it, when the record names one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_session: String,
    /// `open`, `closed`, or `unknown`.
    pub state: String,
    /// How it was chosen: named by the caller, or taken from this checkout's pointer.
    pub selected_by: String,
    /// The record this process read it from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub record: String,
}

/// Why the session subsystem is saying what it is saying.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionProvenance {
    /// The contract this document follows.
    pub schema: String,
    /// The episode being explained, and how it was chosen.
    pub subject: Subject,
    /// One entry per question, in a fixed order.
    pub answers: Vec<Answer>,
}

/// Which question to explain, and about which episode.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplainInput {
    /// The episode: its id (`s-20260909152316-024f`), or the provider session that owns it.
    /// Absent means the episode this checkout's pointer names — which is itself one of the
    /// things `episode.current` explains.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub episode: Option<String>,
    /// One question in dotted form, or a prefix of one (`handover` gives both handover
    /// questions). Absent gives every question.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
}

impl BenchmarkCases for ExplainInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "current-episode",
                ExplainInput {
                    episode: None,
                    question: None,
                },
            ),
            NamedCase::new(
                "one-question",
                ExplainInput {
                    episode: None,
                    question: Some("handover".into()),
                },
            ),
        ]
    }
}

/// One durable record of the local half, as much of it as a reader needs to decide whether
/// to open the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Record {
    /// Repository-relative path. The body is at the path; it is not copied here.
    pub path: String,
    /// When the record says it was written.
    pub created_at: String,
    /// The task it belongs to, or `none`.
    pub task_id: String,
    /// The branch it was written on.
    pub branch: String,
    /// The commit it was written at.
    pub head: String,
    /// Whether the working tree was clean or dirty then.
    pub working_tree: String,
    /// Which tier of the resolution rule matched.
    pub matched: Match,
    /// How far its commit is from this one.
    pub divergence: Divergence,
    /// How old it is, judged against `session.freshness`. Independent of `divergence`: a
    /// record is routinely `advanced` and `stale` at once, and that pair is what a reader
    /// must not collapse into "trustworthy".
    pub freshness: Freshness,
    /// Its age in whole minutes, when it has a timestamp that parses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_minutes: Option<i64>,
    /// Why it was judged that way, in one phrase. Never empty: a verdict a reader cannot
    /// act on is the failure this subsystem is being corrected for.
    pub freshness_reason: String,
    /// The section a resuming worker acts on, when the record has one and is still current.
    /// A handover's `Next Action`; empty for a record that carries no sections, and
    /// deliberately empty for one that is `stale` or `invalid` — see `next_action_withheld`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub next_action: String,
    /// Why `next_action` is empty despite the record having one. Set only when it was
    /// withheld, never when the record simply carries no such section, so that a reader can
    /// tell "there is nothing to do" from "what there was to do is six days old".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub next_action_withheld: String,
}

/// The open episode this checkout points at, when there is one.
///
/// One execution episode belongs to one provider session, and several can be open in one
/// checkout at once — two windows of the same provider are two workers. What is read here
/// is `state/session-current.yaml`, the pointer to the episode of this checkout; a worker
/// that knows its own provider session resolves its own episode instead, which is what
/// `mj_session_here_file` in `lib/common.sh` does and what stamps each ledger line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenSession {
    /// The episode's id.
    pub session_id: String,
    /// When it opened.
    pub started_at: String,
    /// Who opened it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    /// The worker identity, when one was supplied. Never inferred.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub worker: String,
    /// The provider whose event opened it, when one did.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// That provider's own session identity — the string the prompt archive stamps on the
    /// same worker's records, and the name this episode is keyed by. Empty for an episode
    /// opened by hand, which is the one episode no provider session owns.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider_session: String,
    /// The branch it opened on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// The commit it opened at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start_head: String,
    /// True when the open record here belongs to another checkout. Such a record is
    /// reported and never treated as this checkout's episode.
    pub foreign: bool,
}

/// The active task of this checkout, when there is one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ActiveTask {
    /// The task id.
    pub id: String,
    /// What is being worked on.
    pub task: String,
    /// The execution profile it runs under.
    pub profile: String,
    /// Its typed outcome so far: `active`, `handed_over`, or a finished one.
    pub outcome: String,
    /// The paths it claims.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// The obligations it owes before the outcome `completed` is available.
    ///
    /// Beside `scope` and not inside it, because the two are different promises: scope is
    /// containment — where a worker may write — and this is delivery. A change can sit
    /// entirely inside its scope and still be uncommitted on a laptop (ADR 0030).
    /// [`super::obligations`] is what judges each of these against its evidence; here it
    /// is reported as declared.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// When it started.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub started_at: String,
    /// The commit it started at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head: String,
}

/// What the lifecycle of this checkout is holding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Continuity {
    /// Whether this checkout's lifecycle has ever run: a ledger line, a task, an episode or
    /// a record. False in a fresh clone, which is not a fault.
    ///
    /// It is not "the directory exists". Several things create that directory before
    /// anything has been recorded in it, so a reader that took its presence for evidence
    /// would be told the lifecycle had run in a checkout where it never had.
    pub present: bool,
    /// The worktree this answer is about. Every selection below is scoped to it.
    pub worktree: String,
    /// The branch, or `DETACHED`.
    pub branch: String,
    /// The commit this checkout is on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head: String,
    /// `clean` or `dirty`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub working_tree: String,
    /// The open episode, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<OpenSession>,
    /// The active task, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<ActiveTask>,
    /// The record the next worker would resume from, or `None` when nothing resolves here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handover: Option<Record>,
    /// The newest progress note for this worktree and branch, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<Record>,
    /// Unresolved questions on this branch. Every one refuses `finish --outcome completed`,
    /// whichever task opened it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    /// How many records of each kind this checkout holds, against the policy's caps.
    pub tallies: BTreeMap<String, usize>,
    /// What a reader should know before trusting any of the above: a diverged record, a
    /// foreign open session, a malformed file that was skipped. Empty is the good case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// --------------------------------------------------------------------- reading

/// The scalar fields of a YAML mapping, flattened to strings.
fn scalars(map: serde_json::Map<String, serde_json::Value>) -> BTreeMap<String, String> {
    map.into_iter()
        .filter_map(|(k, v)| yaml::scalar_string(&v).map(|s| (k, s)))
        .collect()
}

/// The front matter of a Markdown record. A file without front matter, or with front matter
/// that does not parse, yields `None` and is counted as skipped by the caller: a malformed
/// record degrades the answer, it never fails the call.
fn front(path: &Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let split = frontmatter::split(&text).ok()?;
    Some(scalars(yaml::parse_mapping(split.front?).ok()?))
}

/// A whole YAML document, for the two records of the local half that are not Markdown.
///
/// `session-current.yaml` (a symlink into `state/sessions-open/`) and `current.yaml` carry
/// no `---` fences — they are the state
/// itself rather than a document about it — so reading them with the front-matter splitter
/// finds nothing and reports an absent episode in a checkout that has one. Two shapes, two
/// readers, and the difference stated here rather than discovered.
fn document(path: &Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(scalars(yaml::parse_mapping(&text).ok()?))
}

/// One level-one section of a Markdown record, body only, trimmed. Used to lift the part of
/// a handover a resuming worker acts on without copying the document into every answer.
fn section(path: &Path, want: &str) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let mut out: Vec<&str> = Vec::new();
    let mut on = false;
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("# ") {
            on = head.trim_end() == want;
            continue;
        }
        if on {
            out.push(line);
        }
    }
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

/// Compare a record's commit with this checkout's, the same way the shell tool does: by
/// ancestry, not by equality of timestamps. `git` being unavailable yields `Unknown`, which
/// is the honest answer and not `Exact`.
pub(crate) fn divergence(root: &Path, theirs: &str, ours: Option<&str>) -> Divergence {
    let Some(ours) = ours else {
        return Divergence::Unknown;
    };
    if theirs == ours {
        return Divergence::Exact;
    }
    if theirs.is_empty() {
        return Divergence::Unknown;
    }
    match git::is_ancestor(root, theirs, ours) {
        Some(true) => Divergence::Advanced,
        Some(false) => Divergence::Diverged,
        None => Divergence::Unknown,
    }
}

/// The most relevant record in `dir` for this worktree and branch.
///
/// Two tiers and no third, exactly as `mj_resolve_latest` has them: same worktree and
/// branch, then same branch in another worktree of the same repository, then nothing. The
/// ordering inside a tier is by the timestamp the record asserts — never by filesystem
/// modification time, which does not survive a clone and is not the time the record claims.
///
/// Returns the record, the number of files that were skipped because they could not be
/// read as records, and one [`Candidate`] entry per `.md` file the directory held — the
/// reason each one was rejected, superseded or selected.
///
/// The trace is produced by the selection itself rather than by a second pass over the
/// directory, and that is the whole point of it being here. The reasons this rule acts on
/// have always existed — `mj_resolve_latest` sets `MJ_RES_MATCH` and `MJ_RES_SKIPPED`, and
/// the `continue` arms below each encode one — and they were simply dropped on the floor.
/// A worker who met a six-day-old handover between 2026-09-05 and 2026-09-11 could see
/// *that* it had been chosen and never *why*, which is the failure `continuity.explain`
/// exists to close (ADR 0041). A trace recomputed by a second function would be a second
/// account of the same rule, and the first thing it would do is drift.
fn resolve(
    root: &Path,
    dir: &Path,
    branch: &str,
    head: Option<&str>,
    thresholds: Thresholds,
    now: i64,
) -> (Option<Record>, usize, Vec<Candidate>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        // Not a finding: a checkout that has never written a handover has no directory for
        // them, and reporting that as a skipped file would invent a fault.
        return (None, 0, Vec::new());
    };
    let worktree = root.to_string_lossy().to_string();
    let mut best: Option<(u8, String, Record)> = None;
    let mut skipped = 0usize;
    // Every `.md` file the directory held, with what happened to it. Filled as the rule
    // runs, so it cannot disagree with the rule.
    let mut trace: Vec<Candidate> = Vec::new();
    // (tier, ordering key, path, created_at) for every file that matched a tier. Held back
    // until the winner is known, because "superseded" is a fact about a pair.
    let mut matched: Vec<(u8, String, String, String)> = Vec::new();

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    // A deterministic walk, so two runs over one directory agree about which of two
    // records written in the same second is newer.
    paths.sort();

    for path in paths {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let mut reject = |reason: String| {
            trace.push(Candidate {
                path: rel.clone(),
                outcome: Standing::Rejected,
                reason,
            });
        };
        let Some(f) = front(&path) else {
            skipped += 1;
            reject("it has no front matter that parses, so nothing in it can be read as a record".into());
            continue;
        };
        let (Some(created), Some(rhead)) = (f.get("created_at"), f.get("head")) else {
            skipped += 1;
            reject(
                "its front matter is missing created_at or head, the two fields the ordering and the divergence label are computed from"
                    .into(),
            );
            continue;
        };
        if f.get("schema_version").map(String::as_str) != Some("1") {
            skipped += 1;
            reject(format!(
                "its schema_version is {}, and this reader knows version 1 only",
                f.get("schema_version")
                    .map(String::as_str)
                    .unwrap_or("absent")
            ));
            continue;
        }
        let rbranch = f.get("branch").cloned().unwrap_or_default();
        let rworktree = f.get("worktree").cloned().unwrap_or_default();
        let tier = if rworktree == worktree && rbranch == branch {
            0u8
        } else if branch != "DETACHED" && rbranch == branch {
            1u8
        } else {
            // The one rejection that is a deliberate refusal rather than a defect in the
            // file: the record is well formed and is about somebody else's work. Neither
            // tier is widened to reach it, because a briefing that is quietly about
            // another branch is worse than no briefing at all.
            reject(if branch == "DETACHED" {
                format!(
                    "it was written on branch {rbranch}, and this checkout is on no branch (DETACHED); the second tier matches by branch and there is none to match"
                )
            } else {
                format!(
                    "it was written on branch {rbranch} in worktree {rworktree}; this checkout is on {branch} in {worktree}, and neither tier reaches it"
                )
            });
            continue;
        };
        let record = Record {
            path: rel.clone(),
            created_at: created.clone(),
            task_id: f.get("task_id").cloned().unwrap_or_else(|| "none".into()),
            branch: rbranch,
            head: rhead.clone(),
            working_tree: f.get("working_tree").cloned().unwrap_or_default(),
            matched: if tier == 0 {
                Match::SameWorktreeSameBranch
            } else {
                Match::SameBranch
            },
            divergence: divergence(root, rhead, head),
            freshness: Freshness::Unknown,
            age_minutes: None,
            freshness_reason: String::new(),
            next_action: section(&path, "Next Action"),
            next_action_withheld: String::new(),
        };
        // Judged here rather than by each surface: MCP, the HTTP API and the Cockpit all
        // read this one value, which is the point. A record past the stale threshold keeps
        // its path — knowing what the last worker was doing is worth having — and loses the
        // section a reader would otherwise act on, with the reason in its place.
        let mut record = record;
        let (f, age, why) = thresholds.judge(&record.created_at, now);
        record.freshness = f;
        record.age_minutes = age;
        record.freshness_reason = why;
        if f.history() && !record.next_action.is_empty() {
            record.next_action_withheld = format!(
                "the record is {}: {}. It is history, not an instruction; read it at the path above.",
                f.as_str(),
                record.freshness_reason
            );
            record.next_action.clear();
        }
        let key = created.clone();
        // Every file that reached here matched a tier and is a real candidate. It is kept
        // whether or not it wins, because "there were four and this one is newest" is the
        // answer to *why this record*, and a trace that recorded only the winner would
        // answer a different question.
        matched.push((tier, key.clone(), rel, created.clone()));
        let better = match &best {
            None => true,
            Some((btier, bkey, _)) => tier < *btier || (tier == *btier && key > *bkey),
        };
        if better {
            best = Some((tier, key, record));
        }
    }

    // The winner is known only now, so the verdict on each candidate is written now — from
    // the same two facts the comparison above used, and from nothing else.
    let won = best.as_ref().map(|(t, k, _)| (*t, k.clone()));
    for (tier, key, rel, created) in matched {
        let (outcome, reason) = match &won {
            Some((wt, wk)) if *wt == tier && *wk == key => (
                Standing::Selected,
                format!(
                    "tier {tier} ({}), and the newest of them: it asserts {created}",
                    tier_name(tier)
                ),
            ),
            Some((wt, _)) if tier > *wt => (
                Standing::Superseded,
                format!(
                    "tier {tier} ({}), and a tier {wt} ({}) record exists; the nearer tier wins outright, whatever the timestamps say",
                    tier_name(tier),
                    tier_name(*wt)
                ),
            ),
            Some((_, wk)) => (
                Standing::Superseded,
                format!(
                    "tier {tier} ({}), same as the selected record, but it asserts {created} and the selected one asserts {wk}; the later timestamp wins",
                    tier_name(tier)
                ),
            ),
            // Unreachable while `matched` is non-empty, because a matched candidate always
            // produces a `best`. Written as a verdict rather than an `unwrap` so that a
            // future change to the comparison cannot turn a logic slip into a panic in the
            // one subsystem whose job is to be trusted.
            None => (
                Standing::Rejected,
                "it matched a tier and yet no record was selected; this is a defect in the resolver, not a fact about the record".into(),
            ),
        };
        trace.push(Candidate {
            path: rel,
            outcome,
            reason,
        });
    }
    trace.sort_by(|a, b| a.path.cmp(&b.path));
    (best.map(|(_, _, r)| r), skipped, trace)
}

/// The tier's name, as [`Match`] serialises it. One spelling for the tiers, shared by the
/// selected record's `matched` field and by the sentence that explains a rejection.
fn tier_name(tier: u8) -> &'static str {
    if tier == 0 {
        "same worktree, same branch"
    } else {
        "same branch, another worktree"
    }
}

/// How many `.md` records a directory holds.
fn count(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
                .count()
        })
        .unwrap_or(0)
}

/// The unresolved entries of the blocker store. The line format is machine-written precisely
/// so a reader like this one can rely on it; a line that does not match is not silently
/// dropped, it is simply not unresolved, and `doctor` is what reports a store that does not
/// parse.
///
/// The HTML comment block is skipped, exactly as the shell reader skips it. The store's
/// template documents its own line format inside a comment, so a reader that does not skip
/// it reports a blocker that does not exist in every fresh checkout — and a phantom blocker
/// is worse than a missed one, because it refuses work nobody can unblock.
fn blockers(path: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut in_comment = false;
    for line in text.lines() {
        if line.contains("<!--") {
            in_comment = true;
        }
        if in_comment {
            if line.contains("-->") {
                in_comment = false;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("- [unresolved] ") {
            out.push(rest.trim().to_string());
        }
    }
    out
}

// --------------------------------------------------------------------- handler

/// The freshness thresholds this repository declares, read from the policy the manifest
/// names.
///
/// Read here rather than carried on the [`Context`] because nothing else in this process
/// needs them, and read from the policy rather than written down because a constant in this
/// file would be the second copy of a number the policy owns — the drift `session.freshness`
/// exists in one place to prevent. A repository whose policy predates the key, or whose
/// policy cannot be read at all, yields no thresholds; every record is then reported as
/// `unknown` naming the missing key, which is the honest answer and not a default.
fn thresholds_of(ctx: &Context) -> Thresholds {
    let root = PathBuf::from(&ctx.index.repository.root);
    let Ok(repo) = crate::repository::Repository::open(&root) else {
        return Thresholds::default();
    };
    let Ok(loaded) = crate::policy::LoadedPolicy::load(&repo) else {
        return Thresholds::default();
    };
    Thresholds {
        fresh_minutes: loaded.policy.session.freshness.fresh_minutes,
        stale_minutes: loaded.policy.session.freshness.stale_minutes,
    }
}

fn state(ctx: &Context, _: Empty) -> Result<Continuity, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let dir = root.join(STATE_DIR);

    let (branch, head, working_tree) = match &ctx.index.repository.git {
        GitState::Available(info) => (
            info.branch.clone().unwrap_or_else(|| "DETACHED".into()),
            info.head.clone(),
            info.working_tree.clone(),
        ),
        GitState::Unavailable { .. } => ("DETACHED".into(), None, String::new()),
    };

    let mut findings = Vec::new();
    let mut skipped = 0usize;

    // --- the open episode
    let session = match document(&dir.join("session-current.yaml")) {
        Some(f) => {
            let sworktree = f.get("worktree").cloned().unwrap_or_default();
            let foreign = !sworktree.is_empty() && sworktree != root.to_string_lossy();
            if foreign {
                findings.push(format!(
                    "the open session record here belongs to {sworktree}, not this checkout; nothing about it is about your work"
                ));
            }
            f.get("session_id").map(|id| OpenSession {
                session_id: id.clone(),
                started_at: f.get("started_at").cloned().unwrap_or_default(),
                owner: f.get("owner").cloned().unwrap_or_default(),
                worker: f.get("worker").cloned().unwrap_or_default(),
                provider: f.get("provider").cloned().unwrap_or_default(),
                provider_session: f.get("provider_session").cloned().unwrap_or_default(),
                branch: f.get("branch").cloned().unwrap_or_default(),
                start_head: f.get("start_head").cloned().unwrap_or_default(),
                foreign,
            })
        }
        None => None,
    };

    // --- the active task. Its `scope` is a list, so it is read from the document rather
    // than from the scalar flattening the other fields use.
    let task = read_task(&dir.join("current.yaml"));

    // --- the two resolved records
    // One reading of the policy's thresholds and one reading of the clock for both records,
    // so that two records resolved in the same call cannot be judged against different
    // instants — and so that the numbers come from the policy rather than from this file.
    let thresholds = thresholds_of(ctx);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (handover, s1, _) = resolve(
        &root,
        &dir.join("handovers"),
        &branch,
        head.as_deref(),
        thresholds,
        now,
    );
    let (checkpoint, s2, _) = resolve(
        &root,
        &dir.join("checkpoints"),
        &branch,
        head.as_deref(),
        thresholds,
        now,
    );
    skipped += s1 + s2;
    if skipped > 0 {
        findings.push(format!(
            "{skipped} file(s) under {STATE_DIR} could not be read as records and were skipped; run `majordomus doctor`"
        ));
    }
    for (what, r) in [("handover", &handover), ("checkpoint", &checkpoint)] {
        if let Some(r) = r {
            if !r.divergence.trustworthy() {
                findings.push(format!(
                    "the resolved {what} is {}: it was written at {} and that commit is not in this history; trust git over it",
                    r.divergence.as_str(),
                    &r.head[..7.min(r.head.len())]
                ));
            }
            // Reported separately, because it is a separate fact. A record is routinely
            // `advanced` — its commit an ancestor of HEAD, which reads as agreement — and
            // long dead at the same time, and it was exactly that pair, unreported, that
            // handed six days of workers a finished instruction (ADR 0041).
            if r.freshness.history() {
                findings.push(format!(
                    "the resolved {what} is {}: {}. It is context, not an instruction; what it said to do next is not offered as current.",
                    r.freshness.as_str(),
                    r.freshness_reason
                ));
            }
        }
    }

    let blockers = blockers(&dir.join("open-questions.md"));
    if !blockers.is_empty() {
        findings.push(format!(
            "{} unresolved question(s) on this branch; every one refuses `majordomus finish --outcome completed`",
            blockers.len()
        ));
    }

    let mut tallies = BTreeMap::new();
    tallies.insert("handovers".to_string(), count(&dir.join("handovers")));
    tallies.insert("checkpoints".to_string(), count(&dir.join("checkpoints")));
    // No `sessions` tally. A closed episode is an object of the shared layer, so the index
    // already counts it and `objects.list --kind session` already answers it; a second
    // count here, over a directory the closed records no longer live in, is the drift this
    // repository forbids rather than a convenience.
    tallies.insert("blockers".to_string(), blockers.len());
    tallies.insert(
        "ledger_lines".to_string(),
        std::fs::read_to_string(dir.join("ledger.jsonl"))
            .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
    );

    let present = task.is_some() || session.is_some() || tallies.values().any(|n| *n > 0);

    Ok(Continuity {
        present,
        worktree: root.to_string_lossy().to_string(),
        branch,
        head: head.unwrap_or_default(),
        working_tree,
        session,
        task,
        handover,
        checkpoint,
        blockers,
        tallies,
        findings,
    })
}

/// The active task record. Read whole rather than through the scalar flattening, because
/// `scope` and `requires` are lists, and a task without its scope is a task whose claim
/// nobody can check.
///
/// Shared with [`super::obligations`], which expands `requires` against the evidence in the
/// ledger. One reader for one file: a second parse of the same record is how two surfaces
/// come to disagree about what the task said.
pub(crate) fn read_task(path: &Path) -> Option<ActiveTask> {
    let text = std::fs::read_to_string(path).ok()?;
    let map = yaml::parse_mapping(&text).ok()?;
    let s = |k: &str| map.get(k).and_then(yaml::scalar_string).unwrap_or_default();
    let id = s("id");
    if id.is_empty() {
        return None;
    }
    let list = |k: &str| -> Vec<String> {
        map.get(k)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(yaml::scalar_string).collect())
            .unwrap_or_default()
    };
    let scope = list("scope");
    let requires = list("requires");
    Some(ActiveTask {
        id,
        task: s("task"),
        profile: s("profile"),
        outcome: s("outcome"),
        scope,
        requires,
        started_at: s("started_at"),
        head: s("head"),
    })
}

/// One line of the ledger, with the number a reader would cite it by.
///
/// The envelope's event id is taken **positionally**, from the first `"event":"` in the raw
/// line, and not from the parsed object. That is not fastidiousness. `mj_ledger_append`
/// writes the envelope first and appends the caller's payload after it, and two events
/// declare a payload key that is also an envelope key: `provider.event.received` and
/// `provider.event.failed` both require `event`, meaning the provider's own event name
/// (`SessionStart`, `Stop`). The resulting line carries `"event"` twice, and a JSON parser
/// that keeps the last duplicate — `serde_json` does — reads such a line as an event called
/// `SessionStart`, which is not an event this repository declares. Reading the envelope by
/// position recovers the id that `share/events.yaml` actually registered; the payload copy
/// is read from the parsed value as `provider_event`, which is what it was meant to be.
struct LedgerLine {
    /// One-based, as an editor numbers it.
    number: usize,
    /// The envelope's event id, read positionally.
    event: String,
    /// The whole line, parsed. For a line with a duplicated key this holds the last value.
    value: serde_json::Value,
}

impl LedgerLine {
    /// A string field of the parsed line, or the empty string.
    fn field(&self, key: &str) -> String {
        self.value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    /// The episode this line was stamped with by the envelope, or the empty string for a
    /// line written before episodes were stamped at all.
    fn episode(&self) -> String {
        self.field("session")
    }

    /// A one-line rendering for an evidence `detail`: when, what, and the stamp.
    fn summary(&self) -> String {
        let ts = self.field("ts");
        let stamp = self.episode();
        let mut s = format!("{ts} {}", self.event);
        if !stamp.is_empty() {
            s.push_str(&format!(" (stamped session={stamp})"));
        }
        s
    }
}

/// The envelope's event id: the value of the **first** `"event":"..."` in the line. See
/// [`LedgerLine`] for why the first and not the parsed one.
fn envelope_event(line: &str) -> String {
    let Some(at) = line.find("\"event\":\"") else {
        return String::new();
    };
    let rest = &line[at + 9..];
    match rest.find('"') {
        Some(end) => rest[..end].to_string(),
        None => String::new(),
    }
}

/// Every line of the ledger that parses, numbered. A line that is not JSON is dropped and
/// counted; a ledger that has grown one bad line still holds the rest, and the count is
/// reported rather than swallowed.
fn ledger(path: &Path) -> (Vec<LedgerLine>, usize) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (Vec::new(), 0);
    };
    let mut out = Vec::new();
    let mut unreadable = 0usize;
    for (i, raw) in text.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(raw) {
            Ok(value) => out.push(LedgerLine {
                number: i + 1,
                event: envelope_event(raw),
                value,
            }),
            Err(_) => unreadable += 1,
        }
    }
    (out, unreadable)
}

/// Every open episode this checkout's store holds, as (file name, flattened document).
///
/// The store and not the pointer. `state/sessions-open/` is keyed by provider session, and
/// several episodes are open in one checkout routinely — two windows of one provider are two
/// workers. Reading the pointer alone is how four of five open episodes became invisible to
/// every surface this tool has; the pointer's job here is only to say which of these the
/// checkout is aimed at, which is a separate question with a separate answer.
fn open_episodes(dir: &Path) -> Vec<(String, BTreeMap<String, String>)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    paths.sort();
    paths
        .iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            Some((name, document(p)?))
        })
        .collect()
}

/// The closed record for one episode, as (repository-relative path, front matter).
///
/// A closed episode is an object of the shared layer, so this reads the index rather than
/// walking a directory: the index already holds every `session` object with its front matter
/// parsed, and a second walk would be a second opinion about which records exist.
fn closed_record(ctx: &Context, episode: &str) -> Option<(String, BTreeMap<String, String>)> {
    ctx.index
        .objects
        .iter()
        .filter(|o| o.kind == "session")
        .find(|o| {
            o.metadata
                .get("session_id")
                .and_then(serde_json::Value::as_str)
                == Some(episode)
        })
        .map(|o| {
            let front = o
                .metadata
                .as_object()
                .cloned()
                .map(scalars)
                .unwrap_or_default();
            (o.provenance.path.clone(), front)
        })
}

/// Resolve which episode an explanation is about, and say how.
///
/// Three ways in, in this order: the caller named an episode id; the caller named a provider
/// session; the caller named nothing and the checkout's pointer decides. The order matters
/// only in that an id is checked before a provider session, and the two vocabularies do not
/// overlap — an episode id is `s-<timestamp>-<suffix>` and a provider session is whatever the
/// provider calls itself.
fn subject_of(
    ctx: &Context,
    dir: &Path,
    named: Option<&str>,
) -> Result<(Subject, Option<BTreeMap<String, String>>), CapabilityError> {
    let open = open_episodes(&dir.join("sessions-open"));
    let get = |f: &BTreeMap<String, String>, k: &str| f.get(k).cloned().unwrap_or_default();

    if let Some(want) = named {
        // An open episode, by its id or by the provider session that keys its file.
        if let Some((name, f)) = open.iter().find(|(name, f)| {
            get(f, "session_id") == want
                || get(f, "provider_session") == want
                || name.trim_end_matches(".yaml") == want
        }) {
            return Ok((
                Subject {
                    episode: get(f, "session_id"),
                    provider_session: get(f, "provider_session"),
                    state: "open".into(),
                    selected_by: format!("named by the caller as '{want}'"),
                    record: format!("{STATE_DIR}/sessions-open/{name}"),
                },
                Some(f.clone()),
            ));
        }
        // A closed one, by its id. There is no provider-session index over closed records,
        // so a provider session that owns only closed episodes does not resolve here — and
        // that is reported as not found rather than guessed at.
        if let Some((path, f)) = closed_record(ctx, want) {
            return Ok((
                Subject {
                    episode: want.to_string(),
                    provider_session: get(&f, "provider_session"),
                    state: "closed".into(),
                    selected_by: format!("named by the caller as '{want}'"),
                    record: path,
                },
                Some(f),
            ));
        }
        return Err(CapabilityError::NotFound(format!(
            "no episode '{want}': it is not one of the {} open in {STATE_DIR}/sessions-open, \
             and no closed session record carries it as session_id. \
             `continuity.explain` with no episode explains the one this checkout points at.",
            open.len()
        )));
    }

    match document(&dir.join("session-current.yaml")) {
        Some(f) => Ok((
            Subject {
                episode: get(&f, "session_id"),
                provider_session: get(&f, "provider_session"),
                state: "open".into(),
                selected_by: format!("this checkout's pointer, {STATE_DIR}/session-current.yaml"),
                record: format!("{STATE_DIR}/session-current.yaml"),
            },
            Some(f),
        )),
        // Not an error. A checkout with no open episode is an ordinary state — a fresh
        // clone, or every episode closed — and the questions below still have answers, most
        // of them "no, and here is why". Refusing the call would turn the normal case into a
        // failure and teach a reader to stop asking.
        None => Ok((
            Subject {
                episode: String::new(),
                provider_session: String::new(),
                state: "unknown".into(),
                selected_by: format!(
                    "nothing: {STATE_DIR}/session-current.yaml does not exist or does not parse"
                ),
                record: String::new(),
            },
            None,
        )),
    }
}

/// Why this episode is the current one — and what the pointer passed over.
fn q_episode_current(
    root: &Path,
    dir: &Path,
    subject: &Subject,
) -> Answer {
    const Q: &str = "episode.current";
    let pointer = dir.join("session-current.yaml");
    let rel_pointer = format!("{STATE_DIR}/session-current.yaml");
    let open = open_episodes(&dir.join("sessions-open"));

    // The pointer is a symlink, and for this question the link target *is* the answer: it
    // names the file, and the file name is the provider session. Read as a link first, so
    // that "it points at X" is reported as the fact it is rather than inferred from the
    // contents of whatever it resolved to.
    let target = std::fs::read_link(&pointer)
        .ok()
        .map(|t| t.to_string_lossy().to_string());

    let mut evidence = Vec::new();
    match &target {
        Some(t) => evidence.push(Evidence {
            kind: EvidenceKind::Symlink,
            locator: rel_pointer.clone(),
            detail: format!("-> {t}"),
        }),
        None if pointer.exists() => evidence.push(Evidence::file(
            &rel_pointer,
            "a regular file, not a symlink into sessions-open/: this checkout predates the \
             per-provider layout, so the pointer is the episode rather than naming one",
        )),
        None => evidence.push(Evidence::absent(
            &rel_pointer,
            "no pointer: this checkout is aimed at no episode",
        )),
    }

    // Every open episode is a candidate for "the current one", and exactly one can win.
    let mut considered: Vec<Candidate> = Vec::new();
    for (name, f) in &open {
        let path = format!("{STATE_DIR}/sessions-open/{name}");
        let id = f.get("session_id").cloned().unwrap_or_default();
        let their_worktree = f.get("worktree").cloned().unwrap_or_default();
        let ours = root.to_string_lossy().to_string();
        let points_here = target.as_deref().is_some_and(|t| t.ends_with(name));
        let (outcome, reason) = if points_here {
            (
                Standing::Selected,
                format!("the pointer names this file, and it holds episode {id}"),
            )
        } else if !their_worktree.is_empty() && their_worktree != ours {
            (
                Standing::Rejected,
                format!(
                    "episode {id} is open in worktree {their_worktree}, not this one; it is \
                     somebody else's work and no pointer here could make it yours"
                ),
            )
        } else {
            (
                Standing::Superseded,
                format!(
                    "episode {id} is open in this same checkout and the pointer does not name \
                     it; `session start` re-aims the pointer, so this is a concurrently open \
                     episode rather than a closed one"
                ),
            )
        };
        considered.push(Candidate {
            path,
            outcome,
            reason,
        });
    }

    if subject.episode.is_empty() {
        return Answer::unknown(
            Q,
            format!(
                "this checkout points at no episode: {}. {} episode(s) are open in the store, \
                 so the answer is not 'none are open' — it is that nothing says which of them \
                 this checkout is working in.",
                subject.selected_by,
                open.len()
            ),
            evidence,
        )
        .weighing(considered);
    }

    evidence.push(Evidence::file(
        &subject.record,
        format!(
            "session_id: {}{}",
            subject.episode,
            if subject.provider_session.is_empty() {
                String::new()
            } else {
                format!(", provider_session: {}", subject.provider_session)
            }
        ),
    ));

    let others = open.len().saturating_sub(1);
    Answer::known(
        Q,
        &subject.episode,
        format!(
            "{}. {}",
            subject.selected_by,
            if others == 0 {
                "It is the only episode open in this checkout's store.".to_string()
            } else {
                format!(
                    "{others} other episode(s) are open in the store and the pointer does not \
                     name them; each is listed below with the reason."
                )
            }
        ),
        evidence,
    )
    .weighing(considered)
}

/// Which provider event closed this episode, and whether the close was clean.
///
/// Two facts, and they come from different places. *Whether* it closed and *how* is the
/// closed record's `outcome` and the `session.closed` line that wrote it. *Which provider
/// event* caused the close is only knowable if the adapter left a receipt — the
/// `provider.` namespace of the ledger — and in a repository where it did not, this says so
/// instead of naming the plausible one.
fn q_episode_close(subject: &Subject, lines: &[LedgerLine], closed: Option<&BTreeMap<String, String>>) -> Answer {
    const Q: &str = "episode.close";

    // Ledger lines stamped with this episode, in the `provider.` namespace. The namespace is
    // structural — `share/events.yaml` groups the adapter's receipts under it — so this
    // filters on the prefix rather than on a list of event ids compiled into this file,
    // which would go stale the moment a third receipt is declared.
    let receipts: Vec<&LedgerLine> = lines
        .iter()
        .filter(|l| l.episode() == subject.episode && l.event.starts_with("provider."))
        .collect();
    let closes: Vec<&LedgerLine> = lines
        .iter()
        .filter(|l| l.episode() == subject.episode && l.event == "session.closed")
        .collect();

    let mut evidence = Vec::new();

    if subject.state == "open" {
        evidence.push(Evidence::file(
            &subject.record,
            "an open episode record: it sits in sessions-open/ and carries no closed_at",
        ));
        for l in &closes {
            evidence.push(Evidence::ledger(l.number, l.summary()));
        }
        return Answer::known(
            Q,
            "not closed",
            if closes.is_empty() {
                "the episode is open and no session.closed line in the ledger is stamped with \
                 it. Nothing has closed it, so no event did."
                    .to_string()
            } else {
                format!(
                    "the episode record is still in sessions-open/, and yet {} session.closed \
                     line(s) are stamped with it. Those two disagree; trust neither until \
                     `majordomus recover` has looked at it.",
                    closes.len()
                )
            },
            evidence,
        );
    }

    let Some(front) = closed else {
        return Answer::unknown(
            Q,
            format!(
                "episode {} has no closed record and is not in the open store. It may have been \
                 closed in a clone this checkout cannot see, or its record may never have been \
                 written. Nothing here can tell those apart.",
                subject.episode
            ),
            vec![Evidence::absent(
                ".ai/repo/sessions/",
                format!(
                    "no indexed session object carries session_id: {}",
                    subject.episode
                ),
            )],
        );
    };

    let outcome = front.get("outcome").cloned().unwrap_or_default();
    evidence.push(Evidence::file(
        &subject.record,
        format!(
            "outcome: {}, closed_at: {}",
            if outcome.is_empty() { "absent" } else { &outcome },
            front.get("closed_at").map(String::as_str).unwrap_or("absent")
        ),
    ));
    for l in &closes {
        evidence.push(Evidence::ledger(
            l.number,
            format!("{} outcome={}", l.summary(), l.field("outcome")),
        ));
    }

    // The word itself. `interrupted` is not a failure to record — it is the recorded fact
    // that the episode ended without the close path running to the end, and the difference
    // between it and `closed` is the difference between "the worker finished" and "the
    // process went away", which no reader should have to guess at.
    let how = match outcome.as_str() {
        "closed" => "the close path ran and wrote this record itself".to_string(),
        "interrupted" => "the episode ended without the close path completing; the record was \
                          written for it rather than by it"
            .to_string(),
        "" => "the record declares no outcome, which no writer this repository ships does"
            .to_string(),
        other => format!("the record declares outcome '{other}'"),
    };

    if receipts.is_empty() {
        let mut e = evidence;
        e.push(Evidence::absent(
            format!("{STATE_DIR}/ledger.jsonl"),
            format!(
                "no line in the `provider.` namespace is stamped with session={}",
                subject.episode
            ),
        ));
        return Answer::unknown(
            Q,
            format!(
                "the episode is {outcome} — {how} — but *which provider event* closed it is not \
                 recorded. The ledger holds no receipt in the `provider.` namespace for it: \
                 either the close was run by hand, or the adapter closed it and left no \
                 receipt, and from here those are the same absence. \
                 The outcome above is known; the cause is not."
            ),
            e,
        );
    }

    let last = receipts[receipts.len() - 1];
    // The provider's own event name is the payload's `event`, which the parsed value holds
    // because it is the *second* key of that name on the line; see `LedgerLine`.
    let provider_event = last.field("event");
    let provider = last.field("provider");
    for l in &receipts {
        evidence.push(Evidence::ledger(
            l.number,
            format!(
                "{} provider={} provider_event={}",
                l.summary(),
                l.field("provider"),
                l.field("event")
            ),
        ));
    }
    Answer::known(
        Q,
        format!("{outcome}, on {provider} {provider_event}"),
        format!(
            "{how}. The last receipt stamped with this episode is {provider}'s {provider_event} \
             at ledger line {}; {} receipt(s) are stamped with it in total.",
            last.number,
            receipts.len()
        ),
        evidence,
    )
}

/// Which task this episode is linked to, and by what.
///
/// ADR 0041 makes a task an optional relation an episode carries, not the thing an episode
/// is. So `none` is a first-class answer here and is reported as one; it used to be the
/// condition under which the whole lifecycle silently did nothing.
fn q_episode_task(
    subject: &Subject,
    record: Option<&BTreeMap<String, String>>,
    lines: &[LedgerLine],
) -> Answer {
    const Q: &str = "episode.task";

    let declared = record
        .and_then(|f| f.get("task_id"))
        .cloned()
        .filter(|t| !t.is_empty() && t != "none");

    // Lines stamped with this episode that name a task. The strongest evidence there is:
    // a task the episode actually acted on, at a line number.
    let mut from_ledger: Vec<(&LedgerLine, String)> = Vec::new();
    for l in lines.iter().filter(|l| l.episode() == subject.episode) {
        let t = {
            let a = l.field("task_id");
            if a.is_empty() {
                l.field("task")
            } else {
                a
            }
        };
        if !t.is_empty() && t != "none" {
            from_ledger.push((l, t));
        }
    }

    let mut evidence = Vec::new();
    if let Some(task) = &declared {
        evidence.push(Evidence::file(
            &subject.record,
            format!("task_id: {task}"),
        ));
    }
    for (l, t) in &from_ledger {
        evidence.push(Evidence::ledger(
            l.number,
            format!("{} task={t}", l.summary()),
        ));
    }

    match (&declared, from_ledger.is_empty()) {
        (Some(task), _) => Answer::known(
            Q,
            task,
            format!(
                "the episode's own record declares it, and {} ledger line(s) stamped with this \
                 episode name a task.",
                from_ledger.len()
            ),
            evidence,
        ),
        (None, false) => {
            // The record says nothing and the ledger says something. Report the ledger's
            // answer as the ledger's, not as the record's.
            let task = from_ledger[from_ledger.len() - 1].1.clone();
            Answer::known(
                Q,
                &task,
                format!(
                    "the episode's record declares no task_id, but {} ledger line(s) stamped \
                     with this episode name one; the most recent names {task}. This is the \
                     ledger's answer, not the record's — they can disagree, and here the \
                     record is simply silent.",
                    from_ledger.len()
                ),
                evidence,
            )
        }
        (None, true) => {
            evidence.push(Evidence::absent(
                &subject.record,
                "no task_id, and no ledger line stamped with this episode names a task",
            ));
            Answer::known(
                Q,
                "none",
                "the episode carries no task. That is a legal state and not a gap: ADR 0041 \
                 makes a task an optional relation of an episode, and making the lifecycle \
                 depend on one is what silenced this subsystem for six days."
                    .to_string(),
                evidence,
            )
        }
    }
}

/// Which prompt records belong to this episode.
///
/// Two joins, and they are not equally good. A prompt record written since the archive
/// learned about episodes carries `episode` outright, and that is the answer. An older one
/// carries only the provider's own session identity, and joining it to an episode by that is
/// an inference — a correct one, since the open-episode file is *keyed* by provider session,
/// but an inference, and it is labelled as one. Older still are the records that carry
/// neither, marked `unlinked-legacy` by the archive itself; measured in this repository on
/// 2026-09-11, 379 of 1014 prompt records were in that state and a further 493 predate the
/// field entirely. Those cannot be joined to anything, and the answer says so with the count
/// rather than quietly leaving them out.
fn q_episode_prompts(root: &Path, subject: &Subject) -> Answer {
    const Q: &str = "episode.prompts";
    const REL: &str = ".ai/local/prompts";
    let dir = root.join(REL);

    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Answer::unknown(
            Q,
            format!(
                "there is no prompt archive at {REL} in this checkout, so no prompt record can \
                 be joined to any episode. Capture has never run here, or its output lives \
                 elsewhere; this is not evidence that the episode had no prompts."
            ),
            vec![Evidence::absent(REL, "the directory does not exist")],
        );
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();

    let mut by_episode: Vec<String> = Vec::new();
    let mut by_provider_session: Vec<String> = Vec::new();
    let mut unjoinable = 0usize;
    let mut unreadable = 0usize;
    let total = paths.len();

    for path in &paths {
        let Ok(text) = std::fs::read_to_string(path) else {
            unreadable += 1;
            continue;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            unreadable += 1;
            continue;
        };
        let s = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let episode = s("episode");
        if !episode.is_empty() {
            if episode == subject.episode {
                by_episode.push(rel);
            }
            continue;
        }
        // No recorded episode. The provider session is the only remaining hook, and it only
        // reaches an episode whose record names the same provider session.
        let session = s("session");
        if !subject.provider_session.is_empty() && session == subject.provider_session {
            by_provider_session.push(rel);
        } else if session.is_empty() {
            unjoinable += 1;
        }
    }

    let mut evidence = Vec::new();
    // A bounded sample: the point is that a reader can open one and check, not that the
    // answer carries a thousand paths through three transports.
    for rel in by_episode.iter().take(3) {
        evidence.push(Evidence::file(rel, "episode recorded on the record itself"));
    }
    for rel in by_provider_session.iter().take(3) {
        evidence.push(Evidence::file(
            rel,
            format!(
                "no episode field; joined by session == {}, the provider session this \
                 episode's record names",
                subject.provider_session
            ),
        ));
    }
    evidence.push(Evidence::file(
        REL,
        format!(
            "{total} prompt record(s) in the archive; {} unreadable",
            unreadable
        ),
    ));
    if unjoinable > 0 {
        evidence.push(Evidence::absent(
            REL,
            format!(
                "{unjoinable} record(s) carry neither an episode nor a provider session and \
                 cannot be joined to any episode, this one included"
            ),
        ));
    }

    if subject.episode.is_empty() {
        return Answer::unknown(
            Q,
            "there is no episode to join prompt records to; see episode.current.".to_string(),
            evidence,
        );
    }

    let n = by_episode.len() + by_provider_session.len();
    let mut reason = format!(
        "{} record(s) name this episode outright; {} more carry no episode and were joined by \
         the provider session {} that this episode's record names, which is an inference from \
         the archive's key rather than something the record states.",
        by_episode.len(),
        by_provider_session.len(),
        if subject.provider_session.is_empty() {
            "(none: this episode names no provider session, so that join was unavailable)"
        } else {
            &subject.provider_session
        }
    );
    if unjoinable > 0 {
        reason.push_str(&format!(
            " {unjoinable} record(s) in the archive can be joined to no episode at all; whether \
             any of them belongs to this one is not knowable from here."
        ));
    }
    Answer::known(Q, format!("{n}"), reason, evidence)
}

/// Which recovery action touched this episode.
///
/// Recovery is recognised by its shape rather than by an event name compiled into this file.
/// The envelope of a ledger line names the episode that *wrote* the line; a recovery closes
/// somebody else's episode, so it writes a line stamped with the recovering episode that
/// names the recovered one in its payload. That structure is the signature, and it holds for
/// any event a future version declares, without this reader having to learn its name.
fn q_episode_recovery(subject: &Subject, lines: &[LedgerLine]) -> Answer {
    const Q: &str = "episode.recovery";

    if subject.episode.is_empty() {
        return Answer::unknown(
            Q,
            "there is no episode to look for recovery actions against; see episode.current."
                .to_string(),
            vec![Evidence::absent(
                format!("{STATE_DIR}/ledger.jsonl"),
                "no episode was resolved, so no line can be matched against one",
            )],
        );
    }

    let hits: Vec<&LedgerLine> = lines
        .iter()
        .filter(|l| {
            // named in the payload as the subject of the line...
            let named = l.field("session_id") == subject.episode;
            // ...by an envelope that is somebody else. A line an episode wrote about itself
            // is its own history, not a recovery of it.
            named && l.episode() != subject.episode
        })
        .collect();

    if hits.is_empty() {
        return Answer::known(
            Q,
            "none",
            format!(
                "no ledger line names episode {} from outside it. Recovery writes a line \
                 stamped with the recovering episode that names the recovered one in its \
                 payload; no line of that shape exists here.",
                subject.episode
            ),
            vec![Evidence::absent(
                format!("{STATE_DIR}/ledger.jsonl"),
                format!(
                    "{} line(s) read; none names session_id={} under a different envelope stamp",
                    lines.len(),
                    subject.episode
                ),
            )],
        );
    }

    let evidence: Vec<Evidence> = hits
        .iter()
        .map(|l| {
            Evidence::ledger(
                l.number,
                format!(
                    "{} names session_id={} — reason: {}",
                    l.summary(),
                    l.field("session_id"),
                    if l.field("reason").is_empty() {
                        "(the line records none)".to_string()
                    } else {
                        l.field("reason")
                    }
                ),
            )
        })
        .collect();
    let last = hits[hits.len() - 1];
    Answer::known(
        Q,
        format!("{} ({})", last.event, hits.len()),
        format!(
            "{} line(s) name this episode from another episode's envelope; the most recent is \
             `{}` at line {}, written by episode {}.",
            hits.len(),
            last.event,
            last.number,
            if last.episode().is_empty() {
                "(unstamped)".to_string()
            } else {
                last.episode()
            }
        ),
        evidence,
    )
}

/// Why this record was selected, and what the rule passed over.
///
/// The trace comes from [`resolve`] itself — the same pass that chose the record — so this
/// function decides nothing. It reports.
fn q_selected(kind: &str, record: Option<&Record>, trace: Vec<Candidate>, dir_rel: &str) -> Answer {
    let q = format!("{kind}.selected");
    match record {
        Some(r) => {
            let selected = trace
                .iter()
                .find(|c| c.outcome == Standing::Selected)
                .map(|c| c.reason.clone())
                .unwrap_or_else(|| "the resolver chose it".to_string());
            Answer::known(
                &q,
                &r.path,
                format!(
                    "{selected}. {} other file(s) in {dir_rel} were weighed and are listed \
                     below with the reason each is not the answer.",
                    trace.len().saturating_sub(1)
                ),
                vec![
                    Evidence::file(
                        &r.path,
                        format!(
                            "created_at: {}, branch: {}, head: {}, matched tier: {}",
                            r.created_at,
                            r.branch,
                            &r.head[..7.min(r.head.len())],
                            match r.matched {
                                Match::SameWorktreeSameBranch => "same worktree, same branch",
                                Match::SameBranch => "same branch, another worktree",
                            }
                        ),
                    ),
                ],
            )
            .weighing(trace)
        }
        None => Answer::known(
            &q,
            "none",
            format!(
                "no file in {dir_rel} matches this worktree and this branch, and the rule has \
                 no third tier: a record from an unrelated branch is never offered, because a \
                 briefing that is quietly about somebody else's work is worse than none. \
                 {} file(s) were looked at and rejected.",
                trace.len()
            ),
            vec![Evidence::absent(
                dir_rel,
                format!("{} file(s), none matching either tier", trace.len()),
            )],
        )
        .weighing(trace),
    }
}

/// Why this record is fresh, aging or stale — with the threshold it was judged against and
/// the file that declares it.
///
/// The verdict, the age and the sentence are [`Thresholds::judge`]'s, computed once in
/// [`resolve`] and carried on the [`Record`]. Nothing is recomputed here; recomputing would
/// be a second engine for a number the policy owns, which is the drift `session.freshness`
/// exists in one place to prevent.
fn q_freshness(
    kind: &str,
    record: Option<&Record>,
    thresholds: Thresholds,
    policy_path: &str,
) -> Answer {
    let q = format!("{kind}.freshness");

    let mut evidence = Vec::new();
    match (thresholds.fresh_minutes, thresholds.stale_minutes) {
        (Some(f), Some(s)) => {
            evidence.push(Evidence::policy(
                policy_path,
                "session.freshness.fresh_minutes",
                format!("{f} minutes ({})", span(f)),
            ));
            evidence.push(Evidence::policy(
                policy_path,
                "session.freshness.stale_minutes",
                format!("{s} minutes ({})", span(s)),
            ));
        }
        _ => evidence.push(Evidence::absent(
            policy_path,
            "session.freshness declares no fresh_minutes/stale_minutes",
        )),
    }

    let Some(r) = record else {
        return Answer::known(
            &q,
            "none",
            format!("there is no {kind} to judge; see {kind}.selected."),
            evidence,
        );
    };

    evidence.push(Evidence::file(
        &r.path,
        format!("created_at: {}", r.created_at),
    ));

    if r.freshness == Freshness::Unknown {
        return Answer::unknown(
            &q,
            format!(
                "the record's age cannot be judged: {}. It is not thereby old — an unjudgeable \
                 record is unjudged, and reporting it as fresh would be the invention this \
                 answer exists to refuse.",
                r.freshness_reason
            ),
            evidence,
        );
    }

    let mut reason = format!(
        "{}. Divergence and freshness are independent: this record is `{}` against git and \
         `{}` against the clock, and it is exactly that pair, uncollapsed, that a reader \
         needs — a record is routinely `advanced` (its commit an ancestor of HEAD, which \
         reads as agreement) and long dead at the same time.",
        r.freshness_reason,
        r.divergence.as_str(),
        r.freshness.as_str()
    );
    if !r.next_action_withheld.is_empty() {
        reason.push_str(&format!(" {}", r.next_action_withheld));
    }
    Answer::known(
        &q,
        format!(
            "{}{}",
            r.freshness.as_str(),
            r.age_minutes
                .map(|m| format!(", {} old", span(m)))
                .unwrap_or_default()
        ),
        reason,
        evidence,
    )
}

/// The handler: every question, or the ones under a prefix.
fn explain(ctx: &Context, input: ExplainInput) -> Result<SessionProvenance, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let dir = root.join(STATE_DIR);

    let (subject, record) = subject_of(ctx, &dir, input.episode.as_deref())?;

    let (branch, head) = match &ctx.index.repository.git {
        GitState::Available(info) => (
            info.branch.clone().unwrap_or_else(|| "DETACHED".into()),
            info.head.clone(),
        ),
        GitState::Unavailable { .. } => ("DETACHED".into(), None),
    };

    // One reading of the policy and one reading of the clock for the whole answer, so that
    // two questions in one response cannot be judged against different instants.
    let (thresholds, policy_path) = thresholds_and_path(ctx);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let (lines, unreadable) = ledger(&dir.join("ledger.jsonl"));

    let (handover, _, htrace) = resolve(
        &root,
        &dir.join("handovers"),
        &branch,
        head.as_deref(),
        thresholds,
        now,
    );
    let (checkpoint, _, ctrace) = resolve(
        &root,
        &dir.join("checkpoints"),
        &branch,
        head.as_deref(),
        thresholds,
        now,
    );

    let closed = if subject.state == "closed" {
        record.clone()
    } else {
        None
    };

    let mut answers = vec![
        q_episode_current(&root, &dir, &subject),
        q_episode_close(&subject, &lines, closed.as_ref()),
        q_episode_task(&subject, record.as_ref(), &lines),
        q_episode_prompts(&root, &subject),
        q_episode_recovery(&subject, &lines),
        q_selected(
            "handover",
            handover.as_ref(),
            htrace,
            &format!("{STATE_DIR}/handovers"),
        ),
        q_freshness("handover", handover.as_ref(), thresholds, &policy_path),
        q_selected(
            "checkpoint",
            checkpoint.as_ref(),
            ctrace,
            &format!("{STATE_DIR}/checkpoints"),
        ),
        q_freshness("checkpoint", checkpoint.as_ref(), thresholds, &policy_path),
    ];

    // A ledger that has grown a line nobody can parse degrades every answer drawn from it,
    // so it is said once, here, rather than left for a reader to notice that the counts do
    // not add up.
    if unreadable > 0 {
        for a in answers.iter_mut() {
            if a.question.starts_with("episode.") {
                a.reason.push_str(&format!(
                    " {unreadable} ledger line(s) are not JSON and were not read; anything \
                     drawn from the ledger is that much incomplete."
                ));
            }
        }
    }

    if let Some(want) = &input.question {
        let want = want.trim();
        let selected: Vec<Answer> = answers
            .iter()
            .filter(|a| a.question == *want || a.question.starts_with(&format!("{want}.")))
            .cloned()
            .collect();
        if selected.is_empty() {
            return Err(CapabilityError::NotFound(format!(
                "no question named '{want}'; `continuity.explain` with no question answers every \
                 one of: {}",
                answers
                    .iter()
                    .map(|a| a.question.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        answers = selected;
    }

    Ok(SessionProvenance {
        schema: "majordomus/session-provenance/v1".into(),
        subject,
        answers,
    })
}

/// The freshness thresholds and the repository-relative path of the file that declares them.
///
/// A wrapper over the same load [`thresholds_of`] does, kept beside it rather than folded
/// into it because only the provenance answer needs the path, and a value that carries where
/// it came from is the whole point here: "stale past 48h" is an assertion, and
/// "`.ai/repo/policy.yaml#session.freshness.stale_minutes` says 2880" is a fact.
fn thresholds_and_path(ctx: &Context) -> (Thresholds, String) {
    let root = PathBuf::from(&ctx.index.repository.root);
    let fallback = ".ai/repo/policy.yaml".to_string();
    let Ok(repo) = crate::repository::Repository::open(&root) else {
        return (Thresholds::default(), fallback);
    };
    let Ok(loaded) = crate::policy::LoadedPolicy::load(&repo) else {
        return (Thresholds::default(), fallback);
    };
    (
        Thresholds {
            fresh_minutes: loaded.policy.session.freshness.fresh_minutes,
            stale_minutes: loaded.policy.session.freshness.stale_minutes,
        },
        loaded.path.clone(),
    )
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "continuity",
        title: "Continuity",
        description: "What this checkout's lifecycle is holding: the open episode, the record the next worker would resume from with the label that says how far to trust it, the newest progress note, and what is blocking acceptance. Read from the local half of the layer, which this process serves to the worker in front of it and never publishes.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "continuity.state",
                title: "What the lifecycle is holding",
                description: "The open episode, the active task, the handover and checkpoint that resolve for this worktree and branch, each with its divergence label, the unresolved questions that refuse completion, and the record tallies. Selection is two-tiered and never repository-wide: a record from an unrelated worktree or branch is not offered, because a briefing that is quietly about somebody else is worse than none. Absence is reported as absence.",
                input: Empty,
                output: Continuity,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_continuity".into()),
                        resource: Some(McpResource { uri: CONTINUITY_URI.into(), name: "continuity".into() }),
                    }),
                    http: get("/api/v1/continuity"),
                    cli: None,
                },
                tags: ["continuity", "session", "handover"],
                // Short-lived: the records under it are written by another process, and a
                // reader that cached them for a minute would answer with an episode that
                // had already closed.
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: state,
            },
            capability! {
                id: "continuity.explain",
                title: "Why the lifecycle is saying what it is saying",
                description: "The provenance of one episode: which pointer resolved it and which open episodes were passed over, which resolution tier selected the handover and why every other candidate lost, how old a record is against the threshold the policy declares, which provider event closed the episode and whether the close was clean, which task it carries, which prompt records join to it, and which recovery action touched it. Every answer cites a file, a ledger line or a policy key, and an answer this process cannot establish is reported as unknown with the reason rather than guessed.",
                input: ExplainInput,
                output: SessionProvenance,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_continuity_explain"),
                    http: get("/api/v1/continuity/explain"),
                    // No command line, for the reason every capability over the local half
                    // has none: a command line is how a value reaches a script, a log and
                    // eventually a commit, and everything under `.ai/local/` names this
                    // machine. A worktree path is a fact about a disk, not about the
                    // repository (ADR 0014). This answer is for the worker sitting in front
                    // of the checkout, over MCP and the loopback server.
                    cli: None,
                },
                tags: ["continuity", "session", "provenance", "introspection"],
                // Not cached. It reads the clock — a record's age is most of what it
                // answers — and the executor holds that a cached capability answers the
                // same value warm as cold. A cache that may change the answer is a cache
                // that lies, and in the one capability whose product is being checkable
                // that is worse than in any other.
                cache: CachePolicy::Disabled,
                handler: explain,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thresholds, both sides of each and exactly on it. "At or beyond" and "beyond"
    /// are different contracts, and prose cannot be trusted to say which one is implemented.
    #[test]
    fn freshness_is_decided_at_the_thresholds_the_policy_declares() {
        let t = Thresholds {
            fresh_minutes: Some(720),
            stale_minutes: Some(2880),
        };
        let now = epoch_seconds("2026-06-15T12:00:00Z").expect("a parseable instant");
        let at = |minutes: i64| {
            let secs = now - minutes * 60;
            // Only the verdict is under test; the timestamp is built by the same arithmetic
            // the judgement uses, so a bug in one cancels in the other. Hence the round
            // trip through the formatter this crate already proves elsewhere.
            crate::peers::rfc3339(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64),
            )
        };
        let verdict = |minutes: i64| t.judge(&at(minutes), now).0;

        assert_eq!(verdict(1), Freshness::Fresh);
        assert_eq!(verdict(719), Freshness::Fresh);
        assert_eq!(verdict(720), Freshness::Aging);
        assert_eq!(verdict(721), Freshness::Aging);
        assert_eq!(verdict(2879), Freshness::Aging);
        assert_eq!(verdict(2880), Freshness::Stale);
        assert_eq!(verdict(2881), Freshness::Stale);
        // the 2026-09-05 outage, to the day
        assert_eq!(verdict(6 * 24 * 60), Freshness::Stale);
    }

    /// A verdict a reader cannot act on is the failure this subsystem is being corrected
    /// for, so every one of them carries a reason.
    #[test]
    fn every_verdict_says_why() {
        let t = Thresholds {
            fresh_minutes: Some(720),
            stale_minutes: Some(2880),
        };
        let now = epoch_seconds("2026-06-15T12:00:00Z").expect("a parseable instant");
        for ts in [
            "2026-06-15T11:59:00Z",
            "2026-06-14T00:00:00Z",
            "2026-06-01T00:00:00Z",
            "2026-06-15T13:00:00Z",
            "not a timestamp",
            "",
        ] {
            let (_, _, why) = t.judge(ts, now);
            assert!(!why.is_empty(), "no reason given for {ts:?}");
        }
    }

    /// The three answers that are not an age. They are distinct on purpose: a record with
    /// no timestamp is not thereby old, one dated in the future is evidence that something
    /// wrote it wrongly, and a policy with no thresholds is a repository that has not said
    /// what it means by stale — none of which is a default this file may invent.
    #[test]
    fn what_cannot_be_judged_is_reported_as_itself() {
        let t = Thresholds {
            fresh_minutes: Some(720),
            stale_minutes: Some(2880),
        };
        let now = epoch_seconds("2026-06-15T12:00:00Z").expect("a parseable instant");

        assert_eq!(t.judge("", now).0, Freshness::Unknown);
        assert_eq!(t.judge("not a timestamp", now).0, Freshness::Invalid);

        let (f, _, why) = t.judge("2026-06-15T13:00:00Z", now);
        assert_eq!(f, Freshness::Invalid);
        assert!(why.contains("future"), "a future timestamp did not say so: {why}");

        let none = Thresholds::default();
        let (f, age, why) = none.judge("2026-06-01T00:00:00Z", now);
        assert_eq!(f, Freshness::Unknown);
        // 2026-06-01T00:00:00Z to 2026-06-15T12:00:00Z: 14 days and 12 hours.
        assert_eq!(
            age,
            Some(14 * 1440 + 720),
            "the age is known even when the verdict is not"
        );
        assert!(why.contains("session.freshness"), "the missing key is not named: {why}");
    }

    /// `stale` and `invalid` are the two a reader must refuse to present as current.
    /// `unknown` is not one of them.
    #[test]
    fn only_stale_and_invalid_are_history() {
        assert!(Freshness::Stale.history());
        assert!(Freshness::Invalid.history());
        assert!(!Freshness::Fresh.history());
        assert!(!Freshness::Aging.history());
        assert!(!Freshness::Unknown.history());
    }

    /// The parser is the inverse of the formatter beside it, over instants this layer
    /// actually writes, and rejects what is not one rather than guessing.
    #[test]
    fn the_parser_inverts_the_formatter() {
        for secs in [0i64, 1_788_000_000, 1_757_000_000, 253_370_764_800] {
            let text = crate::peers::rfc3339(
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64),
            );
            assert_eq!(epoch_seconds(&text), Some(secs), "round trip failed for {text}");
        }
        for bad in [
            "",
            "2026-09-05",
            "2026-09-05T12:34:56",
            "2026-09-05T12:34:56+01:00",
            "2026-13-05T12:34:56Z",
            "2026-09-32T12:34:56Z",
            "2026-09-05T24:34:56Z",
            "xxxx-09-05T12:34:56Z",
        ] {
            assert_eq!(epoch_seconds(bad), None, "{bad:?} was accepted");
        }
    }

    /// The declaration is the only place the id, the tool name, the resource URI and the
    /// route exist. A refactor that dropped one of them would still compile, and every
    /// suite that tests the report itself would still pass; this is the assertion that
    /// would not — which is what `project.rust-command-tested-in-file` asks of a command.
    #[test]
    fn the_declaration_yields_the_identity_and_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "continuity");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["continuity.state", "continuity.explain"]);

        let c = &m.capabilities[0].capability;
        let mcp = c.exposure.mcp.as_ref().expect("an MCP projection");
        assert_eq!(mcp.tool.as_deref(), Some("majordomus_continuity"));
        assert_eq!(
            mcp.resource.as_ref().map(|r| r.uri.as_str()),
            Some(CONTINUITY_URI)
        );
        assert_eq!(
            c.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/continuity")
        );
        // read-only, and the registry refuses an executable capability that is not
        assert!(c.kind.is_read_only() && c.kind.is_executable());
        // and the module stamps its own namespace on what it composes
        assert!(ids.iter().all(|id| id.starts_with("continuity.")));

        let e = &m.capabilities[1].capability;
        assert_eq!(e.id.as_str(), "continuity.explain");
        assert_eq!(
            e.exposure
                .mcp
                .as_ref()
                .and_then(|m| m.tool.as_deref()),
            Some("majordomus_continuity_explain")
        );
        assert_eq!(
            e.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/continuity/explain")
        );
        // No command line, like every capability over the local half: `.ai/local/` names
        // this machine, and a command line is how a value reaches a script and a commit.
        assert!(e.exposure.cli.is_none());
        // And no cache: the answer reads the clock, and a cache a caller can observe in a
        // capability whose whole product is being checkable is worse than useless.
        assert!(!e.cache.is_enabled());
    }

    // ---------------------------------------------------------------- provenance

    /// A repository with a state directory, and a handful of records in it.
    fn with_state(records: &[(&str, &str)]) -> crate::synthetic::SyntheticRepository {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        for (rel, body) in records {
            let p = repo.root().join(rel);
            std::fs::create_dir_all(p.parent().expect("a parent")).expect("mkdir");
            std::fs::write(p, body).expect("write");
        }
        repo
    }

    /// A well-formed handover record for the synthetic repository, which has no git and so
    /// reports its branch as DETACHED.
    fn record_at(root: &Path, created: &str) -> String {
        format!(
            "---\nschema_version: 1\ncreated_at: {created}\nhead: 0000000000000000000000000000000000000000\nbranch: DETACHED\nworktree: {}\nworking_tree: clean\n---\n\n# Next Action\n\nfinish the thing\n",
            root.display()
        )
    }

    /// The reasons were always computed; this is the assertion that they now leave the
    /// function. One selected, one superseded by its timestamp, one rejected for not being
    /// a record at all — and every entry carries the field that decided it.
    #[test]
    fn the_resolution_rule_reports_why_each_candidate_lost() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let root = repo.root().to_path_buf();
        let dir = root.join(STATE_DIR).join("handovers");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("a.md"), record_at(&root, "2026-09-05T03:50:02Z")).expect("a");
        std::fs::write(dir.join("b.md"), record_at(&root, "2026-09-11T09:00:00Z")).expect("b");
        std::fs::write(dir.join("c.md"), "no front matter at all\n").expect("c");

        let (record, skipped, trace) = resolve(
            &root,
            &dir,
            "DETACHED",
            Some("0000000000000000000000000000000000000000"),
            Thresholds::default(),
            0,
        );
        assert_eq!(skipped, 1, "the file with no front matter is skipped");
        assert!(record.expect("a record").path.ends_with("b.md"));
        assert_eq!(trace.len(), 3, "every .md file the directory held is accounted for");

        let by = |suffix: &str| {
            trace
                .iter()
                .find(|c| c.path.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is in the trace"))
                .clone()
        };
        assert_eq!(by("b.md").outcome, Standing::Selected);
        assert_eq!(by("a.md").outcome, Standing::Superseded);
        assert!(
            by("a.md").reason.contains("2026-09-05T03:50:02Z")
                && by("a.md").reason.contains("2026-09-11T09:00:00Z"),
            "a superseded record names both timestamps, so the tie-break can be checked: {}",
            by("a.md").reason
        );
        assert_eq!(by("c.md").outcome, Standing::Rejected);
        assert!(by("c.md").reason.contains("front matter"));
        // and no reason is ever empty, whatever the outcome
        assert!(trace.iter().all(|c| !c.reason.is_empty()));
    }

    /// A record on another branch is refused rather than widened to, and the refusal says
    /// which branch it was on. This is the tier rule's one deliberate rejection, and a
    /// reader must be able to tell it from a malformed file.
    #[test]
    fn a_record_from_another_branch_is_rejected_with_the_branch_named() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let root = repo.root().to_path_buf();
        let dir = root.join(STATE_DIR).join("handovers");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let foreign = record_at(&root, "2026-09-11T09:00:00Z").replace("DETACHED", "feature/other");
        std::fs::write(dir.join("x.md"), foreign).expect("x");

        let (record, skipped, trace) =
            resolve(&root, &dir, "DETACHED", None, Thresholds::default(), 0);
        assert!(record.is_none(), "no tier reaches another branch");
        assert_eq!(skipped, 0, "it is a valid record; it is simply not yours");
        assert_eq!(trace[0].outcome, Standing::Rejected);
        assert!(
            trace[0].reason.contains("feature/other"),
            "the rejection names the branch: {}",
            trace[0].reason
        );
    }

    /// The invariant the whole module is held to: no sentence without something to check it
    /// against, and no verdict without a reason.
    #[test]
    fn every_answer_carries_a_reason_and_something_to_check() {
        let repo = with_state(&[]);
        let ctx = repo.context().expect("a context");
        let out = explain(&ctx, ExplainInput::default()).expect("an explanation");
        assert!(!out.answers.is_empty());
        for a in &out.answers {
            assert!(!a.reason.is_empty(), "{} has no reason", a.question);
            assert!(
                !a.evidence.is_empty(),
                "{} cites nothing a reader could check",
                a.question
            );
            assert!(
                a.known != a.answer.is_empty() || !a.known,
                "{} claims to be known and answers nothing",
                a.question
            );
            if !a.known {
                assert!(
                    a.answer.is_empty(),
                    "{} is not known and yet answers something",
                    a.question
                );
            }
        }
    }

    /// A checkout with no pointer is not an error and is not an empty answer: it is an
    /// unknown that names what it looked for. The whole standard in one assertion.
    #[test]
    fn a_checkout_with_no_open_episode_says_so_rather_than_inventing_one() {
        let repo = with_state(&[]);
        let ctx = repo.context().expect("a context");
        let out = explain(&ctx, ExplainInput::default()).expect("an explanation");
        let current = out
            .answers
            .iter()
            .find(|a| a.question == "episode.current")
            .expect("the question is always asked");
        assert!(!current.known);
        assert!(current.answer.is_empty());
        assert!(current.reason.contains("points at no episode"));
        assert_eq!(current.evidence[0].kind, EvidenceKind::Absent);
        assert!(current.evidence[0]
            .locator
            .ends_with("session-current.yaml"));
    }

    /// A record judged stale must name the threshold it crossed *and* the file that
    /// declares it, because "stale past 48h" is an assertion and
    /// `.ai/repo/policy.yaml#session.freshness.stale_minutes says 2880` is a fact.
    ///
    /// The two judgements must also stay apart in the sentence. The record below is
    /// `advanced` against git and `stale` against the clock at the same time, and it was
    /// exactly that pair — collapsed into "trustworthy" — that handed six days of workers
    /// a finished instruction.
    #[test]
    fn the_verdict_names_the_threshold_it_crossed_and_the_file_that_declares_it() {
        let thresholds = Thresholds {
            fresh_minutes: Some(720),
            stale_minutes: Some(2880),
        };
        // 2026-09-11T12:00:00Z, six days after the record: the outage, to the day.
        let now = epoch_seconds("2026-09-11T12:00:00Z").expect("a parseable instant");
        let created = "2026-09-05T03:50:02Z";
        let (freshness, age, why) = thresholds.judge(created, now);
        assert_eq!(freshness, Freshness::Stale, "six days is past any 48h threshold");

        let r = Record {
            path: ".ai/local/state/handovers/20260905T035003Z--stale.md".into(),
            created_at: created.into(),
            task_id: "none".into(),
            branch: "master".into(),
            head: "2bba3cf0000000000000000000000000000000".into(),
            working_tree: "clean".into(),
            matched: Match::SameWorktreeSameBranch,
            // The word that made the outage possible: true about git, silent about age.
            divergence: Divergence::Advanced,
            freshness,
            age_minutes: age,
            freshness_reason: why,
            next_action: String::new(),
            next_action_withheld: "the record is stale; it is history, not an instruction".into(),
        };
        let a = q_freshness("handover", Some(&r), thresholds, ".ai/repo/policy.yaml");

        assert!(a.known);
        assert!(
            a.answer.starts_with("stale"),
            "a record from 2026-09-05 judged six days later is stale, not {}",
            a.answer
        );
        let keys: Vec<&str> = a.evidence.iter().map(|e| e.locator.as_str()).collect();
        assert!(
            keys.iter()
                .any(|k| k.contains("session.freshness.stale_minutes")),
            "the verdict did not name the threshold it crossed: {keys:?}"
        );
        assert!(
            keys.iter().any(|k| k.contains("policy.yaml")),
            "the threshold was not traced to the file that declares it: {keys:?}"
        );
        assert!(
            a.evidence.iter().any(|e| e.kind == EvidenceKind::PolicyKey),
            "a threshold cited as anything but a policy key is a number from nowhere"
        );
        // Both judgements, separately, in the sentence a reader acts on.
        assert!(
            a.reason.contains("advanced") && a.reason.contains("stale"),
            "the two judgements were collapsed into one: {}",
            a.reason
        );
        // And the reason says what was withheld and why, rather than silently emptying it.
        assert!(a.reason.contains("history, not an instruction"), "{}", a.reason);
    }

    /// End to end, through the handler, over a repository whose policy declares no
    /// thresholds: the record is still selected and the tier is still named — that half of
    /// the answer needs no policy — and the freshness half reports `unknown` naming the key
    /// it wanted. Two questions, two independent sources, and the missing one does not take
    /// the other down with it.
    #[test]
    fn a_selected_record_is_explained_even_when_its_age_cannot_be_judged() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        // The record's `worktree` is matched against the root the *index* holds, which on
        // macOS is the resolved path (`/private/var/...`) while the temporary directory
        // hands out the symlinked one (`/var/...`). Writing the record against
        // `repo.root()` produced a record that matched no tier and a test that failed for
        // a reason that had nothing to do with freshness.
        let root = PathBuf::from(&ctx.index.repository.root);
        let dir = root.join(STATE_DIR).join("handovers");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(dir.join("a.md"), record_at(&root, "2026-09-05T03:50:02Z")).expect("a");

        let out = explain(
            &ctx,
            ExplainInput {
                episode: None,
                question: Some("handover".into()),
            },
        )
        .expect("an explanation");
        let by = |q: &str| {
            out.answers
                .iter()
                .find(|a| a.question == q)
                .unwrap_or_else(|| panic!("{q} is answered"))
        };

        let sel = by("handover.selected");
        assert!(sel.known, "the selection needs no policy to be explained");
        assert!(sel.answer.ends_with("a.md"), "{}", sel.answer);
        assert!(
            sel.reason.contains("tier 0"),
            "the selection did not say which tier matched: {}",
            sel.reason
        );

        let fr = by("handover.freshness");
        assert!(!fr.known, "a repository with no thresholds cannot judge age");
        assert!(fr.answer.is_empty(), "and so it answers nothing");
        assert!(
            fr.reason.contains("session.freshness"),
            "it did not name the key it wanted: {}",
            fr.reason
        );
    }

    /// And when the policy genuinely declares nothing, the answer is `unknown` naming the
    /// missing key — never a default, and never `fresh`. A threshold nobody declared is not
    /// a threshold, and judging against one would be the invention this module refuses.
    #[test]
    fn a_record_with_no_thresholds_to_judge_it_by_is_unjudged_rather_than_fresh() {
        let r = Record {
            path: "x.md".into(),
            created_at: "2026-09-05T03:50:02Z".into(),
            task_id: "none".into(),
            branch: "master".into(),
            head: "abc1234".into(),
            working_tree: "clean".into(),
            matched: Match::SameWorktreeSameBranch,
            divergence: Divergence::Advanced,
            freshness: Freshness::Unknown,
            age_minutes: None,
            freshness_reason: "the policy declares no session.freshness thresholds".into(),
            next_action: String::new(),
            next_action_withheld: String::new(),
        };
        let a = q_freshness(
            "handover",
            Some(&r),
            Thresholds::default(),
            ".ai/repo/policy.yaml",
        );
        assert!(!a.known, "an unjudgeable record is unjudged");
        assert!(a.answer.is_empty(), "and answers nothing at all");
        assert!(
            a.reason.contains("session.freshness"),
            "the reason names the key that is missing: {}",
            a.reason
        );
        assert_eq!(a.evidence[0].kind, EvidenceKind::Absent);
    }

    /// Narrowing, exactly as `environment.explain` narrows: one question, or a prefix.
    #[test]
    fn a_question_can_be_narrowed_by_name_or_by_prefix() {
        let repo = with_state(&[]);
        let ctx = repo.context().expect("a context");
        let all = explain(&ctx, ExplainInput::default()).expect("every question");
        let one = explain(
            &ctx,
            ExplainInput {
                episode: None,
                question: Some("handover.selected".into()),
            },
        )
        .expect("one question");
        assert_eq!(one.answers.len(), 1);
        let prefix = explain(
            &ctx,
            ExplainInput {
                episode: None,
                question: Some("handover".into()),
            },
        )
        .expect("a prefix");
        assert_eq!(prefix.answers.len(), 2);
        assert!(prefix.answers.iter().all(|a| a.question.starts_with("handover.")));
        assert!(prefix.answers.len() < all.answers.len());
    }

    #[test]
    fn a_question_that_does_not_exist_says_so_and_lists_the_ones_that_do() {
        let repo = with_state(&[]);
        let ctx = repo.context().expect("a context");
        match explain(
            &ctx,
            ExplainInput {
                episode: None,
                question: Some("nothing.like.this".into()),
            },
        ) {
            Err(CapabilityError::NotFound(m)) => {
                assert!(m.contains("nothing.like.this"));
                assert!(m.contains("episode.current"), "it lists what does exist: {m}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_episode_that_does_not_exist_says_so_rather_than_falling_back_to_the_current_one() {
        let repo = with_state(&[]);
        let ctx = repo.context().expect("a context");
        match explain(
            &ctx,
            ExplainInput {
                episode: Some("s-19700101000000-0000".into()),
                question: None,
            },
        ) {
            Err(CapabilityError::NotFound(m)) => assert!(m.contains("s-19700101000000-0000")),
            other => panic!("{other:?}"),
        }
    }

    /// `provider.event.received` declares a payload key named `event`, which is also the
    /// envelope's key, so such a ledger line carries `"event"` twice. `serde_json` keeps the
    /// last, which would read the line as an event called `SessionStart` — an id this
    /// repository does not declare. The envelope is read by position for exactly that
    /// reason, and this is the assertion that says so.
    #[test]
    fn the_envelope_event_is_read_past_a_duplicated_payload_key() {
        let line = r#"{"ts":"2026-09-11T09:00:00Z","event":"provider.event.received","head":"abc","branch":"master","by":"majordomus/0.5.0","session":"s-1","provider":"claude-code","event":"SessionStart"}"#;
        assert_eq!(envelope_event(line), "provider.event.received");
        let parsed: serde_json::Value = serde_json::from_str(line).expect("it is still JSON");
        assert_eq!(
            parsed.get("event").and_then(|v| v.as_str()),
            Some("SessionStart"),
            "the parsed value keeps the payload copy, which is what the provider called it"
        );
    }

    #[test]
    fn a_ledger_line_that_is_not_json_is_counted_and_the_rest_are_read() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let dir = repo.root().join(STATE_DIR);
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(
            dir.join("ledger.jsonl"),
            "{\"ts\":\"2026-09-11T09:00:00Z\",\"event\":\"session.started\",\"session\":\"s-1\"}\nnot json\n\n",
        )
        .expect("write");
        let (lines, unreadable) = ledger(&dir.join("ledger.jsonl"));
        assert_eq!(lines.len(), 1);
        assert_eq!(unreadable, 1);
        assert_eq!(lines[0].number, 1, "line numbers are one-based, as an editor counts");
        assert_eq!(lines[0].event, "session.started");
        assert_eq!(lines[0].episode(), "s-1");
    }
}
