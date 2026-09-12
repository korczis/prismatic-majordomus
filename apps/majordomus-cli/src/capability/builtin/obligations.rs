//! The `obligations` module: what a task owes before it may be called completed, and
//! whether the evidence that discharged each one still describes this tree.
//!
//! Two halves, and the difference between them decides the design.
//!
//! **The vocabulary is shipped.** `share/obligations.yaml` is data the distribution
//! carries beside `share/events.yaml` and `share/kinds.yaml`: a token, what discharges
//! it, and the pathspecs its evidence is taken over. It is identical in every clone and
//! in every installation, so it is read here the way `commands.list` reads the command
//! registry — through [`crate::share::Share`] — and it answers in a checkout that has
//! never run the lifecycle.
//!
//! **The closure is local.** The task record and the ledger live under
//! `.ai/local/state/`, the half [`super::continuity`] documents: never tracked, never
//! published, one file per checkout. A served instance answers about the checkout the
//! process was started in and about no other, and a fresh clone has nothing to report —
//! which is reported as nothing rather than as zero obligations discharged.
//!
//! **It is read, never written.** `majordomus evidence` records; this reads. A second
//! writer for the same ledger would be a second account of events, which is the thing
//! ADR 0030 refuses.
//!
//! **The judgement is the validator's, not a second opinion.** `mj_validate_obligations`
//! in `lib/evidence.sh` decides whether a task may be completed; everything below
//! reproduces its rule — a remote fact is labelled against the commit it was taken at, a
//! tree-bound fact is re-hashed over the pathspecs its token declares, and the label is
//! [`Divergence`], whose four words are `mj_git_label`'s. No fifth vocabulary is invented
//! here, and where the shell is generous — a token that names neither inputs nor a remote
//! fact can never go stale — this is generous in the same place rather than stricter in
//! private.
//!
//! **Where the two can differ, and in which direction.** `mj_obligation_inputs_hash`
//! interpolates the token's pathspecs unquoted, so the shell expands them as filename
//! globs before `git ls-files` ever sees them; without `globstar`, `**` matches one level.
//! `docs`, whose pathspecs include `.ai/repo/**/README.md`, therefore selects 255 files
//! there and 261 here, and `implementation`, whose pathspec is `*`, selects 1915 there and
//! 2480 here — a bare `*` expands to the top-level entries that are not dotfiles, so the
//! whole of `.ai/` and `.github/` is outside the hash the shell takes. This reader passes
//! the pathspecs to git verbatim, which is what the vocabulary's `inputs` plainly mean, so
//! its selection is a superset and the disagreement can only run one way: it may call
//! evidence stale that `finish` would still accept, and never the reverse. [`inputs_files`]
//! is on every entry so the difference is measurable from the surface rather than argued
//! about. The repair belongs to `lib/evidence.sh` — `set -f` around the `git ls-files`
//! call, so that word splitting still happens and pathname expansion does not.
//!
//! [`inputs_files`]: ObligationClosure::inputs_files
//!
//! ```
//! use majordomus_cli::capability::builtin::obligations;
//!
//! // two capabilities: what a token means, and where this checkout's task stands
//! let m = obligations::module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["obligations.vocabulary", "obligations.closure"]);
//!
//! // and both report; neither records. `majordomus evidence` is the writer.
//! assert!(m.capabilities.iter().all(|e| e.capability.kind.is_read_only()));
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::git::{self, GitState};
use crate::metadata::yaml;
use crate::{capability, module};

use super::continuity::{self, ActiveTask, Divergence};
use super::{get, Empty};

/// The URI under which the shipped vocabulary is read as an MCP resource.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::OBLIGATIONS_URI;
/// assert_eq!(OBLIGATIONS_URI, "majordomus://obligations");
/// ```
pub const OBLIGATIONS_URI: &str = "majordomus://obligations";

/// The URI under which this checkout's closure is read as an MCP resource.
///
/// Under the vocabulary's path rather than beside it: the closure is the vocabulary
/// applied to one task, and a reader that found the first has found the second.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::{CLOSURE_URI, OBLIGATIONS_URI};
/// assert!(CLOSURE_URI.starts_with(OBLIGATIONS_URI));
/// ```
pub const CLOSURE_URI: &str = "majordomus://obligations/closure";

/// The vocabulary file inside the distribution's share directory.
const VOCABULARY_FILE: &str = "obligations.yaml";

/// The local half of the layer, relative to the repository root. The same constant
/// [`super::continuity`] states, for the same reason: the shell tool decides where its
/// state lives and a second opinion about the path would be a second source of truth.
const STATE_DIR: &str = ".ai/local/state";

/// The ledger event that carries evidence, as `share/events.yaml` registers it.
const EVIDENCE_EVENT: &str = "task.evidence";

