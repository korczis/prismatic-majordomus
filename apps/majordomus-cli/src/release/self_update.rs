//! `majordomus self update`: an installation moves to the published stable release by
//! re-running the published installer, and never by a second implementation of it (ADR 0161,
//! I2720).
//!
//! The risky part of an upgrade (the download, the digest and size check, the unpacking and
//! the atomic swap of the launchers) is `install.sh`, tested end to end by case 85. This
//! module adds the two things the installer cannot decide for an installation that is
//! already running:
//!
//! - **what applies**: the running version, read from the installation's own tree, against
//!   the version the published metadata names, judged forward only and stable only
//!   ([`decide`]); and
//! - **where the installation is**: the prefix and the launcher directory this executable
//!   was installed into ([`Installation::locate`]), so that the installer is handed the
//!   installation's own places and not its defaults.
//!
//! [`apply`] then runs the installer the installation's `share/distribution.yaml` names, with
//! `--version <tag>`, `--prefix` and `--install-dir`, and reads the installed version back
//! from the launcher it wrote.
//!
//! The lifecycle, end to end: judge, then (only for an upgrade) apply.
//!
//! ```
//! use majordomus_cli::release::self_update::{decide, stated_version, Verdict};
//! use majordomus_cli::release::version::Version;
//! // the running version, as the installation's share/version.txt states it
//! let running = Version::parse(&stated_version("# GENERATED\nversion=0.9.0\n").unwrap()).unwrap();
//! // the version releases/latest.json names
//! let stable = Version::parse("1.0.0").unwrap();
//! let decision = decide(&running, Some(&stable), None);
//! assert_eq!(decision.verdict, Verdict::Upgrade, "only now would apply() run install.sh");
//! // the same installation asked to go back is refused, and nothing runs
//! assert_eq!(decide(&stable, Some(&running), Some("v0.9.0")).verdict, Verdict::Refused);
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::version::Version;

/// Why an upgrade does not apply, or why the version asked for is refused.
///
/// ```
/// use majordomus_cli::release::self_update::Refusal;
/// assert_eq!(serde_json::to_value(Refusal::Downgrade).unwrap(), "downgrade");
/// assert_eq!(serde_json::to_value(Refusal::Prerelease).unwrap(), "prerelease");
/// assert_eq!(serde_json::to_value(Refusal::Unlisted).unwrap(), "unlisted");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    /// The target is lower than the running version: going back is an operator's act through
    /// `install.sh --version`, never this command's (ADR 0161, forward only).
    Downgrade,
    /// The target is a pre-release: a node follows the stable channel only.
    Prerelease,
    /// The published metadata lists no such release.
    Unlisted,
}

/// The verdict of comparing the running version with a target.
///
/// ```
/// use majordomus_cli::release::self_update::Verdict;
/// assert_eq!(serde_json::to_value(Verdict::Upgrade).unwrap(), "upgrade");
/// assert_eq!(serde_json::to_value(Verdict::Current).unwrap(), "current");
/// assert_eq!(serde_json::to_value(Verdict::Refused).unwrap(), "refused");
/// assert_eq!(serde_json::to_value(Verdict::Unanswered).unwrap(), "unanswered");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The target is newer than the running version, stable and listed: `--apply` installs it.
    Upgrade,
    /// The running version is the target: nothing applies.
    Current,
    /// The target is refused, for the reason beside it.
    Refused,
    /// The question could not be answered: the metadata or the installer could not be read.
    /// Not a refusal and never a yes; nothing is installed.
    Unanswered,
}

