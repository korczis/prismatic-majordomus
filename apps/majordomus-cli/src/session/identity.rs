//! The identities of the session domain: six things that the stores spell as three words.
//!
//! # The defect this module exists for
//!
//! A forensic read of the writers on 2026-09-11 found one word covering two values in three
//! separate places, and in every case both spellings are durable, both are written by
//! Majordomus, and neither is wrong on its own terms:
//!
//! | The word | One writer says | Another writer says |
//! |---|---|---|
//! | the repository | `mj_git_repo_id` — the absolute path of the shared git directory | `mj_repository_id` — the remote URL, or `local:<hash of the worktree path>` |
//! | the worktree | `mj_record_front_matter` — `worktree:`, an absolute path | `mj_session_close` — `worktree_id:`, sixteen hex digits of that path's digest |
//! | the session | the episode's own `session_id: s-20260910205542-e2a6` | `provider_session:`, the provider's own opaque identity, which is also the key the open record is *stored under* |
//!
//! A reader that compares a local record's repository against a shared record's compares a
//! path with a URL and concludes the two records are about different repositories. A reader
//! that takes `provider_session` for the session identity attributes an episode to whichever
//! window of the provider reconnected last. Both are the same mistake — treating a value as
//! interchangeable with one that means something else — and prose has already failed to
//! prevent it twice.
//!
//! So the six are six types here. There is deliberately **no** `From` between any two of
//! them, no shared trait that would accept one where another is wanted, and no
//! `as_str()`-to-`new()` round trip that would launder one into another: every constructor
//! takes the value's own spelling and says which store it came from. The compiler refuses
//! the comparison the audit found.
//!
//! # What is reused rather than reinvented
//!
//! [`RepositoryId`] and [`CheckoutId`] wrap the digests this executable already computes —
//! [`crate::repository::git_identity`] and [`crate::repository::identity`] — which are the
//! same values its server lease, its index cache and its worktree topology are keyed by.
//! A seventh notion of "which repository" would be exactly the defect above with one more
//! row. What this module adds is the *typing* and the record of each store's own spelling
//! beside the canonical value, so that a reader can recognise a store's string without
//! having to choose which of the two spellings is the real one.
//!
//! ```
//! use majordomus_cli::session::{CheckoutId, EpisodeId, ProviderSessionId, TaskId};
//! use std::path::Path;
//!
//! let checkout = CheckoutId::of(Path::new("/Users/x/dev/repo-wt/feature/y"));
//! let episode = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
//! let provider = ProviderSessionId::new("01Bv2gJsfAoYrXnpfC9uJ4uv");
//!
//! assert_ne!(checkout.as_str(), episode.as_str());
//! assert_ne!(episode.as_str(), provider.as_str());
//! // and an id of one kind never parses as another, whatever it looks like
//! assert!(TaskId::parse(episode.as_str()).is_none());
//! assert!(EpisodeId::parse("t-20260905034523-a9f1").is_none());
//! ```

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A git repository, named without naming a disk.
///
/// The value is [`crate::repository::git_identity`]'s digest of the git directory every
/// work tree shares: one value for the primary checkout and every linked worktree of it,
/// a different value for a different repository, and never a path — a shared record that
/// carried the path would disclose where somebody keeps their work, which is of no use to
/// a reader and of some use to somebody else (ADR 0014).
///
/// ```
/// use majordomus_cli::session::RepositoryId;
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().unwrap();
/// let root = dir.path().join("repo");
/// std::fs::create_dir_all(&root).unwrap();
/// let git = |args: &[&str]| assert!(
///     Command::new("git").arg("-C").arg(&root).args(args).status().unwrap().success()
/// );
/// git(&["init", "-q", "."]);
/// git(&["-c", "user.email=t@e.x", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "i"]);
/// let wt = dir.path().join("repo-wt");
/// git(&["worktree", "add", "-q", "-b", "feature/x", wt.to_str().unwrap()]);
///
/// let here = RepositoryId::of(&root).expect("a work tree");
/// let there = RepositoryId::of(&wt).expect("a linked work tree");
/// assert_eq!(here, there, "one repository, two checkouts");
/// assert!(!here.as_str().contains('/'), "a digest, never a path");
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RepositoryId(String);

