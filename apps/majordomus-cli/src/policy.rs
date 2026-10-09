//! The canonical policy, typed: `.ai/repo/policy.yaml` and the profiles beside it. The
//! policy is the one canonical input of the provider projections; the shell tool reads the
//! same file with the same subset reader, so the two agree byte for byte on what a
//! projection is and which hash it carries.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::metadata::yaml;
use crate::repository::Repository;

/// The manifest section that holds the policy file.
pub const POLICY_SECTION: &str = "policy";
/// The manifest section that holds the profiles directory.
pub const PROFILES_SECTION: &str = "profiles";

/// How a projection owns its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionMode {
    /// The whole file is generated; its first line is the stamp.
    #[default]
    File,
    /// Only the text between `majordomus:begin` and `majordomus:end` markers is generated.
    Region,
}

/// One declared provider projection: `projections[]` in the policy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Projection {
    /// The provider whose template renders the target: `agents`, `claude-code`, ...
    pub provider: String,
    /// Repository-relative target path, e.g. `AGENTS.md`.
    pub target: String,
    /// Whether every worker loads this target without asking; bounded by the budget.
    #[serde(default)]
    pub always_loaded: bool,
    /// Whole file or a marked region.
    #[serde(default)]
    pub mode: ProjectionMode,
}

/// `context:` of the policy, the part the projections need.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ContextPolicy {
    /// Hard cap, in lines, on an `always_loaded` projection.
    #[serde(default)]
    pub always_loaded_budget_lines: Option<u64>,
}

/// `profiles:` of the policy, the part the projections need.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ProfilesPolicy {
    /// The profile a worker starts with.
    #[serde(default)]
    pub default: Option<String>,
    /// The checkpoint interval the default profile sets.
    #[serde(default)]
    pub checkpoint_interval_default: Option<String>,
}

/// `session.freshness:` of the policy: how old a continuation record may be before it stops
/// being current.
///
/// These are the only numbers that decide it, and they are declared once. Divergence
/// (`exact`/`advanced`/`diverged`/`different_context`) answers a different question — where
/// the record's commit sits relative to HEAD — and answers it identically on the day a
/// record is written and a month later. Keeping a default here would be a second source of
/// truth for the same thresholds, so absence is carried as absence and reported as
/// `unknown`, naming the missing key.
///
/// Both keys are independently optional, and the pair is what
/// [`Thresholds::judge`](crate::capability::builtin::continuity::Thresholds::judge) needs:
/// with one of them missing it has no band to place a record in and answers `unknown`
/// rather than inventing the other half.
///
/// ```
/// use majordomus_cli::policy::FreshnessPolicy;
/// use serde_json::json;
///
/// // the declared form: the two numbers that draw fresh | aging | stale
/// let declared: FreshnessPolicy =
///     serde_json::from_value(json!({"fresh_minutes": 30, "stale_minutes": 240})).unwrap();
/// assert_eq!(declared.fresh_minutes, Some(30));
/// assert_eq!(declared.stale_minutes, Some(240));
///
/// // a policy that predates the key: absent, not defaulted. Nothing in this file
/// // supplies a number the repository never declared.
/// let silent: FreshnessPolicy = serde_json::from_value(json!({})).unwrap();
/// assert_eq!(silent, FreshnessPolicy::default());
/// assert!(silent.fresh_minutes.is_none() && silent.stale_minutes.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct FreshnessPolicy {
    /// Below this age a record is `fresh`.
    #[serde(default)]
    pub fresh_minutes: Option<i64>,
    /// At or beyond this age a record is `stale`, and is never presented as current.
    #[serde(default)]
    pub stale_minutes: Option<i64>,
}

