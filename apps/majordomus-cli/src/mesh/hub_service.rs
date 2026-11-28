//! The hub as a service of its host (ADR 0199, I4816): one writer renders the hub's launchd
//! agent on macOS or its systemd user unit on Linux, beside a timer that brings the hub to
//! the published release every hour; one reader says whether both are written as rendered,
//! loaded, running, answering, linked and current; and one updater installs a newer release,
//! moves the hub's checkout to its tag, restarts the hub and puts the previous release back
//! when the restarted hub does not answer.
//!
//! Until this module every hub unit was a file somebody wrote by hand (`docs/MESH.md`, The
//! hubs as services), and the files drifted from the hubs they were meant to keep: on
//! 2026-11-05 this MacBook's agent pointed at a deleted checkout and the hub was dead for
//! eleven days; on 2026-11-16 lundra's unit was disabled while a hub started from a shell
//! served instead; and on 2026-11-28 every hub served 1.2.0 a day after 1.3.0 was published,
//! because no machine carried the update timer the owner had ordered on 2026-11-22.
//!
//! Pure where it can be: [`render`] is a function of a [`Spec`], [`decide`] of the versions,
//! and every verdict of a service manager's output is a function of that text. The host is
//! reached only through a [`Host`], whose closures a test replaces, so every branch here runs
//! in-process without a service manager, a network or a release.
//!
//! What this is not: a supervisor. The service manager restarts the hub; the mesh's own
//! rendezvous and link loops re-register and redial after a restart or an outage
//! (`rendezvous.rs`, `cooperation.rs`). This module makes sure there is a hub for them to
//! keep, at the release the mesh speaks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The launchd label of the hub.
pub(crate) const HUB_LABEL: &str = "dev.majordomus.hub";
/// The launchd label of the hub's update agent.
pub(crate) const UPDATE_LABEL: &str = "dev.majordomus.hub-update";
/// The systemd user unit of the hub.
pub(crate) const HUB_UNIT: &str = "majordomus-hub.service";
/// The systemd user unit the update timer starts.
pub(crate) const UPDATE_UNIT: &str = "majordomus-hub-update.service";
/// The systemd user timer of the update.
pub(crate) const UPDATE_TIMER: &str = "majordomus-hub-update.timer";
/// The hub port when the declaration names none for this machine.
pub(crate) const DEFAULT_PORT: u16 = 8791;
/// How often the update asks for a newer release.
pub(crate) const UPDATE_INTERVAL_SECONDS: u64 = 3600;
/// Where the published release and the installer are read.
pub(crate) const RELEASE_BASE: &str = "https://majordomus.dev";
/// How long a restarted hub has to answer with the version it was restarted onto.
const ANSWER_WITHIN: Duration = Duration::from_secs(90);
/// How often a restarted hub is asked within [`ANSWER_WITHIN`].
const ASK_EVERY: Duration = Duration::from_secs(3);
/// The first line of every file this module writes: what distinguishes it from a hand-made one.
const WRITTEN_BY: &str = "written by majordomus mesh hub install (I4816); rewritten by it, never by hand";

/// The service manager of a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Manager {
    /// launchd, per user (macOS): `~/Library/LaunchAgents`.
    Launchd,
    /// systemd, per user (Linux): `~/.config/systemd/user`.
    SystemdUser,
    /// Neither: no hub service can be written here.
    Unsupported,
}

impl Manager {
    /// The manager of the platform `os` names (`std::env::consts::OS`).
    pub(crate) fn of(os: &str) -> Self {
        match os {
            "macos" => Manager::Launchd,
            "linux" => Manager::SystemdUser,
            _ => Manager::Unsupported,
        }
    }
}

/// What one file is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnitRole {
    /// The hub itself, kept running.
    Hub,
    /// The update, run by its schedule.
    Update,
    /// The schedule of the update (systemd only: launchd keeps it in the update's agent).
    UpdateTimer,
}

/// Everything a hub service is rendered from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spec {
    /// The service manager.
    pub(crate) manager: Manager,
    /// The user's home: every path is under it.
    pub(crate) home: PathBuf,
    /// The executable the units run: the installer's launcher, whose path does not move.
    pub(crate) executable: PathBuf,
    /// The hub's dedicated checkout.
    pub(crate) repo: PathBuf,
    /// The port the hub listens on, on every address.
    pub(crate) port: u16,
}

/// One rendered file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rendered {
    /// What it is for.
    pub(crate) role: UnitRole,
    /// The service manager's name of it: a launchd label or a systemd unit.
    pub(crate) name: &'static str,
    /// Where it is written.
    pub(crate) path: PathBuf,
    /// What it holds.
    pub(crate) content: String,
}

impl Spec {
    /// The argv the hub runs: a serve of the dedicated checkout on every address, never idle.
    pub(crate) fn hub_argv(&self) -> Vec<String> {
        vec![
            self.executable.display().to_string(),
            "serve".into(),
            "--repo".into(),
            self.repo.display().to_string(),
            "--host".into(),
            "0.0.0.0".into(),
            "--port".into(),
            self.port.to_string(),
            "--idle".into(),
            "0".into(),
        ]
    }

    /// The argv the update runs.
    pub(crate) fn update_argv(&self) -> Vec<String> {
        vec![
            self.executable.display().to_string(),
            "mesh".into(),
            "hub".into(),
            "update".into(),
            "--apply".into(),
            "--port".into(),
            self.port.to_string(),
            "--repo".into(),
            self.repo.display().to_string(),
        ]
    }

    /// The PATH the units run with: the launcher's directory first, then where git, curl
    /// and gh live on the platform, because a service manager hands its jobs a bare PATH.
    pub(crate) fn path_env(&self) -> String {
        let local = self.home.join(".local/bin").display().to_string();
        match self.manager {
            Manager::Launchd => {
                format!("{local}:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin")
            }
            _ => format!("{local}:/usr/local/bin:/usr/bin:/bin"),
        }
    }

    /// The log of a launchd job.
    pub(crate) fn log(&self, role: UnitRole) -> PathBuf {
        let name = match role {
            UnitRole::Hub => "majordomus-hub.log",
            _ => "majordomus-hub-update.log",
        };
        self.home.join("Library/Logs").join(name)
    }
}

/// Where the update records its runs.
pub(crate) fn state_path(home: &Path) -> PathBuf {
    home.join(".local/state/majordomus/hub-update.json")
}

/// The files a spec renders: the hub and its update under launchd, the hub, the update and
/// its timer under systemd, nothing where neither runs.
pub(crate) fn render(spec: &Spec) -> Vec<Rendered> {
    match spec.manager {
        Manager::Launchd => {
            let dir = spec.home.join("Library/LaunchAgents");
            vec![
                Rendered {
                    role: UnitRole::Hub,
                    name: HUB_LABEL,
                    path: dir.join(format!("{HUB_LABEL}.plist")),
                    content: plist(spec, UnitRole::Hub),
                },
                Rendered {
                    role: UnitRole::Update,
                    name: UPDATE_LABEL,
                    path: dir.join(format!("{UPDATE_LABEL}.plist")),
                    content: plist(spec, UnitRole::Update),
                },
            ]
        }
        Manager::SystemdUser => {
            let dir = spec.home.join(".config/systemd/user");
            vec![
                Rendered {
                    role: UnitRole::Hub,
                    name: HUB_UNIT,
                    path: dir.join(HUB_UNIT),
                    content: systemd_hub(spec),
                },
                Rendered {
                    role: UnitRole::Update,
                    name: UPDATE_UNIT,
                    path: dir.join(UPDATE_UNIT),
                    content: systemd_update(spec),
                },
                Rendered {
                    role: UnitRole::UpdateTimer,
                    name: UPDATE_TIMER,
                    path: dir.join(UPDATE_TIMER),
                    content: systemd_timer(),
                },
            ]
        }
        Manager::Unsupported => Vec::new(),
    }
}

