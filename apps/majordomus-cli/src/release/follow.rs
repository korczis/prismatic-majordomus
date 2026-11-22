//! A node follows the stable release by itself, by consent, and hands its server over in
//! order (ADR 0161, ADR 0235; I2721).
//!
//! `majordomus self update` ([`super::self_update`]) is what a person runs. This module is
//! what a running server decides on its own, every check interval: whether the upgrade the
//! published metadata offers may be applied here, now, without a person. Everything risky
//! stays where it already is — the judgement is [`super::self_update::decide`], the
//! download, verification and swap are `install.sh` — and what is added is the consent:
//!
//! - **what the declaration says** ([`FollowConfig`], the `self_update` section of the mesh
//!   declaration): `follow: stable` applies, `follow: pinned` only reports, `follow: off`
//!   starts no check at all; `check_interval_seconds` is how often (300 by default);
//!   `dirty: defer` (the default) waits for a clean checkout, `dirty: apply` does not;
//! - **what this machine says** ([`Override`], `MAJORDOMUS_SELF_UPDATE`), which can only
//!   narrow, except that `follow-dirty` is a machine's own consent to apply over a dirty
//!   checkout;
//! - **what the installation is**: only an executable `install.sh` laid out is ever
//!   upgraded; a build in a checkout is updated by building it, and never by this;
//! - **what the checkout is**: a working tree with uncommitted changes defers the upgrade,
//!   because a newer tool would judge a task in progress by another generation's rules;
//! - **what happened before**: a version that failed to install here is not retried until a
//!   newer one is published.
//!
//! [`gate`] is that decision, pure, so every branch of it is a unit test.
//!
//! ```
//! use majordomus_cli::release::follow::{gate, Facts, FollowConfig, Mode, Step};
//! use majordomus_cli::release::self_update::Verdict;
//! let mode = Mode::resolve(&FollowConfig::default(), None);
//! let clean = Facts { verdict: Verdict::Upgrade, target: "1.3.0".into(), installed: Ok(()),
//!     dirty: Some(false), failed_before: false };
//! assert_eq!(gate(&mode, &clean).step, Step::Apply);
//! let dirty = Facts { dirty: Some(true), ..clean.clone() };
//! assert_eq!(gate(&mode, &dirty).step, Step::Hold, "a dirty checkout defers without consent");
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::self_update::Verdict;

/// The default seconds between two checks of the stable pointer: five minutes.
pub const DEFAULT_CHECK_INTERVAL: u64 = 300;

/// The shortest interval a declaration may state: a minute. A shorter one would ask the site
/// more often than a release can be published; `MAJORDOMUS_SELF_UPDATE_INTERVAL` may go
/// lower, for a test.
pub const MIN_CHECK_INTERVAL: u64 = 60;

/// What a node does with the stable pointer.
///
/// ```
/// use majordomus_cli::release::follow::Follow;
/// assert_eq!(serde_json::to_value(Follow::Stable).unwrap(), "stable");
/// assert_eq!(Follow::default(), Follow::Stable);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Follow {
    /// Check every interval, and apply a newer stable release when consent allows.
    #[default]
    Stable,
    /// Check and report; apply only when a person runs `majordomus self update --apply`.
    Pinned,
    /// Start no check at all.
    Off,
}

/// Whether a checkout with uncommitted changes holds the upgrade back.
///
/// ```
/// use majordomus_cli::release::follow::Dirty;
/// assert_eq!(Dirty::default(), Dirty::Defer);
/// assert_eq!(serde_json::to_value(Dirty::Apply).unwrap(), "apply");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dirty {
    /// Wait until the checkout is clean, and check again next interval.
    #[default]
    Defer,
    /// Apply whatever the checkout holds: the declaration's consent for every node.
    Apply,
}

