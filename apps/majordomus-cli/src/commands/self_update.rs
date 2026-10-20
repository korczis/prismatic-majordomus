//! `majordomus self update`: report, and on `--apply` install, the stable release this
//! installation should run (ADR 0161). The deciding is
//! `crate::distribution::self_update`'s; the installing is the published installer's; this
//! file reads the environment, asks both, and says what happened.
//!
//! Exit codes: 0 when a decision was reached (up to date, an upgrade is available, or it was
//! applied), 10 when the update is refused (a downgrade, a pre-release, a version the metadata
//! does not list), 12 when nothing could be decided or installed (not an installed copy, no
//! metadata, the installer failed — and then the installation that ran still does).

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{OutputFormat, SelfArgs, SelfCommand};
use crate::distribution::self_update::{
    self as su, Base, Decision, Installation, Pointer, Verdict,
};
use crate::distribution::Model;
use crate::error::{Error, Result};
use crate::share::Share;

/// The exit code when an update is refused.
pub const EXIT_REFUSED: u8 = 10;

/// The exit code when nothing could be decided or installed.
pub const EXIT_UNDECIDED: u8 = 12;

/// Run `majordomus self`.
pub fn run(args: SelfArgs) -> Result<u8> {
    match args.command {
        SelfCommand::Update { apply, .. } => {
            let inputs = Inputs::from_process();
            let (report, code) = update(&inputs, apply, &su::fetch, &su::run_installer);
            let text = match args.format {
                OutputFormat::Json => serde_json::to_string_pretty(&report)
                    .map_err(|e| Error::Protocol { reason: e.to_string() })?,
                OutputFormat::Text => render(&report),
            };
            let stdout = std::io::stdout();
            writeln!(stdout.lock(), "{}", text.trim_end()).map_err(Error::Transport)?;
            Ok(code)
        }
    }
}

/// What the process was started with: everything `update` reads from outside itself.
#[derive(Debug, Clone, Default)]
pub(crate) struct Inputs {
    /// The running executable.
    pub(crate) exe: Option<PathBuf>,
    /// `MAJORDOMUS_LAUNCHER`.
    pub(crate) launcher: Option<PathBuf>,
    /// `HOME`.
    pub(crate) home: Option<PathBuf>,
    /// The installer's base URL and file name, from the distribution model.
    pub(crate) model: Option<(String, String)>,
    /// `MAJORDOMUS_RELEASE_BASE_URL`.
    pub(crate) env_base: Option<String>,
    /// `MAJORDOMUS_INSECURE_BASE_URL=1`.
    pub(crate) insecure: bool,
    /// Where a downloaded installer is kept while it runs.
    pub(crate) scratch: PathBuf,
}

impl Inputs {
    fn from_process() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let model = Share::locate(None, &cwd)
            .ok()
            .and_then(|s| Model::locate(&s).ok().flatten())
            .map(|m| (m.installer.base_url, m.installer.script));
        Inputs {
            exe: std::env::current_exe().ok(),
            launcher: var(su::LAUNCHER_ENV).map(PathBuf::from),
            home: var("HOME").map(PathBuf::from),
            model,
            env_base: var(su::BASE_ENV),
            insecure: var(su::INSECURE_ENV).as_deref() == Some("1"),
            scratch: std::env::temp_dir()
                .join(format!("majordomus-self-update-{}", std::process::id())),
        }
    }
}

/// What `self update` found and did: the document `--format json` prints.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct Report {
    /// `check` or `apply`.
    pub(crate) mode: &'static str,
    /// The installation, when the executable is an installed copy.
    pub(crate) installation: Option<Installation>,
    /// The version its launcher reports.
    pub(crate) installed: Option<String>,
    /// Where the metadata was read from.
    pub(crate) base: Option<Base>,
    /// What the stable pointer names.
    pub(crate) stable: Option<Pointer>,
    /// What was decided.
    pub(crate) decision: Option<Decision>,
    /// The version the launcher reports after `--apply` installed it.
    pub(crate) applied: Option<String>,
    /// Why nothing could be decided or installed.
    pub(crate) error: Option<String>,
}

type Fetch<'a> = &'a dyn Fn(&str, bool, Option<&Path>) -> std::result::Result<String, String>;
type Install<'a> =
    &'a dyn Fn(&Path, &str, &Installation) -> std::result::Result<(String, String), String>;