fn xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn plist(spec: &Spec, role: UnitRole) -> String {
    let (label, argv) = match role {
        UnitRole::Hub => (HUB_LABEL, spec.hub_argv()),
        _ => (UPDATE_LABEL, spec.update_argv()),
    };
    let args: String = argv
        .iter()
        .map(|a| format!("    <string>{}</string>\n", xml(a)))
        .collect();
    let log = xml(&spec.log(role).display().to_string());
    // The hub is kept alive whatever it exits with: a hub that stops is a mesh that shrinks.
    // The update runs at load (a boot or a login) and then every interval.
    let schedule = match role {
        UnitRole::Hub => "  <key>RunAtLoad</key>\n  <true/>\n  <key>KeepAlive</key>\n  <true/>\n  \
             <key>ThrottleInterval</key>\n  <integer>30</integer>\n"
            .to_string(),
        _ => format!(
            "  <key>RunAtLoad</key>\n  <true/>\n  <key>StartInterval</key>\n  \
             <integer>{UPDATE_INTERVAL_SECONDS}</integer>\n  \
             <key>ProcessType</key>\n  <string>Background</string>\n"
        ),
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <!-- {WRITTEN_BY} -->\n\
         <plist version=\"1.0\">\n<dict>\n\
         \x20 <key>Label</key>\n  <string>{label}</string>\n\
         \x20 <key>ProgramArguments</key>\n  <array>\n{args}  </array>\n\
         \x20 <key>WorkingDirectory</key>\n  <string>{repo}</string>\n\
         \x20 <key>EnvironmentVariables</key>\n  <dict>\n    <key>PATH</key>\n    \
         <string>{path}</string>\n  </dict>\n\
         {schedule}\
         \x20 <key>StandardInPath</key>\n  <string>/dev/null</string>\n\
         \x20 <key>StandardOutPath</key>\n  <string>{log}</string>\n\
         \x20 <key>StandardErrorPath</key>\n  <string>{log}</string>\n\
         </dict>\n</plist>\n",
        repo = xml(&spec.repo.display().to_string()),
        path = xml(&spec.path_env()),
    )
}

/// One argument as systemd's ExecStart reads it: quoted when it holds a space or a quote,
/// and `%` and `$` doubled, which systemd would otherwise expand.
fn systemd_word(word: &str) -> String {
    let escaped = word.replace('%', "%%").replace('$', "$$");
    if escaped.is_empty() || escaped.contains([' ', '"', '\\', '\t']) {
        format!("\"{}\"", escaped.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        escaped
    }
}

/// A value systemd reads as is, apart from its `%` specifiers: a path or a description.
fn systemd_value(text: &str) -> String {
    text.replace('%', "%%")
}

/// The PATH assignment of `Environment=`: quoted, which systemd unquotes, with `%` doubled;
/// `$` is not expanded there and stays as it is.
fn systemd_environment(spec: &Spec) -> String {
    format!("\"PATH={}\"", systemd_value(&spec.path_env()))
}

fn exec_line(argv: &[String]) -> String {
    argv.iter().map(|a| systemd_word(a)).collect::<Vec<_>>().join(" ")
}

fn systemd_hub(spec: &Spec) -> String {
    // StartLimitIntervalSec=0: systemd never gives up on the hub, however often it exits.
    format!(
        "# {WRITTEN_BY}\n\
         [Unit]\n\
         Description=Majordomus mesh hub of {repo}\n\
         StartLimitIntervalSec=0\n\
         \n\
         [Service]\n\
         Type=simple\n\
         WorkingDirectory={repo_word}\n\
         Environment={path}\n\
         ExecStart={exec}\n\
         Restart=always\n\
         RestartSec=10\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        repo = systemd_value(&spec.repo.display().to_string()),
        repo_word = systemd_value(&spec.repo.display().to_string()),
        path = systemd_environment(spec),
        exec = exec_line(&spec.hub_argv()),
    )
}

fn systemd_update(spec: &Spec) -> String {
    format!(
        "# {WRITTEN_BY}\n\
         [Unit]\n\
         Description=Bring the Majordomus hub of {repo} to the published release\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         WorkingDirectory={repo_word}\n\
         Environment={path}\n\
         ExecStart={exec}\n",
        repo = systemd_value(&spec.repo.display().to_string()),
        repo_word = systemd_value(&spec.repo.display().to_string()),
        path = systemd_environment(spec),
        exec = exec_line(&spec.update_argv()),
    )
}

fn systemd_timer() -> String {
    format!(
        "# {WRITTEN_BY}\n\
         [Unit]\n\
         Description=Ask for a newer Majordomus release every hour\n\
         \n\
         [Timer]\n\
         OnBootSec=5min\n\
         OnCalendar=hourly\n\
         RandomizedDelaySec=5min\n\
         Persistent=true\n\
         Unit={UPDATE_UNIT}\n\
         \n\
         [Install]\n\
         WantedBy=timers.target\n"
    )
}

/// The host, as this module reaches it. Every effect goes through a closure, so a test
/// replaces the host whole.
pub(crate) struct Host<'a> {
    /// The service manager.
    pub(crate) manager: Manager,
    /// The user's numeric id: launchd's `gui/<uid>` domain.
    pub(crate) uid: u32,
    /// The user's name: systemd's lingering is asked by it.
    pub(crate) user: String,
    /// This process's id: what the update's lock names.
    pub(crate) pid: u32,
    /// Files only: a staging root, where no command runs and no hub is asked.
    pub(crate) staging: bool,
    /// Run one argv to its end: whether it succeeded, and what it said.
    pub(crate) exec: &'a dyn Fn(&[String]) -> (bool, String),
    /// GET one path of a base URL as JSON: the body, or nothing when there is no answer.
    pub(crate) get: &'a dyn Fn(&str, &str) -> Option<String>,
    /// Whether a process of this id lives.
    pub(crate) alive: &'a dyn Fn(u32) -> bool,
    /// Seconds since the epoch.
    pub(crate) now: &'a dyn Fn() -> u64,
    /// Wait.
    pub(crate) sleep: &'a dyn Fn(Duration),
}

fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

/// The first line of the launcher the installer writes: what tells it from any other
/// executable.
const LAUNCHER_MARK: &str = "Generated by the Majordomus installer";

/// Whether `path` is the installer's launcher: the one executable the installer keeps
/// pointing at the newest release, and so the one a unit that must follow the release runs.
pub(crate) fn is_launcher(path: &Path) -> bool {
    std::fs::read(path)
        .ok()
        .filter(|b| b.len() < 64 * 1024)
        .is_some_and(|b| String::from_utf8_lossy(&b).contains(LAUNCHER_MARK))
}

/// A file against what is rendered for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FileState {
    /// Not there.
    Missing,
    /// There, with other content: a hand-made unit, or one an older release rendered.
    Drifted,
    /// There, as rendered.
    Current,
}