/// The `self_update` section of the mesh declaration. Every field has the safe default, and
/// the section as a whole is optional: a declaration that says nothing follows `stable`,
/// every five minutes, deferring over a dirty checkout.
///
/// ```
/// use majordomus_cli::release::follow::{Dirty, Follow, FollowConfig};
/// let stated: FollowConfig = serde_json::from_value(serde_json::json!({
///     "follow": "pinned", "check_interval_seconds": 900 })).unwrap();
/// assert_eq!((stated.follow, stated.check_interval_seconds, stated.dirty),
///     (Follow::Pinned, 900, Dirty::Defer));
/// assert!(FollowConfig { check_interval_seconds: 5, ..FollowConfig::default() }.validate().is_err());
/// assert!(serde_json::from_value::<FollowConfig>(serde_json::json!({"every": 5})).is_err(),
///     "a typo is an error, not a silently ignored wish");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FollowConfig {
    /// `stable` (default), `pinned` or `off`.
    #[serde(default)]
    pub follow: Follow,
    /// Seconds between two checks of `releases/latest.json`; 300 when absent, at least 60.
    #[serde(default = "default_interval")]
    pub check_interval_seconds: u64,
    /// `defer` (default) or `apply`: whether a checkout with uncommitted changes holds the
    /// upgrade back.
    #[serde(default)]
    pub dirty: Dirty,
}

fn default_interval() -> u64 {
    DEFAULT_CHECK_INTERVAL
}

impl Default for FollowConfig {
    fn default() -> Self {
        FollowConfig {
            follow: Follow::Stable,
            check_interval_seconds: DEFAULT_CHECK_INTERVAL,
            dirty: Dirty::Defer,
        }
    }
}

impl FollowConfig {
    /// Refuse an interval shorter than [`MIN_CHECK_INTERVAL`].
    ///
    /// ```
    /// use majordomus_cli::release::follow::{FollowConfig, MIN_CHECK_INTERVAL};
    /// assert!(FollowConfig::default().validate().is_ok());
    /// let hasty = FollowConfig {
    ///     check_interval_seconds: MIN_CHECK_INTERVAL - 1,
    ///     ..FollowConfig::default()
    /// };
    /// assert!(hasty.validate().unwrap_err().contains("below the minimum"));
    /// ```
    pub fn validate(&self) -> Result<(), String> {
        if self.check_interval_seconds < MIN_CHECK_INTERVAL {
            return Err(format!(
                "self_update.check_interval_seconds {} is below the minimum of {MIN_CHECK_INTERVAL}",
                self.check_interval_seconds
            ));
        }
        Ok(())
    }
}

/// What a machine says for itself, in `MAJORDOMUS_SELF_UPDATE`: the service unit or the shell
/// profile of one node, never the repository.
///
/// ```
/// use majordomus_cli::release::follow::Override;
/// assert_eq!(Override::parse("off"), Some(Override::Off));
/// assert_eq!(Override::parse("follow-dirty"), Some(Override::FollowDirty));
/// assert_eq!(Override::parse("whatever"), None);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Override {
    /// This machine starts no check.
    Off,
    /// This machine checks and reports, and applies nothing.
    Check,
    /// The declaration decides (the same as no override).
    Follow,
    /// The declaration decides, and this machine consents to an upgrade over a dirty checkout.
    FollowDirty,
}

impl Override {
    /// Read one spelling; anything else is no override (and the reader says so).
    ///
    /// ```
    /// use majordomus_cli::release::follow::Override;
    /// assert_eq!(Override::parse(" check "), Some(Override::Check));
    /// assert_eq!(Override::parse("on"), None);
    /// ```
    pub fn parse(word: &str) -> Option<Override> {
        match word.trim() {
            "off" => Some(Override::Off),
            "check" => Some(Override::Check),
            "follow" => Some(Override::Follow),
            "follow-dirty" => Some(Override::FollowDirty),
            _ => None,
        }
    }

    /// The machine's override, from the environment.
    ///
    /// ```
    /// use majordomus_cli::release::follow::Override;
    /// // whatever the environment says, it is one of the spellings or no override at all
    /// let read = Override::from_env();
    /// let said = std::env::var("MAJORDOMUS_SELF_UPDATE").ok();
    /// assert_eq!(read, said.as_deref().and_then(Override::parse));
    /// ```
    pub fn from_env() -> Option<Override> {
        std::env::var("MAJORDOMUS_SELF_UPDATE")
            .ok()
            .and_then(|w| Override::parse(&w))
    }
}

/// The effective mode on this machine: the declaration, narrowed by the machine's override.
///
/// ```
/// use majordomus_cli::release::follow::{Follow, FollowConfig, Mode, Override};
/// let declared = FollowConfig::default();
/// assert_eq!(Mode::resolve(&declared, Some(Override::Check)).follow, Follow::Pinned);
/// assert_eq!(Mode::resolve(&declared, Some(Override::Off)).follow, Follow::Off);
/// assert!(Mode::resolve(&declared, Some(Override::FollowDirty)).dirty_consent);
/// // a machine cannot turn on what the declaration turned off
/// let off = FollowConfig { follow: Follow::Off, ..FollowConfig::default() };
/// assert_eq!(Mode::resolve(&off, Some(Override::Follow)).follow, Follow::Off);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Mode {
    /// What the node does.
    pub follow: Follow,
    /// Whether an upgrade may be applied over a dirty checkout.
    pub dirty_consent: bool,
    /// Seconds between checks.
    pub interval_seconds: u64,
    /// Where each part came from, in one line a person reads.
    pub source: String,
}