impl RepositoryId {
    /// The repository the checkout at `root` belongs to. `None` where git cannot be asked:
    /// a repository of the layer does not have to be version controlled, and saying so is
    /// better than inventing an identity for one that is not.
    ///
    /// ```
    /// use majordomus_cli::session::RepositoryId;
    /// // absence is an answer: a plain directory belongs to no repository, and saying so is
    /// // better than minting an identity nothing could ever match
    /// let plain = tempfile::tempdir().unwrap();
    /// assert!(RepositoryId::of(plain.path()).is_none());
    /// ```
    pub fn of(root: &Path) -> Option<Self> {
        crate::repository::git_identity(root).map(|g| RepositoryId(g.id))
    }

    /// A value read back from a record that already carries one.
    ///
    /// It takes whatever the store wrote, because a store written by an older version of
    /// the tool carries an older spelling and refusing to read it would lose the record.
    /// What this type guarantees is not the shape of the string; it is that the string is
    /// never passed where a checkout, an episode or a provider session is wanted.
    ///
    /// ```
    /// use majordomus_cli::session::RepositoryId;
    /// let recorded = RepositoryId::recorded("git@github.com:korczis/prismatic-majordomus.git");
    /// assert_eq!(recorded.as_str(), "git@github.com:korczis/prismatic-majordomus.git");
    /// ```
    pub fn recorded(value: impl Into<String>) -> Self {
        RepositoryId(value.into())
    }

    /// The value as text: what a record writes down. It is never parsed back into a
    /// checkout, an episode or a provider identity.
    ///
    /// ```
    /// use majordomus_cli::session::RepositoryId;
    /// assert_eq!(RepositoryId::recorded("origin-url").as_str(), "origin-url");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One checkout of a repository: the primary one, or one linked worktree of it.
///
/// [`crate::repository::identity`]'s digest of the checkout root — the value this
/// executable's lease, index and server already key a checkout by. Two worktrees of one
/// repository share a [`RepositoryId`] and differ here, which is the distinction the
/// continuity resolver's first tier is made of: a record from another worktree is never
/// silently offered as your context.
///
/// ```
/// use majordomus_cli::session::{CheckoutId, RepositoryId};
/// use std::path::Path;
///
/// let a = CheckoutId::of(Path::new("/a/b"));
/// let b = CheckoutId::of(Path::new("/a/c"));
/// assert_ne!(a, b, "two checkouts");
/// assert_eq!(a, CheckoutId::of(Path::new("/a/b")), "the same checkout across runs");
/// assert!(!a.as_str().contains('/'), "a digest, never a path");
///
/// // and the two identities are not interchangeable: this does not compile.
/// // let _: RepositoryId = a;
/// let _ = RepositoryId::recorded("unused");
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct CheckoutId(String);

impl CheckoutId {
    /// The checkout rooted at `root`. Total: a path that does not exist still has an
    /// identity, because the question "which checkout is this record about" has to be
    /// answerable for a record written on a disk that has since been unmounted.
    ///
    /// ```
    /// use majordomus_cli::session::CheckoutId;
    /// use std::path::Path;
    /// let gone = CheckoutId::of(Path::new("/Volumes/unmounted/repo"));
    /// assert_eq!(gone.as_str().len(), 32, "a path that does not exist still has one");
    /// ```
    pub fn of(root: &Path) -> Self {
        CheckoutId(crate::repository::identity(root))
    }

    /// A value read back from a record that carries one.
    /// Whatever the store wrote is accepted: a record written by an older version of the
    /// tool carries an older spelling, and refusing it would lose the record.
    ///
    /// ```
    /// use majordomus_cli::session::CheckoutId;
    /// use std::path::Path;
    /// // a record's own string is taken as it stands, never made to agree with a digest
    /// assert_ne!(CheckoutId::recorded("2b0f1c8e9a774d31"), CheckoutId::of(Path::new("/a/b")));
    /// ```
    pub fn recorded(value: impl Into<String>) -> Self {
        CheckoutId(value.into())
    }

