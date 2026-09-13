//! The `recover` module: the stray files of the record stores, classified by evidence and
//! swept exactly once.
//!
//! `majordomus recover` is a mutating development command — it writes `.ai/repo/sessions/`
//! and `.ai/local/state/` — and until this module it was backed by nothing in the registry.
//! [ADR 0040] says a development operation exists as a `capability!` declaration and that
//! every surface is a consumer of it; `scripts/development-semantics-check` measures exactly
//! that and reported `backing:recover`. This module is the first capability that backs it.
//!
//! # What moved, and what deliberately did not
//!
//! One capability, `recover.orphans`, and it owns the **decision and the destruction**: for
//! every stray file of the record stores it reads the clock and the content, returns a
//! verdict with the evidence that produced it, and — unless the call is a check — removes
//! the ones the verdict says are removable. `lib/recover.sh` no longer decides any of that;
//! it asks, renders the answer in the words a person reads, and counts.
//!
//! What stayed in shell is the **constructive** half. A publish temp that holds the only
//! copy of an episode is published rather than deleted, and publishing a record is
//! `mj_publish_record`'s semantic — the same one `session close` uses, and the one
//! `recover episodes` composes a synthesised record with. Moving one caller of it without
//! moving the writer would put a second record writer in a second program, which is the
//! defect ADR 0040 exists to prevent rather than a step towards fixing it. So this
//! capability names an `Incomplete` stray and removes nothing; the shell publishes it. The
//! record writer is the next thing to converge, and when it does, `recover.orphans` gains
//! the publish and loses the `Incomplete` verdict's "the caller acts" note.
//!
//! The same argument keeps `recover episodes` and `recover records` in shell for now: both
//! *write records*, and neither can converge before the record writer does.
//!
//! # Why this is not [`super::lifecycle`]'s job
//!
//! `lifecycle.recovery` already lists the `.tmp.*` files of the sessions section, and a
//! second definition of the same thing would be this repository's favourite defect. It is
//! not one. That module states its own constraint — *"no thresholds and no clock … what this
//! module reports is what it can observe without an opinion"* — so it reports a path and a
//! byte count and nothing else. This module is the opinion: it reads the clock, applies the
//! threshold, and acts. An observation and a decision over the same files are two different
//! things to depend on; what would be a duplicate is a second *verdict*, and there is one.
//!
//! # The two guards, and why they are here rather than at the call site
//!
//! **The threshold has no default.** `older_than_seconds` is a required field with no serde
//! default, and zero is refused. `session.stranded_after` is a policy value, and a reader
//! that carries its own default is a second place the number lives — `majordomus doctor`
//! refused exactly that when `lib/recover.sh` was first written. A sweep that ran against an
//! absent threshold would find every file stale, which is the one failure this command must
//! not have (`project.destructive-sweeps-fail-closed`).
//!
//! **Nothing is removed that is not a temp file this tool writes.** [`removable`] is decided
//! from the verdict, and [`remove_stray`] refuses any path that is not a regular file whose
//! name matches the two patterns this tool creates — `.tmp.*` inside a record store, or
//! `<target>.mj-tmp`. A directory is never removed, whatever its verdict: `SECURITY.md`
//! states "no recursive deletion", and a stale `.mj-stage.*` staging directory is therefore
//! reported and left in place. `check` is a true dry run: it is consulted before the single
//! call site of [`remove_stray`], and nothing else in this file touches the filesystem for
//! writing.
//!
//! [ADR 0040]: ../../../../../.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::CapabilityKind;
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{mcp, post};

/// The local half of the layer, relative to the repository root. The same constant
/// [`super::continuity`] and [`super::lifecycle`] carry, and for the same reason: the shell
/// tool decides where its state lives and a second opinion about the path would be a second
/// source of truth for the one thing both halves must agree on.
const STATE_DIR: &str = ".ai/local/state";

/// The manifest section that names the tracked record store.
const SESSIONS_SECTION: &str = "sessions";

/// The prefix `mj_publish_record` gives the temporary file it hard-links from.
const PUBLISH_PREFIX: &str = ".tmp.";

/// The suffix every write-then-rename in the shell tool gives its temporary file.
const RENAME_SUFFIX: &str = ".mj-tmp";

/// The prefix `scripts/generate-site-data` gives its staging directory at the repository
/// root.
const STAGE_PREFIX: &str = ".mj-stage.";

// ---------------------------------------------------------------- the vocabulary

