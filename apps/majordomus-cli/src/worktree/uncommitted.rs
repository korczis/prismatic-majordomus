//! Uncommitted work in the linked work trees, and whether anything still holds it (I1995).
//!
//! Twice in two days finished work existed on one disk only: 45 uncommitted files in the
//! work tree of a session that had ended on 2026-10-07, 21 staged files in another on
//! 2026-10-08. Neither had a commit or a remote ref, and both were found because somebody
//! happened to read that directory. `doctor` named branches whose commits reach no remote and
//! said nothing about an index that holds work no commit carries.
//!
//! This is the reading that names them. For every linked work tree with staged, unstaged,
//! untracked or conflicted paths it says how many, how old the oldest change is, and what
//! holds the work — and from those, whether it is abandoned:
//!
//! * **held** — an active task record (`.ai/local/state/current.yaml` with outcome
//!   `active`, the record `start` writes and `finish` closes), an open episode
//!   (`.ai/local/state/sessions-open/`) that is not stranded, or a process whose working
//!   directory is inside it. The first two are the lifecycle's own records, read with the
//!   readers that already exist for them ([`crate::session::open_episodes`],
//!   [`Episode::stranded_after`](crate::session::Episode::stranded_after)); the third is the
//!   `lsof` reading `worktree cleanup` refuses a removal on. Nothing new is stored.
//! * **abandoned** — nothing holds it and its oldest change is older than the threshold.
//! * **recent** — nothing holds it and it is younger than that.
//!
//! The threshold is `session.stranded_after` from the policy: the silence after which an
//! episode's client is taken to be gone. One number answers both questions it is asked
//! here — whether an open episode still holds the work, and whether work nobody holds has
//! waited long enough to be called abandoned — so there is no second number to drift from
//! it. A caller with better evidence than a clock names its own (`older_than`).
//!
//! The reading writes nothing. Every `git status` runs under `GIT_OPTIONAL_LOCKS=0`, so no
//! index refresh and no lock reaches any work tree, and another work tree's `.ai/local/` is
//! read, never written. What it suggests for an abandoned work tree — the command that puts
//! the work on a remote ref — is a suggestion; nothing here runs it.
//!
//! # The lifecycle, end to end
//!
//! A linked work tree gets a staged file; the same work is `recent` against a day and
//! `abandoned` against an hour once its file is years old; and an active task record then
//! holds it. That the readings leave every index byte-identical is the module's own test.
//!
//! ```
//! use majordomus_cli::worktree::{Threshold, WorkStanding, WorktreeService};
//! use std::path::Path;
//! use std::process::Command;
//!
//! let dir = tempfile::tempdir().unwrap();
//! let git = |cwd: &Path, args: &[&str]| {
//!     let ok = Command::new("git")
//!         .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
//!         .args(args)
//!         .current_dir(cwd)
//!         .status()
//!         .unwrap();
//!     assert!(ok.success(), "git {args:?}");
//! };
//! let repo = dir.path().join("repo");
//! std::fs::create_dir_all(&repo).unwrap();
//! git(&repo, &["init", "-q"]);
//! git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
//! let left = dir.path().join("left");
//! git(&repo, &["worktree", "add", "-q", "-b", "work/left", left.to_str().unwrap()]);
//! std::fs::write(left.join("a.txt"), "a\n").unwrap();
//! git(&left, &["add", "a.txt"]);
//!
//! let service = WorktreeService::open(&repo).unwrap();
//! let day = Threshold::resolve(None, Some("1d")).unwrap();
//! let fresh = service.uncommitted(&day).unwrap();
//! assert_eq!(fresh.tallies.uncommitted, 1, "the primary checkout is never listed");
//! assert_eq!(fresh.worktrees[0].work.staged, 1);
//! assert_eq!(fresh.worktrees[0].standing, WorkStanding::Recent);
//!
//! // the file is made years old: against an hour, nothing holding it, it is abandoned
//! let touched = Command::new("touch")
//!     .args(["-t", "202001010000"])
//!     .arg(left.join("a.txt"))
//!     .status()
//!     .unwrap();
//! assert!(touched.success());
//! let hour = Threshold::resolve(None, Some("1h")).unwrap();
//! let old = service.uncommitted(&hour).unwrap();
//! assert_eq!(old.worktrees[0].standing, WorkStanding::Abandoned);
//! assert!(old.worktrees[0].remedy.contains("HEAD:refs/heads/rescue/work-left-"));
//!
//! // an active task record holds it, however old it is
//! let state = left.join(".ai/local/state");
//! std::fs::create_dir_all(&state).unwrap();
//! std::fs::write(state.join("current.yaml"), "id: t-1\noutcome: active\n").unwrap();
//! let held = service.uncommitted(&hour).unwrap();
//! let entry = held.worktrees.iter().find(|w| w.label == "work/left").unwrap();
//! assert_eq!(entry.standing, WorkStanding::Held);
//! assert_eq!(entry.task.as_deref(), Some("t-1"));
//! ```

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::Result;
use super::model::DirtyState;
use super::path::detached_label;
use super::service::WorktreeService;
use super::state;