/// What [`decide`] concluded, with the sentence a person reads: the verdict, the refusal
/// when there is one, and the reason, so that a report and a script read the same answer.
///
/// ```
/// use majordomus_cli::release::self_update::{Decision, Refusal, Verdict};
/// let d: Decision = serde_json::from_str(
///     r#"{"verdict":"refused","refusal":"downgrade","reason":"v0.9.0 is older"}"#).unwrap();
/// assert_eq!((d.verdict, d.refusal), (Verdict::Refused, Some(Refusal::Downgrade)));
/// // an upgrade carries no refusal, and none is serialized
/// let up = Decision { verdict: Verdict::Upgrade, refusal: None, reason: "newer".into() };
/// assert!(!serde_json::to_string(&up).unwrap().contains("refusal"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Decision {
    /// The verdict.
    pub verdict: Verdict,
    /// Why a target is refused; absent unless the verdict is `refused`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Refusal>,
    /// The reason, in one sentence.
    pub reason: String,
}

/// Judge a target against the running version: forward only and stable only.
///
/// `target` is the version the published metadata names (`None` when it lists no such
/// release); `asked` is the version a person named, for the sentence (the stable pointer when
/// absent). The pre-release check comes before the order: a pre-release is refused whether or
/// not it is newer.
///
/// ```
/// use majordomus_cli::release::self_update::{decide, Refusal, Verdict};
/// use majordomus_cli::release::version::Version;
/// let v = |s: &str| Version::parse(s).unwrap();
/// // newer, stable and listed: an upgrade
/// assert_eq!(decide(&v("0.9.0"), Some(&v("1.0.0")), None).verdict, Verdict::Upgrade);
/// // a pre-release build moves to its own stable release: 1.0.0-alpha.10 < 1.0.0
/// assert_eq!(decide(&v("1.0.0-alpha.10"), Some(&v("1.0.0")), None).verdict, Verdict::Upgrade);
/// assert_eq!(decide(&v("1.0.0"), Some(&v("1.0.0")), None).verdict, Verdict::Current);
/// let down = decide(&v("1.0.0"), Some(&v("0.9.0")), Some("v0.9.0"));
/// assert_eq!((down.verdict, down.refusal), (Verdict::Refused, Some(Refusal::Downgrade)));
/// let pre = decide(&v("0.9.0"), Some(&v("1.1.0-rc.1")), Some("v1.1.0-rc.1"));
/// assert_eq!(pre.refusal, Some(Refusal::Prerelease));
/// let missing = decide(&v("0.9.0"), None, Some("v0.0.2"));
/// assert_eq!(missing.refusal, Some(Refusal::Unlisted));
/// assert!(missing.reason.contains("v0.0.2"), "{}", missing.reason);
/// ```
pub fn decide(running: &Version, target: Option<&Version>, asked: Option<&str>) -> Decision {
    let refused = |refusal: Refusal, reason: String| Decision {
        verdict: Verdict::Refused,
        refusal: Some(refusal),
        reason,
    };
    let Some(target) = target else {
        let asked = asked.unwrap_or("the stable release");
        return refused(
            Refusal::Unlisted,
            format!("the published metadata lists no release {asked}; nothing is installed"),
        );
    };
    if target.is_prerelease() {
        return refused(
            Refusal::Prerelease,
            format!(
                "v{target} is a pre-release, and an installation follows the stable channel only; \
                 install a pre-release by hand with install.sh --version v{target}"
            ),
        );
    }
    match target.cmp(running) {
        std::cmp::Ordering::Less => refused(
            Refusal::Downgrade,
            format!(
                "v{target} is older than the running v{running}, and an update moves forward \
                 only; going back is install.sh --version v{target}, run by a person"
            ),
        ),
        std::cmp::Ordering::Equal => Decision {
            verdict: Verdict::Current,
            refusal: None,
            reason: format!("v{running} is running, and it is the target; nothing applies"),
        },
        std::cmp::Ordering::Greater => Decision {
            verdict: Verdict::Upgrade,
            refusal: None,
            reason: format!("v{target} is newer than the running v{running}, stable and listed"),
        },
    }
}