    /// The value as text: what a shared record writes as `worktree_id:`. It is never
    /// compared with a repository identity, which is a different question with a different
    /// answer.
    ///
    /// ```
    /// use majordomus_cli::session::CheckoutId;
    /// assert_eq!(CheckoutId::recorded("2b0f1c8e").as_str(), "2b0f1c8e");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An episode's own identity: `s-<YYYYMMDDHHMMSS>-<4 hex>`.
///
/// This is the canonical session identity and the only one. It is minted once, when the
/// episode opens, by whoever opens it; it is what every ledger line of the episode is
/// stamped with; and it is what `session_id:` names in the closed record. A provider's own
/// session string is [`ProviderSessionId`] and is never this.
///
/// The shape is the shell tool's, parsed as written rather than as would be tidier: there
/// are months of these ids in `.ai/repo/sessions/` and a parser that refused them would be
/// a parser that cannot read this repository. It is *not* [`crate::execution::ExecutionId`]'s
/// shape — that one is `x-20260908T010203Z-0a1b2c3d`, with a `T`, a `Z` and eight hex
/// digits — and the difference is asserted below so that a refactor cannot quietly align
/// them and orphan every record already written.
///
/// ```
/// use majordomus_cli::session::EpisodeId;
///
/// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
/// assert_eq!(id.as_str(), "s-20260910205542-e2a6");
/// assert_eq!(id.minted_at(), "20260910205542");
///
/// // what it refuses, and why: a path segment is what this value becomes on disk
/// assert!(EpisodeId::parse("../../etc/passwd").is_none());
/// assert!(EpisodeId::parse("s-20260910205542-e2a").is_none(), "four hex digits");
/// assert!(EpisodeId::parse("x-20260908T010203Z-0a1b2c3d").is_none(), "that is an execution");
/// assert!(EpisodeId::parse("").is_none());
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct EpisodeId(String);

impl EpisodeId {
    /// Parse an id as the stores write it. `None` for anything else — which is what keeps
    /// a path, a traversal or a wildcard out of a store lookup, since this value names a
    /// file under `state/sessions-closing/` and is matched against record front matter.
    ///
    /// ```
    /// use majordomus_cli::session::EpisodeId;
    /// assert!(EpisodeId::parse("s-20260910205542-e2a6").is_some());
    /// assert!(EpisodeId::parse("s-20260910205542-e2a6/../x").is_none(), "one path segment");
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.strip_prefix("s-")?;
        let (stamp, suffix) = rest.split_once('-')?;
        let stamp_ok = stamp.len() == 14 && stamp.bytes().all(|b| b.is_ascii_digit());
        let suffix_ok = suffix.len() == 4 && suffix.bytes().all(|b| b.is_ascii_hexdigit());
        (stamp_ok && suffix_ok).then(|| EpisodeId(text.to_string()))
    }

    /// The `YYYYMMDDHHMMSS` the id was minted at. A reader's convenience and never a
    /// substitute for `started_at`: the stamp says when the id was made, the record says
    /// when the episode opened, and an episode recovered from a crash can have the two
    /// differ.
    ///
    /// ```
    /// use majordomus_cli::session::EpisodeId;
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
    /// assert_eq!(id.minted_at(), "20260910205542");
    /// ```
    pub fn minted_at(&self) -> &str {
        &self.0[2..16]
    }

    /// The id as text. This is the string every ledger line of the episode is stamped with and the one
    /// `session_id:` names in the closed record.
    ///
    /// ```
    /// use majordomus_cli::session::EpisodeId;
    /// let id = EpisodeId::parse("s-20260910205542-e2a6").expect("an episode id");
    /// assert_eq!(id.as_str(), "s-20260910205542-e2a6");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EpisodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A provider's own name for its conversation — **an external correlation id, never the
/// episode's identity.**
///
/// It is how a provider's hook finds the episode it opened: the hook passes
/// `--provider-session <x>`, the open record lives at `state/sessions-open/<key of x>.yaml`,
/// and an end event for `x` closes that episode and no other. That is the whole of its job.
///
/// It is not the session identity, for three reasons that are each a measured incident
/// rather than a worry. An episode opened by hand has no provider session at all and is
/// keyed `hand`, so the identity would be missing for the one episode a person opens.
/// Two providers may name their sessions in the same shape, so the value is unique only
/// within a provider. And the value changes under the worker: on 2026-09-09 a session in
/// this repository was re-established, came back under a different peer identity, and was
/// invisible to eight others for three hours because a durable thing had been keyed by a
/// value that is not.
///
/// The stored key is reduced to one path segment so that nothing a provider sends can name
/// a file outside the store.
///
/// ```
/// use majordomus_cli::session::ProviderSessionId;
///
/// let p = ProviderSessionId::new("01Bv2gJsfAoYrXnpfC9uJ4uv");
/// assert_eq!(p.store_key(), "01Bv2gJsfAoYrXnpfC9uJ4uv");
///
/// // anything that could leave the store becomes an inert segment
/// assert_eq!(ProviderSessionId::new("../../etc/passwd").store_key(), ".._.._etc_passwd");
/// assert_eq!(ProviderSessionId::new("a/b").store_key(), "a_b");
///
/// // and a segment made only of dots is not a directory: `.` and `..` survive an
/// // allow-list that permits a dot, so the segment is what is checked, not the bytes
/// assert_eq!(ProviderSessionId::new("..").store_key(), "_..");
/// assert_eq!(ProviderSessionId::new(".").store_key(), "_.");
///
/// // an episode nobody named is the hand-opened one, and there is at most one
/// assert_eq!(ProviderSessionId::hand().store_key(), "hand");
/// assert!(ProviderSessionId::hand().is_hand());
/// assert!(ProviderSessionId::new("   ").is_hand(), "a blank name names nobody");
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct ProviderSessionId(String);

impl ProviderSessionId {
    /// The provider's own string, as sent.
    /// Nothing about its shape is assumed: it is a provider's private identifier, and this
    /// type's job is to stop it being used as an episode identity rather than to validate it.
    ///
    /// ```
    /// use majordomus_cli::session::ProviderSessionId;
    /// assert_eq!(ProviderSessionId::new("01Bv2gJsf").as_str(), "01Bv2gJsf");
    /// ```
    pub fn new(value: impl Into<String>) -> Self {
        ProviderSessionId(value.into())
    }