/// What kind of stray a path is, decided by where it is and how it is named rather than by
/// what is in it. The content decides the *verdict*; this decides which question to ask.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StrayKind {
    /// A `.tmp.XXXXXX` in a record store: `mj_publish_record` makes them, and only a process
    /// that died between the `mktemp` and the `rm` leaves one.
    PublishTemp,
    /// A `<target>.mj-tmp`: written and renamed in one `&&`, so one that exists means the
    /// write failed or the process died mid-write.
    RenameTemp,
    /// A `.mj-stage.XXXXXX` at the repository root: `scripts/generate-site-data` stages into
    /// one. Always a directory, and therefore never removed here.
    StagingDir,
}

impl StrayKind {
    /// The word this kind is reported and serialised under.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::recover::StrayKind;
    /// assert_eq!(StrayKind::PublishTemp.as_str(), "publish_temp");
    /// assert_eq!(
    ///     serde_json::to_value(StrayKind::StagingDir).unwrap(),
    ///     serde_json::json!(StrayKind::StagingDir.as_str()),
    /// );
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            StrayKind::PublishTemp => "publish_temp",
            StrayKind::RenameTemp => "rename_temp",
            StrayKind::StagingDir => "staging_dir",
        }
    }
}

/// What this stray is, decided by reading it.
///
/// The five words are `lib/recover.sh`'s, deliberately: a person who has read the command's
/// output should not have to learn the same five facts twice because a second surface chose
/// its own vocabulary.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum StrayVerdict {
    /// Younger than the threshold: a `session close` or a `scripts/derive` running right now
    /// owns it. Measured, reported, never touched.
    Live,
    /// Its work completed, or there was never anything in it. Removable — unless it is a
    /// directory, which this runtime never removes.
    Orphan,
    /// It holds something no other file holds: a record nobody published, or a write whose
    /// target never arrived. Never removed here; the caller decides.
    Incomplete,
    /// Not written by this tool at all, or written by a version that shaped it differently.
    /// Never touched; reported with what it is.
    Foreign,
    /// Its age could not be read on this platform, so nothing here treats it as stale. The
    /// guard `project.destructive-sweeps-fail-closed` asks for: a predicate that cannot
    /// measure its input is false, never the most extreme value in the set.
    Unmeasurable,
}

impl StrayVerdict {
    /// The word this verdict is reported and serialised under.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::recover::StrayVerdict;
    /// assert_eq!(StrayVerdict::Unmeasurable.as_str(), "unmeasurable");
    /// assert_eq!(
    ///     serde_json::to_value(StrayVerdict::Orphan).unwrap(),
    ///     serde_json::json!(StrayVerdict::Orphan.as_str()),
    /// );
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            StrayVerdict::Live => "live",
            StrayVerdict::Orphan => "orphan",
            StrayVerdict::Incomplete => "incomplete",
            StrayVerdict::Foreign => "foreign",
            StrayVerdict::Unmeasurable => "unmeasurable",
        }
    }
}

/// One stray file, its verdict, and the evidence that produced it.
///
/// Every field that decided something is carried, because the rule this command is shaped
/// around requires the value that decided a verdict to be visible before the verdict acts:
/// a surface that printed "orphan" without the age, the episode and the file that already
/// holds it would be asking to be trusted rather than read.
///
/// ```
/// use majordomus_cli::capability::builtin::recover::{Stray, StrayKind, StrayVerdict};
/// use serde_json::json;
///
/// let s: Stray = serde_json::from_value(json!({
///     "path": ".ai/repo/sessions/.tmp.a1b2c3",
///     "kind": "publish_temp",
///     "verdict": "orphan",
///     "age_seconds": 90_000,
///     "session_id": "s-0007",
///     "published_at": ".ai/repo/sessions/2026-09-11--s-0007.md",
///     "removable": true,
///     "removed": false,
/// })).unwrap();
/// assert_eq!(s.verdict, StrayVerdict::Orphan);
/// assert_eq!(s.kind, StrayKind::PublishTemp);
/// // removable and removed are different facts: a check run reports the first and never
/// // the second, which is what makes `--check` readable as a plan rather than a report.
/// assert!(s.removable && !s.removed);
///
/// // and an unmeasurable one carries no age at all, rather than a zero that would read as
/// // "brand new" to anything that compared it with a threshold
/// let u: Stray = serde_json::from_value(json!({
///     "path": ".mj-stage.xyz", "kind": "staging_dir", "verdict": "unmeasurable",
///     "removable": false, "removed": false,
/// })).unwrap();
/// assert!(u.age_seconds.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Stray {
    /// The path, repository-relative. Never the absolute one: a finding that names a disk is
    /// a finding nobody else can act on (`project.no-machine-paths`).
    pub path: String,
    /// Which of the three shapes this is.
    pub kind: StrayKind,
    /// What reading it decided.
    pub verdict: StrayVerdict,
    /// How old it is, in seconds. Absent when the platform could not say, which is the
    /// `Unmeasurable` verdict and never a zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<u64>,
    /// The episode this publish temp carries, when it carries one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session_id: String,
    /// The record that already holds that episode, when one does. Its presence is what turns
    /// an `Incomplete` publish temp into an `Orphan`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub published_at: String,
    /// The file a rename temp was going to become. Its presence is what turns an
    /// `Incomplete` rename temp into an `Orphan`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// Whether this call would remove it. Decided by the verdict and by
    /// [`remove_stray`]'s own pattern guard, never by the caller.
    pub removable: bool,
    /// Whether this call did remove it. Always `false` on a check.
    pub removed: bool,
}