impl Mode {
    /// The declaration's section, narrowed by `machine`.
    ///
    /// ```
    /// use majordomus_cli::release::follow::{Dirty, FollowConfig, Mode, Override};
    /// let consenting = FollowConfig { dirty: Dirty::Apply, ..FollowConfig::default() };
    /// assert!(Mode::resolve(&consenting, None).dirty_consent);
    /// assert!(!Mode::resolve(&FollowConfig::default(), Some(Override::Follow)).dirty_consent);
    /// ```
    pub fn resolve(declared: &FollowConfig, machine: Option<Override>) -> Mode {
        let follow = match (declared.follow, machine) {
            (Follow::Off, _) | (_, Some(Override::Off)) => Follow::Off,
            (Follow::Pinned, _) | (_, Some(Override::Check)) => Follow::Pinned,
            (Follow::Stable, _) => Follow::Stable,
        };
        let dirty_consent =
            declared.dirty == Dirty::Apply || machine == Some(Override::FollowDirty);
        let interval_seconds = std::env::var("MAJORDOMUS_SELF_UPDATE_INTERVAL")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|s| *s > 0)
            .unwrap_or(declared.check_interval_seconds);
        let source = match machine {
            Some(m) => format!(
                "the mesh declaration, narrowed by MAJORDOMUS_SELF_UPDATE={}",
                serde_json::to_value(m)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default()
            ),
            None => "the mesh declaration".to_string(),
        };
        Mode {
            follow,
            dirty_consent,
            interval_seconds,
            source,
        }
    }
}

/// What one round knows when it asks whether to apply.
///
/// ```
/// use majordomus_cli::release::follow::{gate, Facts, FollowConfig, Mode, Step};
/// use majordomus_cli::release::self_update::Verdict;
/// let unknown = Facts { verdict: Verdict::Upgrade, target: "1.3.0".into(), installed: Ok(()),
///     dirty: None, failed_before: false };
/// // a checkout whose state could not be told holds the upgrade back
/// assert_eq!(gate(&Mode::resolve(&FollowConfig::default(), None), &unknown).step, Step::Hold);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The verdict of [`super::self_update::decide`] on the stable pointer.
    pub verdict: Verdict,
    /// The version the stable pointer names (empty when unread).
    pub target: String,
    /// Whether the executable is an installation `install.sh` laid out, or why not.
    pub installed: Result<(), String>,
    /// Whether the served checkout has uncommitted changes; `None` when that could not be
    /// told, which holds the upgrade back like a dirty checkout does.
    pub dirty: Option<bool>,
    /// Whether this version failed to install on this installation before.
    pub failed_before: bool,
}

/// Apply now, or hold and check again next interval.
///
/// ```
/// use majordomus_cli::release::follow::Step;
/// assert_eq!(Step::default(), Step::Hold, "a record without a step applied nothing");
/// assert_eq!(serde_json::to_value(Step::Apply).unwrap(), "apply");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Run `self update --apply` now.
    Apply,
    /// Apply nothing this round, for the reason beside it.
    #[default]
    Hold,
}

/// The step, with the sentence that says why.
///
/// ```
/// use majordomus_cli::release::follow::{gate, Facts, FollowConfig, Mode, Step};
/// use majordomus_cli::release::self_update::Verdict;
/// let clean = Facts { verdict: Verdict::Upgrade, target: "1.3.0".into(), installed: Ok(()),
///     dirty: Some(false), failed_before: false };
/// let gated = gate(&Mode::resolve(&FollowConfig::default(), None), &clean);
/// assert_eq!(gated.step, Step::Apply);
/// assert!(!gated.reason.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Gated {
    /// Apply or hold.
    pub step: Step,
    /// Why, in one sentence.
    pub reason: String,
}