/// Where an installation made by `install.sh` lives, found from the executable's own path.
///
/// The installer lays a release out as `<prefix>/versions/<version>/` holding `bin/`,
/// `libexec/` and `share/`, and writes launchers into `<install dir>` that export
/// `MAJORDOMUS_LAUNCHER` as their own path (`share/install/install.sh.in`, `write_launcher`).
/// [`apply`] hands the installer exactly these places, so that an update lands where the
/// installation already is and not at the installer's defaults.
///
/// ```
/// use majordomus_cli::release::self_update::Installation;
/// let at = Installation {
///     prefix: "/home/me/.local/share/majordomus".into(),
///     install_dir: "/home/me/.local/bin".into(),
///     tree: "/home/me/.local/share/majordomus/versions/0.9.0".into(),
///     version: "0.9.0".into(),
/// };
/// assert!(at.tree.starts_with(&at.prefix), "a version tree lives under its prefix");
/// let json = serde_json::to_value(&at).unwrap();
/// assert_eq!(json["install_dir"], "/home/me/.local/bin");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Installation {
    /// The installer's `--prefix`: the directory holding `versions/`.
    pub prefix: PathBuf,
    /// The installer's `--install-dir`: the directory holding the launchers.
    pub install_dir: PathBuf,
    /// The version tree this executable runs from.
    pub tree: PathBuf,
    /// The version that tree states (`share/version.txt`).
    pub version: String,
}

impl Installation {
    /// Find the installation `exe` belongs to, given the launcher path the environment
    /// carries. `Err` says, in a sentence, why `exe` is not one: a build in a checkout, a tree
    /// with no `versions/` parent, no version file, or no launcher.
    ///
    /// ```
    /// use majordomus_cli::release::self_update::Installation;
    /// let root = tempfile::tempdir().unwrap();
    /// let tree = root.path().join("share/majordomus/versions/0.9.0");
    /// std::fs::create_dir_all(tree.join("share")).unwrap();
    /// std::fs::create_dir_all(tree.join("libexec")).unwrap();
    /// // the projection `majordomus generate` writes: a header, then `version=`
    /// std::fs::write(tree.join("share/version.txt"),
    ///     "# GENERATED FILE — DO NOT EDIT DIRECTLY\n# Source: Cargo.toml\nversion=0.9.0\n").unwrap();
    /// let exe = tree.join("libexec/majordomus-cli");
    /// let launcher = root.path().join("bin/majordomus");
    /// let found = Installation::locate(&exe, Some(&launcher)).unwrap();
    /// assert_eq!(found.prefix, root.path().join("share/majordomus"));
    /// assert_eq!(found.install_dir, root.path().join("bin"));
    /// assert_eq!(found.version, "0.9.0");
    /// // a build in a checkout is no installation
    /// let built = root.path().join("target/debug/majordomus-cli");
    /// assert!(Installation::locate(&built, Some(&launcher)).unwrap_err().contains("not installed"));
    /// // nor is a tree the launcher never named
    /// assert!(Installation::locate(&exe, None).unwrap_err().contains("MAJORDOMUS_LAUNCHER"));
    /// ```
    pub fn locate(exe: &Path, launcher: Option<&Path>) -> Result<Installation, String> {
        let tree = exe
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| format!("{} has no installation tree above it", exe.display()))?;
        let versions = tree.parent().filter(|p| p.file_name() == Some("versions".as_ref()));
        let Some(versions) = versions else {
            return Err(format!(
                "{} is not installed by install.sh (no versions/ directory above it): a build \
                 in a checkout is updated by building it",
                exe.display()
            ));
        };
        let prefix = versions
            .parent()
            .ok_or_else(|| format!("{} has no prefix above it", versions.display()))?;
        let version_file = tree.join("share/version.txt");
        let text = std::fs::read_to_string(&version_file)
            .map_err(|e| format!("{} could not be read: {e}", version_file.display()))?;
        let version = stated_version(&text).ok_or_else(|| {
            format!("{} states no version line (version=...)", version_file.display())
        })?;
        let launcher = launcher.ok_or_else(|| {
            "MAJORDOMUS_LAUNCHER is not set, so the launcher this installation runs through is \
             unknown; run it as `majordomus self update`, through the launcher install.sh wrote"
                .to_string()
        })?;
        let install_dir = launcher
            .parent()
            .ok_or_else(|| format!("the launcher {} has no directory", launcher.display()))?;
        Ok(Installation {
            prefix: prefix.to_path_buf(),
            install_dir: install_dir.to_path_buf(),
            tree: tree.to_path_buf(),
            version,
        })
    }
}