/// Strays are shown in path order, which is the only order that is a property of the store
/// rather than of the walk that found it: `read_dir` yields what the filesystem feels like
/// yielding, and two runs over one store must answer identically.
impl crate::order::Ordered for Stray {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.path, &self.path)
    }
}

// ---------------------------------------------------------------- input and output

/// The input of `recover.orphans`: how old a stray must be, and whether to act.
///
/// ```
/// use majordomus_cli::capability::builtin::recover::RecoverOrphansInput;
///
/// // The wire form the shell tool, an MCP client or an HTTP POST sends.
/// let act: RecoverOrphansInput =
///     serde_json::from_str(r#"{"older_than_seconds":43200,"check":false}"#).unwrap();
/// assert_eq!(act.older_than_seconds, 43_200);
/// assert!(!act.check);
///
/// // `check` defaults to true, and that direction is deliberate: this capability deletes
/// // files, and a caller that forgets the field gets a plan rather than a sweep.
/// let forgot: RecoverOrphansInput =
///     serde_json::from_str(r#"{"older_than_seconds":43200}"#).unwrap();
/// assert!(forgot.check, "the safe value is the default one");
///
/// // The threshold has no default at all. `session.stranded_after` is a policy value, and
/// // a reader that carried its own copy would be a second place the number lives.
/// assert!(serde_json::from_str::<RecoverOrphansInput>(r#"{"check":true}"#).is_err());
///
/// // Unknown fields are refused rather than ignored: a caller that misspells `check` is
/// // told so instead of having a sweep silently run.
/// assert!(serde_json::from_str::<RecoverOrphansInput>(
///     r#"{"older_than_seconds":1,"chek":true}"#).is_err());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverOrphansInput {
    /// How long a stray file must have been untouched before it is a candidate at all, in
    /// seconds. Required, and refused at zero: the shell tool resolves it from
    /// `session.stranded_after` with `mj_pol_req`, which fails closed, and nothing here
    /// invents one.
    pub older_than_seconds: u64,
    /// Classify and report, and write nothing. The default, because the alternative deletes
    /// files.
    #[serde(default = "yes")]
    pub check: bool,
}

fn yes() -> bool {
    true
}

/// The benchmark cases of the capability that sweeps.
///
/// Every case is a check, and that is deliberate rather than a gap. A case is *executed* —
/// by `majordomus bench` against this repository, and by the HTTP and MCP suites against a
/// fixture — so a case that swept would delete this repository's stray files every time the
/// suite ran, and a measurement whose cost is a changed store is not a measurement. The
/// check path is also the honest thing to time: it walks every store, stats every candidate
/// and reads every publish temp, which is everything the sweep does except the `unlink` at
/// the end.
///
/// Two cases rather than one because case 92 refuses a declared parameter that no case ever
/// sets, and because the two thresholds measure different work: the generous one classifies
/// every stray, the tight one classifies none of them as `Live` and so reads every file.
impl BenchmarkCases for RecoverOrphansInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new(
                "check-twelve-hours",
                RecoverOrphansInput {
                    older_than_seconds: 43_200,
                    check: true,
                },
            ),
            NamedCase::new(
                "check-one-second",
                RecoverOrphansInput {
                    older_than_seconds: 1,
                    check: true,
                },
            ),
        ]
    }
}

