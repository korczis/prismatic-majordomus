//! How far a checkout is from the remote trunk: the one measurement behind
//! `project.a-checkout-serves-the-current-trunk`.
//!
//! On 2026-10-16 the primary checkout of this repository was 779 commits behind
//! `origin/master`, and its shared server answered every session's plan questions from that
//! tree; the local `master`, which `worktree create` then used as a base, was as far behind.
//! Nothing said so. This module says so: the checkout's `HEAD` and the local trunk branch,
//! each counted against the remote-tracking trunk, judged against the thresholds the policy
//! declares under `checkout.trunk_distance`, and rendered by `worktree status`, by the
//! shared server's `server.status`, by the MCP `initialize` instructions and by `doctor`.
//!
//! Everything is read from git on every call and nothing is fetched: the answer is "as of
//! the last fetch", and the time of that fetch is part of it. A remote trunk that cannot be
//! named is `unmeasured`, a distance with no declared threshold is `unjudged` — neither is
//! ever reported as current, and no threshold is invented here (ADR 0167 records the
//! cooperation model that asks for an always-current checkout; this module is its
//! measurement, not its restatement).
//!
//! The measurement's life, from git's count to the word every surface shows:
//!
//! ```
//! use majordomus_cli::worktree::trunk::{judge, parse_left_right, TrunkStanding, TrunkThresholds};
//!
//! // `git rev-list --left-right --count HEAD...origin/master` on the primary of 2026-10-16
//! let d = parse_left_right("0\t779").unwrap();
//! let declared: TrunkThresholds =
//!     serde_json::from_str(r#"{"warn_behind": 50, "fail_behind": 300}"#).unwrap();
//! let standing = judge(Some(d.behind), declared);
//! assert_eq!(standing, TrunkStanding::Stale);
//! assert_eq!(standing.as_str(), "stale");
//! // nothing declared: the same distance is reported, never judged current
//! assert_eq!(judge(Some(d.behind), TrunkThresholds::default()), TrunkStanding::Unjudged);
//! ```

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::git;
use super::identity::{RepositoryIdentity, Trunk};

/// The policy key the thresholds are declared under.
pub const POLICY_KEY: &str = "checkout.trunk_distance";

/// How many commits two refs have that the other lacks.
///
/// ```
/// use majordomus_cli::worktree::trunk::Distance;
/// let d = Distance { behind: 779, ahead: 0 };
/// assert_eq!(serde_json::to_value(d).unwrap(), serde_json::json!({"behind": 779, "ahead": 0}));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Distance {
    /// Commits of the remote trunk this ref lacks.
    pub behind: u64,
    /// Commits of this ref the remote trunk lacks.
    pub ahead: u64,
}

/// The thresholds `checkout.trunk_distance` declares, in commits behind the remote trunk.
/// Absent is absent: the reader supplies no default, and a distance with nothing to judge
/// it against is reported as `unjudged`.
///
/// ```
/// use majordomus_cli::worktree::trunk::TrunkThresholds;
/// let t: TrunkThresholds = serde_json::from_str(r#"{"warn_behind": 50, "fail_behind": 300}"#).unwrap();
/// assert_eq!((t.warn_behind, t.fail_behind), (Some(50), Some(300)));
/// assert_eq!(TrunkThresholds::default().warn_behind, None, "absent is not a default");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct TrunkThresholds {
    /// At or past this many commits behind, the checkout is `lagging`: a warning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn_behind: Option<u64>,
    /// At or past this many commits behind, the checkout is `stale`: a failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail_behind: Option<u64>,
}