/// `knowledge:` of the policy: the bounds on the review queue of derived knowledge records.
///
/// A candidate is a record the deriver wrote at an episode boundary and nobody has judged
/// (ADR 0091). The queue is measured, never emptied by a machine, and both numbers are
/// declared once in the policy and read from it by the shell validator and by
/// `knowledge_base.candidates` alike. Absence is carried as absence: a repository whose
/// policy predates the keys is reported as declaring no cap, never judged against one
/// nobody wrote down.
///
/// ```
/// use majordomus_cli::policy::KnowledgePolicy;
/// let declared: KnowledgePolicy =
///     serde_json::from_str(r#"{"candidates_max_files": 40}"#).expect("a knowledge block");
/// assert_eq!(declared.candidates_max_files, Some(40));
/// assert_eq!(declared.candidate_max_age_minutes, None, "absent is absent, not a default");
/// assert_eq!(KnowledgePolicy::default().candidates_max_files, None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct KnowledgePolicy {
    /// More records with status `candidate` than this awaiting review is a finding.
    #[serde(default)]
    pub candidates_max_files: Option<usize>,
    /// A candidate waiting longer than this since it entered the queue is a finding.
    #[serde(default)]
    pub candidate_max_age_minutes: Option<i64>,
}

/// The policy, typed to what the projections consume. Every other key is carried through
/// unread: the policy schema under `share/schemas/majordomus/policy/policy.v1.schema.json` owns the full shape.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct Policy {
    /// `version:`.
    #[serde(default)]
    pub version: Option<u64>,
    /// `context:`.
    #[serde(default)]
    pub context: ContextPolicy,
    /// `profiles:`.
    #[serde(default)]
    pub profiles: ProfilesPolicy,
    /// `session:`.
    #[serde(default)]
    pub session: SessionPolicy,
    /// `knowledge:`.
    #[serde(default)]
    pub knowledge: KnowledgePolicy,
    /// `projections:`.
    #[serde(default)]
    pub projections: Vec<Projection>,
    /// `server:` — the timings a lease contest between two processes is judged by. Absent
    /// keeps the constants `lease.rs` declares, so a policy that predates the block, or one
    /// that could not be read, does not silently change how a server is taken over.
    #[serde(default)]
    pub server: ServerPolicy,
    /// `commit:` — what this repository holds a commit message to. Absent is the default,
    /// which is what this repository's own history already satisfies.
    #[serde(default)]
    pub commit: crate::commit::CommitPolicy,
    /// `intent:` — whether work must be bound to what it serves before it starts, and the
    /// exemption classes a worker may give instead. Absent is `off`: a repository whose
    /// policy predates the block starts work exactly as it did (ADR 0111).
    #[serde(default)]
    pub intent: IntentPolicy,
}

/// How strictly a start is held to its binding.
///
/// ```
/// use majordomus_cli::policy::BindingMode;
/// // a policy that says nothing asks for nothing
/// assert_eq!(BindingMode::default(), BindingMode::Off);
/// let m: BindingMode = serde_json::from_str("\"required\"").unwrap();
/// assert_eq!(m, BindingMode::Required);
/// assert_eq!(m.as_str(), "required");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BindingMode {
    /// Nothing is asked unless the worker names an issue, an intent or an exemption.
    #[default]
    Off,
    /// Every start asks; a refused binding is reported and the work starts.
    Advisory,
    /// Every start asks; a refused or unreadable binding does not start.
    Required,
}

impl BindingMode {
    /// The word the policy file and every surface use.
    ///
    /// ```
    /// use majordomus_cli::policy::BindingMode;
    /// assert_eq!(BindingMode::Advisory.as_str(), "advisory");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            BindingMode::Off => "off",
            BindingMode::Advisory => "advisory",
            BindingMode::Required => "required",
        }
    }
}

/// One exemption class: a named reason work may proceed under no intent.
///
/// ```
/// use majordomus_cli::policy::ExemptionClass;
/// let c: ExemptionClass = serde_json::from_str(
///     r#"{"id": "emergency", "description": "Restoring a broken trunk"}"#,
/// )
/// .unwrap();
/// assert_eq!(c.id, "emergency");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
pub struct ExemptionClass {
    /// The word a worker gives: `--exempt <id>`.
    pub id: String,
    /// What the class is for, in one line.
    #[serde(default)]
    pub description: String,
}

