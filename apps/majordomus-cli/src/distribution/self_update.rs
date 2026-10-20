//! `majordomus self update`: an installation moves itself to the stable release by running
//! the published installer again (ADR 0161).
//!
//! Nothing here downloads an archive, verifies a digest or swaps a launcher. The installer
//! does all three, once, and `test/cases/85_installer.sh` proves it does them safely. What
//! is decided here is whether to run it and where:
//!
//! - **whether**: forward only, stable only, and only to a version the release metadata
//!   lists. The stable pointer (`releases/latest.json`) names the version; the pinned
//!   metadata of that tag (`releases/<tag>.json`) must agree with it.
//! - **where**: the installation's own prefix and launcher directory, read from the layout
//!   the installer wrote (`<prefix>/versions/<version>/libexec/majordomus-cli`, and the
//!   launcher that `exec`s into that prefix), never from a default that might be another
//!   installation's.
//!
//! The release metadata is read from the installer's base URL in the distribution model, or
//! from `MAJORDOMUS_RELEASE_BASE_URL` — the same override, with the same rule, as the
//! installer's: a base that is not HTTPS is refused unless `MAJORDOMUS_INSECURE_BASE_URL=1`
//! says it is deliberate, which only the tests do.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use super::release::version_key;

/// The installer's own override of the release metadata's base URL.
pub(crate) const BASE_ENV: &str = "MAJORDOMUS_RELEASE_BASE_URL";

/// The installer's own permission to read that base over plain HTTP.
pub(crate) const INSECURE_ENV: &str = "MAJORDOMUS_INSECURE_BASE_URL";

/// The path every launcher the installer writes exports: the one that does not move when
/// the version does.
pub(crate) const LAUNCHER_ENV: &str = "MAJORDOMUS_LAUNCHER";

/// The tool's name: the launcher's file name and the first word of `version`.
const BINARY: &str = "majordomus";

/// One installation the installer wrote, as read from the running executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Installation {
    /// Where the versioned trees are: `--prefix` of the installer.
    pub(crate) prefix: PathBuf,
    /// Where the launchers are: `--install-dir` of the installer.
    pub(crate) install_dir: PathBuf,
    /// The launcher a person runs.
    pub(crate) launcher: PathBuf,
    /// The versioned tree this executable runs from.
    pub(crate) tree: PathBuf,
}

/// Read the installation the executable at `exe` belongs to.
///
/// `launcher` is the launcher the process was started through, when the environment names
/// one; otherwise `<home>/.local/bin/majordomus`, the installer's default, is tried. Either
/// way the launcher must `exec` into this executable's prefix, or it is another
/// installation's and is refused.
pub(crate) fn locate(
    exe: &Path,
    launcher: Option<&Path>,
    home: Option<&Path>,
) -> Result<Installation, String> {
    let exe = exe
        .canonicalize()
        .map_err(|e| format!("{}: {e}", exe.display()))?;
    let not_installed = || {
        format!(
            "{} is not an installed copy: an installation runs <prefix>/versions/<version>/libexec/{BINARY}-cli, and a checkout's build is upgraded by building it",
            exe.display()
        )
    };
    let libexec = exe.parent().ok_or_else(not_installed)?;
    let tree = libexec.parent().ok_or_else(not_installed)?;
    let versions = tree.parent().ok_or_else(not_installed)?;
    if libexec.file_name().and_then(|n| n.to_str()) != Some("libexec")
        || versions.file_name().and_then(|n| n.to_str()) != Some("versions")
    {
        return Err(not_installed());
    }
    let prefix = versions.parent().ok_or_else(not_installed)?.to_path_buf();
    let candidate = match (launcher, home) {
        (Some(l), _) => l.to_path_buf(),
        (None, Some(h)) => h.join(".local/bin").join(BINARY),
        (None, None) => {
            return Err(format!(
                "no launcher: {LAUNCHER_ENV} is not set and there is no home directory to find the installer's default in"
            ))
        }
    };
    let text = std::fs::read_to_string(&candidate)
        .map_err(|e| format!("the launcher {}: {e}", candidate.display()))?;
    let target = launcher_target(&text).ok_or_else(|| {
        format!(
            "{} is not a launcher the installer wrote: it has no `exec \"<tree>/bin/{BINARY}\"` line",
            candidate.display()
        )
    })?;
    let target = Path::new(&target)
        .canonicalize()
        .map_err(|e| format!("the launcher {} points at {target}: {e}", candidate.display()))?;
    if !target.starts_with(&prefix) {
        return Err(format!(
            "the launcher {} runs {}, which is not under this executable's prefix {}: it belongs to another installation",
            candidate.display(),
            target.display(),
            prefix.display()
        ));
    }
    let install_dir = candidate
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{} has no directory", candidate.display()))?;
    Ok(Installation {
        prefix,
        install_dir,
        launcher: candidate,
        tree: tree.to_path_buf(),
    })
}