/// Decide, and on `apply` install. Every effect goes through `fetch` and `install`, which
/// the command passes as the real ones and a test as stand-ins.
pub(crate) fn update(inputs: &Inputs, apply: bool, fetch: Fetch<'_>, install: Install<'_>) -> (Report, u8) {
    let mut report = Report {
        mode: if apply { "apply" } else { "check" },
        ..Report::default()
    };
    let fail = |mut r: Report, e: String| {
        r.error = Some(e);
        (r, EXIT_UNDECIDED)
    };
    let Some(exe) = inputs.exe.as_deref() else {
        return fail(report, "the running executable's path cannot be read".into());
    };
    let installation = match su::locate(exe, inputs.launcher.as_deref(), inputs.home.as_deref()) {
        Ok(i) => i,
        Err(e) => return fail(report, e),
    };
    report.installation = Some(installation.clone());
    let installed = match su::installed_version(&installation.launcher) {
        Ok(v) => v,
        Err(e) => return fail(report, e),
    };
    report.installed = Some(installed.clone());
    let (model_base, script) = match &inputs.model {
        Some((b, s)) => (Some(b.as_str()), s.as_str()),
        None => (None, "install.sh"),
    };
    let base = match su::base(model_base, inputs.env_base.as_deref(), inputs.insecure) {
        Ok(b) => b,
        Err(e) => return fail(report, e),
    };
    report.base = Some(base.clone());
    let stable = match fetch(&format!("{}/releases/latest.json", base.url), base.secure, None)
        .and_then(|t| Pointer::parse(&t))
    {
        Ok(p) => p,
        Err(e) => return fail(report, format!("the stable pointer could not be read: {e}")),
    };
    report.stable = Some(stable.clone());
    let listed = fetch(
        &format!("{}/releases/{}.json", base.url, stable.tag),
        base.secure,
        None,
    )
    .ok()
    .and_then(|t| Pointer::parse(&t).ok());
    let decision = su::decide(&installed, &stable, listed.as_ref());
    let verdict = decision.verdict;
    report.decision = Some(decision);
    match verdict {
        Verdict::Refused => return (report, EXIT_REFUSED),
        Verdict::UpToDate => return (report, 0),
        Verdict::Upgrade if !apply => return (report, 0),
        Verdict::Upgrade => {}
    }
    if let Err(e) = std::fs::create_dir_all(&inputs.scratch) {
        return fail(report, format!("{}: {e}", inputs.scratch.display()));
    }
    let local = inputs.scratch.join(script);
    let result = fetch(&format!("{}/{script}", base.url), base.secure, Some(&local))
        .map_err(|e| format!("the installer could not be downloaded: {e}"))
        .and_then(|_| install(&local, &stable.tag, &installation));
    let _ = std::fs::remove_dir_all(&inputs.scratch);
    match result {
        Ok((now, _)) if now == stable.version => {
            report.applied = Some(now);
            (report, 0)
        }
        Ok((now, _)) => fail(
            report,
            format!(
                "the installer finished and the launcher reports {now}, not {}",
                stable.version
            ),
        ),
        Err(e) => fail(report, e),
    }
}

