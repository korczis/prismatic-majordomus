//! The executor as a user service (I3913): `prs drain --continuous` kept running outside every
//! agent session, restarted after a crash or an outage and never after a halt.
//!
//! # Why a service
//!
//! From 2026-10-20 to 2026-10-22 the lander was a loop inside one agent session: it died with
//! the session and with the machine, and every landing an agent session attempted met the
//! auto-mode classifier. A service the owner installs runs as the owner, outside any session,
//! and comes back after a reboot (`RunAtLoad`, `WantedBy=default.target`).
//!
//! # What restarts it, and what does not
//!
//! The service manager restarts a process that exits non-zero and leaves one that exits zero
//! alone (launchd `KeepAlive.SuccessfulExit = false`, systemd `Restart=on-failure`), so
//! [`run`] decides the exit, not the manager: [`disposition`] maps the reason a continuous
//! drain ended to [`Disposition::Stop`] (exit 0) or [`Disposition::Restart`] (exit
//! [`RESTART_EXIT`]). A halt, a merge that could not be verified and the rollout gate stop
//! it, because restarting would undo ADR 0101 D7; an outage that outlasted
//! [`super::drain::CONTINUOUS_TRANSIENT_LIMIT`], an unwritable trail, a lease another executor
//! holds and a crash restart it, throttled by [`THROTTLE_SECONDS`], so a network outage of
//! any length is waited out and a dead holder's lease is taken over within a minute.
//!
//! # The unit writer
//!
//! [`Unit`] and its two renderings are the one writer of user units in this crate; the hub's
//! unit (I2722) is meant to be written through it too. Nothing here is written unless the
//! owner runs `prs service install`.

use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::drain::ContinuousReport;

/// The exit a service run ends with when the service manager should start it again.
/// `EX_TEMPFAIL`: the failure is expected to pass.
pub const RESTART_EXIT: u8 = 75;

/// Seconds the service manager waits before it starts a run that ended in [`RESTART_EXIT`]
/// or a crash again.
pub const THROTTLE_SECONDS: u64 = 60;

/// The state file a service run leaves its last decision in, beside the executor's other
/// state under the local half.
pub const STATUS_FILE: &str = "service.json";

/// The log a service run writes, beside [`STATUS_FILE`].
pub const LOG_FILE: &str = "service.log";

/// Whether the service manager should start the executor again after this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// Exit 0: a person looks first, or the run was asked to stop.
    Stop,
    /// Exit [`RESTART_EXIT`]: what ended it is expected to pass.
    Restart,
}

impl Disposition {
    /// The exit code that tells the service manager this.
    pub fn exit_code(self) -> u8 {
        match self {
            Disposition::Stop => 0,
            Disposition::Restart => RESTART_EXIT,
        }
    }
}