impl TrunkThresholds {
    /// The thresholds the policy of the repository holding `root` declares; none when it
    /// declares none or cannot be read.
    ///
    /// ```
    /// use majordomus_cli::worktree::trunk::TrunkThresholds;
    ///
    /// // a directory no repository holds declares nothing, and nothing is invented for it
    /// let away = std::env::temp_dir().join(format!("mj-no-repo-{}", std::process::id()));
    /// std::fs::create_dir_all(&away).unwrap();
    /// assert_eq!(TrunkThresholds::declared_at(&away), TrunkThresholds::default());
    /// std::fs::remove_dir_all(&away).unwrap();
    /// ```
    pub fn declared_at(root: &Path) -> Self {
        crate::repository::Repository::discover(root)
            .ok()
            .and_then(|r| crate::policy::LoadedPolicy::load(&r).ok())
            .and_then(|p| p.policy.checkout.trunk_distance)
            .unwrap_or_default()
    }

    fn is_declared(&self) -> bool {
        self.warn_behind.is_some() || self.fail_behind.is_some()
    }
}

/// Where a checkout stands against the remote trunk.
///
/// ```
/// use majordomus_cli::worktree::trunk::TrunkStanding;
/// let s: TrunkStanding = serde_json::from_str(r#""lagging""#).unwrap();
/// assert_eq!(s, TrunkStanding::Lagging);
/// assert_eq!(serde_json::to_value(TrunkStanding::Unmeasured).unwrap(), "unmeasured");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrunkStanding {
    /// Behind by less than the warn threshold, or not behind at all.
    Current,
    /// At or past the warn threshold: what it serves is old enough to mislead.
    Lagging,
    /// At or past the fail threshold: what it serves is the wrong tree.
    Stale,
    /// Behind, and the policy declares no threshold to judge the distance by.
    Unjudged,
    /// The distance could not be measured: no trunk, no remote-tracking trunk, no `HEAD`.
    Unmeasured,
}

impl TrunkStanding {
    /// The word the JSON answer carries.
    ///
    /// ```
    /// use majordomus_cli::worktree::trunk::TrunkStanding;
    /// for s in [TrunkStanding::Current, TrunkStanding::Lagging, TrunkStanding::Stale,
    ///           TrunkStanding::Unjudged, TrunkStanding::Unmeasured] {
    ///     assert_eq!(serde_json::to_value(s).unwrap(), s.as_str());
    /// }
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            TrunkStanding::Current => "current",
            TrunkStanding::Lagging => "lagging",
            TrunkStanding::Stale => "stale",
            TrunkStanding::Unjudged => "unjudged",
            TrunkStanding::Unmeasured => "unmeasured",
        }
    }
}

/// Judge a distance behind the remote trunk against the declared thresholds. Pure: every
/// branch of the decision is a test.
///
/// ```
/// use majordomus_cli::worktree::trunk::{judge, TrunkStanding, TrunkThresholds};
/// let t = TrunkThresholds { warn_behind: Some(50), fail_behind: Some(300) };
/// assert_eq!(judge(None, t), TrunkStanding::Unmeasured);
/// assert_eq!(judge(Some(0), t), TrunkStanding::Current);
/// assert_eq!(judge(Some(49), t), TrunkStanding::Current);
/// assert_eq!(judge(Some(50), t), TrunkStanding::Lagging);
/// assert_eq!(judge(Some(779), t), TrunkStanding::Stale);
/// // nothing declared: zero is current, anything else has nothing to be judged by
/// assert_eq!(judge(Some(0), TrunkThresholds::default()), TrunkStanding::Current);
/// assert_eq!(judge(Some(3), TrunkThresholds::default()), TrunkStanding::Unjudged);
/// ```
pub fn judge(behind: Option<u64>, thresholds: TrunkThresholds) -> TrunkStanding {
    let Some(behind) = behind else {
        return TrunkStanding::Unmeasured;
    };
    if behind == 0 {
        return TrunkStanding::Current;
    }
    if !thresholds.is_declared() {
        return TrunkStanding::Unjudged;
    }
    if thresholds.fail_behind.is_some_and(|f| behind >= f) {
        TrunkStanding::Stale
    } else if thresholds.warn_behind.is_some_and(|w| behind >= w) {
        TrunkStanding::Lagging
    } else {
        TrunkStanding::Current
    }
}