/// `intent:` — the binding a start is held to (ADR 0111).
///
/// ```
/// use majordomus_cli::policy::{BindingMode, IntentPolicy};
/// // absent: off, and no exemption class exists to give
/// let absent = IntentPolicy::default();
/// assert_eq!(absent.binding, BindingMode::Off);
/// assert!(absent.exemption("maintenance").is_none());
/// let p: IntentPolicy = serde_json::from_str(
///     r#"{"binding": "required", "exemptions": [{"id": "maintenance"}]}"#,
/// )
/// .unwrap();
/// assert!(p.exemption("maintenance").is_some());
/// assert!(p.exemption("because-i-say-so").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
pub struct IntentPolicy {
    /// `intent.binding:` — `off`, `advisory` or `required`.
    #[serde(default)]
    pub binding: BindingMode,
    /// `intent.exemptions:` — the classes a worker may give instead of naming work.
    #[serde(default)]
    pub exemptions: Vec<ExemptionClass>,
    /// `intent.opposition:` — whether a critique must have been stamped by the tool
    /// (ADR 0112). Absent is `off`: a critique written before stamps existed keeps
    /// authorising work, and `intent validate` says it carries none.
    #[serde(default)]
    pub opposition: OppositionMode,
    /// `intent.completion:` — whether a finish is held to what its work serves (ADR 0115).
    /// Absent is `off`: the completion policy's questions about an intent are not asked.
    #[serde(default)]
    pub completion: CompletionMode,
}

/// Whether the completion policy holds a task to the criteria its issue serves and the
/// guards of the intents it serves (ADR 0115).
///
/// ```
/// use majordomus_cli::policy::{CompletionMode, IntentPolicy};
/// assert_eq!(CompletionMode::default(), CompletionMode::Off);
/// let p: IntentPolicy = serde_json::from_str(r#"{"completion": "advisory"}"#).unwrap();
/// assert_eq!(p.completion, CompletionMode::Advisory);
/// assert!(p.completion.asks() && !p.completion.holds());
/// assert!(serde_json::from_str::<IntentPolicy>(r#"{"completion": "sometimes"}"#).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CompletionMode {
    /// The questions are not asked.
    #[default]
    Off,
    /// They are answered; what would be owed is reported and withheld, and refuses nothing.
    Advisory,
    /// What is owed is owed, and refuses `completed` where the policy says completed means
    /// complete.
    Required,
}

impl CompletionMode {
    /// The word the policy file spells and a surface prints.
    ///
    /// ```
    /// use majordomus_cli::policy::CompletionMode;
    /// assert_eq!(CompletionMode::Required.as_str(), "required");
    /// assert_eq!(CompletionMode::Off.as_str(), "off");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            CompletionMode::Off => "off",
            CompletionMode::Advisory => "advisory",
            CompletionMode::Required => "required",
        }
    }

    /// Whether the intent engine is asked at all.
    ///
    /// ```
    /// use majordomus_cli::policy::CompletionMode;
    /// assert!(!CompletionMode::Off.asks());
    /// assert!(CompletionMode::Advisory.asks() && CompletionMode::Required.asks());
    /// ```
    pub fn asks(self) -> bool {
        self != CompletionMode::Off
    }

    /// Whether an answer that is owed is held against the task, rather than withheld.
    ///
    /// ```
    /// use majordomus_cli::policy::CompletionMode;
    /// assert!(CompletionMode::Required.holds());
    /// assert!(!CompletionMode::Advisory.holds());
    /// ```
    pub fn holds(self) -> bool {
        self == CompletionMode::Required
    }
}

/// Whether a review must have been executed and stamped.
///
/// ```
/// use majordomus_cli::policy::OppositionMode;
/// assert_eq!(OppositionMode::default(), OppositionMode::Off);
/// let m: OppositionMode = serde_json::from_str("\"required\"").unwrap();
/// assert!(m.is_required());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OppositionMode {
    /// An unstamped critique is a warning, and a stale one refuses the binding only.
    #[default]
    Off,
    /// An unstamped or stale critique fails `intent validate` and refuses the binding.
    Required,
}