/// Why a service run ended before or after it drained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended<'a> {
    /// The rollout gate refused a continuous drain (ADR 0101 §13).
    RolloutRefused(&'a str),
    /// The lease could not be taken: another executor holds it, or it could not be read.
    LeaseUnavailable(&'a str),
    /// The continuous drain ran and stopped.
    Drained(&'a ContinuousReport),
}

/// What the service manager does after a run that ended so. A halt, an unverified merge and
/// the rollout gate wait for a person; everything else is expected to pass.
pub fn disposition(ended: &Ended<'_>) -> Disposition {
    match ended {
        Ended::RolloutRefused(_) => Disposition::Stop,
        Ended::LeaseUnavailable(_) => Disposition::Restart,
        Ended::Drained(report) => {
            if report.failure.is_some() {
                Disposition::Restart
            } else {
                // a halt, an unverified merge or a stop that was asked for: none restarts
                Disposition::Stop
            }
        }
    }
}

/// The last decision of a service run, as [`STATUS_FILE`] holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LastRun {
    /// When the run ended, RFC 3339.
    pub at: String,
    /// The process that ran.
    pub pid: u32,
    /// Stop or restart.
    pub disposition: Disposition,
    /// Why it ended, in a person's words.
    pub why: String,
    /// The systemic failure it ended on, when one.
    pub failure: Option<String>,
    /// Cycles run.
    pub cycles: usize,
    /// Pull requests merged.
    pub merged: Vec<u64>,
}

impl LastRun {
    /// The record of a run that ended so.
    pub fn of(ended: &Ended<'_>, at: String) -> Self {
        let disposition = disposition(ended);
        let (why, failure, cycles, merged) = match ended {
            Ended::RolloutRefused(r) => (r.to_string(), None, 0, Vec::new()),
            Ended::LeaseUnavailable(r) => (r.to_string(), Some(r.to_string()), 0, Vec::new()),
            Ended::Drained(c) => (
                c.stopped.clone(),
                c.failure.clone(),
                c.cycles,
                c.merged.clone(),
            ),
        };
        LastRun {
            at,
            pid: std::process::id(),
            disposition,
            why,
            failure,
            cycles,
            merged,
        }
    }
}

/// Leave `last` where `prs service status` reads it.
pub fn record_last_run(root: &Path, last: &LastRun) -> Result<(), String> {
    let path = super::state_path(root, STATUS_FILE);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(super::at(dir))?;
    }
    let text = serde_json::to_string_pretty(last).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(super::at(&path))
}