/// The version a `share/version.txt` states, read as `bin/majordomus` reads it: the first
/// `version=` line; comments and blank lines are not versions.
///
/// ```
/// use majordomus_cli::release::self_update::stated_version;
/// assert_eq!(stated_version("# GENERATED\nversion=1.0.0\n").as_deref(), Some("1.0.0"));
/// assert_eq!(stated_version("# only a header\n"), None);
/// ```
pub fn stated_version(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("version="))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// What running the installer came to: the argument vector that ran, its exit status, the
/// tail of what it printed, and the version the launcher reports afterwards. An installer
/// that failed reads back nothing, whatever it printed.
///
/// ```
/// use majordomus_cli::release::self_update::Applied;
/// let a: Applied = serde_json::from_str(
///     r#"{"argv":["sh","install.sh","--version","v1.0.0"],"status":10,"output":["digest mismatch"]}"#,
/// ).unwrap();
/// assert_eq!(a.status, Some(10));
/// assert!(a.installed.is_none(), "a failed run installs nothing it can read back");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Applied {
    /// The command that ran, as an argument vector (the installer's path first after `sh`).
    pub argv: Vec<String>,
    /// The installer's exit status; absent when it was ended by a signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<i32>,
    /// The last lines the installer printed, stdout and stderr together.
    pub output: Vec<String>,
    /// The version the launcher reports after the run (`majordomus version`), when it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
}

/// Run `installer` (a file holding `install.sh`) for `tag` into `at`, then ask the launcher
/// it wrote which version it now runs. The environment is inherited unchanged, so a release
/// base an operator or a test exported (`MAJORDOMUS_RELEASE_BASE_URL`) reaches the installer
/// as it would reach a person's own run of it.
///
/// ```
/// use majordomus_cli::release::self_update::{apply, Installation};
/// let root = tempfile::tempdir().unwrap();
/// let script = root.path().join("install.sh");
/// // a stand-in installer that writes a launcher reporting the version it was asked for
/// std::fs::write(&script, "set -e\nv=$2; dir=$6; mkdir -p \"$dir\"\n\
///   printf '#!/bin/sh\\necho \"majordomus %s\"\\n' \"${v#v}\" > \"$dir/majordomus\"\n\
///   chmod 755 \"$dir/majordomus\"\necho installed\n").unwrap();
/// let at = Installation {
///     prefix: root.path().join("prefix"), install_dir: root.path().join("bin"),
///     tree: root.path().join("prefix/versions/0.9.0"), version: "0.9.0".into(),
/// };
/// let got = apply(&script, "v1.0.0", &at);
/// assert_eq!(got.status, Some(0), "{:?}", got.output);
/// assert_eq!(got.argv[2..4], ["--version".to_string(), "v1.0.0".to_string()]);
/// assert_eq!(got.installed.as_deref(), Some("1.0.0"));
/// ```
pub fn apply(installer: &Path, tag: &str, at: &Installation) -> Applied {
    let argv: Vec<String> = vec![
        "sh".into(),
        installer.display().to_string(),
        "--version".into(),
        tag.into(),
        "--prefix".into(),
        at.prefix.display().to_string(),
        "--install-dir".into(),
        at.install_dir.display().to_string(),
    ];
    let run = Command::new(&argv[0]).args(&argv[1..]).output();
    let (status, output) = match run {
        Err(e) => (None, vec![format!("sh could not be run: {e}")]),
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&o.stderr));
            let lines: Vec<String> = text.lines().map(str::to_string).collect();
            let tail = lines[lines.len().saturating_sub(20)..].to_vec();
            (o.status.code(), tail)
        }
    };
    let installed = (status == Some(0))
        .then(|| read_back(&at.install_dir.join("majordomus")))
        .flatten();
    Applied {
        argv,
        status,
        output,
        installed,
    }
}