impl OppositionMode {
    /// Whether the policy asks for a stamped review.
    ///
    /// ```
    /// use majordomus_cli::policy::OppositionMode;
    /// assert!(!OppositionMode::Off.is_required());
    /// ```
    pub fn is_required(self) -> bool {
        self == OppositionMode::Required
    }
}

impl IntentPolicy {
    /// The declared class with this id, when the policy declares one.
    ///
    /// ```
    /// use majordomus_cli::policy::IntentPolicy;
    /// assert!(IntentPolicy::default().exemption("x").is_none());
    /// ```
    pub fn exemption(&self, id: &str) -> Option<&ExemptionClass> {
        self.exemptions.iter().find(|c| c.id == id)
    }
}

/// `session:` — what the episode boundary does beyond drawing itself.
///
/// Only the half two entry paths both read is typed here. The rest of the block is carried
/// through unread, as every other key is: the policy schema owns the full shape.
///
/// ```
/// use majordomus_cli::policy::SessionPolicy;
/// // A policy that says nothing about it still converges. The default is what this
/// // repository's policy and the skeleton a new one is written from both declare, and a
/// // policy that could not be read must not silently change behaviour.
/// assert!(SessionPolicy::default().ensure_server_on_start);
/// assert!(SessionPolicy::default().knowledge_on_end);
/// assert!(SessionPolicy::default().knowledge_on_compact);
/// // and a repository that has turned one off is read as having turned it off
/// let off: SessionPolicy =
///     serde_json::from_str(r#"{"ensure_server_on_start": false, "knowledge_on_end": false}"#)
///         .expect("a session block");
/// assert!(!off.ensure_server_on_start);
/// assert!(!off.knowledge_on_end);
/// assert!(off.knowledge_on_compact, "the two switches are independent");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SessionPolicy {
    /// `session.freshness:` — how old a continuation record may be before it stops being
    /// current. A separate question from divergence, which is about git topology and says
    /// nothing about age.
    #[serde(default)]
    pub freshness: FreshnessPolicy,
    /// Whether entering the repository converges on a ready shared server.
    ///
    /// One switch, both entry paths: the provider's start event (ADR 0035) and the file a
    /// shell evaluates on entry (ADR 0043). It defaults to `true` because that is what
    /// this repository's policy and the skeleton a new one is written from both declare,
    /// and because a policy that could not be read must not silently change behaviour —
    /// an unreadable policy is `doctor`'s finding, not a reason to stop converging.
    #[serde(default = "yes")]
    pub ensure_server_on_start: bool,
    /// Whether closing an episode derives the knowledge it produced into candidate records
    /// under the tracked knowledge section (ADR 0091). Read here so that
    /// `knowledge_base.status` judges a silent writer only where the writer is switched on,
    /// and defaults to `true` for the reason `ensure_server_on_start` does: the policy and
    /// the skeleton both declare it, and an unreadable policy must not change behaviour.
    #[serde(default = "yes")]
    pub knowledge_on_end: bool,
    /// Whether a provider's compaction event derives the same records. Independent of
    /// `checkpoint_on_compact`: the two record different things.
    #[serde(default = "yes")]
    pub knowledge_on_compact: bool,
}

impl Default for SessionPolicy {
    fn default() -> Self {
        SessionPolicy {
            freshness: FreshnessPolicy::default(),
            ensure_server_on_start: true,
            knowledge_on_end: true,
            knowledge_on_compact: true,
        }
    }
}

fn yes() -> bool {
    true
}

/// The policy as loaded: the typed value, where it came from, and the hash the stamps
/// carry.
#[derive(Debug, Clone)]
pub struct LoadedPolicy {
    /// The typed policy.
    pub policy: Policy,
    /// Repository-relative path of the policy file.
    pub path: String,
    /// SHA-256 (hex) of the policy file followed by every profile file, in name order,
    /// concatenated: exactly what the shell tool's `mj_policy_cat` hashes.
    pub sha256: String,
}