/// The last decision a service run left, `None` when none ran.
pub fn last_run(root: &Path) -> Option<LastRun> {
    let text = std::fs::read_to_string(super::state_path(root, STATUS_FILE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// The service manager of this platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Manager {
    /// macOS: a LaunchAgent in the user's GUI domain.
    Launchd,
    /// Linux: a `systemd --user` unit.
    SystemdUser,
}

impl Manager {
    /// This platform's manager, `None` where neither is supported.
    pub fn of_this_platform() -> Option<Self> {
        if cfg!(target_os = "macos") {
            Some(Manager::Launchd)
        } else if cfg!(target_os = "linux") {
            Some(Manager::SystemdUser)
        } else {
            None
        }
    }
}

/// A user unit: one program kept running for one user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// The label (launchd) or the unit's name without `.service` (systemd).
    pub label: String,
    /// One line saying what it runs.
    pub description: String,
    /// The program and its arguments.
    pub program: Vec<String>,
    /// The working directory.
    pub working_directory: PathBuf,
    /// `PATH` for the program: a service manager starts it with almost none.
    pub path: String,
    /// Where standard output and standard error go.
    pub log: PathBuf,
}

/// Escape a string for a plist `<string>`.
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Quote one word for a systemd `ExecStart=` line.
fn systemd_word(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-=:,+@".contains(c))
    {
        s.to_string()
    } else {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

impl Unit {
    /// The LaunchAgent: started at login, restarted after a non-zero exit, throttled.
    pub fn launchd_plist(&self) -> String {
        let args: String = self
            .program
            .iter()
            .map(|a| format!("    <string>{}</string>\n", xml(a)))
            .collect();
        let log = xml(&self.log.display().to_string());
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<!-- Written by `majordomus prs service install` (I3913). Run that again rather than editing this file. -->
<!-- {description} -->
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
{args}  </array>
  <key>WorkingDirectory</key>
  <string>{wd}</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>{path}</string>
  </dict>
  <key>StandardInPath</key>
  <string>/dev/null</string>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>ThrottleInterval</key>
  <integer>{throttle}</integer>
  <key>ProcessType</key>
  <string>Background</string>
</dict>
</plist>
"#,
            description = xml(&self.description),
            label = xml(&self.label),
            wd = xml(&self.working_directory.display().to_string()),
            path = xml(&self.path),
            throttle = THROTTLE_SECONDS,
        )
    }

    /// The `systemd --user` unit: started with the user's manager, restarted on failure.
    pub fn systemd_unit(&self) -> String {
        let exec: Vec<String> = self.program.iter().map(|a| systemd_word(a)).collect();
        let log = self.log.display();
        format!(
            "# Written by `majordomus prs service install` (I3913). Run that again rather than editing this file.\n\
             [Unit]\n\
             Description={description}\n\
             After=network-online.target\n\
             Wants=network-online.target\n\
             StartLimitIntervalSec=0\n\
             \n\
             [Service]\n\
             Type=simple\n\
             ExecStart={exec}\n\
             WorkingDirectory={wd}\n\
             Environment={path}\n\
             StandardInput=null\n\
             StandardOutput=append:{log}\n\
             StandardError=append:{log}\n\
             Restart=on-failure\n\
             RestartSec={throttle}\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n",
            description = self.description,
            exec = exec.join(" "),
            wd = systemd_word(&self.working_directory.display().to_string()),
            path = systemd_word(&format!("PATH={}", self.path)),
            throttle = THROTTLE_SECONDS,
        )
    }

    /// The unit's text for `manager`.
    pub fn render(&self, manager: Manager) -> String {
        match manager {
            Manager::Launchd => self.launchd_plist(),
            Manager::SystemdUser => self.systemd_unit(),
        }
    }

    /// Where `manager` reads this unit from under `home`.
    pub fn path_under(&self, manager: Manager, home: &Path) -> PathBuf {
        match manager {
            Manager::Launchd => home
                .join("Library/LaunchAgents")
                .join(format!("{}.plist", self.label)),
            Manager::SystemdUser => home
                .join(".config/systemd/user")
                .join(format!("{}.service", self.label)),
        }
    }
}

/// The label of the executor's unit for the checkout at `root`: one per checkout, stable
/// across installs, so a second install finds the first.
pub fn label_for(root: &Path) -> String {
    use sha2::{Digest, Sha256};
    let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let digest = Sha256::digest(canonical.display().to_string().as_bytes());
    let hex: String = digest.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("dev.majordomus.prs-drain.{hex}")
}

/// What a service runs the executor with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorOptions {
    /// The executable the unit starts: a stable install, not a build directory.
    pub executable: PathBuf,
    /// Seconds between cycles.
    pub interval: u64,
    /// Merges per cycle at most.
    pub max: usize,
    /// Whether a cycle may bring master into a pull request.
    pub refresh: bool,
}

/// The executor's unit for the checkout at `root`.
pub fn executor_unit(root: &Path, opts: &ExecutorOptions, path: String) -> Unit {
    let mut program = vec![
        opts.executable.display().to_string(),
        "prs".into(),
        "--repo".into(),
        root.display().to_string(),
        "service".into(),
        "run".into(),
        "--interval".into(),
        opts.interval.to_string(),
        "--max".into(),
        opts.max.to_string(),
    ];
    if opts.refresh {
        program.push("--refresh".into());
    }
    Unit {
        label: label_for(root),
        description: format!("majordomus integration executor of {}", root.display()),
        program,
        working_directory: root.to_path_buf(),
        path,
        log: super::state_path(root, LOG_FILE),
    }
}

/// Runs the service manager's commands; a test answers them from a script.
pub trait Runner {
    /// Run `program` with `args`; the exit status and the combined output.
    fn run(&mut self, program: &str, args: &[String]) -> Result<(i32, String), String>;
}

/// The real service manager.
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&mut self, program: &str, args: &[String]) -> Result<(i32, String), String> {
        let out = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok((out.status.code().unwrap_or(-1), text))
    }
}

/// The user's launchd domain.
fn gui_domain() -> String {
    // SAFETY: getuid has no preconditions and cannot fail.
    format!("gui/{}", unsafe { libc::getuid() })
}

/// What `install` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InstallReport {
    /// The service manager.
    pub manager: Manager,
    /// The unit's label.
    pub label: String,
    /// Where the unit was written.
    pub path: String,
    /// Whether the file on disk changed; a second install of the same unit changes nothing.
    pub changed: bool,
    /// The service manager commands that were run, in order.
    pub ran: Vec<String>,
}