/// The schema of [`UncommittedReport`].
pub const UNCOMMITTED_SCHEMA: &str = "majordomus-worktree-uncommitted/v1";

/// The policy key the threshold is read from.
pub const THRESHOLD_KEY: &str = "session.stranded_after";

/// Where a work tree's active task record lives, relative to it.
const TASK_RECORD: &str = ".ai/local/state/current.yaml";

/// The age past which uncommitted work nothing holds is abandoned, and where it came from.
///
/// It is reported with every reading, so a reader can see what the verdicts were judged
/// against; a threshold nothing declares is carried as absent, not filled in.
///
/// ```
/// use majordomus_cli::worktree::Threshold;
/// let declared = Threshold::resolve(Some("12h"), None).unwrap();
/// let wire = serde_json::to_value(&declared).unwrap();
/// assert_eq!(wire["declared"], "12h");
/// assert_eq!(wire["seconds"], 43_200);
/// let absent: Threshold =
///     serde_json::from_value(serde_json::json!({"source": "absent"})).unwrap();
/// assert_eq!((absent.declared, absent.seconds), (None, None), "absent is not a default");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Threshold {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The value as written (`12h`). Absent when nothing declares one.
    pub declared: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The same value, in seconds.
    pub seconds: Option<u64>,
    /// [`THRESHOLD_KEY`], `older_than` when the caller named one, or `absent`.
    pub source: String,
}

impl Threshold {
    /// The threshold from the caller's own value when it gave one, else from the policy's.
    /// A value that is not a duration is refused rather than read as none: a reading that
    /// judged against nothing would call no work abandoned and say so with confidence.
    ///
    /// ```
    /// use majordomus_cli::worktree::Threshold;
    /// let t = Threshold::resolve(Some("12h"), None).unwrap();
    /// assert_eq!((t.seconds, t.source.as_str()), (Some(43_200), "session.stranded_after"));
    /// let o = Threshold::resolve(Some("12h"), Some("30m")).unwrap();
    /// assert_eq!((o.seconds, o.source.as_str()), (Some(1_800), "older_than"));
    /// let none = Threshold::resolve(None, None).unwrap();
    /// assert_eq!((none.seconds, none.source.as_str()), (None, "absent"));
    /// assert!(Threshold::resolve(Some("soon"), None).is_err());
    /// ```
    pub fn resolve(
        declared: Option<&str>,
        older_than: Option<&str>,
    ) -> std::result::Result<Threshold, String> {
        let (value, source) = match (older_than, declared) {
            (Some(v), _) => (v, "older_than"),
            (None, Some(v)) => (v, THRESHOLD_KEY),
            (None, None) => {
                return Ok(Threshold {
                    declared: None,
                    seconds: None,
                    source: "absent".into(),
                })
            }
        };
        let seconds = crate::cli::parse_duration(value).map_err(|e| format!("{source}: {e}"))?;
        Ok(Threshold {
            declared: Some(value.to_string()),
            seconds: Some(seconds),
            source: source.into(),
        })
    }