impl LoadedPolicy {
    /// Read the policy and profiles of a repository.
    pub fn load(repository: &Repository) -> Result<Self> {
        let rel =
            repository
                .section_path(POLICY_SECTION)
                .ok_or_else(|| Error::InvalidManifest {
                    path: repository.root().join(crate::repository::MANIFEST),
                    reason: format!("the manifest names no `{POLICY_SECTION}` section"),
                })?;
        let path = repository.root().join(&rel);
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let policy: Policy = yaml::parse_into(&text).map_err(|reason| Error::InvalidPolicy {
            path: path.clone(),
            reason,
        })?;
        // The one point a repository's policy is read, and therefore the one place the lease
        // timings can be declared for this process. `declare_timings` is a OnceLock set, so
        // reading the policy twice does not move a judgement already in flight.
        crate::lease::declare_timings(&policy.server);
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        for profile in profile_files(repository)? {
            let bytes = std::fs::read(&profile).map_err(|e| Error::io(&profile, e))?;
            hasher.update(&bytes);
        }
        Ok(LoadedPolicy {
            policy,
            path: rel,
            sha256: format!("{:x}", hasher.finalize()),
        })
    }
}

/// The profile files, `<profiles>/*.yaml`, sorted by name. An absent directory is no
/// profile, not an error.
pub fn profile_files(repository: &Repository) -> Result<Vec<PathBuf>> {
    let Some(rel) = repository.section_path(PROFILES_SECTION) else {
        return Ok(Vec::new());
    };
    let dir = repository.root().join(rel);
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(Error::io(&dir, e)),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml") && p.is_file())
        .collect();
    files.sort();
    Ok(files)
}

/// The repository-relative directory where a repository may override a provider template.
pub fn repository_providers_dir(repository: &Repository) -> PathBuf {
    repository
        .ai_dir()
        .join(&repository.manifest().repo.path)
        .join("providers")
}

/// Hex SHA-256 of a text.
pub fn sha256_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

/// Hex SHA-256 of arbitrary bytes: the same digest [`sha256_hex`] takes, for content that
/// is not necessarily text. A file the repository tracks may hold anything, and decoding it
/// to hash it would be a decision about its encoding that nothing here is entitled to make.
///
/// ```
/// use majordomus_cli::policy::{sha256_bytes_hex, sha256_hex};
/// assert_eq!(sha256_bytes_hex(b"majordomus"), sha256_hex("majordomus"));
/// assert_ne!(sha256_bytes_hex(&[0xff, 0xfe]), sha256_bytes_hex(&[0xfe, 0xff]));
/// ```
pub fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