/// Write `unit` for `manager` under `home` and have the manager run it. A unit already on
/// disk with the same text is left alone and, when the manager already runs it, nothing
/// is asked of the manager either.
pub fn install(
    unit: &Unit,
    manager: Manager,
    home: &Path,
    runner: &mut dyn Runner,
) -> Result<InstallReport, String> {
    let path = unit.path_under(manager, home);
    let text = unit.render(manager);
    let changed = std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str());
    if changed {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(super::at(dir))?;
        }
        if let Some(dir) = unit.log.parent() {
            std::fs::create_dir_all(dir).map_err(super::at(dir))?;
        }
        std::fs::write(&path, &text).map_err(super::at(&path))?;
    }
    let mut ran = Vec::new();
    let mut ask = |program: &str, args: Vec<String>, ran: &mut Vec<String>| {
        ran.push(format!("{program} {}", args.join(" ")));
        runner.run(program, &args)
    };
    match manager {
        Manager::Launchd => {
            let target = format!("{}/{}", gui_domain(), unit.label);
            let (loaded, _) = ask("launchctl", vec!["print".into(), target.clone()], &mut ran)?;
            if loaded == 0 && changed {
                // a loaded agent keeps the definition it was loaded with until it is booted out
                ask(
                    "launchctl",
                    vec!["bootout".into(), target.clone()],
                    &mut ran,
                )?;
            }
            if loaded != 0 || changed {
                let (code, out) = ask(
                    "launchctl",
                    vec!["bootstrap".into(), gui_domain(), path.display().to_string()],
                    &mut ran,
                )?;
                if code != 0 {
                    return Err(format!(
                        "launchctl bootstrap {}: {}",
                        path.display(),
                        out.trim()
                    ));
                }
            }
        }
        Manager::SystemdUser => {
            let name = format!("{}.service", unit.label);
            if changed {
                ask(
                    "systemctl",
                    vec!["--user".into(), "daemon-reload".into()],
                    &mut ran,
                )?;
            }
            let (code, out) = ask(
                "systemctl",
                vec![
                    "--user".into(),
                    "enable".into(),
                    "--now".into(),
                    name.clone(),
                ],
                &mut ran,
            )?;
            if code != 0 {
                return Err(format!(
                    "systemctl --user enable --now {name}: {}",
                    out.trim()
                ));
            }
        }
    }
    Ok(InstallReport {
        manager,
        label: unit.label.clone(),
        path: path.display().to_string(),
        changed,
        ran,
    })
}

/// Stop the unit and remove it; nothing is left behind. Uninstalling what is not installed
/// is not an error, and asks the service manager nothing: there is no unit to stop.
pub fn uninstall(
    label: &str,
    manager: Manager,
    home: &Path,
    runner: &mut dyn Runner,
) -> Result<Option<String>, String> {
    let probe = Unit {
        label: label.to_string(),
        description: String::new(),
        program: Vec::new(),
        working_directory: PathBuf::new(),
        path: String::new(),
        log: PathBuf::new(),
    };
    let path = probe.path_under(manager, home);
    if !path.exists() {
        return Ok(None);
    }
    match manager {
        Manager::Launchd => {
            runner.run(
                "launchctl",
                &["bootout".into(), format!("{}/{label}", gui_domain())],
            )?;
        }
        Manager::SystemdUser => {
            runner.run(
                "systemctl",
                &[
                    "--user".into(),
                    "disable".into(),
                    "--now".into(),
                    format!("{label}.service"),
                ],
            )?;
        }
    }
    std::fs::remove_file(&path).map_err(super::at(&path))?;
    if manager == Manager::SystemdUser {
        runner.run("systemctl", &["--user".into(), "daemon-reload".into()])?;
    }
    Ok(Some(path.display().to_string()))
}