/// Decide one round. The order is the order of the consent: nothing to apply, no
/// installation, a mode that only reports, a version that failed here, a checkout whose
/// state is unknown or dirty without consent — and only then apply.
///
/// ```
/// use majordomus_cli::release::follow::{gate, Facts, FollowConfig, Follow, Mode, Step};
/// use majordomus_cli::release::self_update::Verdict;
/// let mode = Mode::resolve(&FollowConfig::default(), None);
/// let f = Facts { verdict: Verdict::Upgrade, target: "1.3.0".into(), installed: Ok(()),
///     dirty: Some(false), failed_before: false };
/// let built = Facts { installed: Err("a build in a checkout".into()), ..f.clone() };
/// assert!(gate(&mode, &built).reason.contains("a build in a checkout"));
/// let failed = Facts { failed_before: true, ..f.clone() };
/// assert_eq!(gate(&mode, &failed).step, Step::Hold);
/// let pinned = Mode { follow: Follow::Pinned, ..mode.clone() };
/// assert!(gate(&pinned, &f).reason.contains("self update --apply"));
/// ```
pub fn gate(mode: &Mode, facts: &Facts) -> Gated {
    let hold = |reason: String| Gated {
        step: Step::Hold,
        reason,
    };
    let v = &facts.target;
    match facts.verdict {
        Verdict::Upgrade => {}
        Verdict::Current => return hold(format!("v{v} is running; nothing to apply")),
        Verdict::Refused => {
            return hold(format!(
                "the stable pointer names v{v}, which an update may not move to"
            ))
        }
        Verdict::Unanswered => {
            return hold("the stable pointer could not be read; checked again next interval".into())
        }
    }
    if let Err(why) = &facts.installed {
        return hold(format!(
            "v{v} is available, but this executable is not an installation: {why}"
        ));
    }
    match mode.follow {
        Follow::Off => return hold("follow is off".into()),
        Follow::Pinned => {
            return hold(format!(
                "v{v} is available; this node is pinned, so only a person applies it, with \
                 majordomus self update --apply"
            ))
        }
        Follow::Stable => {}
    }
    if facts.failed_before {
        return hold(format!(
            "v{v} failed to install here before; it is retried only once a newer release is published"
        ));
    }
    match (facts.dirty, mode.dirty_consent) {
        (None, false) => {
            return hold(format!(
                "v{v} is available, but whether the checkout is clean could not be told; deferred"
            ))
        }
        (Some(true), false) => {
            return hold(format!(
                "v{v} is available, and the checkout has uncommitted changes: deferred until it \
                 is clean (consent: self_update.dirty: apply in the declaration, or \
                 MAJORDOMUS_SELF_UPDATE=follow-dirty on this machine)"
            ))
        }
        _ => {}
    }
    Gated {
        step: Step::Apply,
        reason: format!("v{v} is newer, stable and listed, and every consent holds"),
    }
}

/// What the follower last did on this installation, kept at `<prefix>/self-update.json` so
/// that every server of the installation, and `self update --check`, read the same record.
///
/// ```
/// use majordomus_cli::release::follow::{FollowState, Step};
/// let s: FollowState = serde_json::from_str(r#"{"checked_at_unix":1,"running":"1.2.0",
///   "stable":"1.3.0","step":"hold","reason":"dirty"}"#).unwrap();
/// assert_eq!(s.step, Step::Hold);
/// assert!(s.failed.is_empty() && s.handover.is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FollowState {
    /// When the round ran, seconds since the Unix epoch.
    pub checked_at_unix: u64,
    /// The version running when it ran.
    pub running: String,
    /// The version the stable pointer named, when it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable: Option<String>,
    /// What the round decided.
    #[serde(default)]
    pub step: Step,
    /// Why.
    pub reason: String,
    /// The version the round installed and read back, when it applied one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
    /// How the running server was handed over to the new version, when one was installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handover: Option<String>,
    /// Versions that failed to install here, never retried.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<String>,
}

/// Where an installation's follower record lives.
///
/// ```
/// use majordomus_cli::release::follow::state_path;
/// let at = state_path(std::path::Path::new("/opt/majordomus"));
/// assert_eq!(at, std::path::Path::new("/opt/majordomus/self-update.json"));
/// ```
pub fn state_path(prefix: &Path) -> PathBuf {
    prefix.join("self-update.json")
}