// ---------------------------------------------------------------- the vocabulary

/// One token a task may declare in `requires`, as the distribution ships it.
///
/// `remote` and `inputs` are what decide how its evidence is judged, and they are the
/// file's decision rather than this reader's: a fact the working tree cannot establish is
/// bound to the commit it was taken at, and everything else is bound to the bytes of the
/// files its pathspecs select.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::Obligation;
/// let push: Obligation = serde_json::from_str(
///     r#"{"id":"push","title":"The commit reached the remote",
///         "summary":"The branch's head exists on the remote it tracks.",
///         "discharged_by":"git","remote":true}"#,
/// )
/// .unwrap();
/// assert!(push.remote);
/// assert!(push.inputs.is_empty(), "a remote fact is bound to a commit, not to a tree");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Obligation {
    /// The token, as a task's `requires` names it.
    pub id: String,
    /// The obligation as a heading.
    pub title: String,
    /// What a worker is being asked to have done.
    pub summary: String,
    /// The command that produces the evidence. `none` for a token held by another line of
    /// the contract and listed so that a report can say it rather than leave a hole.
    pub discharged_by: String,
    /// The pathspecs the evidence is hashed over. Empty for a token whose fact is remote,
    /// and empty for one bound to neither: see [`ObligationClosure::staleness`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// True when the fact cannot be established from the working tree alone — a push, a
    /// publication, a deployment. Such evidence is bound to a commit, not to a tree.
    pub remote: bool,
    /// What the vocabulary says about the token beyond its summary.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// The vocabulary the distribution ships: every token there is.
///
/// `count` is measured from `obligations` rather than written down, so a token added to
/// the file is counted by the same act that declares it.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::Vocabulary;
/// let v: Vocabulary = serde_json::from_str(
///     r#"{"version":1,"source":"/opt/majordomus/share/obligations.yaml","count":1,
///         "obligations":[{"id":"commit","title":"The work is committed",
///                         "summary":"In the branch's history, not the working tree.",
///                         "discharged_by":"git","remote":false}]}"#,
/// )
/// .unwrap();
/// assert_eq!(v.count, v.obligations.len());
/// assert_eq!(v.version, 1, "the only shape this reader accepts");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Vocabulary {
    /// The file's own format version. `1` is the only one this reader accepts.
    pub version: u64,
    /// Where it was read from, absolute: the distribution's share directory, which is not
    /// necessarily inside the repository.
    pub source: String,
    /// How many tokens there are. No number anywhere is written down.
    pub count: usize,
    /// Every token, in the order the file declares them.
    pub obligations: Vec<Obligation>,
}

/// `share/obligations.yaml` as written. Unknown fields are not refused: the vocabulary
/// belongs to the shell tool, this is one of its readers, and `doctor` is what proves the
/// file well-formed. A reader that refused a field it had not been taught would turn
/// somebody else's addition into this process's failure.
#[derive(Debug, Default, Deserialize)]
struct VocabularyFile {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    obligations: Vec<Obligation>,
}

/// Read the shipped vocabulary from `share`, the distribution this process located, or —
/// when the caller has none, which is an index built without an application around it —
/// the way every other reader locates one, from `root`. A distribution that ships no
/// vocabulary declares no tokens, which is not an error but is reported as such by the
/// closure's findings when a task names one.
///
/// The distribution is passed in rather than resolved here because the two answers differ:
/// a process told which distribution to read (`--share`, or a released binary run inside a
/// repository that is not its own) has already resolved one, and a second resolution from
/// the repository root would find another distribution or, when the executable lives
/// outside the repository, none.
fn vocabulary_at(share: Option<&Path>, root: &Path) -> Result<Vocabulary, String> {
    let share = crate::share::Share::locate(share, root)
        .map_err(|e| format!("the distribution's share directory was not found: {e}"))?;
    let path = share.dir().join(VOCABULARY_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: VocabularyFile =
        yaml::parse_into(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.version != 1 {
        return Err(format!(
            "{}: the obligation vocabulary must be version 1, and this one is {}",
            path.display(),
            file.version
        ));
    }
    Ok(Vocabulary {
        version: file.version,
        source: path.to_string_lossy().to_string(),
        count: file.obligations.len(),
        obligations: file.obligations,
    })
}

// ---------------------------------------------------------------- the closure

/// Where one obligation stands.
///
/// The words are the validator's verdicts, not a severity scale: `stale` is not a worse
/// `owed`, it is evidence that was true and no longer describes what it proved.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::ObligationState;
/// // only one of the four lets a task be called completed
/// let completable = |s: ObligationState| s == ObligationState::Discharged;
/// assert!(completable(ObligationState::Discharged));
/// assert!(!completable(ObligationState::Stale), "evidence that no longer describes the tree");
/// assert!(!completable(ObligationState::Owed));
/// assert!(!completable(ObligationState::Undeclared));
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ObligationState {
    /// Declared, and no evidence for it was ever recorded.
    Owed,
    /// Evidence exists and still describes this tree, or this commit.
    Discharged,
    /// Evidence exists and no longer describes what it proved: the inputs changed, or the
    /// commit it named is not this one.
    Stale,
    /// The task requires a token the shipped vocabulary does not declare. Nothing can
    /// discharge it, because nothing knows what would.
    Undeclared,
}

impl ObligationState {
    /// The word as serialised, for a caller that tallies or renders states without
    /// carrying a copy of the vocabulary: the tallies of a [`Closure`] are keyed by
    /// exactly these strings, and so is anything that groups the entries by standing.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::obligations::ObligationState;
    /// assert_eq!(ObligationState::Stale.as_str(), "stale");
    /// assert_eq!(ObligationState::Undeclared.as_str(), "undeclared");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ObligationState::Owed => "owed",
            ObligationState::Discharged => "discharged",
            ObligationState::Stale => "stale",
            ObligationState::Undeclared => "undeclared",
        }
    }
}

