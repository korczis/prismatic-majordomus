//! Whether the trunk is released: how much landed work it holds past the last release, since
//! when, and at which version (I3070).
//!
//! The cooperation model's fourth rule says the release seat releases whenever the trunk holds
//! landed work past the last tag (`project.the-cooperation-model-is-the-owners-word`), and the
//! owner's order of 2026-10-19 asks for a measured version after every session and milestone.
//! Neither was measured. The trunk could hold a day of landed work past the last release while
//! every check, the brief, doctor and the delivery screen stayed green. This module is the one
//! measurement: `release.currency` answers it, and every surface that shows it reads that answer.
//!
//! Three readings make it, each of git as it is when asked:
//!
//! - **The release.** The newer of two: the release the layer records
//!   ([`super::compat::last_release`]), and the newest version tag the trunk contains. A tag is
//!   pushed before its record lands, so a tag newer than every record is a release that has
//!   happened and is not recorded yet.
//! - **The landed work.** Every commit the trunk holds past that release, except merges and
//!   release commits (`chore(release)`): what the next release would ship.
//! - **How long it has waited.** A landing is a commit of the trunk's first-parent history: a
//!   merge of a branch or a top, or a commit made on the trunk itself. The work has waited since
//!   the oldest landing that brought anything but release commits arrived on the trunk: when
//!   this clone's remote-tracking trunk first held it, from that ref's reflog. A top is
//!   composed for hours before it is pushed, so its commit time says when it was made, not when
//!   it landed. Where the reflog does not show the landing arrive, the commit time is used as
//!   what it is, an upper bound on the wait, and alone it never makes the work overdue
//!   ([`LandedAt`]).
//!
//! The answer reads refs and the clock, so it is a runtime read and is never committed. A
//! committed derivation that read refs would go stale with every tag.
//!
//! ```
//! use majordomus_cli::release::currency::{measure, Currency, Stance, OVERDUE_AFTER};
//! use std::time::{Duration, SystemTime};
//!
//! // a directory with no trunk is not measured, and the answer says why
//! let nowhere = tempfile::tempdir().unwrap();
//! let answer: Currency = measure(nowhere.path(), &[], SystemTime::now());
//! assert_eq!(answer.stance, Stance::Unknown);
//!
//! // on a trunk, the stance follows the landed work and how long it has waited
//! assert_eq!(Stance::of(0, OVERDUE_AFTER), Stance::Current);
//! assert_eq!(Stance::of(3, Duration::from_secs(600)), Stance::Owed);
//! assert_eq!(Stance::of(3, OVERDUE_AFTER), Stance::Overdue);
//! ```

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{Change, ChangeKind};
use super::version::Version;
use crate::capability::registry::CapabilityRegistry;
use crate::model::Object;

/// How long landed work may wait past the last release before it is overdue: six hours, the
/// release seat's word (I3070). That is longer than a session, and shorter than the day of
/// landed work the owner's order closes.
pub const OVERDUE_AFTER: Duration = Duration::from_secs(6 * 60 * 60);

/// The most landed changes an answer lists. The count is exact however long the list.
pub const LISTED: usize = 50;

/// Whether the trunk's landed work is released.
///
/// ```
/// use majordomus_cli::release::currency::{Stance, OVERDUE_AFTER};
/// use std::time::Duration;
/// assert_eq!(Stance::of(0, OVERDUE_AFTER), Stance::Current, "nothing landed, nothing owed");
/// assert_eq!(Stance::of(2, Duration::from_secs(60)), Stance::Owed);
/// assert_eq!(Stance::of(2, OVERDUE_AFTER), Stance::Overdue);
/// assert_eq!(Stance::Overdue.as_str(), "overdue");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseCurrencyStance")]
pub enum Stance {
    /// Nothing has landed past the last release.
    Current,
    /// Work has landed past it, and has waited less than [`OVERDUE_AFTER`].
    Owed,
    /// Work has landed past it, and has waited at least [`OVERDUE_AFTER`].
    Overdue,
    /// It could not be measured: no trunk, no release, or a release this clone cannot read.
    Unknown,
}

impl Stance {
    /// The stance of `landed` changes, the oldest of which has waited `waited`.
    ///
    /// ```
    /// use majordomus_cli::release::currency::{Stance, OVERDUE_AFTER};
    /// assert_eq!(Stance::of(1, OVERDUE_AFTER / 2), Stance::Owed);
    /// ```
    pub fn of(landed: usize, waited: Duration) -> Stance {
        match (landed, waited >= OVERDUE_AFTER) {
            (0, _) => Stance::Current,
            (_, false) => Stance::Owed,
            (_, true) => Stance::Overdue,
        }
    }