/// A checkout's distance from the remote trunk, judged.
///
/// ```
/// use majordomus_cli::worktree::trunk::{Distance, TrunkDistance, TrunkStanding};
/// let d: TrunkDistance = serde_json::from_value(serde_json::json!({
///     "standing": "stale", "local_trunk_standing": "stale",
///     "worktree": "/repo", "on_trunk": true,
///     "checkout": {"behind": 779, "ahead": 0},
///     "thresholds": {"warn_behind": 50, "fail_behind": 300},
///     "message": "779 commits behind origin/master",
/// })).unwrap();
/// assert_eq!(d.standing, TrunkStanding::Stale);
/// assert_eq!(d.checkout, Some(Distance { behind: 779, ahead: 0 }));
/// // what is absent stays absent in the answer
/// assert!(serde_json::to_value(&d).unwrap().get("remedy").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrunkDistance {
    /// Where the checkout stands: its `HEAD` — with the commit a merge in progress brings
    /// in — judged against the thresholds. This is what its server serves.
    pub standing: TrunkStanding,
    /// Where the local trunk branch stands, judged the same way: what a branch made from it
    /// starts from. The same as `standing` when the checkout holds the trunk.
    pub local_trunk_standing: TrunkStanding,
    /// The work tree measured.
    pub worktree: String,
    /// The work tree holds the trunk branch itself.
    pub on_trunk: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The trunk branch, short.
    pub trunk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The remote-tracking trunk counted against (`origin/master`).
    pub remote_trunk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit the remote-tracking trunk names.
    pub remote_head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The checkout's `HEAD` against the remote trunk. While a merge is in progress, the
    /// commits it brings in count as present: the commit being made will hold them, so the
    /// merge that brings a checkout current is never judged by the tree it is replacing.
    pub checkout: Option<Distance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The local trunk branch against the remote trunk: what a branch made from the local
    /// trunk starts from.
    pub local_trunk: Option<Distance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// When the repository last fetched (the time of `FETCH_HEAD`), RFC 3339: the moment
    /// this answer is true as of. Absent when it never fetched.
    pub fetched_at: Option<String>,
    /// The thresholds judged against, as `checkout.trunk_distance` declares them.
    pub thresholds: TrunkThresholds,
    /// What the standing means here, in one line.
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commands that bring the checkout and the local trunk current, when either is
    /// behind. Suggested, never run.
    pub remedy: Option<String>,
}

impl TrunkDistance {
    fn unmeasured(worktree: &Path, trunk: Option<String>, why: String) -> Self {
        TrunkDistance {
            standing: TrunkStanding::Unmeasured,
            local_trunk_standing: TrunkStanding::Unmeasured,
            worktree: worktree.display().to_string(),
            on_trunk: false,
            trunk,
            remote_trunk: None,
            remote_head: None,
            checkout: None,
            local_trunk: None,
            fetched_at: None,
            thresholds: TrunkThresholds::default(),
            message: why,
            remedy: None,
        }
    }
}