/// The `task.evidence` line that discharged an obligation, as the ledger holds it.
///
/// Nothing older is consulted: evidence is superseded by evidence, and the ledger keeps
/// the history for a reader that wants it.
/// ```
/// use majordomus_cli::capability::builtin::obligations::Evidence;
/// let e: Evidence = serde_json::from_str(
///     r#"{"recorded_at":"2026-09-09T21:01:00Z","head":"f00ba7","branch":"master",
///         "kind":"ci","command":"scripts/ci/reference-check","inputs_hash":"7f68b9"}"#,
/// )
/// .unwrap();
/// assert_eq!(e.head, "f00ba7", "the envelope's commit is what a remote fact is judged against");
/// assert!(e.artifact.is_empty(), "one of command and artifact; narrative is not evidence");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    /// When the line was appended.
    pub recorded_at: String,
    /// The commit the ledger's envelope stamped on it.
    pub head: String,
    /// The branch it was recorded on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub branch: String,
    /// How it was taken: `test`, `build`, `ci`, `artifact` or `manual`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// The command that produced it. Narrative is not evidence, so one of this and
    /// `artifact` is always present.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    /// The reference it points at, such as a published URL.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub artifact: String,
    /// What the command said, when its output was the point.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub result: String,
    /// The episode that recorded it, when one was open.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub session: String,
    /// The hash of the token's declared inputs at the moment it was taken. Empty for a
    /// token that declares none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub inputs_hash: String,
}

/// One obligation the task declared, joined with what the vocabulary says about it and
/// with the evidence that does or does not discharge it.
///
/// The vocabulary's fields are repeated here on purpose: a client asking what this task
/// owes gets the token's title, its summary and the command that would discharge it in the
/// same answer, and needs no second call to render a report.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::{ObligationClosure, ObligationState};
/// let owed: ObligationClosure = serde_json::from_str(
///     r#"{"id":"pages","title":"The published site serves this commit","remote":true,
///         "state":"owed","detail":"owed, and no evidence was recorded",
///         "reproduce":"majordomus evidence --covers pages --command 'scripts/pages verify'"}"#,
/// )
/// .unwrap();
/// assert_eq!(owed.state, ObligationState::Owed);
/// assert!(owed.evidence.is_none() && owed.staleness.is_none(), "nothing to label");
/// assert!(owed.reproduce.starts_with("majordomus evidence"), "a finding carries its repair");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObligationClosure {
    /// The token, as the task's `requires` names it.
    pub id: String,
    /// From the vocabulary; the token itself when it declares none.
    pub title: String,
    /// From the vocabulary.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    /// From the vocabulary: the command that produces the evidence.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub discharged_by: String,
    /// From the vocabulary: whether the fact is remote.
    pub remote: bool,
    /// From the vocabulary: the pathspecs the evidence is hashed over.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// Where it stands.
    pub state: ObligationState,
    /// How far the evidence is from this checkout, in the repository's one staleness
    /// vocabulary. `None` when there is no evidence to label.
    ///
    /// A remote fact is labelled against the commit it was taken at, because the site that
    /// serves a commit goes on serving it while the tree moves underneath. A tree-bound
    /// fact is labelled by re-hashing: equal hashes are `exact`, and a difference takes the
    /// commit's label, which is `advanced` when the working tree has merely moved on and
    /// `diverged` when the history it named is gone. A token that declares neither inputs
    /// nor a remote fact is bound to nothing and stays `exact` once recorded — that is the
    /// validator's behaviour, and it is reproduced rather than tightened here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staleness: Option<Divergence>,
    /// The line that discharged it, or `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    /// What the token's inputs hash to in this tree now — the other half of "stale against
    /// what". `None` for a token that declares no inputs, and `None` when git could not be
    /// asked which files they select.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_hash_now: Option<String>,
    /// How many tracked files that hash was taken over. Reported because the shell's
    /// selection and this one are not always the same set — see the module header — and a
    /// count is the cheapest way for a reader to see it rather than be told it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs_files: Option<usize>,
    /// One line: what this obligation's standing actually is, in the words the validator
    /// uses when it refuses.
    pub detail: String,
    /// The command that would discharge it. Every finding here carries the way to act on
    /// it, as `project.finding-carries-reproduce` asks.
    pub reproduce: String,
}