    /// The stance as one word, as the brief, doctor and the delivery screen say it.
    ///
    /// ```
    /// use majordomus_cli::release::currency::Stance;
    /// assert_eq!(Stance::Current.as_str(), "current");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Stance::Current => "current",
            Stance::Owed => "owed",
            Stance::Overdue => "overdue",
            Stance::Unknown => "unknown",
        }
    }
}

/// How the oldest landing was dated, and so which way its wait may be wrong (the plan
/// critique of 2026-10-19: a top lands by a fast-forward push hours after its commits).
///
/// ```
/// use majordomus_cli::release::currency::LandedAt;
/// assert_eq!(serde_json::to_value(LandedAt::Reflog).unwrap(), "reflog");
/// assert_eq!(serde_json::to_value(LandedAt::Commit).unwrap(), "commit");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseCurrencyLandedAt")]
pub enum LandedAt {
    /// When this clone's remote-tracking trunk first held it, from that ref's reflog. A landing
    /// is seen no earlier than it lands, so the wait counted from here is a lower bound, and it
    /// is the one date that can make the stance overdue.
    Reflog,
    /// Its commit time, where the reflog does not show it arrive: a commit lands no earlier than
    /// it is made, so the wait counted from here is an upper bound, and it never makes the
    /// stance overdue by itself.
    Commit,
}

/// The release the trunk is measured from.
///
/// ```
/// use majordomus_cli::release::currency::Released;
/// let tagged = Released {
///     version: "0.30.1".into(),
///     reference: "v0.30.1".into(),
///     at: "v0.30.1".into(),
///     recorded: false,
/// };
/// // a tag pushed before its record landed is read at the tag itself
/// assert_eq!(tagged.at, tagged.reference);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseCurrencyRelease")]
pub struct Released {
    /// `0.30.1`.
    pub version: String,
    /// `v0.30.1`: the release as a reader recognises it.
    pub reference: String,
    /// What the trunk is measured from: the commit its record names, else its tag.
    pub at: String,
    /// Whether the layer holds a record for it yet.
    pub recorded: bool,
}

/// The answer of `release.currency`: whether the trunk's landed work is released.
///
/// ```
/// use majordomus_cli::release::currency::{measure, Currency, Stance};
/// let nowhere = tempfile::tempdir().unwrap();
/// let answer: Currency = measure(nowhere.path(), &[], std::time::SystemTime::now());
/// // a directory with no trunk is not measured, and says why
/// assert_eq!(answer.stance, Stance::Unknown);
/// assert!(answer.summary.contains("trunk"), "{}", answer.summary);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseCurrency")]
pub struct Currency {
    /// Whether the landed work is released.
    pub stance: Stance,
    /// The answer in one sentence: the brief's and doctor's line.
    pub summary: String,
    /// The remote-tracking ref of the trunk it was measured on (`origin/master`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// The last release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<Released>,
    /// Landed commits past the last release, merges and release commits left out.
    pub landed: usize,
    /// The landings of the trunk's first-parent history that brought them.
    pub landings: usize,
    /// When the oldest of those landings landed, RFC 3339, dated as [`landed_at`](Self::landed_at)
    /// says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oldest_landing: Option<String>,
    /// How [`oldest_landing`](Self::oldest_landing) was dated: when this clone saw it arrive
    /// on the trunk, or its commit time as an upper bound on the wait.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landed_at: Option<LandedAt>,
    /// How long the work has waited since then, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waited_seconds: Option<u64>,
    /// How long it may wait before it is overdue ([`OVERDUE_AFTER`]).
    pub overdue_after_seconds: u64,
    /// The version the public contract requires for the landed work (ADR 0051), when it was
    /// decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// Why that version was not decided, when it was asked for and could not be.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_unmeasured: Option<String>,
    /// The landed changes, newest first, at most [`LISTED`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<Change>,
}

impl Currency {
    fn unknown(trunk: Option<String>, reason: String) -> Currency {
        Currency {
            stance: Stance::Unknown,
            summary: reason,
            trunk,
            release: None,
            landed: 0,
            landings: 0,
            oldest_landing: None,
            landed_at: None,
            waited_seconds: None,
            overdue_after_seconds: OVERDUE_AFTER.as_secs(),
            next: None,
            next_unmeasured: None,
            changes: Vec::new(),
        }
    }
}