/// One unit file, as found.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct UnitView {
    /// What it is for.
    pub(crate) role: UnitRole,
    /// The service manager's name of it.
    pub(crate) name: String,
    /// Where it is.
    pub(crate) path: String,
    /// Against what is rendered.
    pub(crate) state: FileState,
    /// Whether the manager has it: loaded (launchd), or enabled or static (systemd).
    pub(crate) loaded: bool,
    /// Whether the manager may start it: not disabled (launchd), enabled or static (systemd).
    pub(crate) enabled: bool,
    /// Whether it runs now (the hub) or is scheduled (the timer).
    pub(crate) active: bool,
}

fn file_state(path: &Path, content: &str) -> FileState {
    match std::fs::read_to_string(path) {
        Ok(text) if text == content => FileState::Current,
        Ok(_) => FileState::Drifted,
        Err(_) => FileState::Missing,
    }
}

/// Whether `launchctl print` says the job runs.
pub(crate) fn launchd_running(print: &str) -> bool {
    print.lines().any(|l| l.trim() == "state = running")
}

/// Whether `launchctl print-disabled` lists `label` as disabled: `"label" => disabled` on
/// current macOS, `"label" => true` on older ones.
pub(crate) fn launchd_disabled(print_disabled: &str, label: &str) -> bool {
    let quoted = format!("\"{label}\"");
    print_disabled.lines().any(|l| {
        let l = l.trim();
        l.starts_with(&quoted) && (l.ends_with("=> disabled") || l.ends_with("=> true"))
    })
}

/// Whether `systemctl is-enabled` answers a unit the manager will start: an enabled one, or
/// a static one, which has no `[Install]` because its timer starts it.
pub(crate) fn systemd_enabled(answer: &str) -> bool {
    matches!(answer.trim(), "enabled" | "enabled-runtime" | "static" | "linked")
}

fn views(host: &Host<'_>, rendered: &[Rendered]) -> Vec<UnitView> {
    let disabled = match (host.staging, host.manager) {
        (false, Manager::Launchd) => {
            (host.exec)(&argv(&["launchctl", "print-disabled", &format!("gui/{}", host.uid)])).1
        }
        _ => String::new(),
    };
    rendered
        .iter()
        .map(|r| {
            let (loaded, enabled, active) = if host.staging {
                (false, false, false)
            } else {
                match host.manager {
                    Manager::Launchd => {
                        let (ok, out) = (host.exec)(&argv(&[
                            "launchctl",
                            "print",
                            &format!("gui/{}/{}", host.uid, r.name),
                        ]));
                        (ok, !launchd_disabled(&disabled, r.name), ok && launchd_running(&out))
                    }
                    Manager::SystemdUser => {
                        let (_, e) =
                            (host.exec)(&argv(&["systemctl", "--user", "is-enabled", r.name]));
                        let (_, a) =
                            (host.exec)(&argv(&["systemctl", "--user", "is-active", r.name]));
                        let on = systemd_enabled(&e);
                        (on, on, a.trim() == "active")
                    }
                    Manager::Unsupported => (false, false, false),
                }
            };
            UnitView {
                role: r.role,
                name: r.name.to_string(),
                path: r.path.display().to_string(),
                state: file_state(&r.path, &r.content),
                loaded,
                enabled,
                active,
            }
        })
        .collect()
}

/// One step a plan owes, and what came of it when it was carried out.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct Step {
    /// What the step does, in words or as the command line.
    pub(crate) line: String,
    /// Whether it ran: a plan, a staging root and a step after a failed one run nothing.
    pub(crate) ran: bool,
    /// Whether it held, when it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ok: Option<bool>,
    /// What it said, when it ran.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) output: String,
}

enum Act {
    Write(PathBuf, String),
    Remove(PathBuf),
    Run(Vec<String>),
    /// A command that may fail while the manager is still letting go of the job it replaces:
    /// asked again, a second apart, up to this many times.
    Retry(Vec<String>, u32),
}

impl Act {
    fn line(&self) -> String {
        match self {
            Act::Write(p, _) => format!("write {}", p.display()),
            Act::Remove(p) => format!("remove {}", p.display()),
            Act::Run(a) | Act::Retry(a, _) => a.join(" "),
        }
    }

    fn runs(&self) -> bool {
        matches!(self, Act::Run(_) | Act::Retry(..))
    }
}

fn write_file(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("")))?;
    let tmp = path.with_extension(format!("mj-tmp-{}", std::process::id()));
    std::fs::write(&tmp, content).and_then(|()| std::fs::rename(&tmp, path))
}

/// Carry `acts` out in order, stopping at the first that fails; a staging root writes and
/// removes files and runs nothing; a plan does nothing at all.
fn carry_out(host: &Host<'_>, acts: Vec<Act>, apply: bool) -> (Vec<Step>, bool) {
    let mut steps = Vec::new();
    let mut held = true;
    for act in acts {
        let line = act.line();
        if !apply || !held || (host.staging && act.runs()) {
            steps.push(Step { line, ran: false, ok: None, output: String::new() });
            continue;
        }
        let (ok, output) = match &act {
            Act::Write(p, c) => match write_file(p, c) {
                Ok(()) => (true, String::new()),
                Err(e) => (false, e.to_string()),
            },
            Act::Remove(p) => match std::fs::remove_file(p) {
                Ok(()) => (true, String::new()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (true, String::new()),
                Err(e) => (false, e.to_string()),
            },
            Act::Run(a) => (host.exec)(a),
            Act::Retry(a, times) => {
                let mut last = (host.exec)(a);
                for _ in 1..*times {
                    if last.0 {
                        break;
                    }
                    (host.sleep)(Duration::from_secs(1));
                    last = (host.exec)(a);
                }
                last
            }
        };
        held &= ok;
        steps.push(Step { line, ran: true, ok: Some(ok), output: output.trim().to_string() });
    }
    (steps, held)
}

/// The report of `mesh hub install` and `mesh hub uninstall`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct HubServiceReport {
    /// `plan`, `apply` or `staging`.
    pub(crate) mode: String,
    /// The service manager.
    pub(crate) manager: Manager,
    /// The hub's checkout the units serve.
    pub(crate) repo: String,
    /// The executable the units run.
    pub(crate) executable: String,
    /// The hub port.
    pub(crate) port: u16,
    /// The unit files before anything was done.
    pub(crate) units: Vec<UnitView>,
    /// What was owed, and what came of it.
    pub(crate) steps: Vec<Step>,
    /// Why nothing could be done, when nothing could.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) refusal: Option<String>,
    /// What still needs a person: lingering, the firewall, trust.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) notes: Vec<String>,
    /// No refusal, and every step that ran held.
    pub(crate) ok: bool,
}

fn mode(host: &Host<'_>, apply: bool) -> String {
    match (apply, host.staging) {
        (false, _) => "plan".into(),
        (true, true) => "staging".into(),
        (true, false) => "apply".into(),
    }
}

fn report(host: &Host<'_>, spec: &Spec, apply: bool) -> HubServiceReport {
    HubServiceReport {
        mode: mode(host, apply),
        manager: spec.manager,
        repo: spec.repo.display().to_string(),
        executable: spec.executable.display().to_string(),
        port: spec.port,
        units: Vec::new(),
        steps: Vec::new(),
        refusal: None,
        notes: Vec::new(),
        ok: true,
    }
}