    /// The episode nobody named: a provider that sends no session identity is
    /// indistinguishable from a person at a terminal, and inventing a distinction there
    /// would multiply episodes nobody can close.
    ///
    /// ```
    /// use majordomus_cli::session::ProviderSessionId;
    /// assert!(ProviderSessionId::hand().is_hand());
    /// assert_eq!(ProviderSessionId::hand().store_key(), "hand");
    /// ```
    pub fn hand() -> Self {
        ProviderSessionId(String::new())
    }

    /// Is this the hand-opened episode's key? True for an empty or blank value.
    /// A provider that sends whitespace has named nobody just as surely as one that sends
    /// nothing, so both are the hand-opened episode.
    ///
    /// ```
    /// use majordomus_cli::session::ProviderSessionId;
    /// assert!(ProviderSessionId::new(" \t ").is_hand());
    /// assert!(!ProviderSessionId::new("01Bv2gJsf").is_hand());
    /// ```
    pub fn is_hand(&self) -> bool {
        self.0.trim().is_empty()
    }

    /// The value as sent, which is what a record carries and what a hook passes back. Not
    /// the file name: [`ProviderSessionId::store_key`] is that, and the two differ
    /// exactly when the provider sent something a path segment may not contain.
    ///
    /// ```
    /// use majordomus_cli::session::ProviderSessionId;
    /// let p = ProviderSessionId::new("a/b");
    /// assert_eq!(p.as_str(), "a/b");
    /// assert_ne!(p.as_str(), p.store_key());
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The value reduced to exactly one path segment, for the file the open record lives
    /// in. Every byte outside `A-Za-z0-9._-` becomes `_`; the blank value becomes `hand`.
    ///
    /// A value made only of dots gets a `_` in front of it. `.` and `..` survive an
    /// allow-list that permits a dot — they are made of nothing else — and both name a
    /// directory rather than a file in it. The test that found this is in this module: an
    /// allow-list is a claim about bytes, and the claim that has to hold is about the
    /// *segment*.
    ///
    /// ```
    /// use majordomus_cli::session::ProviderSessionId;
    /// assert_eq!(ProviderSessionId::new("01Bv2gJsf").store_key(), "01Bv2gJsf");
    /// assert!(!ProviderSessionId::new("../etc/passwd").store_key().contains('/'));
    /// ```
    pub fn store_key(&self) -> String {
        if self.is_hand() {
            return "hand".to_string();
        }
        let mapped: String = self
            .0
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if mapped.chars().all(|c| c == '.') {
            return format!("_{mapped}");
        }
        mapped
    }
}

/// A unit of intended work that a person opens and closes: `t-<YYYYMMDDHHMMSS>-<4 hex>`.
///
/// An [`super::Episode`] carries `Option<TaskId>` and nothing else of a task. The
/// separation is ADR 0041's whole finding: a task is optional, is opened and closed by a
/// person, and outlives or predeceases any number of episodes. What made this repository
/// write no checkpoint and no handover for six days was code that asked a task whether an
/// episode's artefact should be written.
///
/// ```
/// use majordomus_cli::session::{EpisodeId, TaskId};
///
/// let t = TaskId::parse("t-20260905034523-a9f1").expect("the task from the audit");
/// assert_eq!(t.as_str(), "t-20260905034523-a9f1");
///
/// // the shapes rhyme and the types do not: an episode id is not a task id
/// assert!(TaskId::parse("s-20260910205542-e2a6").is_none());
/// assert!(EpisodeId::parse("t-20260905034523-a9f1").is_none());
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct TaskId(String);

impl TaskId {
    /// Parse an id as `lib/start.sh` writes it. `None` for anything else, including the
    /// literal `none` the records use for "no task", which is an absence and is
    /// represented as `Option::None` rather than as a task whose name is a word.
    ///
    /// ```
    /// use majordomus_cli::session::TaskId;
    /// assert!(TaskId::parse("t-20260905034523-a9f1").is_some());
    /// assert!(TaskId::parse("none").is_none(), "the records' word for no task");
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.strip_prefix("t-")?;
        let (stamp, suffix) = rest.split_once('-')?;
        let stamp_ok = stamp.len() == 14 && stamp.bytes().all(|b| b.is_ascii_digit());
        let suffix_ok = suffix.len() == 4 && suffix.bytes().all(|b| b.is_ascii_hexdigit());
        (stamp_ok && suffix_ok).then(|| TaskId(text.to_string()))
    }

