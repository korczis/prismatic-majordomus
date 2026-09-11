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

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::git::{self, GitState};
use crate::metadata::frontmatter;
use crate::metadata::yaml;
use crate::{capability, module};

use super::{get, Empty};

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
pub(crate) fn document(path: &Path) -> Option<BTreeMap<String, String>> {
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
        assert_eq!(ids, ["continuity.state"]);

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
    }
}