/// The remote-tracking ref of the trunk, short (`origin/master`): the trunk's configured
/// upstream, else the remote whose `HEAD` names the trunk, else the first remote that has
/// a branch of the trunk's name. `None` when no remote carries it.
///
/// ```
/// use majordomus_cli::worktree::trunk::remote_trunk_of;
/// use std::{path::Path, process::Command};
/// let git = |dir: &Path, args: &[&str]| assert!(Command::new("git").current_dir(dir).args(args)
///     .env("GIT_AUTHOR_NAME", "a").env("GIT_AUTHOR_EMAIL", "a@b.c")
///     .env("GIT_COMMITTER_NAME", "a").env("GIT_COMMITTER_EMAIL", "a@b.c")
///     .status().unwrap().success());
/// // a clone one commit behind its origin, as of its last fetch
/// let base = std::env::temp_dir().join(format!("mj-remote-trunk-of-{}", std::process::id()));
/// let (up, co) = (base.join("up"), base.join("co"));
/// std::fs::create_dir_all(&up).unwrap();
/// git(&up, &["init", "-q", "-b", "master"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "base"]);
/// git(&base, &["clone", "-q", "up", "co"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "ahead"]);
/// git(&co, &["fetch", "-q"]);
///
/// assert_eq!(remote_trunk_of(&co, "master").as_deref(), Some("origin/master"));
/// // the upstream itself has no remote that carries its trunk
/// assert_eq!(remote_trunk_of(&up, "master"), None);
/// std::fs::remove_dir_all(&base).unwrap();
/// ```
pub fn remote_trunk_of(cwd: &Path, trunk: &str) -> Option<String> {
    let upstream = git::try_run(
        cwd,
        &[
            "rev-parse",
            "--symbolic-full-name",
            "--end-of-options",
            &format!("refs/heads/{trunk}@{{upstream}}"),
        ],
    )
    .ok()?;
    if upstream.status == Some(0) {
        if let Some(short) = upstream
            .text()
            .ok()
            .and_then(|t| t.strip_prefix("refs/remotes/").map(str::to_string))
        {
            return Some(short);
        }
    }
    let remotes = git::try_run(cwd, &["remote"]).ok()?.text().ok()?;
    let remotes: Vec<&str> = remotes
        .lines()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .collect();
    for remote in &remotes {
        let head = git::try_run(
            cwd,
            &["symbolic-ref", "-q", &format!("refs/remotes/{remote}/HEAD")],
        )
        .ok()?;
        if head.status == Some(0)
            && head.text().ok().as_deref() == Some(&format!("refs/remotes/{remote}/{trunk}"))
        {
            return Some(format!("{remote}/{trunk}"));
        }
    }
    remotes
        .iter()
        .map(|r| format!("{r}/{trunk}"))
        .find(|r| git::rev_exists(cwd, &format!("refs/remotes/{r}")).unwrap_or(false))
}

/// How far the commits `revs` hold are from `remote_trunk`: the trunk's commits none of
/// them reaches (behind), and the first one's commits the trunk lacks (ahead).
///
/// ```
/// use majordomus_cli::worktree::trunk::{distance, Distance};
/// use std::{path::Path, process::Command};
/// let git = |dir: &Path, args: &[&str]| assert!(Command::new("git").current_dir(dir).args(args)
///     .env("GIT_AUTHOR_NAME", "a").env("GIT_AUTHOR_EMAIL", "a@b.c")
///     .env("GIT_COMMITTER_NAME", "a").env("GIT_COMMITTER_EMAIL", "a@b.c")
///     .status().unwrap().success());
/// // a clone one commit behind its origin, as of its last fetch
/// let base = std::env::temp_dir().join(format!("mj-distance-{}", std::process::id()));
/// let (up, co) = (base.join("up"), base.join("co"));
/// std::fs::create_dir_all(&up).unwrap();
/// git(&up, &["init", "-q", "-b", "master"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "base"]);
/// git(&base, &["clone", "-q", "up", "co"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "ahead"]);
/// git(&co, &["fetch", "-q"]);
///
/// assert_eq!(distance(&co, &["HEAD"], "origin/master"), Some(Distance { behind: 1, ahead: 0 }));
/// assert_eq!(distance(&co, &["HEAD"], "origin/no-such-branch"), None);
/// std::fs::remove_dir_all(&base).unwrap();
/// ```
pub fn distance(cwd: &Path, revs: &[&str], remote_trunk: &str) -> Option<Distance> {
    let remote = format!("refs/remotes/{remote_trunk}");
    let count = |args: Vec<&str>| -> Option<u64> {
        let out = git::try_run(cwd, &args).ok()?;
        if out.status != Some(0) {
            return None;
        }
        out.text().ok()?.trim().parse().ok()
    };
    let first = *revs.first()?;
    let mut behind = vec!["rev-list", "--count", remote.as_str(), "--not"];
    behind.extend_from_slice(revs);
    let behind = count(behind)?;
    let ahead = count(vec!["rev-list", "--count", first, "--not", remote.as_str()])?;
    Some(Distance { behind, ahead })
}