/// Measure the trunk of the repository at `root` against its last release, as of `now`. Git
/// is read as it is, the remote-tracking trunk as of the last fetch. The version the contract
/// requires is not decided here ([`decide_next`]), because that reads the last release's
/// registry, and the brief and doctor do not need it.
///
/// ```
/// use majordomus_cli::release::currency::{measure, Stance};
/// let nowhere = tempfile::tempdir().unwrap();
/// let answer = measure(nowhere.path(), &[], std::time::SystemTime::now());
/// assert_eq!((answer.stance, answer.landed), (Stance::Unknown, 0));
/// ```
pub fn measure(root: &Path, objects: &[Object], now: SystemTime) -> Currency {
    let trunk = match crate::delivery::revision::trunk_ref(root) {
        Ok(trunk) => trunk,
        Err(why) => return Currency::unknown(None, format!("the trunk cannot be told: {why}")),
    };
    let release = newest(
        super::compat::last_release(root, objects),
        newest_tag_on(root, &trunk),
    );
    let Some(release) = release.filter(|r| resolves(root, &r.at)) else {
        return Currency::unknown(
            Some(trunk.clone()),
            format!(
                "no release to measure {trunk} from: no release record or version tag whose commit this clone holds"
            ),
        );
    };
    let range = format!("{}..{trunk}", release.at);
    let changes: Vec<Change> = super::commits::in_range(root, &range, objects)
        .into_iter()
        .filter(|c| !is_release(c))
        .collect();
    let landed = changes.len();
    let landings = landings(root, &range);
    let dated = landings
        .first()
        .filter(|_| landed > 0)
        .map(|(sha, committed)| match first_seen(root, &trunk, sha) {
            Some(seen) => (LandedAt::Reflog, seen),
            None => (LandedAt::Commit, *committed),
        });
    let now = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let waited = dated.map(|(_, at)| Duration::from_secs(now.saturating_sub(at)));
    let stance = match (Stance::of(landed, waited.unwrap_or_default()), dated) {
        // a commit time bounds the wait from above: it says the work may be overdue, never that
        // it is, so a top composed hours before it landed does not read overdue as it lands
        (Stance::Overdue, Some((LandedAt::Commit, _))) => Stance::Owed,
        (stance, _) => stance,
    };
    let how = dated.map(|(how, _)| how);
    Currency {
        summary: summary(
            stance,
            landed,
            landings.len(),
            &release,
            &trunk,
            waited,
            how,
        ),
        stance,
        trunk: Some(trunk),
        release: Some(release),
        landed,
        landings: landings.len(),
        oldest_landing: dated
            .map(|(_, at)| crate::peers::rfc3339(UNIX_EPOCH + Duration::from_secs(at))),
        landed_at: how,
        waited_seconds: waited.map(|w| w.as_secs()),
        overdue_after_seconds: OVERDUE_AFTER.as_secs(),
        next: None,
        next_unmeasured: None,
        changes: changes.into_iter().take(LISTED).collect(),
    }
}

/// Decide the version the public contract requires for the landed work, where it can be
/// measured: the release the trunk is measured from must be recorded, because the contract is
/// measured from a recorded release (ADR 0051), and this checkout must be at the trunk, because
/// the contract is this tree's. Otherwise `next_unmeasured` says which of the two it is not.
/// Nothing is decided when nothing has landed.
///
/// ```
/// use majordomus_cli::capability::registry::CapabilityRegistry;
/// use majordomus_cli::release::currency::{decide_next, measure};
/// let nowhere = tempfile::tempdir().unwrap();
/// let mut answer = measure(nowhere.path(), &[], std::time::SystemTime::now());
/// let registry = CapabilityRegistry::builder().build().unwrap();
/// decide_next(&mut answer, nowhere.path(), &registry, &[]);
/// assert_eq!((answer.next, answer.next_unmeasured), (None, None), "nothing landed, nothing decided");
/// ```
pub fn decide_next(
    answer: &mut Currency,
    root: &Path,
    registry: &CapabilityRegistry,
    objects: &[Object],
) {
    let Some(release) = answer.release.as_ref().filter(|_| answer.landed > 0) else {
        return;
    };
    let why = if release.recorded {
        away_from_trunk(root, answer.trunk.as_deref().unwrap_or_default())
    } else {
        Some(format!(
            "{} has no release record yet, and the contract is measured from a recorded release",
            release.reference
        ))
    };
    match why {
        Some(why) => answer.next_unmeasured = Some(why),
        None => {
            let report = super::version::select(root, registry, objects).report;
            answer.next = report.next;
            answer.next_unmeasured = report.contract_unreadable;
        }
    }
}