/// The version a launcher reports, from its `majordomus <version>` line.
fn read_back(launcher: &Path) -> Option<String> {
    let out = Command::new(launcher).arg("version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find_map(|l| l.strip_prefix("majordomus "))
        .map(|v| v.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    /// The evidence I2720 names: a downgrade, a pre-release and an unlisted version are each
    /// refused with their own reason, and none of them is ever an upgrade.
    #[test]
    fn self_update_refuses_downgrade_test() {
        let running = v("1.0.0");
        let down = decide(&running, Some(&v("0.30.2")), Some("v0.30.2"));
        assert_eq!(down.verdict, Verdict::Refused);
        assert_eq!(down.refusal, Some(Refusal::Downgrade));
        assert!(down.reason.contains("forward only"), "{}", down.reason);
        // a newer pre-release is refused as a pre-release, not offered as newer
        let pre = decide(&running, Some(&v("1.1.0-alpha.1")), Some("v1.1.0-alpha.1"));
        assert_eq!(pre.refusal, Some(Refusal::Prerelease));
        // an older pre-release is still a pre-release first
        let old_pre = decide(&running, Some(&v("0.9.0-rc.1")), None);
        assert_eq!(old_pre.refusal, Some(Refusal::Prerelease));
        let unlisted = decide(&running, None, Some("v9.9.9"));
        assert_eq!(unlisted.refusal, Some(Refusal::Unlisted));
        assert!(unlisted.reason.contains("v9.9.9"));
    }

    /// The evidence I2720 names: --apply hands the installer the version and the
    /// installation's own places, and reports what the new launcher says, nothing it assumed.
    #[test]
    fn self_update_wraps_installer_test() {
        let root = tempfile::tempdir().unwrap();
        let script = root.path().join("install.sh");
        let seen = root.path().join("argv");
        std::fs::write(
            &script,
            format!(
                "printf '%s\\n' \"$@\" > '{}'\nmkdir -p \"$6\"\n\
                 printf '#!/bin/sh\\necho \"majordomus 1.0.0\"\\n' > \"$6/majordomus\"\n\
                 chmod 755 \"$6/majordomus\"\n",
                seen.display()
            ),
        )
        .unwrap();
        let at = Installation {
            prefix: root.path().join("p"),
            install_dir: root.path().join("b"),
            tree: root.path().join("p/versions/0.9.0"),
            version: "0.9.0".into(),
        };
        let got = apply(&script, "v1.0.0", &at);
        assert_eq!(got.status, Some(0), "{:?}", got.output);
        let argv = std::fs::read_to_string(&seen).unwrap();
        let want = format!(
            "--version\nv1.0.0\n--prefix\n{}\n--install-dir\n{}\n",
            at.prefix.display(),
            at.install_dir.display()
        );
        assert_eq!(argv, want, "the installer is handed the tag and this installation's places");
        assert_eq!(got.installed.as_deref(), Some("1.0.0"));

        // an installer that fails reports its status and installs nothing it can read back
        std::fs::write(&script, "echo 'the digest does not match' >&2\nexit 10\n").unwrap();
        let failed = apply(&script, "v1.0.0", &at);
        assert_eq!(failed.status, Some(10));
        assert!(failed.installed.is_none());
        assert!(failed.output.iter().any(|l| l.contains("digest")));
    }
}