/// The path a launcher `exec`s: the first quoted word of its `exec` line.
fn launcher_target(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("exec ")?.trim_start();
        let rest = rest.strip_prefix('"')?;
        rest.split_once('"').map(|(path, _)| path.to_string())
    })
}

/// The version a launcher reports: what the installer itself compares against.
pub(crate) fn installed_version(launcher: &Path) -> Result<String, String> {
    let out = Command::new(launcher)
        .arg("version")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{} could not run: {e}", launcher.display()))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{BINARY} ")))
        .map(|v| v.trim().to_string());
    match (out.status.success(), version) {
        (true, Some(v)) if !v.is_empty() => Ok(v),
        _ => Err(format!(
            "{} version answered no version (exit {:?}): {}",
            launcher.display(),
            out.status.code(),
            text.trim()
        )),
    }
}

/// Where the release metadata and the installer are read from, and whether that is HTTPS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Base {
    /// Without a trailing slash.
    pub(crate) url: String,
    /// False only when the override is plain HTTP and the override says so is deliberate.
    pub(crate) secure: bool,
}

/// The base: the override when it is set, the model's otherwise, under the installer's rule.
pub(crate) fn base(model_base: Option<&str>, env_base: Option<&str>, insecure: bool) -> Result<Base, String> {
    let url = match env_base.filter(|b| !b.is_empty()).or(model_base) {
        Some(u) => u.trim_end_matches('/').to_string(),
        None => {
            return Err(format!(
                "no release metadata base: the distribution model names none and {BASE_ENV} is not set"
            ))
        }
    };
    if url.starts_with("https://") {
        return Ok(Base { url, secure: true });
    }
    if url.starts_with("http://") && insecure {
        return Ok(Base { url, secure: false });
    }
    Err(format!(
        "the release metadata base {url} is not HTTPS; set {INSECURE_ENV}=1 as well if this is deliberate, which is meant for the installer's own tests"
    ))
}

/// What a release metadata document says about itself. The rest of the document is the
/// installer's to read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Pointer {
    /// The semantic version, without a leading `v`.
    pub(crate) version: String,
    /// The git tag.
    pub(crate) tag: String,
    /// `stable` or `prerelease`; absent in a document older than the field.
    #[serde(default)]
    pub(crate) channel: Option<String>,
}

impl Pointer {
    /// Parse a metadata document.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("not release metadata: {e}"))
    }

    fn is_stable(&self) -> bool {
        self.channel.as_deref().unwrap_or("stable") == "stable" && !self.version.contains('-')
    }
}

/// What `self update` decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Verdict {
    /// The installation runs the stable release.
    UpToDate,
    /// A newer stable release is published and listed: `--apply` installs it.
    Upgrade,
    /// Nothing may be installed, and the reason says why.
    Refused,
}