/// The newer of the recorded release and the newest tag the trunk contains.
fn newest(recorded: Option<super::compat::LastRelease>, tag: Option<String>) -> Option<Released> {
    let from_record = recorded.map(|r| Released {
        version: r.version,
        reference: r.reference,
        at: r.read_at,
        recorded: r.recorded,
    });
    let from_tag = tag.map(|t| Released {
        version: t.trim_start_matches('v').to_string(),
        reference: t.clone(),
        at: t,
        recorded: false,
    });
    match (from_record, from_tag) {
        (Some(r), Some(t)) if Version::parse(&t.version) > Version::parse(&r.version) => Some(t),
        (Some(r), _) => Some(r),
        (None, t) => t,
    }
}

/// The newest version tag whose commit the trunk contains.
fn newest_tag_on(root: &Path, trunk: &str) -> Option<String> {
    super::compat::tags(root)
        .into_iter()
        .rev()
        .find(|tag| succeeds(root, &["merge-base", "--is-ancestor", tag, trunk]))
}

/// Whether `rev` names a commit this clone holds.
fn resolves(root: &Path, rev: &str) -> bool {
    succeeds(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    )
}

/// Why this checkout is not where `trunk` is, when it is not.
fn away_from_trunk(root: &Path, trunk: &str) -> Option<String> {
    let head = git(root, &["rev-parse", "HEAD"]);
    let there = git(root, &["rev-parse", trunk]);
    (head.trim() != there.trim()).then(|| {
        format!(
            "this checkout is at {}, not at {trunk} ({}): the contract is measured where the trunk is checked out",
            short(&head),
            short(&there)
        )
    })
}

fn short(sha: &str) -> &str {
    let sha = sha.trim();
    &sha[..sha.len().min(11)]
}

/// A release commit: what releasing writes (`chore(release): ...`), which no release ships.
fn is_release(change: &Change) -> bool {
    change.kind == ChangeKind::Chore && change.scope.as_deref() == Some("release")
}

/// One landing of the trunk's first-parent history.
#[derive(Debug, PartialEq, Eq)]
struct Landing<'a> {
    sha: &'a str,
    /// Its committer time, in seconds since the epoch: when it was made, which is no later than
    /// when it landed.
    at: u64,
    merge: bool,
}

/// The landings of `log` (`--format=%H %ct %P`), in its order. A line that is not one is left
/// out rather than guessed.
fn first_parents(log: &str) -> Vec<Landing<'_>> {
    log.lines()
        .filter_map(|line| {
            let mut words = line.split(' ');
            let sha = words.next().filter(|w| !w.is_empty())?;
            let at = words.next()?.parse().ok()?;
            Some(Landing {
                sha,
                at,
                merge: words.count() > 1,
            })
        })
        .collect()
}

/// Every landing in `range` that brought anything but release commits, oldest first: its
/// commit and its commit time.
fn landings(root: &Path, range: &str) -> Vec<(String, u64)> {
    let log = git(
        root,
        &[
            "log",
            "--first-parent",
            "--reverse",
            "--format=%H %ct %P",
            range,
        ],
    );
    first_parents(&log)
        .into_iter()
        .filter(|l| brings_work(root, l))
        .map(|l| (l.sha.to_string(), l.at))
        .collect()
}

/// When this clone's `trunk` first held `sha`, from the trunk's reflog: the time of the oldest
/// entry of the run of entries, newest first, that contain it. `None` when no entry contains
/// it, or when every entry does, because then the reflog does not show it arrive (the clone
/// was made after it landed, or its reflog was expired).
fn first_seen(root: &Path, trunk: &str, sha: &str) -> Option<u64> {
    let log = git(
        root,
        &["reflog", "show", "--date=unix", "--format=%H %gd", trunk],
    );
    let mut seen = None;
    for (held, at) in reflog_entries(&log) {
        if succeeds(root, &["merge-base", "--is-ancestor", sha, held]) {
            seen = Some(at);
        } else {
            return seen;
        }
    }
    None
}