/// The report, for a person.
pub(crate) fn render(r: &Report) -> String {
    let mut out = Vec::new();
    if let (Some(i), Some(v)) = (&r.installation, &r.installed) {
        out.push(format!(
            "installed  {v}  ({} -> {})",
            i.launcher.display(),
            i.tree.display()
        ));
    }
    if let (Some(s), Some(b)) = (&r.stable, &r.base) {
        out.push(format!("stable     {}  ({}/releases/latest.json)", s.version, b.url));
    }
    if let Some(d) = &r.decision {
        let word = match d.verdict {
            Verdict::UpToDate => "up to date",
            Verdict::Upgrade => "upgrade",
            Verdict::Refused => "refused",
        };
        out.push(format!("verdict    {word}: {}", d.reason));
        if d.verdict == Verdict::Upgrade && r.mode == "check" {
            out.push("next       majordomus self update --apply".into());
        }
    }
    if let Some(v) = &r.applied {
        out.push(format!("applied    {v} is installed, through the published installer"));
    }
    if let Some(e) = &r.error {
        out.push(format!("error      {e}"));
    }
    out.join("\n")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn layout(dir: &Path, version: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let tree = dir.join(format!("prefix/versions/{version}"));
        std::fs::create_dir_all(tree.join("libexec")).unwrap();
        std::fs::create_dir_all(tree.join("bin")).unwrap();
        let exe = tree.join("libexec/majordomus-cli");
        std::fs::write(&exe, "").unwrap();
        std::fs::write(tree.join("bin/majordomus"), "").unwrap();
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let launcher = bin.join("majordomus");
        std::fs::write(
            &launcher,
            format!(
                "#!/bin/sh\n[ \"$1\" = version ] && {{ echo 'majordomus {version}'; exit 0; }}\nexec \"{}/bin/majordomus\" \"$@\"\n",
                tree.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
        (exe, launcher)
    }

    fn inputs(dir: &Path, exe: PathBuf, launcher: PathBuf) -> Inputs {
        Inputs {
            exe: Some(exe),
            launcher: Some(launcher),
            home: None,
            model: Some(("https://example.invalid".into(), "install.sh".into())),
            env_base: None,
            insecure: false,
            scratch: dir.join("scratch"),
        }
    }

    fn metadata(latest: &'static str) -> impl Fn(&str, bool, Option<&Path>) -> std::result::Result<String, String> {
        move |url, secure, dest| {
            assert!(secure, "{url} was asked for without HTTPS");
            if let Some(d) = dest {
                std::fs::write(d, "#!/bin/sh\n").unwrap();
                return Ok(String::new());
            }
            let v = if url.ends_with("/latest.json") || url.ends_with(&format!("/v{latest}.json")) {
                latest
            } else {
                return Err(format!("{url}: 404"));
            };
            Ok(format!(r#"{{ "version": "{v}", "tag": "v{v}", "channel": "stable" }}"#))
        }
    }

    #[test]
    fn a_check_reports_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (exe, launcher) = layout(dir.path(), "0.25.0");
        let never = |_: &Path, _: &str, _: &Installation| -> std::result::Result<(String, String), String> {
            panic!("a check ran the installer")
        };
        let (r, code) = update(&inputs(dir.path(), exe, launcher), false, &metadata("0.26.0"), &never);
        assert_eq!(code, 0, "{r:?}");
        assert_eq!(r.decision.as_ref().unwrap().verdict, Verdict::Upgrade);
        assert!(render(&r).contains("next       majordomus self update --apply"));
        assert!(!dir.path().join("scratch").exists());
    }

    #[test]
    fn an_apply_runs_the_installer_and_reads_the_launcher_back() {
        let dir = tempfile::tempdir().unwrap();
        let (exe, launcher) = layout(dir.path(), "0.25.0");
        let ran = |script: &Path, tag: &str, _: &Installation| -> std::result::Result<(String, String), String> {
            assert!(script.ends_with("install.sh"));
            assert_eq!(tag, "v0.26.0");
            Ok(("0.26.0".into(), "installed successfully".into()))
        };
        let (r, code) = update(&inputs(dir.path(), exe.clone(), launcher.clone()), true, &metadata("0.26.0"), &ran);
        assert_eq!(code, 0, "{r:?}");
        assert_eq!(r.applied.as_deref(), Some("0.26.0"));
        assert!(render(&r).contains("applied    0.26.0"));
        assert!(!dir.path().join("scratch").exists(), "the downloaded installer was left behind");

        let wrong = |_: &Path, _: &str, _: &Installation| -> std::result::Result<(String, String), String> {
            Ok(("0.25.0".into(), String::new()))
        };
        let (r, code) = update(&inputs(dir.path(), exe.clone(), launcher.clone()), true, &metadata("0.26.0"), &wrong);
        assert_eq!(code, EXIT_UNDECIDED);
        assert!(r.error.unwrap().contains("not 0.26.0"));

        let failed = |_: &Path, _: &str, _: &Installation| -> std::result::Result<(String, String), String> {
            Err("the installer refused v0.26.0".into())
        };
        let (r, code) = update(&inputs(dir.path(), exe, launcher), true, &metadata("0.26.0"), &failed);
        assert_eq!(code, EXIT_UNDECIDED);
        assert!(render(&r).contains("error      the installer refused"));
    }

    #[test]
    fn a_downgrade_is_refused_and_installs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (exe, launcher) = layout(dir.path(), "0.27.0");
        let never = |_: &Path, _: &str, _: &Installation| -> std::result::Result<(String, String), String> {
            panic!("a refused update ran the installer")
        };
        let (r, code) = update(&inputs(dir.path(), exe, launcher), true, &metadata("0.26.0"), &never);
        assert_eq!(code, EXIT_REFUSED);
        assert!(render(&r).contains("refused"));
    }

    #[test]
    fn what_cannot_be_decided_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let (exe, launcher) = layout(dir.path(), "0.26.0");
        let never = |_: &Path, _: &str, _: &Installation| -> std::result::Result<(String, String), String> {
            panic!("ran the installer")
        };
        let mut i = inputs(dir.path(), exe.clone(), launcher.clone());
        i.exe = None;
        assert_eq!(update(&i, false, &metadata("0.26.0"), &never).1, EXIT_UNDECIDED);

        let mut i = inputs(dir.path(), exe.clone(), launcher.clone());
        i.model = None;
        let (r, code) = update(&i, false, &metadata("0.26.0"), &never);
        assert_eq!(code, EXIT_UNDECIDED);
        assert!(r.error.unwrap().contains("no release metadata base"));

        let offline = |url: &str, _: bool, _: Option<&Path>| -> std::result::Result<String, String> {
            Err(format!("{url}: could not resolve host"))
        };
        let (r, code) = update(&inputs(dir.path(), exe.clone(), launcher), false, &offline, &never);
        assert_eq!(code, EXIT_UNDECIDED);
        assert!(render(&r).contains("the stable pointer could not be read"));

        let build = dir.path().join("target/debug/majordomus");
        std::fs::create_dir_all(build.parent().unwrap()).unwrap();
        std::fs::write(&build, "").unwrap();
        let mut i = inputs(dir.path(), build, exe);
        i.launcher = None;
        let (r, code) = update(&i, false, &metadata("0.26.0"), &never);
        assert_eq!(code, EXIT_UNDECIDED);
        assert!(r.error.unwrap().contains("not an installed copy"));
    }
}