/// A verdict and its reason, in one sentence a person reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Decision {
    /// What was decided.
    pub(crate) verdict: Verdict,
    /// Why.
    pub(crate) reason: String,
}

/// Decide, from the installed version, the stable pointer and the metadata the pointer's
/// tag resolves to (`None` when that document could not be found): forward only, stable
/// only, and only what the metadata lists.
pub(crate) fn decide(installed: &str, stable: &Pointer, listed: Option<&Pointer>) -> Decision {
    let refused = |reason: String| Decision {
        verdict: Verdict::Refused,
        reason,
    };
    if stable.tag != format!("v{}", stable.version) {
        return refused(format!(
            "the stable pointer's tag {} and version {} do not agree; nothing is installed from metadata that contradicts itself",
            stable.tag, stable.version
        ));
    }
    if !stable.is_stable() {
        return refused(format!(
            "the stable pointer names {}, which is a pre-release; a pre-release is installed by a person with install.sh --version, never by self update",
            stable.tag
        ));
    }
    match listed {
        None => {
            return refused(format!(
                "releases/{}.json is not published: the stable pointer names a version the release metadata does not list",
                stable.tag
            ))
        }
        Some(l) if l.version != stable.version || l.tag != stable.tag => {
            return refused(format!(
                "releases/{}.json describes {}, not the {} the stable pointer names",
                stable.tag, l.tag, stable.tag
            ))
        }
        Some(l) if !l.is_stable() => {
            return refused(format!(
                "releases/{}.json is a pre-release; self update installs stable releases only",
                l.tag
            ))
        }
        Some(_) => {}
    }
    let (have, want) = (version_key(installed), version_key(&stable.version));
    if want < have {
        return refused(format!(
            "{installed} is installed and the stable release is {}: self update moves forward only, and going back is install.sh --version",
            stable.version
        ));
    }
    if want == have {
        return Decision {
            verdict: Verdict::UpToDate,
            reason: format!("{installed} is installed, and it is the stable release"),
        };
    }
    Decision {
        verdict: Verdict::Upgrade,
        reason: format!(
            "{installed} is installed and {} is the stable release",
            stable.version
        ),
    }
}