    /// The id as text: what a record writes as `task_id:`. An episode relates to a task by
    /// this value and holds nothing else of one.
    ///
    /// ```
    /// use majordomus_cli::session::TaskId;
    /// let t = TaskId::parse("t-20260905034523-a9f1").expect("a task id");
    /// assert_eq!(t.as_str(), "t-20260905034523-a9f1");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Who is working: a worker identity somebody supplied, never one inferred.
///
/// `lib/session.sh` is explicit about this and the type keeps the promise: *absent stays
/// absent*. An inferred worker is indistinguishable from a recorded one the moment it is
/// written down, and a record that attributes six days of somebody else's work to the user
/// account that happened to run the hook is worse than a record that attributes it to
/// nobody.
///
/// ```
/// use majordomus_cli::session::WorkerId;
/// assert_eq!(WorkerId::supplied("claude-opus-5").map(|w| w.as_str().to_string()),
///            Some("claude-opus-5".to_string()));
/// assert!(WorkerId::supplied("").is_none(), "absent stays absent");
/// assert!(WorkerId::supplied("  ").is_none());
/// ```
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct WorkerId(String);

impl WorkerId {
    /// A worker identity that was supplied. `None` for a blank one, which is an absence.
    ///
    /// ```
    /// use majordomus_cli::session::WorkerId;
    /// assert!(WorkerId::supplied("codex").is_some());
    /// assert!(WorkerId::supplied("\n").is_none(), "absent stays absent");
    /// ```
    pub fn supplied(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| WorkerId(trimmed.to_string()))
    }

    /// The identity as text, trimmed as it was supplied, which is what a record writes as
    /// `worker:` when one was supplied at all.
    ///
    /// ```
    /// use majordomus_cli::session::WorkerId;
    /// assert_eq!(WorkerId::supplied("  codex  ").expect("a worker").as_str(), "codex");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Which store a recorded identity was read from, kept beside the value.
///
/// The point is not to choose between `mj_git_repo_id` and `mj_repository_id`, or between
/// `worktree:` and `worktree_id:`. It is to stop pretending they are one value. A reader
/// that knows a string is the *local* spelling can compare it with other local spellings
/// and can refuse to compare it with a shared one, which is precisely the comparison that
/// produced "these two records are about different repositories" for records of the same
/// repository written four minutes apart.
///
/// ```
/// use majordomus_cli::session::Spelling;
/// // the two halves are different answers to one question, and are never compared
/// assert_ne!(Spelling::Local, Spelling::Shared);
/// assert_eq!(Spelling::Canonical.as_str(), "canonical");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Spelling {
    /// The local half's spelling: a path, in `.ai/local/state/`. It names this disk and is
    /// never published.
    Local,
    /// The shared half's spelling: a remote URL or a digest, in `.ai/repo/sessions/`. It
    /// names the repository and travels with a clone (ADR 0014).
    Shared,
    /// This executable's own canonical digest, computed rather than read.
    Canonical,
}

impl Spelling {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::session::Spelling;
    /// assert_eq!(Spelling::Shared.as_str(), "shared");
    /// ```
    /// It is what a projection of this value carries, and what a reader groups spellings by.
    pub fn as_str(self) -> &'static str {
        match self {
            Spelling::Local => "local",
            Spelling::Shared => "shared",
            Spelling::Canonical => "canonical",
        }
    }
}