    /// The threshold a repository declares, or the caller's own: the policy of the
    /// checkout at `root` is read for [`THRESHOLD_KEY`], and a policy that cannot be read
    /// declares nothing.
    ///
    /// ```
    /// use majordomus_cli::worktree::Threshold;
    /// // a directory with no layer declares nothing, and the caller's own value still counts
    /// let plain = tempfile::tempdir().unwrap();
    /// let none = Threshold::declared_at(plain.path(), None).unwrap();
    /// assert_eq!((none.seconds, none.source.as_str()), (None, "absent"));
    /// let own = Threshold::declared_at(plain.path(), Some("2h")).unwrap();
    /// assert_eq!((own.seconds, own.source.as_str()), (Some(7_200), "older_than"));
    /// assert!(Threshold::declared_at(plain.path(), Some("later")).is_err());
    /// ```
    pub fn declared_at(root: &Path, older_than: Option<&str>) -> std::result::Result<Self, String> {
        let declared = crate::repository::Repository::discover(root)
            .ok()
            .and_then(|r| crate::policy::LoadedPolicy::load(&r).ok())
            .and_then(|p| p.policy.session.stranded_after);
        Threshold::resolve(declared.as_deref(), older_than)
    }
}

/// What a work tree's uncommitted work is, judged: held by something, recent, or abandoned.
/// [`judge`] is the one place it is decided, and `doctor` warns only on `abandoned`.
///
/// ```
/// use majordomus_cli::worktree::uncommitted::{judge, WorkStanding};
/// let standing = judge(false, Some(90_000), Some(43_200));
/// assert_eq!(standing, WorkStanding::Abandoned);
/// assert_eq!(serde_json::to_value(standing).unwrap(), "abandoned");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkStanding {
    /// A task, an episode or a process holds it.
    Held,
    /// Nothing holds it, and it is younger than the threshold.
    Recent,
    /// Nothing holds it, and it is older than the threshold — or its age cannot be read.
    Abandoned,
}

impl WorkStanding {
    /// The standing as one lowercase word, the same word the JSON answer carries, for the
    /// text rendering of `worktree uncommitted` and for doctor's selection of what to warn on.
    ///
    /// ```
    /// use majordomus_cli::worktree::WorkStanding;
    /// for s in [WorkStanding::Held, WorkStanding::Recent, WorkStanding::Abandoned] {
    ///     assert_eq!(serde_json::to_value(s).unwrap(), s.as_str(), "one word on both surfaces");
    /// }
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            WorkStanding::Held => "held",
            WorkStanding::Recent => "recent",
            WorkStanding::Abandoned => "abandoned",
        }
    }
}

/// An open episode in a work tree, as far as holding its work goes: it holds the work only
/// while `stranded` is `Some(false)`; a stranded or unjudgeable one holds nothing.
///
/// ```
/// use majordomus_cli::worktree::OpenEpisode;
/// let unjudged = OpenEpisode {
///     id: "s-20260101000000-0a0a".into(),
///     last_sign_of_life: None,
///     stranded: None,
/// };
/// let wire = serde_json::to_value(&unjudged).unwrap();
/// assert!(wire.get("stranded").is_none(), "what could not be judged is absent, not false");
/// let back: OpenEpisode = serde_json::from_value(wire).unwrap();
/// assert_eq!(back, unjudged);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OpenEpisode {
    /// The episode.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Its newest sign of life, RFC 3339 in UTC.
    pub last_sign_of_life: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Silent longer than the threshold. Absent when that cannot be judged — no threshold,
    /// or no readable sign of life — and then it holds nothing.
    pub stranded: Option<bool>,
}