/// Read one URL into memory, or into `dest`, through `curl` — the downloader the installer
/// itself prefers — restricted to HTTPS, redirects included, unless the base is not.
pub(crate) fn fetch(url: &str, secure: bool, dest: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new("curl");
    cmd.args(["-sSL", "--fail", "--connect-timeout", "15", "--max-time", "120"]);
    if secure {
        cmd.args(["--proto", "=https", "--proto-redir", "=https"]);
    }
    if let Some(d) = dest {
        cmd.arg("-o").arg(d);
    }
    cmd.arg(url).stdin(Stdio::null());
    let out = cmd
        .output()
        .map_err(|e| format!("curl could not run ({e}); self update reads the release metadata with curl"))?;
    if !out.status.success() {
        return Err(format!(
            "{url}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run the installer at `script` for `tag` into this installation, and read back the
/// version its launcher then reports. The installer's own output is returned beside it,
/// for the report; a failure carries its last lines.
pub(crate) fn run_installer(
    script: &Path,
    tag: &str,
    installation: &Installation,
) -> Result<(String, String), String> {
    let out = Command::new("sh")
        .arg(script)
        .arg("--version")
        .arg(tag)
        .arg("--prefix")
        .arg(&installation.prefix)
        .arg("--install-dir")
        .arg(&installation.install_dir)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("sh could not run the installer: {e}"))?;
    let mut log = String::from_utf8_lossy(&out.stdout).into_owned();
    log.push_str(&String::from_utf8_lossy(&out.stderr));
    if !out.status.success() {
        let tail: Vec<&str> = log.lines().rev().take(12).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(format!(
            "the installer refused {tag} (exit {:?}); it replaces the launcher only after the archive is verified, so the installation that was running still is:\n{}",
            out.status.code(),
            tail.join("\n")
        ));
    }
    let now = installed_version(&installation.launcher)?;
    Ok((now, log))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: &str) -> Pointer {
        Pointer {
            version: v.into(),
            tag: format!("v{v}"),
            channel: Some("stable".into()),
        }
    }

    #[test]
    fn self_update_refuses_downgrade_test() {
        let d = decide("0.27.0", &p("0.26.0"), Some(&p("0.26.0")));
        assert_eq!(d.verdict, Verdict::Refused);
        assert!(d.reason.contains("forward only"), "{}", d.reason);
        // a pre-release installed by a person is above its own stable predecessor only
        assert_eq!(
            decide("0.27.0-rc.1", &p("0.27.0"), Some(&p("0.27.0"))).verdict,
            Verdict::Upgrade
        );
        assert_eq!(
            decide("0.27.0-rc.1", &p("0.26.0"), Some(&p("0.26.0"))).verdict,
            Verdict::Refused
        );
    }

    #[test]
    fn a_prerelease_or_an_unlisted_version_is_refused() {
        let mut pre = p("0.27.0-rc.1");
        assert_eq!(decide("0.26.0", &pre, Some(&pre)).verdict, Verdict::Refused);
        pre.version = "0.27.0".into();
        pre.tag = "v0.27.0".into();
        pre.channel = Some("prerelease".into());
        assert_eq!(decide("0.26.0", &pre, Some(&pre)).verdict, Verdict::Refused);
        assert_eq!(decide("0.26.0", &p("0.27.0"), None).verdict, Verdict::Refused);
        assert_eq!(
            decide("0.26.0", &p("0.27.0"), Some(&p("0.27.1"))).verdict,
            Verdict::Refused
        );
        let mut bad = p("0.27.0");
        bad.tag = "v0.28.0".into();
        assert_eq!(decide("0.26.0", &bad, Some(&bad)).verdict, Verdict::Refused);
    }

    #[test]
    fn versions_are_ordered_as_numbers_not_text() {
        assert_eq!(
            decide("0.9.0", &p("0.10.0"), Some(&p("0.10.0"))).verdict,
            Verdict::Upgrade
        );
    }

    #[test]
    fn the_base_follows_the_installers_rule() {
        let b = base(Some("https://majordomus.dev"), None, false).unwrap();
        assert!(b.secure);
        assert_eq!(b.url, "https://majordomus.dev");
        assert!(base(Some("https://x"), Some("http://127.0.0.1:1"), false).is_err());
        let b = base(Some("https://x"), Some("http://127.0.0.1:1/"), true).unwrap();
        assert!(!b.secure);
        assert_eq!(b.url, "http://127.0.0.1:1");
        assert!(base(None, None, false).is_err());
        assert!(base(Some("ftp://x"), None, true).is_err());
    }

    #[cfg(unix)]
    fn script(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// The installer is the one that downloads, verifies and swaps: here a stand-in records
    /// the arguments it was given and moves the launcher forward, the way the real one does
    /// once it has verified an archive; a failing one must leave the launcher as it was.
    #[cfg(unix)]
    #[test]
    fn self_update_wraps_installer_test() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let launcher = bin.join("majordomus");
        script(&launcher, "echo 'majordomus 0.25.0'");
        let installation = Installation {
            prefix: dir.path().join("prefix"),
            install_dir: bin.clone(),
            launcher: launcher.clone(),
            tree: dir.path().join("prefix/versions/0.25.0"),
        };
        assert_eq!(installed_version(&launcher).unwrap(), "0.25.0");

        let args = dir.path().join("args");
        let good = dir.path().join("install.sh");
        script(
            &good,
            &format!(
                "printf '%s\\n' \"$@\" > {args}\nprintf '#!/bin/sh\\necho \"majordomus 0.26.0\"\\n' > {l}\necho 'majordomus v0.26.0 installed successfully.'",
                args = args.display(),
                l = launcher.display()
            ),
        );
        let (now, log) = run_installer(&good, "v0.26.0", &installation).unwrap();
        assert_eq!(now, "0.26.0");
        assert!(log.contains("installed successfully"), "{log}");
        let given = std::fs::read_to_string(&args).unwrap();
        let given: Vec<&str> = given.lines().collect();
        assert_eq!(
            given,
            [
                "--version",
                "v0.26.0",
                "--prefix",
                &installation.prefix.display().to_string(),
                "--install-dir",
                &bin.display().to_string(),
            ]
        );

        script(&launcher, "echo 'majordomus 0.25.0'");
        let bad = dir.path().join("bad.sh");
        script(&bad, "echo 'the downloaded archive does not match its digest' >&2; exit 1");
        let e = run_installer(&bad, "v0.26.0", &installation).unwrap_err();
        assert!(e.contains("does not match its digest"), "{e}");
        assert!(e.contains("still is"), "{e}");
        assert_eq!(installed_version(&launcher).unwrap(), "0.25.0");

        script(&launcher, "echo nothing; exit 3");
        assert!(installed_version(&launcher).is_err());
    }

    #[test]
    fn metadata_is_read_through_curl() {
        if Command::new("curl").arg("--version").output().is_err() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("latest.json");
        std::fs::write(
            &doc,
            r#"{ "schema": "release/v1", "version": "0.26.0", "tag": "v0.26.0", "channel": "stable", "artifacts": {} }"#,
        )
        .unwrap();
        let url = format!("file://{}", doc.display());
        let text = fetch(&url, false, None).unwrap();
        assert_eq!(Pointer::parse(&text).unwrap(), p("0.26.0"));
        // a secure fetch refuses anything that is not HTTPS
        assert!(fetch(&url, true, None).is_err());
        let copy = dir.path().join("copy.json");
        fetch(&url, false, Some(&copy)).unwrap();
        assert!(copy.is_file());
        assert!(Pointer::parse("not json").is_err());
    }

    #[test]
    fn an_installation_is_read_from_the_layout_the_installer_wrote() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("share/majordomus");
        let tree = prefix.join("versions/0.25.0");
        std::fs::create_dir_all(tree.join("libexec")).unwrap();
        std::fs::create_dir_all(tree.join("bin")).unwrap();
        let exe = tree.join("libexec/majordomus-cli");
        std::fs::write(&exe, "").unwrap();
        std::fs::write(tree.join("bin/majordomus"), "").unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let launcher = bin.join("majordomus");
        std::fs::write(
            &launcher,
            format!(
                "#!/bin/sh\nMAJORDOMUS_LAUNCHER=\"{}\"; export MAJORDOMUS_LAUNCHER\nexec \"{}/bin/majordomus\" \"$@\"\n",
                launcher.display(),
                tree.display()
            ),
        )
        .unwrap();
        let i = locate(&exe, Some(&launcher), None).unwrap();
        assert_eq!(i.prefix, prefix.canonicalize().unwrap());
        assert_eq!(i.install_dir, bin);
        assert_eq!(i.tree, tree.canonicalize().unwrap());

        // a launcher of another installation is refused
        let other = dir.path().join("other/versions/0.1.0/bin");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("majordomus"), "").unwrap();
        std::fs::write(
            &launcher,
            format!("#!/bin/sh\nexec \"{}/majordomus\" \"$@\"\n", other.display()),
        )
        .unwrap();
        let e = locate(&exe, Some(&launcher), None).unwrap_err();
        assert!(e.contains("another installation"), "{e}");

        // a checkout's build is not an installed copy
        let build = dir.path().join("target/debug/majordomus");
        std::fs::create_dir_all(build.parent().unwrap()).unwrap();
        std::fs::write(&build, "").unwrap();
        let e = locate(&build, Some(&launcher), None).unwrap_err();
        assert!(e.contains("not an installed copy"), "{e}");
    }
}