/// Read `rev-list --left-right --count` output: the left side is ahead, the right behind.
///
/// ```
/// use majordomus_cli::worktree::trunk::{parse_left_right, Distance};
/// assert_eq!(parse_left_right("2\t779"), Some(Distance { ahead: 2, behind: 779 }));
/// assert_eq!(parse_left_right("nonsense"), None);
/// ```
pub fn parse_left_right(text: &str) -> Option<Distance> {
    let mut it = text.split_whitespace().map(str::parse::<u64>);
    let ahead = it.next()?.ok()?;
    let behind = it.next()?.ok()?;
    Some(Distance { behind, ahead })
}

/// When the repository last fetched: the modification time of `FETCH_HEAD` in its common
/// git directory.
fn fetched_at(identity: &RepositoryIdentity) -> Option<String> {
    let modified = std::fs::metadata(identity.git_common_dir().path.join("FETCH_HEAD"))
        .and_then(|m| m.modified())
        .ok()?;
    Some(crate::peers::rfc3339(modified))
}

/// Measure the work tree `worktree` of a discovered repository against the remote trunk,
/// judged by `thresholds`. Never fetches and writes nothing.
///
/// ```
/// use majordomus_cli::worktree::RepositoryIdentity;
/// use majordomus_cli::worktree::trunk::{measure, Distance, TrunkStanding, TrunkThresholds};
/// use std::{path::Path, process::Command};
/// let git = |dir: &Path, args: &[&str]| assert!(Command::new("git").current_dir(dir).args(args)
///     .env("GIT_AUTHOR_NAME", "a").env("GIT_AUTHOR_EMAIL", "a@b.c")
///     .env("GIT_COMMITTER_NAME", "a").env("GIT_COMMITTER_EMAIL", "a@b.c")
///     .status().unwrap().success());
/// // a clone one commit behind its origin, as of its last fetch
/// let base = std::env::temp_dir().join(format!("mj-measure-{}", std::process::id()));
/// let (up, co) = (base.join("up"), base.join("co"));
/// std::fs::create_dir_all(&up).unwrap();
/// git(&up, &["init", "-q", "-b", "master"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "base"]);
/// git(&base, &["clone", "-q", "up", "co"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "ahead"]);
/// git(&co, &["fetch", "-q"]);
///
/// let identity = RepositoryIdentity::discover(&co).unwrap();
/// let worktree = identity.current_worktree().path.clone();
/// let d = measure(&identity, &worktree, TrunkThresholds { warn_behind: Some(1), fail_behind: None });
/// assert_eq!(d.standing, TrunkStanding::Lagging);
/// assert_eq!(d.checkout, Some(Distance { behind: 1, ahead: 0 }));
/// assert!(d.remedy.is_some(), "a checkout behind is told how to come current");
/// std::fs::remove_dir_all(&base).unwrap();
/// ```
pub fn measure(
    identity: &RepositoryIdentity,
    worktree: &Path,
    thresholds: TrunkThresholds,
) -> TrunkDistance {
    let trunk: &Trunk = identity.trunk();
    let Some(name) = trunk.branch.clone() else {
        return TrunkDistance::unmeasured(
            worktree,
            None,
            "the trunk could not be determined, so no distance from it can be measured".into(),
        );
    };
    let Some(remote_trunk) = remote_trunk_of(worktree, &name) else {
        return TrunkDistance::unmeasured(
            worktree,
            Some(name.clone()),
            format!("no remote-tracking branch carries the trunk {name}, so this checkout's distance from it cannot be measured (git fetch <remote>)"),
        );
    };
    let remote_head = git::try_run(
        worktree,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("refs/remotes/{remote_trunk}"),
        ],
    )
    .ok()
    .filter(|o| o.status == Some(0))
    .and_then(|o| o.text().ok());
    let merging = git::rev_exists(worktree, "MERGE_HEAD").unwrap_or(false);
    let checkout = git::head_of(worktree).ok().flatten().and_then(|_| {
        let revs: &[&str] = if merging {
            &["HEAD", "MERGE_HEAD"]
        } else {
            &["HEAD"]
        };
        distance(worktree, revs, &remote_trunk)
    });
    let local_ref = format!("refs/heads/{name}");
    let local_trunk = git::branch_exists(worktree, &name)
        .unwrap_or(false)
        .then(|| distance(worktree, &[local_ref.as_str()], &remote_trunk))
        .flatten();
    let on_trunk = git::branch_of(worktree).ok().flatten().as_deref() == Some(name.as_str());
    let mut d = TrunkDistance {
        standing: judge(checkout.map(|c| c.behind), thresholds),
        local_trunk_standing: judge(local_trunk.map(|l| l.behind), thresholds),
        worktree: worktree.display().to_string(),
        on_trunk,
        trunk: Some(name.clone()),
        remote_trunk: Some(remote_trunk.clone()),
        remote_head,
        checkout,
        local_trunk,
        fetched_at: fetched_at(identity),
        thresholds,
        message: String::new(),
        remedy: None,
    };
    d.message = message_of(&d, &name, &remote_trunk);
    d.remedy = remedy_of(identity, &d, worktree, &name, &remote_trunk);
    d
}