/// One linked work tree holding uncommitted work: how much, how old, what holds it, the
/// verdict, and the command that would preserve it. A listing is in canonical order —
/// abandoned first, then recent, then held, the oldest first inside each.
///
/// ```
/// use majordomus_cli::worktree::{DirtyState, UncommittedWorktree, WorkStanding};
/// let entry = |path: &str, standing: WorkStanding, age: u64| UncommittedWorktree {
///     path: path.into(),
///     branch: None,
///     label: path.into(),
///     work: DirtyState { staged: 1, ..DirtyState::default() },
///     paths: 1,
///     oldest_change: None,
///     age_seconds: Some(age),
///     age: None,
///     task: None,
///     episodes: Vec::new(),
///     process_inside: None,
///     held_by: Vec::new(),
///     standing,
///     remedy: String::new(),
/// };
/// let mut listing = vec![
///     entry("/wt/held-old", WorkStanding::Held, 900_000),
///     entry("/wt/young", WorkStanding::Abandoned, 50_000),
///     entry("/wt/older", WorkStanding::Abandoned, 90_000),
/// ];
/// majordomus_cli::order::canonical(&mut listing);
/// let paths: Vec<&str> = listing.iter().map(|w| w.path.as_str()).collect();
/// assert_eq!(paths, ["/wt/older", "/wt/young", "/wt/held-old"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UncommittedWorktree {
    /// Absolute, as git holds it.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch checked out there. Absent when detached.
    pub branch: Option<String>,
    /// The branch, or `detached/<short commit>`.
    pub label: String,
    /// The counts, as `worktree status` gives them.
    pub work: DirtyState,
    /// Every changed path: staged, unstaged, untracked and conflicted together.
    pub paths: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// When the oldest change was made: the earliest modification time among the changed
    /// paths, the index file's standing in for a path that no longer exists. RFC 3339.
    pub oldest_change: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Seconds since then.
    pub age_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The same, for a person (`3d`, `5h`, `40m`).
    pub age: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The active task recorded there.
    pub task: Option<String>,
    /// The episodes open there.
    pub episodes: Vec<OpenEpisode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A process has its working directory inside it. Absent when that could not be read.
    pub process_inside: Option<bool>,
    /// What holds it, in words; empty when nothing does.
    pub held_by: Vec<String>,
    /// The verdict.
    pub standing: WorkStanding,
    /// The command that would put this work on a remote ref. Suggested, never run.
    pub remedy: String,
}

/// Abandoned work first, then recent, then held; inside each, the oldest first; the path
/// breaks a tie, so two readings of one disk list it in one order.
impl crate::order::Ordered for UncommittedWorktree {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        // a standing outweighs any age: a century is ~3.2e9 seconds
        const STANDING: i64 = 1_000_000_000_000;
        let standing = match self.standing {
            WorkStanding::Abandoned => 0,
            WorkStanding::Recent => 1,
            WorkStanding::Held => 2,
        };
        let age = self.age_seconds.map_or(0, |a| {
            i64::try_from(a).unwrap_or(i64::MAX).min(STANDING - 1)
        });
        crate::order::OrderKey::plain(&self.path, &self.path).ranked(standing * STANDING - age)
    }
}

/// The counts of an [`UncommittedReport`]: how many linked work trees were read, how many
/// hold uncommitted work, and how those split into held, recent and abandoned. The three
/// standings always add up to `uncommitted`; `doctor` reads `abandoned` to decide whether it
/// has anything to warn about.
///
/// ```
/// use majordomus_cli::worktree::UncommittedTallies;
/// let wire = serde_json::json!(
///     {"measured": 4, "uncommitted": 3, "held": 1, "recent": 1, "abandoned": 1}
/// );
/// let t: UncommittedTallies = serde_json::from_value(wire.clone()).unwrap();
/// assert_eq!(t.held + t.recent + t.abandoned, t.uncommitted);
/// assert_eq!(serde_json::to_value(&t).unwrap(), wire, "the shape doctor's jq reads");
/// assert_eq!(UncommittedTallies::default().abandoned, 0, "nothing read, nothing abandoned");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UncommittedTallies {
    /// Linked work trees whose status was read.
    pub measured: usize,
    /// Of those, the ones holding uncommitted work: every entry of the listing.
    pub uncommitted: usize,
    /// Held by a task, an episode or a process.
    pub held: usize,
    /// Held by nothing, younger than the threshold.
    pub recent: usize,
    /// Held by nothing, older than the threshold.
    pub abandoned: usize,
}