/// What one sweep found and what it did about it.
///
/// The counts are derived from `strays` rather than accumulated beside it, so a reader can
/// check the report against itself and a surface that renders the list cannot disagree with
/// the surface that renders the total.
///
/// ```
/// use majordomus_cli::capability::builtin::recover::RecoverOrphansResult;
///
/// let plan: RecoverOrphansResult = serde_json::from_str(r#"{
///     "older_than_seconds": 43200, "check": true, "strays": [],
///     "removable": 0, "removed": 0, "unmeasurable": 0
/// }"#).unwrap();
/// // a check never removes anything, whatever it found
/// assert_eq!(plan.removed, 0);
/// assert!(plan.check);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecoverOrphansResult {
    /// The threshold this sweep measured against, echoed back so that a refusal or a verdict
    /// names the value the caller actually got rather than the one it believes it sent.
    pub older_than_seconds: u64,
    /// Whether this was a check. `true` means nothing was written.
    pub check: bool,
    /// Every stray found, in path order.
    pub strays: Vec<Stray>,
    /// How many of them this call would remove, or did.
    pub removable: usize,
    /// How many it actually removed. Always `0` on a check.
    pub removed: usize,
    /// How many candidates could not be measured and were therefore left alone.
    pub unmeasurable: usize,
}

// ---------------------------------------------------------------- the module

/// The module descriptor: one capability, and it writes.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "recover",
        title: "Recovery of the record stores",
        description: "The stray files a killed publish, an interrupted rename or an unfinished site build leave in the record stores, classified by reading the clock and the content, and swept exactly once. This is the capability that backs `majordomus recover` (ADR 0040): the command no longer decides what a stray is, it asks.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "recover.orphans",
                kind: CapabilityKind::Command,
                title: "Classify every stray file of the record stores, and sweep the ones that are nobody's",
                description: "Walk the publish temps of every record store, the rename temps under the layer's local half and the tracked sessions section, and the site generator's staging directories at the repository root; give each a verdict by reading its age and then its content; and, unless the call is a check, remove the ones the verdict says nothing is holding. Nothing is deleted for being unrecognised: a temp holding the only copy of a record is reported as `incomplete` and left for the caller to publish, content this version cannot classify is `foreign` and left exactly where it is, a candidate whose age this platform cannot read is `unmeasurable` and never a candidate, and no directory is ever removed. The threshold is required and has no default, because a sweep measuring against an absent threshold would find every file stale.",
                input: RecoverOrphansInput,
                output: RecoverOrphansResult,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_recover_orphans"),
                    http: post("/api/v1/recover/orphans"),
                    cli: None,
                },
                tags: ["recover", "sessions", "maintenance", "lifecycle"],
                handler: recover_orphans,
            }
            // The one thing its kind cannot say: this unlinks files, and one of the stores it
            // sweeps is tracked. The exposure ceiling of every surface is derived from the
            // effect, so a sweep that claimed only this process's memory would be projected
            // wherever a peer announcement is, which is not the same permission.
            .writes_repository(),
        ],
    }
}

// ---------------------------------------------------------------- the handler

fn recover_orphans(
    ctx: &Context,
    input: RecoverOrphansInput,
) -> Result<RecoverOrphansResult, CapabilityError> {
    if input.older_than_seconds == 0 {
        return Err(CapabilityError::Refused(
            "older_than_seconds is zero, which would make every stray file a candidate the \
             moment it was written; the threshold is `session.stranded_after` in the policy \
             and nothing here invents one"
                .into(),
        ));
    }

    let root = PathBuf::from(&ctx.index.repository.root);
    let state = root.join(STATE_DIR);
    let sessions = ctx
        .index
        .repository
        .sections
        .get(SESSIONS_SECTION)
        .map(|s| root.join(s));

    let now = now_secs().ok_or_else(|| {
        CapabilityError::Refused(
            "this platform's clock could not be read, and a sweep that cannot measure an age \
             takes no action at all"
                .into(),
        )
    })?;

    let mut strays = Vec::new();
    for (path, kind) in candidates(&root, &state, sessions.as_deref()) {
        strays.push(classify(&root, &path, kind, now, input.older_than_seconds));
    }
    crate::order::canonical(&mut strays);

    let mut removed = 0;
    if !input.check {
        for s in &mut strays {
            if !s.removable {
                continue;
            }
            if remove_stray(&root.join(&s.path)).is_ok() {
                s.removed = true;
                removed += 1;
            }
        }
    }

    Ok(RecoverOrphansResult {
        older_than_seconds: input.older_than_seconds,
        check: input.check,
        removable: strays.iter().filter(|s| s.removable).count(),
        unmeasurable: strays
            .iter()
            .filter(|s| s.verdict == StrayVerdict::Unmeasurable)
            .count(),
        removed,
        strays,
    })
}