/// Measure the checkout holding `path` with the thresholds its own policy declares. A path
/// git cannot place in a repository is `unmeasured`, with git's reason.
///
/// ```
/// use majordomus_cli::worktree::trunk::{measure_checkout, TrunkStanding};
/// use std::{path::Path, process::Command};
/// let git = |dir: &Path, args: &[&str]| assert!(Command::new("git").current_dir(dir).args(args)
///     .env("GIT_AUTHOR_NAME", "a").env("GIT_AUTHOR_EMAIL", "a@b.c")
///     .env("GIT_COMMITTER_NAME", "a").env("GIT_COMMITTER_EMAIL", "a@b.c")
///     .status().unwrap().success());
/// // a clone one commit behind its origin, as of its last fetch
/// let base = std::env::temp_dir().join(format!("mj-measure-checkout-{}", std::process::id()));
/// let (up, co) = (base.join("up"), base.join("co"));
/// std::fs::create_dir_all(&up).unwrap();
/// git(&up, &["init", "-q", "-b", "master"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "base"]);
/// git(&base, &["clone", "-q", "up", "co"]);
/// git(&up, &["commit", "-q", "--allow-empty", "-m", "ahead"]);
/// git(&co, &["fetch", "-q"]);
///
/// // its policy declares no thresholds: one commit behind is reported, and not judged
/// assert_eq!(measure_checkout(&co).standing, TrunkStanding::Unjudged);
/// // a path no repository holds is unmeasured, never current
/// assert_eq!(measure_checkout(&base).standing, TrunkStanding::Unmeasured);
/// std::fs::remove_dir_all(&base).unwrap();
/// ```
pub fn measure_checkout(path: &Path) -> TrunkDistance {
    match RepositoryIdentity::discover(path) {
        Ok(identity) => {
            let worktree = identity.current_worktree().path.clone();
            measure(
                &identity,
                &worktree,
                TrunkThresholds::declared_at(&worktree),
            )
        }
        Err(e) => TrunkDistance::unmeasured(path, None, e.to_string()),
    }
}