/// The entries of `git reflog show --date=unix --format='%H %gd'`, newest first, as git lists
/// them: the commit each entry set the ref to, and when, in seconds since the epoch. A line that
/// is not one is left out rather than guessed.
fn reflog_entries(log: &str) -> Vec<(&str, u64)> {
    log.lines()
        .filter_map(|line| {
            let (sha, selector) = line.split_once(' ')?;
            let at = selector
                .rsplit_once("@{")?
                .1
                .strip_suffix('}')?
                .parse()
                .ok()?;
            Some((sha, at))
        })
        .collect()
}

/// Whether a landing brought a commit that is not a release commit: a merge, every commit its
/// second parent added; a commit made on the trunk, itself.
fn brings_work(root: &Path, landing: &Landing<'_>) -> bool {
    let range = if landing.merge {
        format!("{0}^1..{0}", landing.sha)
    } else {
        format!("{}^!", landing.sha)
    };
    git(root, &["log", "--no-merges", "--format=%s", &range])
        .lines()
        .any(|subject| !is_release(&super::commits::parse("", subject, "")))
}

fn summary(
    stance: Stance,
    landed: usize,
    landings: usize,
    release: &Released,
    trunk: &str,
    waited: Option<Duration>,
    how: Option<LandedAt>,
) -> String {
    let unrecorded = if release.recorded {
        ""
    } else {
        " (tagged, its record not landed yet)"
    };
    match stance {
        Stance::Current => format!(
            "{trunk} holds no landed work past {}{unrecorded}",
            release.reference
        ),
        _ => {
            let waited = waited.unwrap_or_default();
            let mut s = format!(
                "{landed} landed commit(s) in {landings} landing(s) wait past {}{unrecorded} on {trunk}",
                release.reference
            );
            match how {
                Some(LandedAt::Commit) => s.push_str(&format!(
                    ", the oldest for at most {}: this clone holds no record of when it landed, so its commit time bounds the wait",
                    age_words(waited)
                )),
                _ => s.push_str(&format!(
                    ", the oldest for {} since this clone saw it land",
                    age_words(waited)
                )),
            }
            if stance == Stance::Overdue {
                s.push_str(&format!(
                    ", longer than the {} a release may wait",
                    age_words(OVERDUE_AFTER)
                ));
            } else if how == Some(LandedAt::Commit) && waited >= OVERDUE_AFTER {
                s.push_str(&format!(
                    "; that bound is past the {} a release may wait, which alone makes nothing overdue",
                    age_words(OVERDUE_AFTER)
                ));
            }
            s
        }
    }
}