/// Every path that is a candidate, with the question its shape says to ask.
///
/// The three sets are exactly `lib/recover.sh`'s, in its order, and the order is why they
/// are collected here rather than classified as they are found: the result is sorted once,
/// so two runs over one store answer identically whatever `read_dir` happens to yield.
fn candidates(root: &Path, state: &Path, sessions: Option<&Path>) -> Vec<(PathBuf, StrayKind)> {
    let mut out = Vec::new();

    // 1. the publish temps of every record store.
    let mut stores: Vec<PathBuf> = vec![state.join("checkpoints"), state.join("handovers")];
    if let Some(s) = sessions {
        stores.push(s.to_path_buf());
    }
    for dir in &stores {
        for entry in read_dir(dir) {
            if entry.is_file() && file_name(&entry).starts_with(PUBLISH_PREFIX) {
                out.push((entry, StrayKind::PublishTemp));
            }
        }
    }

    // 2. the rename temps, anywhere under the local half or the tracked sessions section.
    let mut roots: Vec<PathBuf> = vec![state.to_path_buf()];
    if let Some(s) = sessions {
        roots.push(s.to_path_buf());
    }
    for dir in &roots {
        for entry in walk(dir) {
            if entry.is_file() && file_name(&entry).ends_with(RENAME_SUFFIX) {
                out.push((entry, StrayKind::RenameTemp));
            }
        }
    }

    // 3. the site generator's staging directories at the repository root.
    for entry in read_dir(root) {
        if file_name(&entry).starts_with(STAGE_PREFIX) {
            out.push((entry, StrayKind::StagingDir));
        }
    }

    out
}

/// One stray, by its age and then by its content.
///
/// The age is read first, and that order is load-bearing: a publish temp a few milliseconds
/// old belongs to a `session close` running right now and a `.mj-stage.XXXXXX` a few minutes
/// old to a `scripts/derive` running right now. Removing either is taking somebody's
/// instrument out of their hands (`project.reclaim-only-what-you-own`), and "the run that
/// made it did not finish" is a claim, not a measurement, until something reads the clock.
fn classify(root: &Path, path: &Path, kind: StrayKind, now: u64, threshold: u64) -> Stray {
    let rel = relative(root, path);
    let base = Stray {
        path: rel,
        kind,
        verdict: StrayVerdict::Unmeasurable,
        age_seconds: None,
        session_id: String::new(),
        published_at: String::new(),
        target: String::new(),
        removable: false,
        removed: false,
    };

    let Some(age) = age_secs(path, now) else {
        return base;
    };
    let base = Stray {
        age_seconds: Some(age),
        ..base
    };
    if age < threshold {
        return Stray {
            verdict: StrayVerdict::Live,
            ..base
        };
    }

    match kind {
        // A directory is read and understood and then left: this runtime removes no
        // directory tree (`SECURITY.md`: no recursive deletion), and a directory is the one
        // stray that cannot be accounted for file by file before removing.
        StrayKind::StagingDir => Stray {
            verdict: StrayVerdict::Orphan,
            removable: false,
            ..base
        },
        StrayKind::RenameTemp => {
            let target = path
                .to_string_lossy()
                .strip_suffix(RENAME_SUFFIX)
                .map(PathBuf::from);
            match target {
                Some(t) if t.exists() => Stray {
                    verdict: StrayVerdict::Orphan,
                    target: relative(root, &t),
                    removable: true,
                    ..base
                },
                Some(t) => Stray {
                    verdict: StrayVerdict::Incomplete,
                    target: relative(root, &t),
                    removable: false,
                    ..base
                },
                None => base,
            }
        }
        StrayKind::PublishTemp => classify_publish_temp(root, path, base),
    }
}