/// Why the hub may not be installed for `spec` here, when it may not: no service manager, a
/// machine the declaration does not name as a hub, an executable that would not follow the
/// release, or a checkout that is not dedicated to the hub (one on a branch is somebody's
/// working checkout, whose own server holds its lease, so a hub serving it would exit at
/// once and be restarted for ever).
pub(crate) fn refusal(
    host: &Host<'_>,
    spec: &Spec,
    declared_hub: bool,
    executable_asked: bool,
) -> Option<String> {
    if spec.manager == Manager::Unsupported {
        return Some("this platform has neither launchd nor systemd: no hub service is written here".into());
    }
    if !declared_hub {
        return Some(format!(
            "this machine is not a declared hub: no address of it is under rendezvous.endpoints \
             of the mesh declaration with port {}; declare it there first (docs/MESH.md, Adding a machine)",
            spec.port
        ));
    }
    if !executable_asked && !is_launcher(&spec.executable) {
        return Some(format!(
            "{} is not the installer's launcher, so a hub it runs would not follow the release; \
             install Majordomus with https://majordomus.dev/install.sh and run this through \
             ~/.local/bin/majordomus, or name the executable with --executable",
            spec.executable.display()
        ));
    }
    let repo = spec.repo.display().to_string();
    let (on_branch, _) = (host.exec)(&argv(&["git", "-C", &repo, "symbolic-ref", "-q", "HEAD"]));
    if on_branch {
        return Some(format!(
            "{repo} is on a branch: a hub serves a dedicated checkout detached at a release tag \
             (git worktree add --detach <path> v<version>), never a working checkout"
        ));
    }
    None
}

/// The steps that bring the manager to the rendered units, given how they were found.
fn activation(host: &Host<'_>, rendered: &[Rendered], found: &[UnitView]) -> Vec<Act> {
    let mut acts = Vec::new();
    let changed = |i: usize| found[i].state != FileState::Current;
    for (i, r) in rendered.iter().enumerate() {
        if changed(i) {
            acts.push(Act::Write(r.path.clone(), r.content.clone()));
        }
    }
    match host.manager {
        Manager::Launchd => {
            for (i, r) in rendered.iter().enumerate() {
                let target = format!("gui/{}/{}", host.uid, r.name);
                if changed(i) && found[i].loaded {
                    acts.push(Act::Run(argv(&["launchctl", "bootout", &target])));
                }
                if !found[i].enabled {
                    acts.push(Act::Run(argv(&["launchctl", "enable", &target])));
                }
                if changed(i) || !found[i].loaded {
                    // a bootout returns before the job has let go: bootstrap is asked again
                    acts.push(Act::Retry(
                        argv(&[
                            "launchctl",
                            "bootstrap",
                            &format!("gui/{}", host.uid),
                            &r.path.display().to_string(),
                        ]),
                        10,
                    ));
                } else if r.role == UnitRole::Hub && !found[i].active {
                    // loaded, current and not running: a hub launchd is holding back
                    acts.push(Act::Run(argv(&["launchctl", "kickstart", &target])));
                }
            }
        }
        Manager::SystemdUser => {
            if (0..rendered.len()).any(changed) {
                acts.push(Act::Run(argv(&["systemctl", "--user", "daemon-reload"])));
            }
            // the hub and the timer are what is enabled; the update service is the timer's
            for (i, r) in rendered.iter().enumerate() {
                if r.role == UnitRole::Update {
                    continue;
                }
                if !found[i].enabled {
                    acts.push(Act::Run(argv(&["systemctl", "--user", "enable", r.name])));
                }
                if changed(i) && found[i].active {
                    acts.push(Act::Run(argv(&["systemctl", "--user", "restart", r.name])));
                } else if !found[i].active {
                    acts.push(Act::Run(argv(&["systemctl", "--user", "start", r.name])));
                }
            }
        }
        Manager::Unsupported => {}
    }
    acts
}

/// What still needs a person on this host after an install.
fn notes(host: &Host<'_>) -> Vec<String> {
    let mut notes = Vec::new();
    if host.staging {
        return notes;
    }
    match host.manager {
        Manager::SystemdUser => {
            let (_, linger) = (host.exec)(&argv(&[
                "loginctl",
                "show-user",
                &host.user,
                "--property=Linger",
                "--value",
            ]));
            if linger.trim() != "yes" {
                notes.push(format!(
                    "lingering is off for {user}: the hub starts at login, not at boot; \
                     sudo loginctl enable-linger {user}",
                    user = host.user
                ));
            }
        }
        Manager::Launchd => notes.push(
            "a launch agent starts at the user's login: keep automatic login on, or the hub \
             waits for the first login after a boot"
                .into(),
        ),
        Manager::Unsupported => {}
    }
    notes.push("inbound connections to the hub port need the host firewall: sudo majordomus mesh firewall apply".into());
    notes
}

/// `mesh hub install`: the units rendered, compared with what is there, and with `apply`
/// written and loaded; a second install finds every unit current, loaded, enabled and the
/// hub running, and owes nothing.
pub(crate) fn install(
    host: &Host<'_>,
    spec: &Spec,
    declared_hub: bool,
    executable_asked: bool,
    apply: bool,
) -> HubServiceReport {
    let mut out = report(host, spec, apply);
    if let Some(why) = refusal(host, spec, declared_hub, executable_asked) {
        out.refusal = Some(why);
        out.ok = false;
        return out;
    }
    let rendered = render(spec);
    out.units = views(host, &rendered);
    let acts = activation(host, &rendered, &out.units);
    let (steps, held) = carry_out(host, acts, apply);
    out.steps = steps;
    out.ok = held;
    out.notes = notes(host);
    out
}

/// `mesh hub uninstall`: the units stopped and removed; nothing is refused, because taking a
/// hub away needs no declaration.
pub(crate) fn uninstall(host: &Host<'_>, spec: &Spec, apply: bool) -> HubServiceReport {
    let mut out = report(host, spec, apply);
    let rendered = render(spec);
    out.units = views(host, &rendered);
    let mut acts = Vec::new();
    for (r, found) in rendered.iter().zip(&out.units) {
        match host.manager {
            Manager::Launchd if found.loaded => acts.push(Act::Run(argv(&[
                "launchctl",
                "bootout",
                &format!("gui/{}/{}", host.uid, r.name),
            ]))),
            Manager::SystemdUser if r.role != UnitRole::Update && (found.loaded || found.active) => {
                acts.push(Act::Run(argv(&["systemctl", "--user", "disable", "--now", r.name])))
            }
            _ => {}
        }
    }
    for (r, found) in rendered.iter().zip(&out.units) {
        if found.state != FileState::Missing {
            acts.push(Act::Remove(r.path.clone()));
        }
    }
    if host.manager == Manager::SystemdUser && out.units.iter().any(|u| u.state != FileState::Missing) {
        acts.push(Act::Run(argv(&["systemctl", "--user", "daemon-reload"])));
    }
    let (steps, held) = carry_out(host, acts, apply);
    out.steps = steps;
    out.ok = held;
    out
}