/// What this checkout's active task owes, and how much of it is still true.
///
/// A clone that has never run the lifecycle answers this too, and answers it as absence:
/// `present` false, no task, and a finding saying so. "Nothing owed" and "nothing to owe
/// it" are different facts, and a served instance must not report the second as the first.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::Closure;
/// let fresh: Closure = serde_json::from_str(
///     r#"{"present":false,"worktree":"/srv/clone","branch":"master","closed":false,
///         "tallies":{},
///         "findings":["no active task in this checkout (.ai/local/state/current.yaml); nothing owes anything here"]}"#,
/// )
/// .unwrap();
/// assert!(!fresh.present && !fresh.closed, "nothing that does not exist is closed");
/// assert!(fresh.task.is_none() && fresh.obligations.is_empty());
/// assert_eq!(fresh.findings.len(), 1, "absence is reported, not implied");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Closure {
    /// Whether this checkout has a task to report about at all. False in a clone that has
    /// never run the lifecycle, which is not a fault and is not "nothing owed".
    pub present: bool,
    /// The worktree this answer is about. Every reading below is scoped to it, and to no
    /// other checkout of the same repository.
    pub worktree: String,
    /// The branch, or `DETACHED`.
    pub branch: String,
    /// The commit this checkout is on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub head: String,
    /// `clean` or `dirty`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub working_tree: String,
    /// The active task, or `None`. Its `requires` is the list the entries below expand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<ActiveTask>,
    /// True when the task declares obligations and every one of them is discharged and
    /// current — that is, when `majordomus.obligation-closure` would not refuse
    /// `finish --outcome completed`. False when anything is owed, stale or undeclared, and
    /// false when there is no task: nothing that does not exist is closed.
    pub closed: bool,
    /// How many obligations stand where, by state word. Absent states are absent rather
    /// than zero, so a reader never has to know the vocabulary to read the tallies.
    pub tallies: BTreeMap<String, usize>,
    /// Every obligation the task declared, in the order it declared them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub obligations: Vec<ObligationClosure>,
    /// What a reader should know before trusting any of the above: an unreadable
    /// vocabulary, a ledger that could not be read, a task that declares nothing. Empty is
    /// the ordinary case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

// ---------------------------------------------------------------- the input hash

/// The hash `mj_inputs_hash` computes, reproduced.
///
/// The shell hashes each selected file, renders one `<path> <sha256>` line per file in
/// git's own index order, and hashes that listing. Both halves matter: the order is git's,
/// so two runs over one tree agree, and the listing carries the path, so moving a file
/// changes the hash even when its bytes do not.
///
/// A file git tracks and the working tree does not hold is skipped rather than failed, for
/// the same reason the shell skips it — `shasum` writes to stderr and `xargs` carries on —
/// so a deleted-but-tracked file makes the hash differ rather than making the answer an
/// error.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::listing_hash;
/// // the listing is what is hashed, and it is one line per file, path first
/// assert_eq!(
///     listing_hash("a.txt e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n"),
///     listing_hash("a.txt e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\n")
/// );
/// assert_ne!(listing_hash("a.txt 00\n"), listing_hash("b.txt 00\n"));
/// ```
pub fn listing_hash(listing: &str) -> String {
    crate::policy::sha256_hex(listing)
}

/// The hash of one obligation's declared inputs in this tree and how many files it was
/// taken over, or `None` when the token declares none — which is the empty string in the
/// ledger and is compared as such.
pub(crate) fn inputs_hash(root: &Path, specs: &[String]) -> Option<(String, usize)> {
    if specs.is_empty() {
        return None;
    }
    let refs: Vec<&str> = specs.iter().map(String::as_str).collect();
    let files = git::ls_files_any(root, &refs).ok()?;
    if files.is_empty() {
        // the shell's empty answer: no file selected hashes to nothing at all, and the
        // recorded value for such a token is the empty string
        return Some((String::new(), 0));
    }
    let mut listing = String::new();
    for file in &files {
        let Ok(bytes) = std::fs::read(root.join(file)) else {
            continue;
        };
        listing.push_str(file);
        listing.push(' ');
        listing.push_str(&crate::policy::sha256_bytes_hex(&bytes));
        listing.push('\n');
    }
    Some((listing_hash(&listing), files.len()))
}