fn message_of(d: &TrunkDistance, trunk: &str, remote_trunk: &str) -> String {
    let mut parts = Vec::new();
    match d.checkout {
        Some(c) => parts.push(format!(
            "this checkout is {} commit(s) behind and {} ahead of {remote_trunk}",
            c.behind, c.ahead
        )),
        None => parts.push("this checkout's HEAD names no commit".into()),
    }
    if let Some(l) = d.local_trunk.filter(|_| !d.on_trunk) {
        parts.push(format!(
            "the local {trunk} is {} behind and {} ahead ({})",
            l.behind,
            l.ahead,
            d.local_trunk_standing.as_str()
        ));
    }
    let judged = match d.standing {
        TrunkStanding::Current => "current".to_string(),
        TrunkStanding::Lagging => format!(
            "lagging: at or past {POLICY_KEY}.warn_behind {}",
            d.thresholds.warn_behind.unwrap_or_default()
        ),
        TrunkStanding::Stale => format!(
            "stale: at or past {POLICY_KEY}.fail_behind {}, so what it serves is not the trunk",
            d.thresholds.fail_behind.unwrap_or_default()
        ),
        TrunkStanding::Unjudged => format!("unjudged: the policy declares no {POLICY_KEY}"),
        TrunkStanding::Unmeasured => "unmeasured".to_string(),
    };
    let since = match &d.fetched_at {
        Some(t) => format!("as of the last fetch at {t}"),
        None => "the repository has never fetched".into(),
    };
    format!("{}; {judged} ({since})", parts.join("; "))
}