/// The version an executable says it is: the last word of `--version`.
pub(crate) fn version_of(host: &Host<'_>, executable: &Path) -> Option<String> {
    let (ok, out) = (host.exec)(&[executable.display().to_string(), "--version".into()]);
    ok.then(|| out.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string))
        .flatten()
        .and_then(|l| l.split_whitespace().last().map(str::to_string))
}

/// The version the release origin publishes as the latest.
pub(crate) fn published(host: &Host<'_>, base: &str) -> Option<String> {
    let url = format!("{}/releases/latest.json", base.trim_end_matches('/'));
    let (ok, body) = (host.exec)(&argv(&["curl", "-fsS", "-m", "20", &url]));
    ok.then(|| serde_json::from_str::<Value>(&body).ok())
        .flatten()
        .and_then(|v| v["version"].as_str().map(|s| s.trim_start_matches('v').to_string()))
}

/// The release the hub's checkout is detached at: its exact tag, without the `v`.
pub(crate) fn checkout_version(host: &Host<'_>, repo: &Path) -> Option<String> {
    let (ok, out) = (host.exec)(&argv(&[
        "git",
        "-C",
        &repo.display().to_string(),
        "describe",
        "--tags",
        "--exact-match",
        "HEAD",
    ]));
    let tag = out.trim();
    (ok && !tag.is_empty()).then(|| tag.trim_start_matches('v').to_string())
}

/// What the hub answers on its port: its version, how many sessions are attached to it, and
/// the runtimes it is linked with.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct HubAnswer {
    /// Whether it answered at all.
    pub(crate) answering: bool,
    /// The version it serves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    /// The sessions attached to the hub's own board.
    pub(crate) attached: u64,
    /// Every runtime it is linked with, and the version each speaks: what the mesh
    /// broadcasts about its members.
    pub(crate) linked: Vec<LinkedRuntime>,
}

/// A runtime the hub is linked with.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct LinkedRuntime {
    /// The machine's short name.
    pub(crate) machine: String,
    /// The runtime.
    pub(crate) runtime: String,
    /// The version it runs.
    pub(crate) version: String,
}

fn hub_base(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

/// The version the hub on `port` serves, from one request.
fn served_version(host: &Host<'_>, port: u16) -> Option<String> {
    (host.get)(&hub_base(port), "/")
        .and_then(|b| serde_json::from_str::<Value>(&b).ok())
        .and_then(|v| v["version"].as_str().map(str::to_string))
}

/// Ask the hub on `port` of this machine.
pub(crate) fn ask_hub(host: &Host<'_>, port: u16) -> HubAnswer {
    let base = hub_base(port);
    let Some(version) = served_version(host, port) else {
        return HubAnswer::default();
    };
    let attached = (host.get)(&base, "/api/v1/peers")
        .and_then(|b| serde_json::from_str::<Value>(&b).ok())
        .and_then(|v| {
            v["boards"]
                .as_array()?
                .iter()
                .find(|b| b["this_checkout"] == Value::Bool(true))
                .and_then(|b| b["attached"].as_u64())
        })
        .unwrap_or(0);
    let linked = (host.get)(&base, "/api/v1/mesh/cooperation")
        .and_then(|b| serde_json::from_str::<Value>(&b).ok())
        .map(|v| linked_runtimes(&v))
        .unwrap_or_default();
    HubAnswer { answering: true, version: Some(version), attached, linked }
}

/// The connected peers of a `mesh.cooperation` answer.
pub(crate) fn linked_runtimes(cooperation: &Value) -> Vec<LinkedRuntime> {
    cooperation["peers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["state"] == "connected")
        .map(|p| LinkedRuntime {
            machine: p["name"].as_str().unwrap_or("").to_string(),
            runtime: p["runtime"].as_str().unwrap_or("").to_string(),
            version: p["version"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

/// How long sessions attached to the hub may hold its restart back.
pub(crate) const DEFER_AT_MOST_SECONDS: u64 = 86_400;

/// What the update does about the versions it found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub(crate) enum Decision {
    /// The hub and its checkout are at the newest release there is: nothing to do.
    Current,
    /// A newer release is published than installed: install it, then decide again.
    Install {
        /// The version to install.
        version: String,
    },
    /// The installed release is newer than the hub serves or its checkout is at: move the
    /// checkout to its tag and restart the hub onto it.
    Activate {
        /// The version the hub moves to.
        version: String,
    },
    /// Sessions are attached to the hub: the restart waits, a day at most.
    Deferred {
        /// How many.
        attached: u64,
    },
    /// The version failed on this node before: it is not tried again until a newer one is
    /// published, or the update is asked to retry.
    Failed {
        /// The version.
        version: String,
    },
    /// A version could not be read, so nothing is done.
    Unknown {
        /// Which.
        reason: String,
    },
}

/// What [`decide`] reads.
#[derive(Debug, Clone, Default)]
pub(crate) struct Versions<'a> {
    /// The published release.
    pub(crate) published: Option<&'a str>,
    /// The release the executable is.
    pub(crate) installed: Option<&'a str>,
    /// The release the hub serves.
    pub(crate) served: Option<&'a str>,
    /// The release the hub's checkout is detached at.
    pub(crate) checkout: Option<&'a str>,
    /// Sessions attached to the hub.
    pub(crate) attached: u64,
    /// Whether the restart has been deferred for longer than it may be.
    pub(crate) deferred_too_long: bool,
}

/// The update's decision, a function of the versions alone.
pub(crate) fn decide(v: &Versions<'_>, failed: &[String]) -> Decision {
    use std::cmp::Ordering::Greater;
    let cmp = crate::order::version_cmp;
    let Some(installed) = v.installed else {
        return Decision::Unknown { reason: "the executable did not say its version".into() };
    };
    if let Some(p) = v.published {
        if cmp(p, installed) == Greater {
            return if failed.iter().any(|f| f == p) {
                Decision::Failed { version: p.to_string() }
            } else {
                Decision::Install { version: p.to_string() }
            };
        }
    }
    // a checkout at no tag, or at an older one, is behind the executable: the executable
    // refuses a manifest newer than itself, and a hub left behind falls a protocol behind
    let checkout_behind = v.checkout.is_none_or(|c| cmp(installed, c) == Greater);
    let hub_behind = match v.served {
        Some(s) => cmp(installed, s) == Greater,
        None if checkout_behind => true,
        None => return Decision::Unknown { reason: "the hub does not answer on its port".into() },
    };
    if !hub_behind && !checkout_behind {
        return Decision::Current;
    }
    if failed.iter().any(|f| f == installed) {
        return Decision::Failed { version: installed.to_string() };
    }
    if v.attached > 0 && !v.deferred_too_long {
        return Decision::Deferred { attached: v.attached };
    }
    Decision::Activate { version: installed.to_string() }
}

/// What the update remembers between runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub(crate) struct UpdateState {
    /// The last run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last: Option<UpdateRun>,
    /// Versions that failed on this node and are not tried again.
    #[serde(default)]
    pub(crate) failed: Vec<String>,
    /// Since when the restart has been deferred, while it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) deferred_since: Option<u64>,
}

/// One run of the update.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct UpdateRun {
    /// When it ran, in seconds since the epoch.
    pub(crate) at: u64,
    /// `current`, `updated`, `deferred`, `rolled_back`, `failed`, `skipped` or `unknown`.
    pub(crate) result: String,
    /// What it said.
    pub(crate) detail: String,
}

pub(crate) fn read_state(home: &Path) -> UpdateState {
    std::fs::read_to_string(state_path(home))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn write_state(home: &Path, state: &UpdateState) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(state).map_err(std::io::Error::other)?;
    write_file(&state_path(home), &(text + "\n"))
}

/// The lock that keeps two updates of one home apart.
fn lock_path(home: &Path) -> PathBuf {
    home.join(".local/state/majordomus/hub-update.lock")
}

/// Take the update's lock: `Err` names the live process that holds it. A lock whose holder
/// is gone is taken over.
fn take_lock(host: &Host<'_>, home: &Path) -> Result<(), String> {
    let path = lock_path(home);
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new("")));
    for _ in 0..2 {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                use std::io::Write;
                let _ = writeln!(f, "{}", host.pid);
                return Ok(());
            }
            Err(_) => {
                let holder = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|t| t.trim().parse::<u32>().ok());
                match holder {
                    Some(pid) if pid != host.pid && (host.alive)(pid) => {
                        return Err(format!("another update runs (pid {pid})"));
                    }
                    _ => {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }
    Err(format!("{} cannot be taken", path.display()))
}

/// The largest a launchd log may grow before the update rotates it.
pub(crate) const LOG_BOUND: u64 = 64 * 1024 * 1024;

/// Rotate a log past [`LOG_BOUND`]: copied to `<log>.1` and truncated in place, because the
/// job keeps writing to the file it opened.
fn rotate(log: &Path) -> Option<Step> {
    let size = std::fs::metadata(log).ok()?.len();
    if size <= LOG_BOUND {
        return None;
    }
    let old = log.with_extension("log.1");
    let done = std::fs::copy(log, &old).and_then(|_| {
        std::fs::OpenOptions::new().write(true).open(log).and_then(|f| f.set_len(0))
    });
    Some(Step {
        line: format!("rotate {} ({size} bytes)", log.display()),
        ran: true,
        ok: Some(done.is_ok()),
        output: done.err().map(|e| e.to_string()).unwrap_or_default(),
    })
}

/// The report of `mesh hub update`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct HubUpdateReport {
    /// `plan` or `apply`.
    pub(crate) mode: String,
    /// The published release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) published: Option<String>,
    /// The release the executable is, before the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) installed: Option<String>,
    /// The release the hub served, before the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) served: Option<String>,
    /// The release the hub's checkout was at, before the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) checkout: Option<String>,
    /// What the versions called for.
    pub(crate) decision: Decision,
    /// What was done.
    pub(crate) steps: Vec<Step>,
    /// How the run ended: the `result` it recorded.
    pub(crate) result: String,
    /// What it said about it.
    pub(crate) detail: String,
    /// Nothing failed, and the hub serves what it should.
    pub(crate) ok: bool,
}