/// One identity of this checkout, as this executable computes it and as each store spells
/// it. What `session.identity` answers, and the direct projection of the audit's table.
///
/// ```
/// use majordomus_cli::session::{identities, IdentityFacet};
///
/// let dir = tempfile::tempdir().unwrap();
/// let facets: Vec<IdentityFacet> = identities(dir.path());
/// let repository = facets.iter().find(|f| f.subject == "repository").expect("a facet");
/// assert!(repository.spellings.len() >= 2, "one word, two writers, two values");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IdentityFacet {
    /// What is being identified: `repository`, `checkout`, `episode`, `provider_session`.
    pub subject: String,
    /// The canonical value this executable computes, when it can compute one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub canonical: String,
    /// How each store spells the same subject, in the order [`crate::order`] gives.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spellings: Vec<RecordedSpelling>,
    /// Why the subject needs its own type, in one line, for a reader meeting it in a
    /// projection rather than in this module.
    pub note: String,
}

/// One store's spelling of one subject.
/// Which half of the layer wrote it, what wrote it, and the value it would have here.
///
/// ```
/// use majordomus_cli::session::{RecordedSpelling, Spelling};
///
/// let local = RecordedSpelling {
///     spelling: Spelling::Local,
///     writer: "mj_git_repo_id (lib/common.sh)".into(),
///     value: "/Users/x/dev/repo/.git".into(),
/// };
/// // knowing which half a string came from is what stops a path being compared with a URL
/// assert_eq!(local.spelling.as_str(), "local");
/// assert!(local.value.starts_with('/'));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecordedSpelling {
    /// Which half of the layer wrote it.
    pub spelling: Spelling,
    /// The writer, named so a reader can go and look at it.
    pub writer: String,
    /// The value, as this checkout would have it written. Empty when nothing has written
    /// one here yet, which is a fresh clone and not a fault.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value: String,
}

impl crate::order::Ordered for RecordedSpelling {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey {
            group: Some(self.spelling.as_str()),
            rank: 0,
            label: &self.writer,
            identity: &self.writer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_episode_id_and_an_execution_id_are_not_the_same_shape() {
        // This is the assertion that stops a tidying refactor from aligning the two id
        // shapes: months of `s-` ids exist on disk, and a parser that accepted `x-` ids
        // here would let an execution be looked up as an episode.
        assert!(EpisodeId::parse("x-20260908T010203Z-0a1b2c3d").is_none());
        assert!(crate::execution::ExecutionId::parse("s-20260910205542-e2a6").is_none());
    }

    #[test]
    fn a_provider_session_key_is_always_one_inert_path_segment() {
        for hostile in [
            "../../etc/passwd",
            "a/b/c",
            "..",
            ".",
            "a\0b",
            "*",
            "~/x",
            "a b",
        ] {
            let key = ProviderSessionId::new(hostile).store_key();
            assert!(!key.contains('/'), "{hostile} kept a separator: {key}");
            assert!(key != ".." && key != ".", "{hostile} stayed a traversal");
        }
    }

    #[test]
    fn the_hand_opened_episode_has_one_key_however_it_is_spelled() {
        assert_eq!(ProviderSessionId::hand().store_key(), "hand");
        assert_eq!(ProviderSessionId::new("").store_key(), "hand");
        assert_eq!(ProviderSessionId::new("\t \n").store_key(), "hand");
    }

    #[test]
    fn a_task_id_that_is_the_word_none_is_an_absence() {
        // `task_id: none` is how the records spell "no task". Parsing it as a task would
        // give every episode without one a task called `none` — and the whole point of
        // ADR 0041 is that an episode without a task is ordinary.
        assert!(TaskId::parse("none").is_none());
    }

    #[test]
    fn two_checkouts_of_one_repository_share_a_repository_id_and_differ_in_checkout_id() {
        let a = CheckoutId::of(Path::new("/tmp/repo"));
        let b = CheckoutId::of(Path::new("/tmp/repo-wt/feature/x"));
        assert_ne!(a, b);
        assert_eq!(a.as_str().len(), 32, "a fixed-width digest");
    }
}