// ---------------------------------------------------------------- the ledger

/// The newest `task.evidence` line per token for one task, read from this checkout's
/// ledger. A line that is not JSON is skipped and counted, because a ledger that has grown
/// one bad line still holds the rest.
fn evidence_for(path: &Path, task: &str) -> (BTreeMap<String, Evidence>, usize) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (BTreeMap::new(), 0);
    };
    let mut out: BTreeMap<String, Evidence> = BTreeMap::new();
    let mut skipped = 0usize;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            skipped += 1;
            continue;
        };
        let s = |k: &str| {
            v.get(k)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        if s("event") != EVIDENCE_EVENT || s("task") != task {
            continue;
        }
        let covers = s("covers");
        if covers.is_empty() {
            skipped += 1;
            continue;
        }
        // last line wins: the file is append-only and ordered, so the last is the newest
        out.insert(
            covers,
            Evidence {
                recorded_at: s("ts"),
                head: s("head"),
                branch: s("branch"),
                kind: s("kind"),
                command: s("command"),
                artifact: s("artifact"),
                result: s("result"),
                session: s("session"),
                inputs_hash: s("inputs_hash"),
            },
        );
    }
    (out, skipped)
}

// ---------------------------------------------------------------- the judgement

/// `mj_git_label`, whole: the branch decides first, then equality, then ancestry. A
/// recorded branch this reader was not given is not a `different_context` — an older
/// ledger line carries no branch, and calling somebody else's work on it would be worse
/// than declining to.
fn label(
    root: &Path,
    recorded_head: &str,
    recorded_branch: &str,
    head: Option<&str>,
    branch: &str,
) -> Divergence {
    if !recorded_branch.is_empty() && recorded_branch != branch {
        return Divergence::DifferentContext;
    }
    continuity::divergence(root, recorded_head, head)
}

/// The standing of one declared token, by the rule `mj_validate_obligations` applies.
fn judge(
    root: &Path,
    token: &str,
    declared: Option<&Obligation>,
    evidence: Option<&Evidence>,
    head: Option<&str>,
    branch: &str,
) -> ObligationClosure {
    let Some(o) = declared else {
        return ObligationClosure {
            id: token.to_string(),
            title: token.to_string(),
            summary: String::new(),
            discharged_by: String::new(),
            remote: false,
            inputs: Vec::new(),
            state: ObligationState::Undeclared,
            staleness: None,
            evidence: evidence.cloned(),
            inputs_hash_now: None,
            inputs_files: None,
            detail: format!(
                "the task requires '{token}', which the shipped obligation vocabulary does not declare"
            ),
            reproduce: "majordomus evidence --help".into(),
        };
    };

    let reproduce = format!(
        "majordomus evidence --covers {} --command '{}'",
        o.id, o.discharged_by
    );
    let measured = if o.remote {
        // a remote fact is bound to a commit; re-hashing a tree it never described would
        // be a number nobody could act on
        None
    } else {
        inputs_hash(root, &o.inputs)
    };
    let hash_now = measured.as_ref().map(|(h, _)| h.clone());
    let files_now = measured.as_ref().map(|(_, n)| *n);

    let Some(ev) = evidence else {
        return ObligationClosure {
            id: o.id.clone(),
            title: o.title.clone(),
            summary: o.summary.clone(),
            discharged_by: o.discharged_by.clone(),
            remote: o.remote,
            inputs: o.inputs.clone(),
            state: ObligationState::Owed,
            staleness: None,
            evidence: None,
            inputs_hash_now: hash_now,
            inputs_files: files_now,
            detail: "owed, and no evidence was recorded".into(),
            reproduce,
        };
    };

    let commit = label(root, &ev.head, &ev.branch, head, branch);
    let short = |h: &str| h.chars().take(12).collect::<String>();

    let (staleness, detail) = if o.remote {
        let detail = match commit {
            Divergence::Exact => "discharged at this commit".to_string(),
            Divergence::Advanced => format!(
                "the evidence names {}, and the branch has moved since; the fact it proved is about the older commit",
                short(&ev.head)
            ),
            other => format!(
                "the evidence was taken in a {} context ({}); it does not describe this branch",
                other.as_str(),
                short(&ev.head)
            ),
        };
        (commit, detail)
    } else if hash_now.as_deref().unwrap_or("") == ev.inputs_hash {
        (
            Divergence::Exact,
            format!("discharged over inputs {}", short(&ev.inputs_hash)),
        )
    } else {
        // the tree no longer hashes to what the evidence proved. Which of the four words
        // that is, is the commit's answer — with `exact` ruled out, because a tree that
        // hashes differently at the same commit has moved on inside it.
        let moved = match commit {
            Divergence::Exact => Divergence::Advanced,
            other => other,
        };
        (
            moved,
            format!(
                "the evidence was taken over inputs {} and this tree hashes to {} over {} file(s); it no longer describes what it proved",
                short(&ev.inputs_hash),
                short(hash_now.as_deref().unwrap_or("")),
                files_now.unwrap_or(0)
            ),
        )
    };

    ObligationClosure {
        id: o.id.clone(),
        title: o.title.clone(),
        summary: o.summary.clone(),
        discharged_by: o.discharged_by.clone(),
        remote: o.remote,
        inputs: o.inputs.clone(),
        state: if staleness == Divergence::Exact {
            ObligationState::Discharged
        } else {
            ObligationState::Stale
        },
        staleness: Some(staleness),
        evidence: Some(ev.clone()),
        inputs_hash_now: hash_now,
        inputs_files: files_now,
        detail,
        reproduce,
    }
}