/// Restart the hub through its manager.
fn restart(host: &Host<'_>) -> Vec<String> {
    match host.manager {
        Manager::Launchd => argv(&["launchctl", "kickstart", "-k", &format!("gui/{}/{HUB_LABEL}", host.uid)]),
        _ => argv(&["systemctl", "--user", "restart", HUB_UNIT]),
    }
}

/// What a restarted hub answered.
#[derive(Debug, PartialEq, Eq)]
enum Answer {
    /// The version it was restarted onto.
    New,
    /// The version it served before: the restart did not take, or another process serves
    /// the port.
    Old,
    /// Nothing, or something else.
    Nothing,
}

/// Ask the hub up to `ANSWER_WITHIN / ASK_EVERY` times, `ASK_EVERY` apart, for `version`.
fn answers_with(host: &Host<'_>, port: u16, version: &str, previous: Option<&str>) -> Answer {
    let attempts = ANSWER_WITHIN.as_secs() / ASK_EVERY.as_secs();
    let mut last = None;
    for i in 0..attempts {
        last = served_version(host, port);
        if last.as_deref() == Some(version) {
            return Answer::New;
        }
        if i + 1 < attempts {
            (host.sleep)(ASK_EVERY);
        }
    }
    if last.is_some() && last.as_deref() == previous {
        Answer::Old
    } else {
        Answer::Nothing
    }
}

struct Run<'h, 'a> {
    host: &'h Host<'a>,
    spec: &'h Spec,
    base: &'h str,
    steps: Vec<Step>,
}

impl Run<'_, '_> {
    fn run(&mut self, words: Vec<String>) -> (bool, String) {
        let (ok, out) = (self.host.exec)(&words);
        self.steps.push(Step {
            line: words.join(" "),
            ran: true,
            ok: Some(ok),
            output: out.trim().chars().take(2000).collect(),
        });
        (ok, out)
    }

    fn git(&mut self, words: &[&str]) -> (bool, String) {
        let mut a = argv(&["git", "-C", &self.spec.repo.display().to_string()]);
        a.extend(argv(words));
        self.run(a)
    }

    /// Run the installer of the release origin for `version`, into the directory of the
    /// launcher the units run, so the launcher they run is the one it rewrites; `force` lets
    /// it put an older release back.
    fn installer(&mut self, version: &str, force: bool) -> bool {
        let home = &self.spec.home;
        let script = home.join(".local/state/majordomus/install.sh").display().to_string();
        let url = format!("{}/install.sh", self.base.trim_end_matches('/'));
        if !self.run(argv(&["curl", "-fsSL", "-m", "120", "-o", &script, &url])).0 {
            return false;
        }
        let dir = self
            .spec
            .executable
            .parent()
            .map(|d| d.display().to_string())
            .unwrap_or_default();
        let mut words = vec!["env".to_string()];
        if self.base != RELEASE_BASE {
            words.push(format!("MAJORDOMUS_RELEASE_BASE_URL={}", self.base));
        }
        words.extend(argv(&["sh", &script, "--version", version, "--install-dir", &dir]));
        if force {
            words.push("--force".into());
        }
        self.run(words).0
    }
}