/// The executor service of one checkout, as `prs service status` says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceStatus {
    /// The service manager, `None` on a platform with neither.
    pub manager: Option<Manager>,
    /// The unit's label.
    pub label: String,
    /// Where the unit is, when it is installed.
    pub installed: Option<String>,
    /// Whether the manager runs it now.
    pub running: bool,
    /// The running process, when one.
    pub pid: Option<u32>,
    /// What the manager said, one line.
    pub manager_says: Option<String>,
    /// The last decision a run left.
    pub last_run: Option<LastRun>,
    /// What to do next, when anything.
    pub next: Option<String>,
}

/// Read a `key = value` line out of `launchctl print`.
fn launchctl_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().map(str::trim).find_map(|l| {
        l.strip_prefix(key)?
            .trim_start()
            .strip_prefix('=')
            .map(str::trim)
    })
}

/// Read a `Key=value` line out of `systemctl show`.
fn systemd_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix('='))
}

/// The service of the checkout at `root`.
pub fn status(root: &Path, home: &Path, runner: &mut dyn Runner) -> ServiceStatus {
    let label = label_for(root);
    let last_run = last_run(root);
    let Some(manager) = Manager::of_this_platform() else {
        return ServiceStatus {
            manager: None,
            label,
            installed: None,
            running: false,
            pid: None,
            manager_says: Some("no supported service manager on this platform".into()),
            last_run,
            next: None,
        };
    };
    let probe = Unit {
        label: label.clone(),
        description: String::new(),
        program: Vec::new(),
        working_directory: PathBuf::new(),
        path: String::new(),
        log: PathBuf::new(),
    };
    let path = probe.path_under(manager, home);
    let installed = path.exists().then(|| path.display().to_string());
    let (running, pid, says) = match manager {
        Manager::Launchd => {
            match runner.run(
                "launchctl",
                &["print".into(), format!("{}/{label}", gui_domain())],
            ) {
                Ok((0, text)) => {
                    let state = launchctl_field(&text, "state")
                        .unwrap_or("unknown")
                        .to_string();
                    let pid = launchctl_field(&text, "pid").and_then(|p| p.parse().ok());
                    let exit = launchctl_field(&text, "last exit code").unwrap_or("-");
                    (
                        state == "running",
                        pid,
                        Some(format!("state {state}, last exit {exit}")),
                    )
                }
                Ok(_) => (false, None, Some("not loaded".into())),
                Err(e) => (false, None, Some(e)),
            }
        }
        Manager::SystemdUser => {
            let args = [
                "--user".to_string(),
                "show".into(),
                "-p".into(),
                "ActiveState,MainPID,ExecMainStatus".into(),
                format!("{label}.service"),
            ];
            match runner.run("systemctl", &args) {
                Ok((0, text)) => {
                    let state = systemd_field(&text, "ActiveState")
                        .unwrap_or("unknown")
                        .to_string();
                    let pid = systemd_field(&text, "MainPID")
                        .and_then(|p| p.parse().ok())
                        .filter(|p| *p != 0);
                    let exit = systemd_field(&text, "ExecMainStatus").unwrap_or("-");
                    (
                        state == "active",
                        pid,
                        Some(format!("{state}, last exit {exit}")),
                    )
                }
                Ok((_, text)) => (false, None, Some(text.trim().to_string())),
                Err(e) => (false, None, Some(e)),
            }
        }
    };
    let next = if installed.is_none() {
        Some("majordomus prs service install".into())
    } else if !running
        && last_run
            .as_ref()
            .is_some_and(|l| l.disposition == Disposition::Stop)
    {
        Some(match manager {
            Manager::Launchd => format!(
                "a person looks at why it stopped; then `majordomus prs drain --resume-after-failure --max 1` \
                 if a merge could not be verified, and `launchctl kickstart {}/{label}`",
                gui_domain()
            ),
            Manager::SystemdUser => format!(
                "a person looks at why it stopped; then `majordomus prs drain --resume-after-failure --max 1` \
                 if a merge could not be verified, and `systemctl --user restart {label}.service`"
            ),
        })
    } else {
        None
    };
    ServiceStatus {
        manager: Some(manager),
        label,
        installed,
        running,
        pid,
        manager_says: says,
        last_run,
        next,
    }
}