// ---------------------------------------------------------------- handlers

fn obligations_vocabulary(ctx: &Context, _: Empty) -> Result<Vocabulary, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    vocabulary_at(ctx.index.share.as_deref(), &root).map_err(CapabilityError::Internal)
}

fn obligations_closure(ctx: &Context, _: Empty) -> Result<Closure, CapabilityError> {
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
    let task = continuity::read_task(&dir.join("current.yaml"));

    let vocabulary = match vocabulary_at(ctx.index.share.as_deref(), &root) {
        Ok(v) => v.obligations,
        Err(reason) => {
            findings.push(format!(
                "the obligation vocabulary could not be read, so no token can be resolved: {reason}"
            ));
            Vec::new()
        }
    };

    let mut obligations = Vec::new();
    if let Some(t) = &task {
        if t.requires.is_empty() {
            findings.push(format!(
                "task {} declares no obligations, so this rule holds it to nothing beyond the rest of the contract",
                t.id
            ));
        } else {
            let (evidence, skipped) = evidence_for(&dir.join("ledger.jsonl"), &t.id);
            if skipped > 0 {
                findings.push(format!(
                    "{skipped} ledger line(s) could not be read and were skipped; run `majordomus doctor`"
                ));
            }
            for token in &t.requires {
                obligations.push(judge(
                    &root,
                    token,
                    vocabulary.iter().find(|o| &o.id == token),
                    evidence.get(token),
                    head.as_deref(),
                    &branch,
                ));
            }
        }
    } else {
        findings.push(format!(
            "no active task in this checkout ({STATE_DIR}/current.yaml); nothing owes anything here"
        ));
    }

    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for o in &obligations {
        *tallies.entry(o.state.as_str().to_string()).or_default() += 1;
    }

    Ok(Closure {
        present: task.is_some(),
        worktree: root.to_string_lossy().to_string(),
        branch,
        head: head.unwrap_or_default(),
        working_tree,
        closed: !obligations.is_empty()
            && obligations
                .iter()
                .all(|o| o.state == ObligationState::Discharged),
        task,
        tallies,
        obligations,
        findings,
    })
}