/// A path is inside `root` after lexical normalisation: no absolute path, no `..`
/// component, nothing empty.
pub fn is_safe_relative(path: &str) -> bool {
    let p = Path::new(path);
    !path.is_empty()
        && !p.is_absolute()
        && p.components().all(|c| {
            matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `intent.completion` is one of three words; absent is off, and a fourth is refused.
    #[test]
    fn the_completion_mode_is_off_advisory_or_required() {
        let absent: Policy = yaml::parse_into("version: 1\n").expect("a policy");
        assert_eq!(absent.intent.completion, CompletionMode::Off);
        for (word, mode, asks, holds) in [
            ("off", CompletionMode::Off, false, false),
            ("advisory", CompletionMode::Advisory, true, false),
            ("required", CompletionMode::Required, true, true),
        ] {
            let p: Policy =
                yaml::parse_into(&format!("version: 1\nintent:\n  completion: {word}\n"))
                    .expect("a policy with a completion mode");
            assert_eq!(p.intent.completion, mode);
            assert_eq!(mode.as_str(), word);
            assert_eq!((mode.asks(), mode.holds()), (asks, holds), "{word}");
        }
        assert!(
            yaml::parse_into::<Policy>("version: 1\nintent:\n  completion: sometimes\n").is_err()
        );
    }

    #[test]
    fn the_intent_block_is_read_from_the_policy_file_and_absent_means_off() {
        let absent: Policy = yaml::parse_into("version: 1\n").expect("a policy");
        assert_eq!(absent.intent.binding, BindingMode::Off);
        assert!(absent.intent.exemptions.is_empty());
        let p: Policy = yaml::parse_into(
            "version: 1\nintent:\n  binding: required\n  exemptions:\n    - id: emergency\n      description: Restoring a broken trunk\n    - id: maintenance\n",
        )
        .expect("a policy with an intent block");
        assert_eq!(p.intent.binding, BindingMode::Required);
        let class = p.intent.exemption("emergency").expect("the declared class");
        assert_eq!(class.description, "Restoring a broken trunk");
        assert_eq!(p.intent.exemption("maintenance").unwrap().description, "");
        assert!(p.intent.exemption("whim").is_none());
        // each mode is the word the file spells it with
        for (mode, word) in [
            (BindingMode::Off, "off"),
            (BindingMode::Advisory, "advisory"),
            (BindingMode::Required, "required"),
        ] {
            assert_eq!(mode.as_str(), word);
            let read: BindingMode = serde_json::from_value(serde_json::json!(word)).unwrap();
            assert_eq!(read, mode);
        }
    }

    #[test]
    fn projection_defaults_are_file_mode_and_not_always_loaded() {
        let p: Policy = yaml::parse_into(
            "version: 1\nprojections:\n  - provider: agents\n    target: AGENTS.md\n",
        )
        .unwrap();
        assert_eq!(p.projections.len(), 1);
        assert_eq!(p.projections[0].mode, ProjectionMode::File);
        assert!(!p.projections[0].always_loaded);
        assert_eq!(p.profiles.default, None);
    }

    #[test]
    fn unknown_projection_keys_are_refused() {
        let r: std::result::Result<Policy, String> = yaml::parse_into(
            "projections:\n  - provider: agents\n    target: AGENTS.md\n    colour: red\n",
        );
        assert!(r.unwrap_err().contains("colour"));
    }

    #[test]
    fn safe_relative_paths() {
        assert!(is_safe_relative("AGENTS.md"));
        assert!(is_safe_relative("docs/x/y.md"));
        assert!(is_safe_relative("./AGENTS.md"));
        assert!(!is_safe_relative(""));
        assert!(!is_safe_relative("/etc/passwd"));
        assert!(!is_safe_relative("../x"));
        assert!(!is_safe_relative("a/../../x"));
    }
}

/// `server:` — what a lease contest is judged by.
///
/// Every field is optional and absent means "keep the compiled default", which is why there
/// is no reader-side default *value* here: the defaults live in one place, `lease::Timings`,
/// and this block only says which of them this repository overrides.
///
/// ```
/// use majordomus_cli::policy::ServerPolicy;
/// // a policy that predates the block: absent, not defaulted to zero
/// assert_eq!(ServerPolicy::default().probe_timeout_seconds, None);
/// let p: ServerPolicy =
///     serde_json::from_str(r#"{"probe_timeout_seconds": 5}"#).expect("a server block");
/// assert_eq!(p.probe_timeout_seconds, Some(5));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
pub struct ServerPolicy {
    /// `server.probe_timeout_seconds:` — how long a probe waits for the current owner before
    /// that silence counts as evidence that it is gone.
    #[serde(default)]
    pub probe_timeout_seconds: Option<u64>,
    /// `server.bind_grace_seconds:` — how long a lease naming no URL yet is left alone,
    /// because a server writes its lease before it can serve.
    #[serde(default)]
    pub bind_grace_seconds: Option<u64>,
    /// `server.join_timeout_seconds:` — how long a process waits to join or create the lease
    /// file before refusing rather than waiting forever.
    #[serde(default)]
    pub join_timeout_seconds: Option<u64>,
    /// `server.busy_grace_seconds:` — how long a live owner that does not answer is waited
    /// on, and asked again, before its lease is taken over.
    #[serde(default)]
    pub busy_grace_seconds: Option<u64>,
}