/// A wait in the words a person reads it in: minutes, then hours, then days.
fn age_words(waited: Duration) -> String {
    let minutes = waited.as_secs() / 60;
    match (minutes / (24 * 60), (minutes / 60) % 24, minutes % 60) {
        (0, 0, m) => format!("{m} min"),
        (0, h, m) => format!("{h} h {m} min"),
        (d, h, _) => format!("{d} d {h} h"),
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

fn succeeds(root: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const HOUR: u64 = 60 * 60;

    /// A clone of a bare origin, its trunk `master`, with every commit dated `at`.
    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Fixture {
            let dir = tempfile::tempdir().expect("a temporary directory");
            let clone = Fixture { dir };
            clone.run(
                &[
                    "init",
                    "-q",
                    "--bare",
                    "--initial-branch=master",
                    "origin.git",
                ],
                0,
            );
            clone.run(&["clone", "-q", "origin.git", "work"], 0);
            clone.git(&["config", "user.email", "t@example.com"], 0);
            clone.git(&["config", "user.name", "t"], 0);
            clone
        }

        fn work(&self) -> std::path::PathBuf {
            self.dir.path().join("work")
        }

        fn command(&self, cwd: &Path, args: &[&str], at: u64) -> String {
            let out = Command::new("git")
                .current_dir(cwd)
                .args(args)
                .env("GIT_AUTHOR_DATE", format!("@{at} +0000"))
                .env("GIT_COMMITTER_DATE", format!("@{at} +0000"))
                .output()
                .expect("git runs");
            assert!(out.status.success(), "git {args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }

        fn run(&self, args: &[&str], at: u64) -> String {
            self.command(self.dir.path(), args, at)
        }

        fn git(&self, args: &[&str], at: u64) -> String {
            self.command(&self.work(), args, at)
        }

        /// A commit of `subject` on the checked-out branch, dated `at`, and its sha.
        fn commit(&self, subject: &str, at: u64) -> String {
            self.git(&["commit", "-q", "--allow-empty", "-m", subject], at);
            self.git(&["rev-parse", "HEAD"], at)
        }

        /// Push the checked-out branch to the trunk and fetch it back, at `at`: the time the
        /// remote-tracking trunk's reflog records the landing at.
        fn push(&self, at: u64) {
            self.git(&["push", "-q", "origin", "HEAD:master", "--tags"], at);
            self.git(&["fetch", "-q", "origin"], at);
        }
    }

    /// A release record object of the layer's own kind, naming `commit`.
    fn record(version: &str, commit: &str) -> Object {
        let mut metadata = serde_json::Map::new();
        metadata.insert("version".into(), version.into());
        metadata.insert("tag".into(), format!("v{version}").into());
        metadata.insert("commit".into(), commit.into());
        Object {
            kind: super::super::changelog::RELEASE_KIND.into(),
            identity: format!("v{version}"),
            uri: format!("majordomus://release-record/v{version}"),
            title: None,
            description: None,
            metadata: Value::Object(metadata),
            body: String::new(),
            content: String::new(),
            media_type: "application/yaml",
            provenance: crate::model::Provenance {
                path: format!(".ai/repo/releases/v{version}.yaml"),
                directory: ".ai/repo/releases".into(),
                source_class: "release".into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    /// A release with nothing landed past it is current; a fix landed past it is owed, then
    /// overdue once it has waited past the threshold; a release commit is no landed work; and a
    /// tag pushed past the record is the release the trunk is measured from.
    #[test]
    fn landed_work_past_the_release_is_owed_then_overdue() {
        let c = Fixture::new();
        let t0 = 1_700_000_000;
        let released = c.commit("feat: the first", t0);
        c.git(&["tag", "v0.1.0"], t0);
        c.push(t0);
        let objects = vec![record("0.1.0", &released)];
        let work = c.work();

        let current = measure(&work, &objects, at(t0 + HOUR));
        assert_eq!(current.stance, Stance::Current, "{}", current.summary);
        assert_eq!(current.trunk.as_deref(), Some("origin/master"));
        assert!(current.release.as_ref().is_some_and(|r| r.recorded));
        assert!(current.summary.contains("no landed work past v0.1.0"));

        c.commit("fix: the second", t0 + 2 * HOUR);
        c.commit("chore(release): record v0.1.0", t0 + 2 * HOUR);
        c.push(t0 + 2 * HOUR);
        let owed = measure(&work, &objects, at(t0 + 3 * HOUR));
        assert_eq!(owed.stance, Stance::Owed, "{}", owed.summary);
        assert_eq!(owed.landed, 1, "the release commit is no landed work");
        assert_eq!(
            owed.landings, 1,
            "the release commit's landing brought no work"
        );
        assert_eq!(owed.waited_seconds, Some(HOUR));
        assert_eq!(owed.landed_at, Some(LandedAt::Reflog));
        assert_eq!(owed.changes[0].subject, "the second");
        assert!(owed.oldest_landing.is_some());
        assert!(owed
            .summary
            .contains("1 landed commit(s) in 1 landing(s) wait past v0.1.0"));

        let overdue = measure(&work, &objects, at(t0 + 2 * HOUR + OVERDUE_AFTER.as_secs()));
        assert_eq!(overdue.stance, Stance::Overdue);
        assert!(
            overdue.summary.contains("longer than the 6 h 0 min"),
            "{}",
            overdue.summary
        );

        c.git(&["tag", "v0.1.1"], 0);
        c.push(t0 + 3 * HOUR);
        let tagged = measure(&work, &objects, at(t0 + 30 * HOUR));
        assert_eq!(tagged.stance, Stance::Current, "{}", tagged.summary);
        let release = tagged.release.expect("the tag");
        assert_eq!(
            (release.reference.as_str(), release.recorded),
            ("v0.1.1", false)
        );
        assert!(tagged.summary.contains("record not landed yet"));
    }

    /// A top composed hours before it lands waits from when it reached the trunk, as this
    /// clone's reflog saw it. Where the reflog does not show it arrive, its commit time bounds
    /// the wait, and that bound alone never makes the work overdue (the plan critique of
    /// 2026-10-19).
    #[test]
    fn a_landing_waits_from_when_it_reached_the_trunk() {
        let c = Fixture::new();
        let t0 = 1_700_000_000;
        let released = c.commit("feat: the first", t0);
        c.push(t0);
        let objects = vec![record("0.1.0", &released)];
        let work = c.work();

        // composed at t0 + 1 h, pushed to the trunk at t0 + 7 h
        c.commit("fix: a top composed long before it landed", t0 + HOUR);
        c.push(t0 + 7 * HOUR);
        let landed = measure(&work, &objects, at(t0 + 8 * HOUR));
        assert_eq!(
            landed.landed_at,
            Some(LandedAt::Reflog),
            "{}",
            landed.summary
        );
        assert_eq!(
            landed.waited_seconds,
            Some(HOUR),
            "from the push, not the commit"
        );
        assert_eq!(landed.stance, Stance::Owed, "{}", landed.summary);
        assert!(
            landed.summary.contains("since this clone saw it land"),
            "{}",
            landed.summary
        );

        // the same landing in a clone whose reflog does not show it arrive
        std::fs::remove_file(work.join(".git/logs/refs/remotes/origin/master"))
            .expect("the trunk's reflog");
        let bounded = measure(&work, &objects, at(t0 + 8 * HOUR));
        assert_eq!(
            bounded.landed_at,
            Some(LandedAt::Commit),
            "{}",
            bounded.summary
        );
        assert_eq!(
            bounded.waited_seconds,
            Some(7 * HOUR),
            "an upper bound, from the commit"
        );
        assert_eq!(
            bounded.stance,
            Stance::Owed,
            "a bound alone is never overdue: {}",
            bounded.summary
        );
        assert!(
            bounded.summary.contains("at most 7 h 0 min"),
            "{}",
            bounded.summary
        );
        assert!(
            bounded
                .summary
                .contains("which alone makes nothing overdue"),
            "{}",
            bounded.summary
        );
    }

    /// A reflog line that is not an entry is left out rather than guessed.
    #[test]
    fn a_reflog_line_that_is_not_an_entry_is_left_out() {
        // a blank line, no selector, no time, a selector with no braces, an unclosed one, a time
        // that is no number, and the entries around them
        let log = "a origin/master@{1700003600}\nb origin/master@{1700000100}\n\nc\nf origin/master\ng origin/master@{12\nd origin/master@{x}\ne origin/master@{1}";
        assert_eq!(
            reflog_entries(log),
            vec![("a", 1_700_003_600), ("b", 1_700_000_100), ("e", 1)]
        );
    }

    /// A merge is one landing, whatever it brought; a merge that brought only release commits
    /// is none.
    #[test]
    fn a_merge_is_one_landing_and_a_release_merge_is_none() {
        let c = Fixture::new();
        let t0 = 1_700_000_000;
        let released = c.commit("feat: the first", t0);
        c.push(t0);
        let objects = vec![record("0.1.0", &released)];
        c.git(&["checkout", "-q", "-b", "side"], 0);
        c.commit("fix: one", t0 + HOUR);
        c.commit("fix: two", t0 + HOUR);
        c.git(&["checkout", "-q", "master"], 0);
        c.git(
            &["merge", "-q", "--no-ff", "-m", "merge: side", "side"],
            t0 + 2 * HOUR,
        );
        c.push(t0 + 2 * HOUR);
        c.git(&["checkout", "-q", "-b", "record"], 0);
        c.commit("chore(release): record v0.1.0", t0 + 3 * HOUR);
        c.git(&["checkout", "-q", "master"], 0);
        c.git(
            &["merge", "-q", "--no-ff", "-m", "merge: record", "record"],
            t0 + 3 * HOUR,
        );
        c.push(t0 + 3 * HOUR);
        let answer = measure(&c.work(), &objects, at(t0 + 4 * HOUR));
        assert_eq!(
            (answer.landed, answer.landings),
            (2, 1),
            "{}",
            answer.summary
        );
        assert_eq!(
            answer.waited_seconds,
            Some(2 * HOUR),
            "from the merge, not the commits"
        );
    }

    /// What cannot be measured says why: no trunk, or no release this clone can read.
    #[test]
    fn what_cannot_be_measured_says_why() {
        let nowhere = tempfile::tempdir().unwrap();
        let answer = measure(nowhere.path(), &[], SystemTime::now());
        assert_eq!(answer.stance, Stance::Unknown);
        assert!(answer.release.is_none() && answer.trunk.is_none());

        let c = Fixture::new();
        c.commit("feat: the first", 1_700_000_000);
        c.push(1_700_000_000);
        let unreleased = measure(&c.work(), &[], SystemTime::now());
        assert_eq!(unreleased.stance, Stance::Unknown);
        assert!(unreleased
            .summary
            .contains("no release to measure origin/master from"));
        let elsewhere = measure(
            &c.work(),
            &[record("0.1.0", &"f".repeat(40))],
            SystemTime::now(),
        );
        assert_eq!(
            elsewhere.stance,
            Stance::Unknown,
            "a record of a commit this clone lacks"
        );
    }

    /// The version is decided only for landed work, from a recorded release, at the trunk; each
    /// other case says which it is not.
    #[test]
    fn the_version_is_decided_only_where_the_contract_can_be_measured() {
        let c = Fixture::new();
        let t0 = 1_700_000_000;
        let released = c.commit("feat: the first", t0);
        c.push(t0);
        let registry = CapabilityRegistry::builder().build().unwrap();
        let objects = vec![record("0.1.0", &released)];
        let work = c.work();

        let mut nothing = measure(&work, &objects, at(t0));
        decide_next(&mut nothing, &work, &registry, &objects);
        assert_eq!(
            (nothing.next.as_ref(), nothing.next_unmeasured.as_ref()),
            (None, None)
        );

        c.commit("fix: the second", t0 + HOUR);
        c.push(t0 + HOUR);
        let mut here = measure(&work, &objects, at(t0 + HOUR));
        decide_next(&mut here, &work, &registry, &objects);
        // this fixture published no registry, so the contract's own refusal is carried
        assert!(here.next.is_none());
        assert!(here.next_unmeasured.is_some(), "{here:?}");

        c.git(&["checkout", "-q", "-b", "side"], 0);
        c.commit("fix: elsewhere", t0 + HOUR);
        let mut away = measure(&work, &objects, at(t0 + HOUR));
        decide_next(&mut away, &work, &registry, &objects);
        assert!(away
            .next_unmeasured
            .as_deref()
            .is_some_and(|w| w.contains("not at origin/master")));

        c.git(&["checkout", "-q", "master"], 0);
        c.git(&["tag", "v0.1.1"], 0);
        c.commit("fix: after the tag", t0 + 2 * HOUR);
        c.push(t0 + 2 * HOUR);
        let mut unrecorded = measure(&work, &objects, at(t0 + 2 * HOUR));
        decide_next(&mut unrecorded, &work, &registry, &objects);
        assert!(unrecorded
            .next_unmeasured
            .as_deref()
            .is_some_and(|w| w.contains("v0.1.1 has no release record yet")));
    }

    /// The newer of a record and a tag is the release; a lone record or tag is it; none is none.
    #[test]
    fn the_newer_of_the_record_and_the_tag_is_the_release() {
        let last = |version: &str| super::super::compat::LastRelease {
            version: version.into(),
            reference: format!("v{version}"),
            read_at: "c0ffee".into(),
            recorded: true,
        };
        let newer = newest(Some(last("0.1.0")), Some("v0.2.0".into())).unwrap();
        assert_eq!(
            (newer.reference.as_str(), newer.recorded),
            ("v0.2.0", false)
        );
        let older = newest(Some(last("0.3.0")), Some("v0.2.0".into())).unwrap();
        assert_eq!((older.at.as_str(), older.recorded), ("c0ffee", true));
        assert_eq!(newest(Some(last("0.1.0")), None).unwrap().version, "0.1.0");
        assert_eq!(newest(None, Some("v0.1.0".into())).unwrap().at, "v0.1.0");
        assert_eq!(newest(None, None), None);
    }

    /// A log line that is not a landing is left out rather than guessed.
    #[test]
    fn a_line_that_is_not_a_landing_is_left_out() {
        let log = "a 10 p\nb 20 p q\n\nc\nd x\n";
        assert_eq!(
            first_parents(log),
            vec![
                Landing {
                    sha: "a",
                    at: 10,
                    merge: false
                },
                Landing {
                    sha: "b",
                    at: 20,
                    merge: true
                },
            ]
        );
    }

    /// A wait reads in minutes, then hours, then days; every stance has its word.
    #[test]
    fn a_wait_and_a_stance_read_as_words() {
        assert_eq!(age_words(Duration::from_secs(59)), "0 min");
        assert_eq!(age_words(Duration::from_secs(3 * HOUR + 120)), "3 h 2 min");
        assert_eq!(age_words(Duration::from_secs(50 * HOUR)), "2 d 2 h");
        for (stance, word) in [
            (Stance::Current, "current"),
            (Stance::Owed, "owed"),
            (Stance::Overdue, "overdue"),
            (Stance::Unknown, "unknown"),
        ] {
            assert_eq!(stance.as_str(), word);
        }
        assert_eq!(short(" 0123456789abcdef \n"), "0123456789a");
    }
}