/// Every linked work tree holding uncommitted work, with what holds it: the answer of
/// `worktree.uncommitted` on every surface, and what `doctor` renders.
///
/// ```
/// use majordomus_cli::worktree::uncommitted::UNCOMMITTED_SCHEMA;
/// use majordomus_cli::worktree::{Threshold, UncommittedReport, WorktreeService};
/// let dir = tempfile::tempdir().unwrap();
/// let init = std::process::Command::new("git")
///     .args(["init", "-q"])
///     .current_dir(dir.path())
///     .status()
///     .unwrap();
/// assert!(init.success());
/// // a repository with no linked work tree, and a policy that declares no threshold
/// let threshold = Threshold::resolve(None, None).unwrap();
/// let report: UncommittedReport =
///     WorktreeService::open(dir.path()).unwrap().uncommitted(&threshold).unwrap();
/// assert_eq!(report.schema, UNCOMMITTED_SCHEMA);
/// assert!(report.worktrees.is_empty());
/// assert_eq!(report.tallies.measured, 0);
/// assert_eq!(report.findings.len(), 1, "a missing threshold is said, not assumed");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UncommittedReport {
    /// [`UNCOMMITTED_SCHEMA`].
    pub schema: String,
    /// When the reading was taken, RFC 3339 in UTC.
    pub observed_at: String,
    /// The threshold it was judged against.
    pub threshold: Threshold,
    /// Every linked work tree with uncommitted work, abandoned first, then oldest first.
    pub worktrees: Vec<UncommittedWorktree>,
    /// The counts.
    pub tallies: UncommittedTallies,
    /// What could not be read or judged, in words.
    pub findings: Vec<String>,
}

/// A path quoted for a POSIX shell.
///
/// ```
/// use majordomus_cli::worktree::uncommitted::shell_quote;
/// assert_eq!(shell_quote("/a b/c"), "'/a b/c'");
/// assert_eq!(shell_quote("it's"), r"'it'\''s'");
/// ```
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// The command that puts a work tree's uncommitted work on a remote ref named
/// `rescue/<label>-<YYYYMMDD>`: stage everything, commit it there, and push that commit to a
/// branch of its own, so that the branch the work tree holds is not pushed over. The shape
/// the two rescues of October 2026 took (`origin/rescue/completion-doctrine-20261008`).
///
/// ```
/// use majordomus_cli::worktree::uncommitted::rescue_command;
/// let c = rescue_command("/r-wt/feature/x", "feature/x", "2026-10-08T10:00:00Z");
/// assert_eq!(
///     c,
///     "git -C '/r-wt/feature/x' add -A && git -C '/r-wt/feature/x' commit -m 'wip: rescue uncommitted work' \
///      && git -C '/r-wt/feature/x' push origin HEAD:refs/heads/rescue/feature-x-20261008"
/// );
/// ```
pub fn rescue_command(path: &str, label: &str, now: &str) -> String {
    let day: String = now.chars().take(10).filter(|c| *c != '-').collect();
    let name: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let p = shell_quote(path);
    format!(
        "git -C {p} add -A && git -C {p} commit -m 'wip: rescue uncommitted work' \
         && git -C {p} push origin HEAD:refs/heads/rescue/{name}-{day}"
    )
}