fn remedy_of(
    identity: &RepositoryIdentity,
    d: &TrunkDistance,
    worktree: &Path,
    trunk: &str,
    remote_trunk: &str,
) -> Option<String> {
    let mut steps = Vec::new();
    let here = worktree.display();
    let on_trunk = d.on_trunk;
    if d.checkout.is_some_and(|c| c.behind > 0) {
        steps.push(if on_trunk {
            format!("git -C {here} merge --ff-only {remote_trunk}")
        } else {
            format!("git -C {here} merge {remote_trunk}")
        });
    }
    if !on_trunk && d.local_trunk.is_some_and(|l| l.behind > 0) {
        steps.push(match identity.record_of_branch(trunk) {
            Some(r) => format!("git -C {} merge --ff-only {remote_trunk}", r.path.display()),
            None => format!("git -C {here} fetch . refs/remotes/{remote_trunk}:refs/heads/{trunk}"),
        });
    }
    if steps.is_empty() {
        None
    } else {
        // uncommitted work first: a fast-forward over a dirty tree refuses, and nothing here
        // decides what to do with somebody's changes
        Some(format!(
            "git fetch; commit or rescue uncommitted work, then: {}",
            steps.join("; ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(cwd: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A remote with `n` commits on its trunk, and a clone of it.
    fn remote_and_clone(n: usize) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let origin = dir.path().join("origin");
        std::fs::create_dir_all(&origin).unwrap();
        git(&origin, &["init", "-q", "-b", "master"]);
        for i in 0..n {
            git(
                &origin,
                &["commit", "-q", "--allow-empty", "-m", &format!("c{i}")],
            );
        }
        let clone = dir.path().join("clone");
        git(
            dir.path(),
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );
        (dir, clone)
    }

    /// The checkout and the local trunk are each counted against the remote trunk, the
    /// larger distance is judged, and the remedy fast-forwards the trunk checkout.
    #[test]
    fn a_checkout_behind_the_remote_trunk_is_measured_and_judged() {
        let (dir, clone) = remote_and_clone(1);
        let origin = dir.path().join("origin");
        for i in 0..5 {
            git(
                &origin,
                &["commit", "-q", "--allow-empty", "-m", &format!("n{i}")],
            );
        }
        git(&clone, &["fetch", "-q"]);
        let identity = RepositoryIdentity::discover(&clone).unwrap();
        let t = TrunkThresholds {
            warn_behind: Some(2),
            fail_behind: Some(5),
        };
        let d = measure(&identity, &identity.current_worktree().path.clone(), t);
        assert_eq!(d.remote_trunk.as_deref(), Some("origin/master"));
        assert_eq!(
            d.checkout,
            Some(Distance {
                behind: 5,
                ahead: 0
            })
        );
        assert_eq!(
            d.local_trunk,
            Some(Distance {
                behind: 5,
                ahead: 0
            })
        );
        assert_eq!(d.standing, TrunkStanding::Stale);
        assert_eq!(d.local_trunk_standing, TrunkStanding::Stale);
        assert!(d.on_trunk);
        assert!(d.fetched_at.is_some(), "a clone has fetched");
        assert!(d.message.contains("5 commit(s) behind"), "{}", d.message);
        assert!(
            d.remedy
                .as_deref()
                .is_some_and(|r| r.contains("merge --ff-only origin/master")),
            "{:?}",
            d.remedy
        );
        let lagging = measure(
            &identity,
            &identity.current_worktree().path.clone(),
            TrunkThresholds {
                warn_behind: Some(2),
                fail_behind: Some(50),
            },
        );
        assert_eq!(lagging.standing, TrunkStanding::Lagging);
        git(&clone, &["merge", "-q", "--ff-only", "origin/master"]);
        let current = measure(&identity, &identity.current_worktree().path.clone(), t);
        assert_eq!(current.standing, TrunkStanding::Current);
        assert_eq!(current.remedy, None);
    }

    /// A repository without a remote has nothing to be measured against: unmeasured, said
    /// so, never current.
    #[test]
    fn a_repository_without_a_remote_is_unmeasured() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "master"]);
        git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "c"]);
        let d = measure_checkout(dir.path());
        assert_eq!(d.standing, TrunkStanding::Unmeasured);
        assert!(
            d.message.contains("no remote-tracking branch"),
            "{}",
            d.message
        );
        assert_eq!((d.checkout, d.local_trunk), (None, None));
    }

    /// A merge in progress counts the commits it brings in: the commit that makes a stale
    /// branch current is not judged stale by the tree it replaces, and a feature branch is
    /// judged on its own distance while a stale local trunk is reported beside it.
    #[test]
    fn a_merge_in_progress_counts_what_it_brings_in() {
        let (dir, clone) = remote_and_clone(1);
        let origin = dir.path().join("origin");
        git(&clone, &["checkout", "-q", "-b", "feature/x"]);
        std::fs::write(clone.join("f"), "x").unwrap();
        git(&clone, &["add", "f"]);
        git(&clone, &["commit", "-q", "-m", "feature"]);
        for i in 0..4 {
            git(
                &origin,
                &["commit", "-q", "--allow-empty", "-m", &format!("n{i}")],
            );
        }
        git(&clone, &["fetch", "-q"]);
        let identity = RepositoryIdentity::discover(&clone).unwrap();
        let t = TrunkThresholds {
            warn_behind: Some(2),
            fail_behind: Some(4),
        };
        let here = identity.current_worktree().path.clone();
        let before = measure(&identity, &here, t);
        assert!(!before.on_trunk);
        assert_eq!(
            before.checkout,
            Some(Distance {
                behind: 4,
                ahead: 1
            })
        );
        assert_eq!(before.standing, TrunkStanding::Stale);
        assert_eq!(before.local_trunk_standing, TrunkStanding::Stale);
        assert!(
            before
                .remedy
                .as_deref()
                .is_some_and(|r| r.contains("merge origin/master")
                    && r.contains("fetch . refs/remotes/origin/master:refs/heads/master")),
            "{:?}",
            before.remedy
        );
        git(
            &clone,
            &["merge", "-q", "--no-commit", "--no-ff", "origin/master"],
        );
        let merging = measure(&identity, &here, t);
        assert_eq!(
            merging.checkout,
            Some(Distance {
                behind: 0,
                ahead: 1
            })
        );
        assert_eq!(merging.standing, TrunkStanding::Current);
        assert_eq!(
            merging.local_trunk_standing,
            TrunkStanding::Stale,
            "the local trunk did not move"
        );
    }
}