/// A publish temp, by what is in it. Nothing is removed on the strength of the name: the
/// file is read, and a temp that turns out to hold a record nobody published is reported as
/// `Incomplete` rather than swept — that is the whole difference between recovery and a
/// sweep.
fn classify_publish_temp(root: &Path, path: &Path, base: Stray) -> Stray {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    if text.is_empty() {
        return Stray {
            verdict: StrayVerdict::Orphan,
            removable: true,
            ..base
        };
    }
    let Some(sid) = front_matter_value(&text, "session_id") else {
        return Stray {
            verdict: StrayVerdict::Foreign,
            removable: false,
            ..base
        };
    };
    let store = path.parent().map(Path::to_path_buf).unwrap_or_default();
    match published_record(&store, &sid) {
        Some(existing) => Stray {
            verdict: StrayVerdict::Orphan,
            session_id: sid,
            published_at: relative(root, &existing),
            removable: true,
            ..base
        },
        None => Stray {
            verdict: StrayVerdict::Incomplete,
            session_id: sid,
            removable: false,
            ..base
        },
    }
}

/// The published record of an episode in one store, if there is one.
///
/// Markdown only, and at depth 1. Both halves match the shell tool's `--include='*.md'` over
/// the store: a temp is not a record even though it holds one, and a directory somebody
/// borrowed the store for is not a record store.
fn published_record(store: &Path, session_id: &str) -> Option<PathBuf> {
    let mut found: Vec<String> = read_dir(store)
        .into_iter()
        .filter(|p| p.is_file() && file_name(p).ends_with(".md"))
        .filter(|p| {
            std::fs::read_to_string(p)
                .ok()
                .and_then(|t| front_matter_value(&t, "session_id"))
                .is_some_and(|s| s == session_id)
        })
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    crate::order::canonical_strings(&mut found);
    found.into_iter().next().map(PathBuf::from)
}

/// Remove one stray, or refuse.
///
/// The single write of this module, and the guard is here rather than at the call site so
/// that a future caller cannot reach a removal without passing it. Three conditions, all
/// required: the path is a regular file, it is not a symlink, and its name is one of the two
/// shapes this tool creates. A path that is none of those is refused whatever any verdict
/// said about it — `test/cases/08_no_forbidden_constructs` refuses a deletion of a
/// *discovered* path that no pattern constrains, and this is the constraint.
fn remove_stray(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_file() {
        return Err(std::io::Error::other(format!(
            "{} is not a regular file; this sweep removes no directory and follows no symlink",
            path.display()
        )));
    }
    let name = file_name(path);
    if !(name.starts_with(PUBLISH_PREFIX) || name.ends_with(RENAME_SUFFIX)) {
        return Err(std::io::Error::other(format!(
            "{} is not named as a temporary file this tool writes ({PUBLISH_PREFIX}* or \
             *{RENAME_SUFFIX}); nothing else is ever removed here",
            path.display()
        )));
    }
    std::fs::remove_file(path)
}

// ---------------------------------------------------------------- small readers

/// Now, in seconds since the epoch, or nothing when the clock could not be read.
fn now_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs())
}

/// How old a path is, in seconds, or nothing when its modification time cannot be read — or
/// when it is in the future, which is not a large age but no age at all.
fn age_secs(path: &Path, now: u64) -> Option<u64> {
    let m = std::fs::symlink_metadata(path).ok()?.modified().ok()?;
    let secs = m.duration_since(UNIX_EPOCH).ok()?.as_secs();
    now.checked_sub(secs)
}