fn epoch(t: SystemTime) -> Option<u64> {
    t.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Judge one work tree's work from what holds it, its age and the threshold. The one place
/// the verdict is decided.
///
/// ```
/// use majordomus_cli::worktree::uncommitted::{judge, WorkStanding};
/// assert_eq!(judge(true, Some(10), Some(5)), WorkStanding::Held, "held is held, however old");
/// assert_eq!(judge(false, Some(10), Some(5)), WorkStanding::Abandoned);
/// assert_eq!(judge(false, Some(3), Some(5)), WorkStanding::Recent);
/// assert_eq!(judge(false, None, Some(5)), WorkStanding::Abandoned, "an unreadable age is not young");
/// assert_eq!(judge(false, Some(10), None), WorkStanding::Recent, "no threshold judges nothing old");
/// ```
pub fn judge(held: bool, age_seconds: Option<u64>, threshold: Option<u64>) -> WorkStanding {
    if held {
        return WorkStanding::Held;
    }
    match (age_seconds, threshold) {
        (_, None) => WorkStanding::Recent,
        (None, Some(_)) => WorkStanding::Abandoned,
        (Some(age), Some(t)) if age > t => WorkStanding::Abandoned,
        _ => WorkStanding::Recent,
    }
}

impl WorktreeService {
    /// Every linked work tree holding uncommitted work, with how much, how old, what holds
    /// it and whether it is abandoned against `threshold`. Reads every work tree and writes
    /// none: see the module documentation.
    ///
    /// ```
    /// use majordomus_cli::worktree::{Threshold, WorkStanding, WorktreeService};
    /// use std::path::Path;
    /// use std::process::Command;
    /// let dir = tempfile::tempdir().unwrap();
    /// let git = |cwd: &Path, args: &[&str]| {
    ///     let ok = Command::new("git")
    ///         .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
    ///         .args(args)
    ///         .current_dir(cwd)
    ///         .status()
    ///         .unwrap();
    ///     assert!(ok.success(), "git {args:?}");
    /// };
    /// let repo = dir.path().join("repo");
    /// std::fs::create_dir_all(&repo).unwrap();
    /// git(&repo, &["init", "-q"]);
    /// git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
    /// let (clean, dirty) = (dir.path().join("clean"), dir.path().join("dirty"));
    /// git(&repo, &["worktree", "add", "-q", "-b", "work/clean", clean.to_str().unwrap()]);
    /// git(&repo, &["worktree", "add", "-q", "-b", "work/dirty", dirty.to_str().unwrap()]);
    /// std::fs::write(dirty.join("new.txt"), "work\n").unwrap();
    ///
    /// let threshold = Threshold::resolve(Some("12h"), None).unwrap();
    /// let report = WorktreeService::open(&repo).unwrap().uncommitted(&threshold).unwrap();
    /// assert_eq!(report.tallies.measured, 2, "both linked work trees were read");
    /// assert_eq!(report.worktrees.len(), 1, "only the one with work is listed");
    /// let w = &report.worktrees[0];
    /// assert_eq!((w.label.as_str(), w.paths, w.work.untracked), ("work/dirty", 1, 1));
    /// assert_eq!(w.standing, WorkStanding::Recent, "written a moment ago");
    /// ```
    pub fn uncommitted(&self, threshold: &Threshold) -> Result<UncommittedReport> {
        let now = SystemTime::now();
        let now_secs = epoch(now).unwrap_or(0);
        let observed_at = crate::peers::rfc3339(now);
        let records: Vec<_> = self
            .identity()
            .registered_worktrees()
            .iter()
            .filter(|r| !r.bare && !self.identity().is_primary(r) && r.path.is_dir())
            .collect();
        let probes: Vec<Option<&Path>> = records.iter().map(|r| Some(r.path.as_path())).collect();
        let measured = state::probe_all(&probes, |p| state::aged_dirty_state(p).ok());

        let mut findings = Vec::new();
        if threshold.seconds.is_none() {
            findings.push(format!(
                "the policy declares no {THRESHOLD_KEY}, so no work is judged abandoned and no \
                 episode stranded; every listing below is reported as found"
            ));
        }
        let unread = measured.iter().filter(|m| m.is_none()).count();
        if unread > 0 {
            findings.push(format!(
                "{unread} linked work tree(s) would not answer `git status`; their work is not listed"
            ));
        }

        let dirty: Vec<(
            &super::topology::WorktreeRecord,
            DirtyState,
            Option<SystemTime>,
        )> = records
            .iter()
            .zip(measured)
            .filter_map(|(r, m)| m.map(|(d, t)| (*r, d, t)))
            .filter(|(_, d, _)| !d.clean)
            .collect();
        let dirs: Vec<&Path> = dirty.iter().map(|(r, _, _)| r.path.as_path()).collect();
        let inside = state::processes_inside(&dirs);
        if inside.is_none() && !dirs.is_empty() {
            findings.push(
                "whether a process works inside a work tree could not be read (no lsof); \
                 only task and episode records were taken as holding work"
                    .into(),
            );
        }

        let mut worktrees: Vec<UncommittedWorktree> = Vec::new();
        for (i, (record, work, oldest)) in dirty.into_iter().enumerate() {
            let path = super::service::display(&record.path);
            let label = match (&record.branch, &record.head) {
                (Some(b), _) => b.clone(),
                (None, Some(h)) => detached_label(h),
                (None, None) => "detached".into(),
            };
            let task =
                crate::capability::builtin::continuity::read_task(&record.path.join(TASK_RECORD))
                    .filter(|t| t.outcome == "active")
                    .map(|t| t.id);
            let episodes: Vec<OpenEpisode> = crate::session::open_episodes(&record.path)
                .into_iter()
                .map(|e| OpenEpisode {
                    id: e.id.as_str().to_string(),
                    last_sign_of_life: e.last_sign_of_life.and_then(|s| u64::try_from(s).ok()).map(
                        |s| crate::peers::rfc3339(UNIX_EPOCH + std::time::Duration::from_secs(s)),
                    ),
                    stranded: threshold.seconds.and_then(|t| {
                        e.stranded_after(
                            i64::try_from(now_secs).unwrap_or(i64::MAX),
                            i64::try_from(t).unwrap_or(i64::MAX),
                        )
                    }),
                })
                .collect();
            let process_inside = inside.as_ref().and_then(|v| v.get(i).copied());

            let mut held_by = Vec::new();
            if let Some(t) = &task {
                held_by.push(format!("task {t}"));
            }
            for e in episodes.iter().filter(|e| e.stranded == Some(false)) {
                held_by.push(format!("episode {}", e.id));
            }
            if process_inside == Some(true) {
                held_by.push("a process working inside".into());
            }

            let age_seconds = oldest.and_then(epoch).map(|t| now_secs.saturating_sub(t));
            let standing = judge(!held_by.is_empty(), age_seconds, threshold.seconds);
            worktrees.push(UncommittedWorktree {
                remedy: rescue_command(&path, &label, &observed_at),
                path,
                branch: record.branch.clone(),
                label,
                paths: work.staged + work.unstaged + work.untracked + work.conflicted,
                work,
                oldest_change: oldest.map(crate::peers::rfc3339),
                age: age_seconds.map(|s| {
                    crate::capability::builtin::continuity::span(
                        i64::try_from(s / 60).unwrap_or(i64::MAX),
                    )
                }),
                age_seconds,
                task,
                episodes,
                process_inside,
                held_by,
                standing,
            });
        }
        crate::order::canonical(&mut worktrees);

        let count = |s: WorkStanding| worktrees.iter().filter(|w| w.standing == s).count();
        let tallies = UncommittedTallies {
            measured: records.len() - unread,
            uncommitted: worktrees.len(),
            held: count(WorkStanding::Held),
            recent: count(WorkStanding::Recent),
            abandoned: count(WorkStanding::Abandoned),
        };
        Ok(UncommittedReport {
            schema: UNCOMMITTED_SCHEMA.into(),
            observed_at,
            threshold: threshold.clone(),
            worktrees,
            tallies,
            findings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(cwd: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(cwd)
            .status()
            .expect("run git");
        assert!(ok.success(), "git {args:?}");
    }

    fn age_years(path: &Path) {
        let ok = Command::new("touch")
            .args(["-t", "202001010000"])
            .arg(path)
            .status()
            .expect("run touch");
        assert!(ok.success());
    }

    /// A repository with three linked work trees: staged work nobody holds, an old change an
    /// active task holds, and an old untracked file whose only episode has been silent since
    /// 2020. The first and third are abandoned, the second is held, and no index moved.
    #[test]
    fn work_nothing_holds_is_abandoned_and_the_reading_moves_no_index() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        std::fs::write(repo.join("tracked.txt"), "one\n").unwrap();
        git(&repo, &["add", "tracked.txt"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        let wt = |name: &str| {
            let p = dir.path().join(name);
            git(
                &repo,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    &format!("work/{name}"),
                    p.to_str().unwrap(),
                ],
            );
            p
        };
        let (staged, tasked, silent) = (wt("staged"), wt("tasked"), wt("silent"));

        std::fs::write(staged.join("a.txt"), "a\n").unwrap();
        git(&staged, &["add", "a.txt"]);
        age_years(&staged.join("a.txt"));

        std::fs::write(tasked.join("tracked.txt"), "two\n").unwrap();
        age_years(&tasked.join("tracked.txt"));
        std::fs::create_dir_all(tasked.join(".ai/local/state")).unwrap();
        std::fs::write(
            tasked.join(TASK_RECORD),
            "id: t-20260101000000-beef\noutcome: active\n",
        )
        .unwrap();

        std::fs::write(silent.join("c.txt"), "c\n").unwrap();
        age_years(&silent.join("c.txt"));
        let open = silent.join(crate::session::store::OPEN_DIR);
        std::fs::create_dir_all(&open).unwrap();
        std::fs::write(
            open.join("gone.yaml"),
            "session_id: s-20200101000000-0a0a\nstarted_at: 2020-01-01T00:00:00Z\n",
        )
        .unwrap();

        let indexes: Vec<_> = [&staged, &tasked, &silent]
            .iter()
            .map(|w| state::git_dir_of(w).unwrap().join("index"))
            .collect();
        let before: Vec<_> = indexes.iter().map(|i| std::fs::read(i).ok()).collect();

        let service = WorktreeService::open(&repo).unwrap();
        let threshold = Threshold::resolve(Some("12h"), None).unwrap();
        let report = service.uncommitted(&threshold).unwrap();

        let by = |label: &str| {
            report
                .worktrees
                .iter()
                .find(|w| w.label == label)
                .unwrap_or_else(|| panic!("{label} is listed: {report:#?}"))
        };
        let s = by("work/staged");
        assert_eq!(s.standing, WorkStanding::Abandoned);
        assert_eq!((s.paths, s.work.staged), (1, 1));
        assert!(s.age_seconds.unwrap() > 43_200);
        assert!(s.held_by.is_empty());
        let t = by("work/tasked");
        assert_eq!(t.standing, WorkStanding::Held);
        assert_eq!(t.task.as_deref(), Some("t-20260101000000-beef"));
        assert_eq!(t.held_by, ["task t-20260101000000-beef"]);
        let g = by("work/silent");
        assert_eq!(g.standing, WorkStanding::Abandoned, "{g:#?}");
        assert_eq!(g.episodes.len(), 1);
        assert_eq!(g.episodes[0].stranded, Some(true), "silent since 2020");

        assert_eq!(report.tallies.measured, 3);
        assert_eq!(
            (
                report.tallies.abandoned,
                report.tallies.held,
                report.tallies.recent
            ),
            (2, 1, 0)
        );
        assert_ne!(
            report.worktrees.last().map(|w| w.standing),
            Some(WorkStanding::Abandoned),
            "held work is listed after abandoned work"
        );
        let after: Vec<_> = indexes.iter().map(|i| std::fs::read(i).ok()).collect();
        assert_eq!(before, after, "the reading wrote an index");
    }

    /// The caller's own threshold answers both questions the policy's does: against a
    /// hundred thousand days nothing is old, and the episode silent since 2020 holds its work.
    #[test]
    fn the_callers_threshold_also_decides_whether_an_episode_holds_the_work() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
        let silent = dir.path().join("silent");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "work/silent",
                silent.to_str().unwrap(),
            ],
        );
        std::fs::write(silent.join("c.txt"), "c\n").unwrap();
        age_years(&silent.join("c.txt"));
        let open = silent.join(crate::session::store::OPEN_DIR);
        std::fs::create_dir_all(&open).unwrap();
        std::fs::write(
            open.join("gone.yaml"),
            "session_id: s-20200101000000-0a0a\nstarted_at: 2020-01-01T00:00:00Z\n",
        )
        .unwrap();

        let service = WorktreeService::open(&repo).unwrap();
        let generous = Threshold::resolve(Some("12h"), Some("100000d")).unwrap();
        let report = service.uncommitted(&generous).unwrap();
        let w = &report.worktrees[0];
        assert_eq!(w.episodes[0].stranded, Some(false));
        assert_eq!(w.standing, WorkStanding::Held);
        assert_eq!(w.held_by, ["episode s-20200101000000-0a0a"]);

        // no threshold at all: nothing is judged old, nothing stranded, and it is said
        let none = service
            .uncommitted(&Threshold::resolve(None, None).unwrap())
            .unwrap();
        assert_eq!(none.worktrees[0].standing, WorkStanding::Recent);
        assert_eq!(none.worktrees[0].episodes[0].stranded, None);
        assert!(none.findings[0].contains(THRESHOLD_KEY));
    }

    #[test]
    fn a_rescue_names_a_ref_of_its_own_and_quotes_the_path() {
        let c = rescue_command("/a b/it's", "fix/x y", "2026-10-14T16:00:00Z");
        assert!(c.starts_with(r"git -C '/a b/it'\''s' add -A && "), "{c}");
        assert!(
            c.ends_with("push origin HEAD:refs/heads/rescue/fix-x-y-20261014"),
            "{c}"
        );
    }
}