/// The user's home directory, which the service manager reads its units from.
pub fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set: the user's service directory cannot be found".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(stopped: &str, failure: Option<&str>) -> ContinuousReport {
        ContinuousReport {
            cycles: 2,
            merged: vec![7],
            stopped: stopped.into(),
            failure: failure.map(Into::into),
        }
    }

    /// The acceptance case of I3913, proven without a service manager: what ends a run decides
    /// whether the manager starts it again, and a halt never does.
    #[test]
    fn a_halted_executor_is_not_restarted() {
        let halted = report("halted: the trail says a person looks first", None);
        assert_eq!(disposition(&Ended::Drained(&halted)), Disposition::Stop);
        let unverified = report("#7 could not be verified after merging (x)", None);
        assert_eq!(disposition(&Ended::Drained(&unverified)), Disposition::Stop);
        let asked = report("asked to stop; the step in progress finished first", None);
        assert_eq!(disposition(&Ended::Drained(&asked)), Disposition::Stop);
        assert_eq!(
            disposition(&Ended::RolloutRefused("needs 5 verified merges")),
            Disposition::Stop
        );
        assert_eq!(Disposition::Stop.exit_code(), 0);
    }

    #[test]
    fn an_outage_a_held_lease_and_an_unwritable_trail_are_restarted() {
        let outage = report("the repository could not be read", Some("timeout"));
        assert_eq!(disposition(&Ended::Drained(&outage)), Disposition::Restart);
        assert_eq!(
            disposition(&Ended::LeaseUnavailable("held by session x")),
            Disposition::Restart
        );
        assert_eq!(Disposition::Restart.exit_code(), RESTART_EXIT);
        assert_ne!(RESTART_EXIT, 0, "the manager restarts only a non-zero exit");
    }

    fn unit() -> Unit {
        executor_unit(
            Path::new("/r/repo"),
            &ExecutorOptions {
                executable: PathBuf::from("/home/u/.local/bin/majordomus"),
                interval: 60,
                max: 3,
                refresh: true,
            },
            "/usr/bin:/bin".into(),
        )
    }

    #[test]
    fn the_launch_agent_restarts_only_a_failure_and_is_throttled() {
        let p = unit().launchd_plist();
        assert!(p.contains("<key>SuccessfulExit</key>\n    <false/>"), "{p}");
        assert!(p.contains("<key>RunAtLoad</key>\n  <true/>"));
        assert!(p.contains(&format!("<integer>{THROTTLE_SECONDS}</integer>")));
        for word in [
            "--repo",
            "/r/repo",
            "service",
            "run",
            "--interval",
            "60",
            "--refresh",
        ] {
            assert!(
                p.contains(&format!("<string>{word}</string>")),
                "{word} in {p}"
            );
        }
        assert!(
            !p.contains("MAJORDOMUS_SHARE"),
            "a unit must not pin another share"
        );
    }

    #[test]
    fn the_systemd_unit_restarts_only_a_failure_and_never_gives_up() {
        let s = unit().systemd_unit();
        assert!(s.contains("Restart=on-failure"));
        assert!(s.contains(&format!("RestartSec={THROTTLE_SECONDS}")));
        assert!(
            s.contains("StartLimitIntervalSec=0"),
            "an outage of any length is waited out"
        );
        assert!(s.contains("WantedBy=default.target"));
        assert!(
            s.contains("ExecStart=/home/u/.local/bin/majordomus prs --repo /r/repo service run")
        );
    }

    #[test]
    fn a_word_systemd_would_split_is_quoted_and_a_plist_string_is_escaped() {
        assert_eq!(systemd_word("/a b/c"), "\"/a b/c\"");
        assert_eq!(systemd_word("/plain/path"), "/plain/path");
        assert_eq!(xml("a<b&\"c\">"), "a&lt;b&amp;&quot;c&quot;&gt;");
    }

    #[test]
    fn one_checkout_has_one_stable_label() {
        let a = label_for(Path::new("/nonexistent/one"));
        assert_eq!(a, label_for(Path::new("/nonexistent/one")));
        assert_ne!(a, label_for(Path::new("/nonexistent/two")));
        assert!(a.starts_with("dev.majordomus.prs-drain."));
    }

    /// Answers every command from a script and remembers what it was asked.
    struct Scripted {
        asked: Vec<String>,
        print_loaded: bool,
    }

    impl Runner for Scripted {
        fn run(&mut self, program: &str, args: &[String]) -> Result<(i32, String), String> {
            self.asked.push(format!("{program} {}", args.join(" ")));
            if program == "launchctl" && args.first().map(String::as_str) == Some("print") {
                return Ok(if self.print_loaded {
                    (
                        0,
                        "\tstate = running\n\tpid = 42\n\tlast exit code = (never exited)\n".into(),
                    )
                } else {
                    (113, "Could not find service".into())
                });
            }
            Ok((0, String::new()))
        }
    }

    #[test]
    fn a_second_install_changes_nothing_and_uninstall_leaves_nothing() {
        let home = tempfile::tempdir().unwrap();
        let mut r = Scripted {
            asked: Vec::new(),
            print_loaded: false,
        };
        let u = Unit {
            log: home.path().join("logs/service.log"),
            ..unit()
        };
        let first = install(&u, Manager::Launchd, home.path(), &mut r).unwrap();
        assert!(first.changed);
        assert!(first
            .ran
            .iter()
            .any(|c| c.starts_with("launchctl bootstrap")));
        assert!(Path::new(&first.path).exists());

        let mut r = Scripted {
            asked: Vec::new(),
            print_loaded: true,
        };
        let second = install(&u, Manager::Launchd, home.path(), &mut r).unwrap();
        assert!(!second.changed);
        assert_eq!(
            second.ran,
            vec![format!("launchctl print {}/{}", gui_domain(), u.label)]
        );

        let removed = uninstall(&u.label, Manager::Launchd, home.path(), &mut r).unwrap();
        assert_eq!(removed.as_deref(), Some(first.path.as_str()));
        assert!(!Path::new(&first.path).exists());
        let asked = r.asked.len();
        assert_eq!(
            uninstall(&u.label, Manager::Launchd, home.path(), &mut r).unwrap(),
            None
        );
        assert_eq!(
            r.asked.len(),
            asked,
            "nothing installed: the manager is asked nothing"
        );
    }

    #[test]
    fn a_systemd_install_reloads_only_when_the_unit_changed() {
        let home = tempfile::tempdir().unwrap();
        let u = Unit {
            log: home.path().join("logs/service.log"),
            ..unit()
        };
        let mut r = Scripted {
            asked: Vec::new(),
            print_loaded: false,
        };
        let first = install(&u, Manager::SystemdUser, home.path(), &mut r).unwrap();
        assert!(first.ran[0].ends_with("daemon-reload"));
        let second = install(&u, Manager::SystemdUser, home.path(), &mut r).unwrap();
        assert!(!second.changed);
        assert!(second.ran.iter().all(|c| !c.ends_with("daemon-reload")));
    }

    #[test]
    fn a_last_run_names_its_disposition_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let outage = report("the repository could not be read", Some("timeout"));
        let last = LastRun::of(&Ended::Drained(&outage), "2026-10-28T17:00:00Z".into());
        assert_eq!(last.disposition, Disposition::Restart);
        record_last_run(dir.path(), &last).unwrap();
        assert_eq!(last_run(dir.path()), Some(last));
    }

    #[test]
    fn launchctl_and_systemctl_fields_are_read() {
        let t = "\tstate = running\n\tpid = 42\n";
        assert_eq!(launchctl_field(t, "state"), Some("running"));
        assert_eq!(launchctl_field(t, "pid"), Some("42"));
        let s = "ActiveState=active\nMainPID=7\n";
        assert_eq!(systemd_field(s, "MainPID"), Some("7"));
    }
}