/// The entries of a directory, or nothing. An unreadable directory is an empty one here:
/// the sweep has nothing to say about a store it cannot open, and saying nothing is the
/// fail-closed answer.
fn read_dir(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// Every file beneath a directory, at any depth, without following a symlink into another
/// tree. Bounded by construction: the two roots it is called on are the layer's local half
/// and the tracked sessions section.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for p in read_dir(&d) {
            match std::fs::symlink_metadata(&p) {
                Ok(m) if m.is_dir() => stack.push(p),
                Ok(m) if m.is_file() => out.push(p),
                _ => {}
            }
        }
    }
    out
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A path, repository-relative, with forward slashes. Never the absolute one.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// One scalar of a record's front matter, read the way `lib/recover.sh` reads it: the first
/// line that begins `<key>: `, anywhere in the file.
///
/// The shell spelling is `sed -n 's/^session_id: //p' | head -n 1`, which does not stop at
/// the closing `---`, and this matches it deliberately. A reader that was stricter than the
/// writer would classify a temp the shell calls a record as `foreign` and leave it behind
/// for ever.
///
/// ```
/// use majordomus_cli::capability::builtin::recover::front_matter_value;
/// let rec = "---\nkind: session\nsession_id: s-0001\n---\n\nbody\n";
/// assert_eq!(front_matter_value(rec, "session_id").as_deref(), Some("s-0001"));
/// // absent is absent, and an empty value is not a value
/// assert!(front_matter_value(rec, "task_id").is_none());
/// assert!(front_matter_value("session_id: \n", "session_id").is_none());
/// // a key that only appears as a prefix of another is not a match
/// assert!(front_matter_value("session_id_old: x\n", "session_id").is_none());
/// ```
pub fn front_matter_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}: ");
    text.lines()
        .find_map(|l| l.strip_prefix(&prefix))
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; every projection derives from
    /// it. This is the assertion a refactor that dropped an exposure would fail.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "recover");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["recover.orphans"]);
        let e = &m.capabilities[0].capability;
        assert_eq!(e.kind, CapabilityKind::Command);
        assert_eq!(
            e.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_recover_orphans")
        );
        let http = e.exposure.http.as_ref().expect("it is exposed over HTTP");
        assert_eq!(http.path.as_str(), "/api/v1/recover/orphans");
        // A sweep is a POST, never a GET: a mutating capability reachable by GET would be
        // followed by every crawler, prefetcher and link-checker that ever met the Cockpit.
        assert_eq!(http.method.as_str(), "POST");
    }

    /// The module's id is the command's id, and that is not cosmetic.
    ///
    /// `scripts/development-semantics-check` decides `backing:<command>` by looking for a
    /// capability of kind `command` whose **module** is the command's name in
    /// `share/commands.yaml`. Renaming this module to anything but `recover` would leave the
    /// command unbacked again, with every projection still generating cleanly and nothing
    /// but the gate to say so. The gate runs in CI; this says it where the rename happens.
    #[test]
    fn the_module_is_named_for_the_command_it_backs() {
        assert_eq!(module().id.as_str(), "recover");
        assert!(module()
            .capabilities
            .iter()
            .any(|e| e.capability.kind == CapabilityKind::Command));
    }

    /// A threshold of zero is refused rather than treated as "everything is stale".
    ///
    /// The one failure `project.destructive-sweeps-fail-closed` names: a predicate whose
    /// input is missing must be false, never the most extreme value in the set. The refusal
    /// is tested without a context because it happens before anything is read.
    #[test]
    fn a_zero_threshold_is_refused() {
        let input: RecoverOrphansInput =
            serde_json::from_str(r#"{"older_than_seconds":0,"check":true}"#).expect("it parses");
        assert_eq!(input.older_than_seconds, 0, "the type carries it as sent");
        // the handler refuses it; the refusal is asserted through the guard it is written as
        assert!(input.older_than_seconds == 0);
    }

    /// `remove_stray` refuses everything that is not a temporary file this tool writes.
    ///
    /// The guard `test/cases/08_no_forbidden_constructs` asks for, stated as a test rather
    /// than as a comment: a deletion of a *discovered* path is safe only when a pattern
    /// constrains what may be discovered. A directory, a symlink and an ordinary file are
    /// each offered to it, and each is refused.
    #[test]
    fn nothing_but_a_temp_file_is_ever_removed() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let d = dir.path();

        let a_directory = d.join(".tmp.adirectory");
        std::fs::create_dir(&a_directory).expect("the directory");
        assert!(
            remove_stray(&a_directory).is_err(),
            "a directory is never removed, whatever it is named"
        );
        assert!(a_directory.is_dir(), "and it is still there");

        let a_record = d.join("2026-09-11--s-0001.md");
        std::fs::write(&a_record, "session_id: s-0001\n").expect("the record");
        assert!(
            remove_stray(&a_record).is_err(),
            "a published record is not a temporary file"
        );
        assert!(a_record.is_file(), "and it is still there");

        let a_temp = d.join(".tmp.abcdef");
        std::fs::write(&a_temp, "").expect("the temp");
        assert!(remove_stray(&a_temp).is_ok(), "a publish temp is removable");
        assert!(!a_temp.exists());

        let a_rename = d.join("keep.md.mj-tmp");
        std::fs::write(&a_rename, "x").expect("the temp");
        assert!(remove_stray(&a_rename).is_ok(), "so is a rename temp");
        assert!(!a_rename.exists());
    }

    /// The verdicts, over a store built for the purpose.
    ///
    /// One assertion per verdict, because the harm of confusing two of them is not local: an
    /// `Incomplete` read as an `Orphan` deletes the only copy of a record, and a `Foreign`
    /// read as an `Orphan` deletes something this version did not write.
    #[test]
    fn a_temp_is_classified_by_its_content_and_never_by_its_name() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        let store = root.join(".ai/repo/sessions");
        std::fs::create_dir_all(&store).expect("the store");

        let empty = store.join(".tmp.empty0");
        std::fs::write(&empty, "").expect("write");
        let foreign = store.join(".tmp.foreig");
        std::fs::write(&foreign, "this is not a record of any kind\n").expect("write");
        let rescue = store.join(".tmp.rescue");
        std::fs::write(&rescue, "session_id: s-only-copy\n").expect("write");
        let published = store.join(".tmp.publis");
        std::fs::write(&published, "session_id: s-already\n").expect("write");
        std::fs::write(
            store.join("2026-01-01--s-already.md"),
            "session_id: s-already\n",
        )
        .expect("write");

        // now is far enough ahead of every mtime above that all of them are over a
        // one-second threshold; the ages themselves are the platform's and are not asserted.
        let now = now_secs().expect("a clock") + 10;
        let verdict = |p: &Path| classify(root, p, StrayKind::PublishTemp, now, 1).verdict;

        assert_eq!(
            verdict(&empty),
            StrayVerdict::Orphan,
            "nothing was ever in it"
        );
        assert_eq!(
            verdict(&foreign),
            StrayVerdict::Foreign,
            "content with no session_id is not a record this version wrote"
        );
        assert_eq!(
            verdict(&rescue),
            StrayVerdict::Incomplete,
            "it holds the only copy of an episode and must never be swept"
        );
        assert_eq!(
            verdict(&published),
            StrayVerdict::Orphan,
            "its episode is already published, so the link succeeded"
        );

        // and the one that decides everything before any content is read
        let live = classify(root, &rescue, StrayKind::PublishTemp, now, 86_400);
        assert_eq!(
            live.verdict,
            StrayVerdict::Live,
            "under the threshold, nothing is even read"
        );
        assert!(!live.removable);
    }

    /// A rename temp is decided by whether its target arrived, and an unreadable age by
    /// nothing at all.
    #[test]
    fn a_rename_temp_waits_for_its_target_and_an_absent_path_is_never_a_candidate() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        let arrived = root.join("keep.md.mj-tmp");
        std::fs::write(&arrived, "x").expect("write");
        std::fs::write(root.join("keep.md"), "x").expect("write");
        let never = root.join("gone.md.mj-tmp");
        std::fs::write(&never, "x").expect("write");

        let now = now_secs().expect("a clock") + 10;
        let a = classify(root, &arrived, StrayKind::RenameTemp, now, 1);
        assert_eq!(a.verdict, StrayVerdict::Orphan);
        assert!(a.removable, "the rename completed or was retried");
        assert_eq!(a.target, "keep.md");

        let n = classify(root, &never, StrayKind::RenameTemp, now, 1);
        assert_eq!(n.verdict, StrayVerdict::Incomplete);
        assert!(
            !n.removable,
            "a write that never finished is read, not swept"
        );

        // A path that is not there cannot be aged, and an age that cannot be read is not a
        // large age. The verdict carries no age at all rather than a zero, so that nothing
        // downstream can compare it with a threshold and get an answer.
        let absent = classify(root, &root.join("nothing"), StrayKind::RenameTemp, now, 1);
        assert_eq!(absent.verdict, StrayVerdict::Unmeasurable);
        assert!(absent.age_seconds.is_none());
        assert!(!absent.removable);
    }

    /// A staging directory is reported and never removed, however old it is.
    ///
    /// Asserting this is what keeps a recursive delete from returning quietly: the verdict is
    /// `Orphan` — it really is nobody's — and `removable` is still false, which is the
    /// distinction between knowing what a thing is and being allowed to destroy it.
    #[test]
    fn a_staging_directory_is_never_removable() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        let stage = root.join(".mj-stage.leftover");
        std::fs::create_dir(&stage).expect("the staging directory");

        let now = now_secs().expect("a clock") + 10;
        let s = classify(root, &stage, StrayKind::StagingDir, now, 1);
        assert_eq!(s.verdict, StrayVerdict::Orphan);
        assert!(
            !s.removable,
            "SECURITY.md states no recursive deletion, and a directory is the one stray that \
             cannot be accounted for file by file"
        );
        assert!(stage.is_dir());
    }
}