/// `mesh hub update`: ask the release origin, the executable, the checkout and the hub for
/// their versions, decide, and with `apply` carry the decision out and record it. One update
/// runs at a time per home; a second one says so and does nothing.
pub(crate) fn update(host: &Host<'_>, spec: &Spec, base: &str, apply: bool, retry: bool) -> HubUpdateReport {
    let mut state = read_state(&spec.home);
    if retry {
        state.failed.clear();
    }
    let published_v = published(host, base);
    let installed_v = version_of(host, &spec.executable);
    let checkout_v = checkout_version(host, &spec.repo);
    let hub = ask_hub(host, spec.port);
    let served_v = hub.version.clone();
    let now = (host.now)();
    let versions = Versions {
        published: published_v.as_deref(),
        installed: installed_v.as_deref(),
        served: served_v.as_deref(),
        checkout: checkout_v.as_deref(),
        attached: hub.attached,
        deferred_too_long: state
            .deferred_since
            .is_some_and(|since| now.saturating_sub(since) > DEFER_AT_MOST_SECONDS),
    };
    let first = decide(&versions, &state.failed);
    let mut out = HubUpdateReport {
        mode: if apply { "apply".into() } else { "plan".into() },
        published: published_v.clone(),
        installed: installed_v.clone(),
        served: served_v.clone(),
        checkout: checkout_v.clone(),
        decision: first.clone(),
        steps: Vec::new(),
        result: "planned".into(),
        detail: String::new(),
        ok: !matches!(first, Decision::Unknown { .. } | Decision::Failed { .. }),
    };
    if !apply {
        return out;
    }
    if !is_launcher(&spec.executable) {
        out.result = "skipped".into();
        out.detail = format!(
            "{} is not the installer's launcher; update it the way it was installed",
            spec.executable.display()
        );
        out.ok = false;
        return out;
    }
    if let Err(why) = take_lock(host, &spec.home) {
        out.result = "busy".into();
        out.detail = why;
        return out;
    }
    let mut run = Run { host, spec, base, steps: Vec::new() };
    for role in [UnitRole::Hub, UnitRole::Update] {
        if spec.manager == Manager::Launchd {
            run.steps.extend(rotate(&spec.log(role)));
        }
    }
    let mut decision = first;
    let (result, detail) = loop {
        match decision.clone() {
            Decision::Current => break ("current", format!("the hub serves {}", served_v.clone().unwrap_or_default())),
            Decision::Unknown { reason } => break ("unknown", reason),
            Decision::Failed { version } => break ("skipped", format!("{version} failed here before; waiting for a newer release (or --retry)")),
            Decision::Deferred { attached } => break ("deferred", format!("{attached} session(s) attached to the hub")),
            Decision::Install { version } => {
                if !run.installer(&version, false) {
                    break ("failed", format!("the installer did not install {version}"));
                }
                let now_v = version_of(host, &spec.executable);
                if now_v.as_deref() != Some(version.as_str()) {
                    break ("failed", format!("after the installer the executable is {}", now_v.unwrap_or_else(|| "silent".into())));
                }
                decision = decide(&Versions { installed: Some(&version), ..versions.clone() }, &state.failed);
            }
            Decision::Activate { version } => {
                break activate(&mut run, &version, served_v.as_deref(), &mut state);
            }
        }
    };
    state.deferred_since = match result {
        "deferred" => state.deferred_since.or(Some(now)),
        _ => None,
    };
    state.last = Some(UpdateRun { at: now, result: result.to_string(), detail: detail.clone() });
    let recorded = write_state(&spec.home, &state);
    if let Err(e) = &recorded {
        run.steps.push(Step {
            line: format!("record {}", state_path(&spec.home).display()),
            ran: true,
            ok: Some(false),
            output: e.to_string(),
        });
    }
    // a release may render its units differently: the new executable writes them again
    if result == "updated" {
        let mut words = vec![spec.executable.display().to_string()];
        words.extend(argv(&["mesh", "hub", "install", "--apply", "--port", &spec.port.to_string(), "--repo", &spec.repo.display().to_string()]));
        run.run(words);
    }
    let _ = std::fs::remove_file(lock_path(&spec.home));
    out.steps = run.steps;
    out.result = result.to_string();
    out.detail = detail;
    out.ok = recorded.is_ok() && matches!(result, "current" | "updated" | "deferred");
    out
}

/// Move the hub onto `version`: its checkout to the release's tag, then a restart, then the
/// hub's own word. A restart that does not take puts the checkout back and blames nothing; a
/// hub that answers nothing gets the previous release and checkout back, and the version is
/// remembered as failed here.
fn activate(
    run: &mut Run<'_, '_>,
    version: &str,
    previous: Option<&str>,
    state: &mut UpdateState,
) -> (&'static str, String) {
    let repo = run.spec.repo.display().to_string();
    let port = run.spec.port;
    let (_, dirty) = run.git(&["status", "--porcelain", "--untracked-files=no"]);
    if !dirty.trim().is_empty() {
        return ("failed", format!("{repo} has uncommitted changes; the hub is not moved"));
    }
    let (ok, head) = run.git(&["rev-parse", "HEAD"]);
    if !ok {
        return ("failed", format!("{repo} is not a git checkout"));
    }
    let head = head.trim().to_string();
    // a fetch that fails is not fatal: the tag may be here already
    run.git(&["fetch", "--quiet", "--tags", "origin"]);
    let tag = format!("v{version}");
    if !run.git(&["checkout", "--quiet", "--detach", &tag]).0 {
        return ("failed", format!("{repo} has no tag {tag}; the hub is not moved"));
    }
    if !run.run(restart(run.host)).0 {
        run.git(&["checkout", "--quiet", "--detach", &head]);
        return ("failed", "the service manager did not restart the hub; the checkout is back".into());
    }
    match answers_with(run.host, port, version, previous) {
        Answer::New => {
            return ("updated", format!("the hub moved from {} to {version}", previous.unwrap_or("nothing")));
        }
        Answer::Old => {
            run.git(&["checkout", "--quiet", "--detach", &head]);
            return (
                "failed",
                format!(
                    "port {port} still answers with {} after the restart: another process serves it, \
                     or the service manager does not run this hub; the checkout is back",
                    previous.unwrap_or("")
                ),
            );
        }
        Answer::Nothing => {}
    }
    // the new release does not serve: put the previous one back and never try this one again
    state.failed.push(version.to_string());
    let Some(p) = previous else {
        return ("failed", format!("the hub did not answer with {version}, and no previous release is known to put back"));
    };
    if !run.installer(p, true) {
        // the launcher still runs the new release: the checkout stays at its tag, to match
        run.run(restart(run.host));
        return ("failed", format!("the hub did not answer with {version}, and {p} could not be installed again"));
    }
    run.git(&["checkout", "--quiet", "--detach", &head]);
    run.run(restart(run.host));
    let back = answers_with(run.host, port, p, None) == Answer::New;
    (
        "rolled_back",
        format!(
            "the hub did not answer with {version} within {}s; {}",
            ANSWER_WITHIN.as_secs(),
            if back { format!("it serves {p} again") } else { "and it does not answer with the previous release either".into() }
        ),
    )
}

/// How one check of `mesh hub status` came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Level {
    /// Holds.
    Pass,
    /// Holds, with something a person should know.
    Warn,
    /// Does not hold.
    Fail,
}

/// One check of `mesh hub status`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct HubCheck {
    /// Which.
    pub(crate) name: String,
    /// How it came out.
    pub(crate) level: Level,
    /// What was found.
    pub(crate) detail: String,
    /// What to run when it does not hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remedy: Option<String>,
}

/// The report of `mesh hub status`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct HubStatusReport {
    /// The service manager.
    pub(crate) manager: Manager,
    /// The hub's checkout.
    pub(crate) repo: String,
    /// The hub port.
    pub(crate) port: u16,
    /// The unit files.
    pub(crate) units: Vec<UnitView>,
    /// What the hub answered.
    pub(crate) hub: HubAnswer,
    /// The published release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) published: Option<String>,
    /// The release the executable is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) installed: Option<String>,
    /// What the update remembers.
    pub(crate) update: UpdateState,
    /// Every check.
    pub(crate) checks: Vec<HubCheck>,
    /// No check failed.
    pub(crate) ok: bool,
}

fn check(name: &str, level: Level, detail: String, remedy: Option<String>) -> HubCheck {
    HubCheck { name: name.into(), level, detail, remedy }
}