/// Read the record; absent or unreadable is no record.
///
/// ```
/// use majordomus_cli::release::follow::{read_state, state_path};
/// let dir = tempfile::tempdir().unwrap();
/// assert!(read_state(dir.path()).is_none(), "absent");
/// std::fs::write(state_path(dir.path()), "not json").unwrap();
/// assert!(read_state(dir.path()).is_none(), "unreadable");
/// ```
pub fn read_state(prefix: &Path) -> Option<FollowState> {
    let text = std::fs::read_to_string(state_path(prefix)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write the record atomically (a temporary file renamed over it).
///
/// ```
/// use majordomus_cli::release::follow::{read_state, write_state, FollowState};
/// let dir = tempfile::tempdir().unwrap();
/// let state = FollowState { running: "1.2.0".into(), reason: "up to date".into(),
///     ..FollowState::default() };
/// write_state(dir.path(), &state).unwrap();
/// assert_eq!(read_state(dir.path()), Some(state));
/// ```
pub fn write_state(prefix: &Path, state: &FollowState) -> std::io::Result<()> {
    let path = state_path(prefix);
    let tmp = path.with_extension(format!("json.{}", std::process::id()));
    let body = serde_json::to_string_pretty(state).unwrap_or_default();
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &path)
}

/// One applier per installation: a lock file holding the applying process's pid, created
/// exclusively. A lock whose process is gone is taken over; a live one is never waited on.
///
/// ```
/// use majordomus_cli::release::follow::ApplyLock;
/// let dir = tempfile::tempdir().unwrap();
/// {
///     let _held = ApplyLock::take(dir.path()).unwrap();
///     assert!(dir.path().join("self-update.lock").exists());
/// }
/// assert!(!dir.path().join("self-update.lock").exists(), "the lock ends with its holder");
/// ```
pub struct ApplyLock {
    path: PathBuf,
}

impl ApplyLock {
    /// Take the lock, or say who holds it.
    ///
    /// ```
    /// use majordomus_cli::release::follow::ApplyLock;
    /// let dir = tempfile::tempdir().unwrap();
    /// let held = ApplyLock::take(dir.path()).unwrap();
    /// assert!(ApplyLock::take(dir.path()).is_err(), "one applier per installation");
    /// drop(held);
    /// assert!(ApplyLock::take(dir.path()).is_ok(), "released on drop");
    /// // a lock left by a process that is gone is taken over
    /// std::fs::write(dir.path().join("self-update.lock"), "999999999").unwrap();
    /// assert!(ApplyLock::take(dir.path()).is_ok());
    /// ```
    pub fn take(prefix: &Path) -> Result<ApplyLock, String> {
        let path = prefix.join("self-update.lock");
        for _ in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    use std::io::Write;
                    let _ = write!(f, "{}", std::process::id());
                    return Ok(ApplyLock { path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok());
                    match holder {
                        Some(pid) if pid > 0 && alive(pid) => {
                            return Err(format!("process {pid} is applying an update here"))
                        }
                        _ => {
                            let _ = std::fs::remove_file(&path);
                        }
                    }
                }
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Err(format!("{} could not be taken", path.display()))
    }
}

impl Drop for ApplyLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: kill with signal 0 sends nothing; it only asks whether the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Whether the checkout at `root` has uncommitted changes, untracked files included; `None`
/// when git could not answer.
///
/// ```
/// use majordomus_cli::release::follow::checkout_dirty;
/// let dir = tempfile::tempdir().unwrap();
/// assert_eq!(checkout_dirty(dir.path()), None, "no repository, no answer");
/// ```
pub fn checkout_dirty(root: &Path) -> Option<bool> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain=v1"])
        .output()
        .ok()?;
    out.status.success().then_some(!out.stdout.is_empty())
}

/// The wait before the next round: the interval with up to a tenth of it added, so that the
/// nodes of a mesh do not all ask the site in the same second.
///
/// ```
/// use majordomus_cli::release::follow::jittered;
/// use std::time::Duration;
/// let d = jittered(300, 7);
/// assert!(d >= Duration::from_secs(300) && d <= Duration::from_secs(330));
/// assert_eq!(jittered(1, 0), Duration::from_secs(1));
/// ```
pub fn jittered(interval_seconds: u64, seed: u64) -> Duration {
    let spread = interval_seconds / 10;
    let extra = if spread == 0 { 0 } else { seed % (spread + 1) };
    Duration::from_secs(interval_seconds + extra)
}