/// The module: the vocabulary every clone shares, and the closure only this checkout can
/// answer.
///
/// Both are read-only and neither has a command-line projection, for the reason
/// [`super::continuity`] states about the local half: `majordomus check` and
/// `majordomus finish` are already the command line's answer to this question, and a
/// second verb would be a second account of the same judgement.
///
/// ```
/// use majordomus_cli::capability::builtin::obligations::module;
/// let m = module();
/// assert_eq!(m.id.as_str(), "obligations");
/// // every projection is derived from the declaration; none is written anywhere else
/// let routes: Vec<&str> = m
///     .capabilities
///     .iter()
///     .filter_map(|e| e.capability.exposure.http.as_ref().map(|h| h.path.as_str()))
///     .collect();
/// assert_eq!(routes, ["/api/v1/obligations", "/api/v1/obligations/closure"]);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "obligations",
        title: "Obligations",
        description: "What a task owes before it may be called completed, and whether the evidence that discharged each obligation still describes this tree. The vocabulary is data the distribution ships and answers in any clone; the closure is read from the local half of the layer, which this process serves to the worker in front of it and never publishes. Read, never written: `majordomus evidence` records, and a second writer for one ledger would be a second account of the same events.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "obligations.vocabulary",
                title: "Every obligation there is",
                description: "The tokens a task may declare in `requires`: what each one asks of a worker, the command that discharges it, the pathspecs its evidence is hashed over, and whether its fact is remote and therefore bound to a commit rather than to a tree. Shipped data, identical in every clone, so this answers in a checkout that has never run the lifecycle.",
                input: Empty,
                output: Vocabulary,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_obligations".into()),
                        resource: Some(McpResource { uri: OBLIGATIONS_URI.into(), name: "obligations".into() }),
                    }),
                    http: get("/api/v1/obligations"),
                    cli: None,
                },
                tags: ["obligations", "completion"],
                // shipped data, immutable for the life of the process
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: None },
                handler: obligations_vocabulary,
            },
            capability! {
                id: "obligations.closure",
                title: "What this task still owes",
                description: "Every obligation the active task declared, joined with what the vocabulary says about it and with the evidence that does or does not discharge it: what is owed, what is discharged, and what has gone stale — with the recorded input hash and the tree's current one, or the recorded commit and its label, so a reader can see against what. The judgement is the one `finish` applies, reproduced rather than re-decided, and the staleness words are the repository's only four. A checkout with no task reports that, rather than reporting nothing owed.",
                input: Empty,
                output: Closure,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_obligation_closure".into()),
                        resource: Some(McpResource { uri: CLOSURE_URI.into(), name: "obligation-closure".into() }),
                    }),
                    http: get("/api/v1/obligations/closure"),
                    cli: None,
                },
                tags: ["obligations", "completion", "continuity"],
                // Short-lived, for continuity's reason: the ledger and the task record are
                // written by another process, and an answer cached for a minute would
                // report an obligation that had since been discharged.
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: obligations_closure,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these ids, tool names, resource URIs and routes
    /// exist, and every projection — the MCP tool, the MCP resource, the HTTP route, the
    /// OpenAPI operation, the benchmark target — is derived from it. A refactor that
    /// dropped an exposure or renamed a route would still compile, and every suite that
    /// exercised the behaviour behind it would still pass. This is the assertion that
    /// would not, which is what `project.rust-command-tested-in-file` asks of a command.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "obligations");
        let expected: &[(&str, &str, &str, &str)] = &[
            (
                "obligations.vocabulary",
                "majordomus_obligations",
                OBLIGATIONS_URI,
                "/api/v1/obligations",
            ),
            (
                "obligations.closure",
                "majordomus_obligation_closure",
                CLOSURE_URI,
                "/api/v1/obligations/closure",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
        for (executable, (id, tool, uri, path)) in m.capabilities.iter().zip(expected) {
            let c = &executable.capability;
            let mcp = c.exposure.mcp.as_ref().expect("an MCP projection");
            assert_eq!(mcp.tool.as_deref(), Some(*tool), "{id} lost its MCP tool");
            assert_eq!(
                mcp.resource.as_ref().map(|r| r.uri.as_str()),
                Some(*uri),
                "{id} lost its MCP resource"
            );
            assert_eq!(
                c.exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
            // this layer reports and never records, and the registry refuses an executable
            // capability that claims to be read-only and is not
            assert!(
                c.kind.is_read_only() && c.kind.is_executable(),
                "{id} is not read-only"
            );
            assert!(id.starts_with("obligations."));
        }
    }

    /// The vocabulary is read from the file rather than held here, which is the whole
    /// point: a token added to `share/obligations.yaml` is answered by this module without
    /// a line changing in it.
    #[test]
    fn the_vocabulary_is_read_from_the_file_and_not_held_here() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("kinds.yaml"), "version: 1\nkinds: []\n").unwrap();
        std::fs::write(
            dir.path().join(VOCABULARY_FILE),
            "# a comment the reader skips\nversion: 1\nobligations:\n  - id: tests\n    title: The cases were run\n    summary: They passed.\n    discharged_by: usecase impact\n    inputs: [\"lib/**\", \"share/**\"]\n    remote: false\n  - id: push\n    title: The commit reached the remote\n    summary: The branch head exists on the remote.\n    discharged_by: git\n    remote: true\n",
        )
        .unwrap();
        // the distribution is named, not searched for: the reader is handed one the way an
        // application hands it the one the process located, and the environment of whoever
        // runs the tests does not reach it
        let v = vocabulary_at(Some(dir.path()), dir.path()).expect("the vocabulary parses");

        assert_eq!(v.version, 1);
        assert_eq!(v.count, 2, "the count is measured, never written down");
        assert_eq!(v.obligations[0].id, "tests");
        assert_eq!(
            v.obligations[0].inputs,
            vec!["lib/**".to_string(), "share/**".to_string()],
            "an inline list of quoted pathspecs is the shape the file uses"
        );
        assert!(!v.obligations[0].remote);
        assert!(v.obligations[1].remote);
        assert!(
            v.obligations[1].inputs.is_empty(),
            "a remote fact hashes no tree"
        );
    }

    /// The four verdicts, decided the way `mj_validate_obligations` decides them. The
    /// point of the test is that the two surfaces cannot disagree: a token with no
    /// evidence is owed, a remote fact whose branch has moved on is stale, and a
    /// tree-bound fact is judged by its hash and not by its commit.
    #[test]
    fn the_verdict_is_the_validators_verdict() {
        let root = Path::new("/nonexistent-so-git-cannot-answer");
        let remote = Obligation {
            id: "push".into(),
            title: "The commit reached the remote".into(),
            summary: String::new(),
            discharged_by: "git".into(),
            inputs: Vec::new(),
            remote: true,
            note: String::new(),
        };
        let tree = Obligation {
            id: "tests".into(),
            title: "The cases were run".into(),
            summary: String::new(),
            discharged_by: "usecase impact".into(),
            // a pathspec git is never asked about, because the root does not exist: the
            // hash is `None`, which compares equal to the empty recorded hash
            inputs: vec!["lib/**".into()],
            remote: false,
            note: String::new(),
        };
        let ev = |head: &str, branch: &str, hash: &str| Evidence {
            recorded_at: "2026-09-09T00:00:00Z".into(),
            head: head.into(),
            branch: branch.into(),
            kind: "manual".into(),
            command: "git".into(),
            artifact: String::new(),
            result: String::new(),
            session: String::new(),
            inputs_hash: hash.into(),
        };

        // nothing recorded: owed, and the way to discharge it is in the answer
        let owed = judge(root, "push", Some(&remote), None, Some("aaa"), "master");
        assert_eq!(owed.state, ObligationState::Owed);
        assert_eq!(owed.staleness, None);
        assert_eq!(
            owed.reproduce,
            "majordomus evidence --covers push --command 'git'"
        );

        // a remote fact taken at this commit on this branch: discharged, exactly
        let ok = judge(
            root,
            "push",
            Some(&remote),
            Some(&ev("aaa", "master", "")),
            Some("aaa"),
            "master",
        );
        assert_eq!(ok.state, ObligationState::Discharged);
        assert_eq!(ok.staleness, Some(Divergence::Exact));

        // the same fact, recorded on another branch: not about this work at all
        let elsewhere = judge(
            root,
            "push",
            Some(&remote),
            Some(&ev("aaa", "other", "")),
            Some("aaa"),
            "master",
        );
        assert_eq!(elsewhere.state, ObligationState::Stale);
        assert_eq!(elsewhere.staleness, Some(Divergence::DifferentContext));

        // a tree-bound fact whose recorded hash is not this tree's: stale, and the answer
        // carries both hashes so a reader can see against what
        let moved = judge(
            root,
            "tests",
            Some(&tree),
            Some(&ev("aaa", "master", "deadbeefdeadbeef")),
            Some("aaa"),
            "master",
        );
        assert_eq!(moved.state, ObligationState::Stale);
        assert!(moved.detail.contains("deadbeefdead"));

        // a token nobody declares can be discharged by nothing, and says so
        let undeclared = judge(root, "invented", None, None, Some("aaa"), "master");
        assert_eq!(undeclared.state, ObligationState::Undeclared);
        assert!(undeclared.detail.contains("does not declare"));
    }

    /// The evidence a token carries is the newest line for it and nothing older: evidence
    /// is superseded by evidence, and the ledger keeps the history for a reader that wants
    /// it.
    #[test]
    fn the_newest_line_per_token_wins_and_other_tasks_are_not_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"ts":"1","event":"task.start","head":"a","branch":"m","task":"t-1"}"#,
                "\n",
                r#"{"ts":"2","event":"task.evidence","head":"a","branch":"m","task":"t-1","covers":"tests","kind":"test","inputs_hash":"old"}"#,
                "\n",
                r#"{"ts":"3","event":"task.evidence","head":"b","branch":"m","task":"t-1","covers":"tests","kind":"test","inputs_hash":"new"}"#,
                "\n",
                r#"{"ts":"4","event":"task.evidence","head":"c","branch":"m","task":"t-2","covers":"tests","kind":"test","inputs_hash":"other"}"#,
                "\n",
                "not json at all\n",
            ),
        )
        .unwrap();

        let (found, skipped) = evidence_for(&path, "t-1");
        assert_eq!(skipped, 1, "a line that is not JSON is counted, not fatal");
        assert_eq!(found.len(), 1, "another task's evidence is another task's");
        assert_eq!(found["tests"].inputs_hash, "new");
        assert_eq!(found["tests"].head, "b");
    }
}