/// `mesh hub status`: the units, the manager, the hub, the versions, the update's last run
/// and the fleet's versions, each a check.
pub(crate) fn status(host: &Host<'_>, spec: &Spec, declared_hub: bool, base: &str) -> HubStatusReport {
    use std::cmp::Ordering::Less;
    let cmp = crate::order::version_cmp;
    let rendered = render(spec);
    let units = views(host, &rendered);
    let hub = if host.staging { HubAnswer::default() } else { ask_hub(host, spec.port) };
    let published_v = if host.staging { None } else { published(host, base) };
    let installed_v = version_of(host, &spec.executable);
    let update = read_state(&spec.home);
    let now = (host.now)();
    let repo = spec.repo.display();
    let install = format!("majordomus mesh hub install --apply --repo {repo}");
    let mut checks = Vec::new();

    checks.push(if spec.manager == Manager::Unsupported {
        check("manager", Level::Fail, "neither launchd nor systemd runs here".into(), None)
    } else {
        check("manager", Level::Pass, format!("{:?}", spec.manager).to_lowercase(), None)
    });
    checks.push(if declared_hub {
        check("declared", Level::Pass, format!("an address of this machine is a declared hub on port {}", spec.port), None)
    } else {
        check("declared", Level::Warn, "no address of this machine is under rendezvous.endpoints".into(), None)
    });
    checks.push(if is_launcher(&spec.executable) {
        check("launcher", Level::Pass, format!("{} is the installer's launcher", spec.executable.display()), None)
    } else {
        check("launcher", Level::Warn, format!("{} is not the installer's launcher: the units would not follow the release", spec.executable.display()), None)
    });
    for u in &units {
        let name = format!("unit:{}", u.name);
        checks.push(match (u.state, u.loaded && u.enabled) {
            (FileState::Current, true) => check(&name, Level::Pass, format!("{} as rendered, loaded", u.path), None),
            (FileState::Current, false) => check(&name, Level::Fail, format!("{} as rendered, not loaded or disabled", u.path), Some(install.clone())),
            (FileState::Drifted, _) => check(&name, Level::Fail, format!("{} differs from what install renders", u.path), Some(install.clone())),
            (FileState::Missing, _) => check(&name, Level::Fail, format!("{} is missing", u.path), Some(install.clone())),
        });
    }
    checks.push(match &hub.version {
        Some(v) => check("answering", Level::Pass, format!("the hub answers on port {} serving {v}", spec.port), None),
        None => check("answering", Level::Fail, format!("nothing answers on port {}", spec.port), Some(install.clone())),
    });
    if hub.answering {
        checks.push(if hub.linked.is_empty() {
            check("linked", Level::Fail, "the hub is linked with no runtime".into(), Some("majordomus mesh doctor".into()))
        } else {
            check("linked", Level::Pass, format!("linked with {} runtime(s)", hub.linked.len()), None)
        });
    }
    let fresh = update
        .last
        .as_ref()
        .is_some_and(|r| now.saturating_sub(r.at) <= 2 * UPDATE_INTERVAL_SECONDS);
    let last_result = update.last.as_ref().map(|r| r.result.as_str()).unwrap_or("");
    let update_cmd = format!("majordomus mesh hub update --apply --repo {repo}");
    checks.push(match (&published_v, &hub.version) {
        (None, _) => check("current", Level::Warn, "the published release could not be read".into(), None),
        (Some(p), Some(s)) if cmp(s, p) == Less && fresh && last_result == "deferred" => {
            check("current", Level::Warn, format!("the hub serves {s}, {p} is published; the restart waits for attached sessions"), None)
        }
        (Some(p), Some(s)) if cmp(s, p) == Less => check(
            "current",
            Level::Fail,
            format!("the hub serves {s}, {p} is published, and the last update ended {}", if last_result.is_empty() { "never" } else { last_result }),
            Some(update_cmd.clone()),
        ),
        (Some(p), _) => check("current", Level::Pass, format!("{p} is published, installed {}", installed_v.clone().unwrap_or_default()), None),
    });
    checks.push(match &update.last {
        Some(r) if !fresh => check("update", Level::Fail, format!("last run {}s ago: {}", now.saturating_sub(r.at), r.result), Some(install.clone())),
        Some(r) => {
            let level = match r.result.as_str() {
                "current" | "updated" | "deferred" | "busy" => Level::Pass,
                "skipped" => Level::Warn,
                _ => Level::Fail,
            };
            check("update", level, format!("last run {}s ago: {} ({})", now.saturating_sub(r.at), r.result, r.detail), (level == Level::Fail).then(|| update_cmd.clone()))
        }
        None => check("update", Level::Fail, "the update never ran here".into(), Some(install.clone())),
    });
    if !update.failed.is_empty() {
        checks.push(check(
            "failed_versions",
            Level::Warn,
            format!("not tried again here: {}", update.failed.join(", ")),
            Some(format!("{update_cmd} --retry")),
        ));
    }
    if let Some(p) = &published_v {
        let behind: Vec<String> = hub
            .linked
            .iter()
            .filter(|l| cmp(&l.version, p) == Less)
            .map(|l| format!("{} {}", l.machine, l.version))
            .collect();
        checks.push(if behind.is_empty() {
            check("fleet", Level::Pass, format!("every linked runtime runs {p} or newer"), None)
        } else {
            check("fleet", Level::Warn, format!("behind {p}: {}", behind.join(", ")), None)
        });
    }
    let ok = checks.iter().all(|c| c.level != Level::Fail);
    HubStatusReport {
        manager: spec.manager,
        repo: spec.repo.display().to_string(),
        port: spec.port,
        units,
        hub,
        published: published_v,
        installed: installed_v,
        update,
        checks,
        ok,
    }
}

/// The text of a hub report, whichever of the four it is.
pub(crate) fn render_text(v: &Value) -> String {
    let s = |v: &Value, k: &str| v[k].as_str().map(str::to_string).unwrap_or_else(|| v[k].to_string());
    let mut out = String::new();
    for k in ["mode", "manager", "repo", "executable", "port", "published", "installed", "served", "checkout", "result", "detail"] {
        if !v[k].is_null() && v[k] != "" {
            out.push_str(&format!("{k:<11} {}\n", s(v, k)));
        }
    }
    if let Some(d) = v["decision"]["decision"].as_str() {
        out.push_str(&format!("{:<11} {d}\n", "decision"));
    }
    for u in v["units"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "unit        {:<9} {} loaded={} enabled={} active={}\n",
            s(u, "state"),
            s(u, "path"),
            u["loaded"],
            u["enabled"],
            u["active"]
        ));
    }
    for st in v["steps"].as_array().into_iter().flatten() {
        let verdict = match (st["ran"].as_bool(), st["ok"].as_bool()) {
            (Some(true), Some(true)) => "ran",
            (Some(true), _) => "FAILED",
            _ => "owed",
        };
        out.push_str(&format!("{verdict:<11} {}\n", s(st, "line")));
    }
    for c in v["checks"].as_array().into_iter().flatten() {
        out.push_str(&format!("{:<5} {:<28} {}\n", s(c, "level"), s(c, "name"), s(c, "detail")));
        if let Some(r) = c["remedy"].as_str() {
            out.push_str(&format!("      remedy: {r}\n"));
        }
    }
    if let Some(r) = v["refusal"].as_str() {
        out.push_str(&format!("refused     {r}\n"));
    }
    for n in v["notes"].as_array().into_iter().flatten() {
        out.push_str(&format!("note        {}\n", n.as_str().unwrap_or("")));
    }
    out.push_str(&format!("ok          {}\n", v["ok"]));
    out
}

#[cfg(test)]
mod tests;