/// Whether the server this process runs is owned by a client (an MCP session's stdio, a
/// pipe, or a server started with `--idle`), and so may be handed over by starting the new
/// version beside it. A server that runs until it is stopped is a service manager's or an
/// operator's to restart (I2722).
///
/// ```
/// use majordomus_cli::release::follow::client_owned;
/// let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
/// assert!(client_owned(&args(&["majordomus-cli", "mcp"]), false));
/// assert!(client_owned(&args(&["majordomus-cli", "serve", "--idle", "1800"]), false));
/// assert!(client_owned(&args(&["majordomus-cli", "serve", "--idle=60"]), false));
/// assert!(!client_owned(&args(&["majordomus-cli", "serve", "--idle", "0"]), false));
/// assert!(!client_owned(&args(&["majordomus-cli", "serve"]), false));
/// assert!(client_owned(&args(&["majordomus-cli", "serve"]), true), "a pipe owns it");
/// ```
pub fn client_owned(args: &[String], stdin_is_a_pipe: bool) -> bool {
    if stdin_is_a_pipe || args.get(1).map(String::as_str) == Some("mcp") {
        return true;
    }
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let value = if a == "--idle" {
            it.next().cloned()
        } else {
            a.strip_prefix("--idle=").map(str::to_string)
        };
        if let Some(v) = value {
            return v.trim().parse::<u64>().is_ok_and(|s| s > 0);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            verdict: Verdict::Upgrade,
            target: "1.3.0".into(),
            installed: Ok(()),
            dirty: Some(false),
            failed_before: false,
        }
    }

    /// The evidence I2721 names for the consent: a following node applies a newer stable
    /// release over a clean checkout, and holds it back over a dirty one, an unknown one, a
    /// version that failed here, a pinned or switched-off mode and a build in a checkout.
    #[test]
    fn node_follows_stable_pointer_test() {
        let mode = Mode::resolve(&FollowConfig::default(), None);
        assert_eq!(gate(&mode, &facts()).step, Step::Apply);

        let dirty = Facts {
            dirty: Some(true),
            ..facts()
        };
        let held = gate(&mode, &dirty);
        assert_eq!(held.step, Step::Hold);
        assert!(held.reason.contains("uncommitted"), "{}", held.reason);
        // consent, from the declaration or from the machine, applies over it
        let declared = FollowConfig {
            dirty: Dirty::Apply,
            ..FollowConfig::default()
        };
        assert_eq!(
            gate(&Mode::resolve(&declared, None), &dirty).step,
            Step::Apply
        );
        let machine = Mode::resolve(&FollowConfig::default(), Some(Override::FollowDirty));
        assert_eq!(gate(&machine, &dirty).step, Step::Apply);
        // an unknown checkout is held like a dirty one, and consent covers it too
        let unknown = Facts {
            dirty: None,
            ..facts()
        };
        assert_eq!(gate(&mode, &unknown).step, Step::Hold);
        assert_eq!(gate(&machine, &unknown).step, Step::Apply);

        for (m, why) in [
            (
                Mode::resolve(&FollowConfig::default(), Some(Override::Check)),
                "pinned",
            ),
            (
                Mode::resolve(&FollowConfig::default(), Some(Override::Off)),
                "off",
            ),
        ] {
            let g = gate(&m, &facts());
            assert_eq!(g.step, Step::Hold, "{why}");
            assert!(g.reason.contains(why), "{}", g.reason);
        }
        let built = Facts {
            installed: Err("a build in a checkout is updated by building it".into()),
            ..facts()
        };
        assert!(gate(&mode, &built).reason.contains("not an installation"));
        let failed = Facts {
            failed_before: true,
            ..facts()
        };
        assert!(gate(&mode, &failed)
            .reason
            .contains("failed to install here"));
        // nothing to apply is never an apply, whatever the consent
        for verdict in [Verdict::Current, Verdict::Refused, Verdict::Unanswered] {
            let f = Facts { verdict, ..facts() };
            assert_eq!(gate(&machine, &f).step, Step::Hold, "{verdict:?}");
        }
    }

    #[test]
    fn the_record_round_trips_and_is_replaced_whole() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_state(dir.path()).is_none());
        let s = FollowState {
            checked_at_unix: 7,
            running: "1.2.0".into(),
            stable: Some("1.3.0".into()),
            step: Step::Apply,
            reason: "every consent holds".into(),
            installed: Some("1.3.0".into()),
            handover: Some("serve ensure".into()),
            failed: vec![],
        };
        write_state(dir.path(), &s).unwrap();
        assert_eq!(read_state(dir.path()), Some(s));
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(leftovers, vec!["self-update.json".to_string()]);
    }
}
