//! Every branch of the hub service, against a host whose service manager, network and release
//! origin are a script.

use std::cell::RefCell;

use super::*;

/// A scripted host: answers by command prefix, records every command, and serves a hub whose
/// version a restart may change.
struct Fake {
    calls: RefCell<Vec<String>>,
    answers: RefCell<Vec<(String, bool, String)>>,
    served: RefCell<Option<String>>,
    after_restart: RefCell<Vec<Option<String>>>,
    installed: RefCell<String>,
    attached: u64,
    linked: Vec<(&'static str, &'static str)>,
}

impl Fake {
    fn new() -> Self {
        Fake {
            calls: RefCell::new(Vec::new()),
            answers: RefCell::new(Vec::new()),
            served: RefCell::new(None),
            after_restart: RefCell::new(Vec::new()),
            installed: RefCell::new(String::new()),
            attached: 0,
            linked: vec![("25c9758f", "1.3.0")],
        }
    }

    fn answer(&self, prefix: &str, ok: bool, out: &str) -> &Self {
        self.answers
            .borrow_mut()
            .push((prefix.to_string(), ok, out.to_string()));
        self
    }

    fn serving(&self, v: Option<&str>) -> &Self {
        *self.served.borrow_mut() = v.map(str::to_string);
        self
    }

    /// The executable answers `--version` with this, and an installer run for a version
    /// makes it that version.
    fn installed(&self, v: &str) -> &Self {
        *self.installed.borrow_mut() = v.to_string();
        self
    }

    fn restarts_onto(&self, versions: &[Option<&str>]) -> &Self {
        *self.after_restart.borrow_mut() = versions.iter().map(|v| v.map(str::to_string)).collect();
        self
    }

    fn exec(&self, argv: &[String]) -> (bool, String) {
        let line = argv.join(" ");
        self.calls.borrow_mut().push(line.clone());
        let scripted = self
            .answers
            .borrow()
            .iter()
            .find(|(prefix, _, _)| line.starts_with(prefix.as_str()))
            .map(|(_, ok, out)| (*ok, out.clone()));
        if let Some(answer) = scripted {
            return answer;
        }
        if line.contains(" kickstart -k ") || line.contains("restart majordomus-hub.service") {
            let mut next = self.after_restart.borrow_mut();
            if !next.is_empty() {
                *self.served.borrow_mut() = next.remove(0);
            }
        }
        if line.starts_with("env ") {
            if let Some(i) = argv.iter().position(|a| a == "--version") {
                *self.installed.borrow_mut() = argv[i + 1].clone();
            }
        }
        if line.ends_with(" --version") && !self.installed.borrow().is_empty() {
            return (true, format!("majordomus {}\n", self.installed.borrow()));
        }
        (true, String::new())
    }

    fn get(&self, _base: &str, path: &str) -> Option<String> {
        let served = self.served.borrow().clone()?;
        Some(match path {
            "/" => format!("{{\"version\":\"{served}\"}}"),
            "/api/v1/peers" => format!(
                "{{\"boards\":[{{\"this_checkout\":false,\"attached\":5}},\
                 {{\"this_checkout\":true,\"attached\":{}}}]}}",
                self.attached
            ),
            _ => {
                let peers: Vec<String> = self
                    .linked
                    .iter()
                    .map(|(m, v)| {
                        format!("{{\"name\":\"{m}\",\"runtime\":\"{m}-r\",\"version\":\"{v}\",\"state\":\"connected\"}}")
                    })
                    .chain(std::iter::once(
                        "{\"name\":\"gone\",\"version\":\"0.1.0\",\"state\":\"expired\"}".to_string(),
                    ))
                    .collect();
                format!("{{\"peers\":[{}]}}", peers.join(","))
            }
        })
    }

    fn called(&self, needle: &str) -> bool {
        self.calls.borrow().iter().any(|c| c.contains(needle))
    }
}

macro_rules! with_fake {
    ($fake:ident, $manager:expr, $staging:expr, $h:ident) => {
        let exec = |a: &[String]| $fake.exec(a);
        let get = |b: &str, p: &str| $fake.get(b, p);
        let $h = Host {
            manager: $manager,
            uid: 501,
            user: "owner".into(),
            pid: 4242,
            staging: $staging,
            exec: &exec,
            get: &get,
            alive: &|pid| pid == 7,
            now: &|| 1_000_000,
            sleep: &|_| {},
        };
    };
}

/// A spec under `home` whose executable is a launcher there.
fn spec(manager: Manager, home: &Path) -> Spec {
    let executable = home.join(".local/bin/majordomus");
    write_file(
        &executable,
        &format!("#!/bin/sh\n# {LAUNCHER_MARK}. Reinstalling rewrites it.\n"),
    )
    .unwrap();
    Spec {
        manager,
        home: home.to_path_buf(),
        executable,
        repo: PathBuf::from("/srv/mj/hub repo"),
        port: 8791,
    }
}

const GIT: &str = "git -C /srv/mj/hub repo";

#[test]
fn the_manager_follows_the_platform() {
    assert_eq!(Manager::of("macos"), Manager::Launchd);
    assert_eq!(Manager::of("linux"), Manager::SystemdUser);
    assert_eq!(Manager::of("windows"), Manager::Unsupported);
}

#[test]
fn launchd_renders_a_kept_alive_hub_and_an_hourly_update() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let units = render(&s);
    assert_eq!(units.len(), 2);
    let hub = &units[0];
    assert_eq!(
        hub.path,
        home.path()
            .join("Library/LaunchAgents/dev.majordomus.hub.plist")
    );
    assert!(hub.content.contains("<key>KeepAlive</key>\n  <true/>"));
    assert!(hub.content.contains("<string>serve</string>"));
    assert!(hub.content.contains("<string>/srv/mj/hub repo</string>"));
    assert!(hub
        .content
        .contains("<string>--idle</string>\n    <string>0</string>"));
    assert!(hub.content.contains("Library/Logs/majordomus-hub.log"));
    assert!(hub.content.contains(WRITTEN_BY));
    assert!(
        !hub.content.contains("Background"),
        "a hub is not throttled as background work"
    );
    let update = &units[1];
    assert_eq!(update.role, UnitRole::Update);
    assert!(update.content.contains("<integer>3600</integer>"));
    assert!(update.content.contains("<string>--apply</string>"));
    assert!(update.content.contains("<string>Background</string>"));
    assert!(update.content.contains("majordomus-hub-update.log"));
    assert!(!update.content.contains("KeepAlive"));
    assert_eq!(xml("a<b>&\"c"), "a&lt;b&gt;&amp;&quot;c");
}

#[test]
fn systemd_renders_a_hub_that_never_gives_up_and_an_hourly_timer() {
    let home = tempfile::tempdir().unwrap();
    let mut s = spec(Manager::SystemdUser, home.path());
    s.repo = PathBuf::from("/srv/mj/hub 100%");
    let units = render(&s);
    assert_eq!(
        units.iter().map(|u| u.name).collect::<Vec<_>>(),
        [HUB_UNIT, UPDATE_UNIT, UPDATE_TIMER]
    );
    let hub = &units[0].content;
    assert!(hub.contains("Restart=always") && hub.contains("StartLimitIntervalSec=0"));
    assert!(hub.contains("serve --repo \"/srv/mj/hub 100%%\""), "{hub}");
    assert!(
        hub.contains("WorkingDirectory=/srv/mj/hub 100%%\n"),
        "{hub}"
    );
    assert!(hub.contains("Description=Majordomus mesh hub of /srv/mj/hub 100%%\n"));
    assert!(hub.contains("Environment=\"PATH="));
    assert!(
        !hub.contains("network-online"),
        "the user manager has no network-online.target"
    );
    assert!(hub.contains("WantedBy=default.target"));
    assert!(units[1].content.contains("Type=oneshot"));
    assert!(units[1].content.contains("mesh hub update --apply"));
    assert!(
        units[2].content.contains("OnCalendar=hourly")
            && units[2].content.contains("Persistent=true")
    );
    assert!(render(&spec(Manager::Unsupported, home.path())).is_empty());
    assert_eq!(systemd_word("50%$x"), "50%%$$x");
    assert_eq!(systemd_word(""), "\"\"");
    assert_eq!(systemd_word("a\"b\\"), "\"a\\\"b\\\\\"");
}

#[test]
fn the_managers_words_are_read_as_they_are_written() {
    assert!(launchd_running("x\n\tstate = running\n"));
    assert!(!launchd_running("state = waiting"));
    let disabled = "disabled services = {\n\t\"dev.majordomus.hub\" => disabled\n\t\"other\" => enabled\n\t\"old\" => true\n}";
    assert!(launchd_disabled(disabled, "dev.majordomus.hub"));
    assert!(launchd_disabled(disabled, "old"));
    assert!(!launchd_disabled(disabled, "other"));
    assert!(!launchd_disabled(disabled, "dev.majordomus.hub-update"));
    for on in ["enabled\n", "static", "enabled-runtime", "linked"] {
        assert!(systemd_enabled(on), "{on}");
    }
    for off in ["disabled", "masked", ""] {
        assert!(!systemd_enabled(off), "{off}");
    }
}

#[test]
fn only_the_installers_launcher_counts_as_one() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    assert!(is_launcher(&s.executable));
    let other = home.path().join("majordomus-cli");
    std::fs::write(&other, "\x7fELF...").unwrap();
    assert!(!is_launcher(&other));
    assert!(!is_launcher(&home.path().join("absent")));
}

#[test]
fn install_on_launchd_writes_loads_and_a_second_install_does_nothing() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let fake = Fake::new();
    fake.answer(GIT, false, "")
        .answer("launchctl print", false, "");
    with_fake!(fake, Manager::Launchd, false, h);
    let planned = install(&h, &s, true, false, false);
    assert!(planned.ok && planned.mode == "plan");
    assert!(planned.steps.iter().all(|st| !st.ran));
    assert!(planned.units.iter().all(|u| u.state == FileState::Missing));
    assert!(!home.path().join("Library").exists());

    let applied = install(&h, &s, true, false, true);
    assert!(applied.ok, "{applied:?}");
    assert!(fake.called("launchctl bootstrap gui/501"));
    assert!(!fake.called("launchctl enable"), "nothing was disabled");
    assert!(applied.notes.iter().any(|n| n.contains("firewall")));
    assert!(home
        .path()
        .join("Library/LaunchAgents/dev.majordomus.hub.plist")
        .exists());

    // now loaded and running, and every file current: nothing is owed
    let fake2 = Fake::new();
    fake2
        .answer(GIT, false, "")
        .answer("launchctl print gui", true, "\tstate = running\n");
    with_fake!(fake2, Manager::Launchd, false, h2);
    let again = install(&h2, &s, true, false, true);
    assert!(again.ok && again.steps.is_empty(), "{:?}", again.steps);
    assert!(again
        .units
        .iter()
        .all(|u| u.state == FileState::Current && u.loaded && u.enabled && u.active));
}

#[test]
fn a_disabled_agent_is_enabled_and_a_held_back_hub_is_kicked() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    for r in render(&s) {
        write_file(&r.path, &r.content).unwrap();
    }
    let fake = Fake::new();
    fake.answer(GIT, false, "")
        .answer(
            "launchctl print-disabled",
            true,
            "\"dev.majordomus.hub\" => disabled\n",
        )
        .answer("launchctl print gui", true, "state = waiting");
    with_fake!(fake, Manager::Launchd, false, h);
    let r = install(&h, &s, true, false, true);
    assert!(r.ok, "{r:?}");
    assert!(fake.called("launchctl enable gui/501/dev.majordomus.hub"));
    assert!(fake.called("launchctl kickstart gui/501/dev.majordomus.hub"));
    assert!(!fake.called("bootstrap"));
    assert!(!fake.called("kickstart gui/501/dev.majordomus.hub-update"));
}

#[test]
fn a_drifted_loaded_agent_is_booted_out_and_bootstrapped_until_launchd_lets_it() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let hub = &render(&s)[0];
    write_file(&hub.path, "hand made").unwrap();
    let fake = Fake::new();
    fake.answer(GIT, false, "")
        .answer("launchctl print gui", true, "state = running");
    // the first two bootstraps meet a job launchd has not let go of yet
    let tries = RefCell::new(0);
    let exec = |a: &[String]| {
        if a.get(1).map(String::as_str) == Some("bootstrap") {
            *tries.borrow_mut() += 1;
            fake.calls.borrow_mut().push(a.join(" "));
            return (*tries.borrow() > 2, "5: Input/output error".into());
        }
        fake.exec(a)
    };
    let get = |b: &str, p: &str| fake.get(b, p);
    let h = Host {
        manager: Manager::Launchd,
        uid: 501,
        user: "o".into(),
        pid: 1,
        staging: false,
        exec: &exec,
        get: &get,
        alive: &|_| false,
        now: &|| 0,
        sleep: &|_| {},
    };
    let r = install(&h, &s, true, false, true);
    assert!(r.ok, "{r:?}");
    assert_eq!(r.units[0].state, FileState::Drifted);
    assert!(fake.called("launchctl bootout gui/501/dev.majordomus.hub"));
    // three for the hub, then one for the update agent, which was missing
    assert_eq!(*tries.borrow(), 4);
    assert_eq!(std::fs::read_to_string(&hub.path).unwrap(), hub.content);
}

#[test]
fn install_on_systemd_reloads_enables_and_starts_and_notes_lingering() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::SystemdUser, home.path());
    let fake = Fake::new();
    fake.answer(GIT, false, "")
        .answer("systemctl --user is-enabled", false, "disabled\n")
        .answer("systemctl --user is-active", false, "inactive\n")
        .answer("loginctl", true, "no\n");
    with_fake!(fake, Manager::SystemdUser, false, h);
    let r = install(&h, &s, true, false, true);
    assert!(r.ok, "{r:?}");
    for c in [
        "systemctl --user daemon-reload",
        "systemctl --user enable majordomus-hub.service",
        "systemctl --user start majordomus-hub.service",
        "systemctl --user enable majordomus-hub-update.timer",
        "systemctl --user start majordomus-hub-update.timer",
    ] {
        assert!(fake.called(c), "{c} was not run: {:?}", fake.calls.borrow());
    }
    assert!(!fake.called("enable majordomus-hub-update.service"));
    assert!(r.notes.iter().any(|n| n.contains("enable-linger owner")));

    // the update service is static, and that is loaded: a second install owes nothing
    let fake2 = Fake::new();
    fake2
        .answer(GIT, false, "")
        .answer(
            "systemctl --user is-enabled majordomus-hub-update.service",
            true,
            "static\n",
        )
        .answer("systemctl --user is-enabled", true, "enabled\n")
        .answer(
            "systemctl --user is-active majordomus-hub-update.service",
            false,
            "inactive\n",
        )
        .answer("systemctl --user is-active", true, "active\n")
        .answer("loginctl", true, "yes\n");
    with_fake!(fake2, Manager::SystemdUser, false, h2);
    let again = install(&h2, &s, true, false, true);
    assert!(again.ok && again.steps.is_empty(), "{:?}", again.steps);
    assert!(again.units.iter().all(|u| u.loaded && u.enabled));

    // a drifted unit of an active, enabled hub is restarted, not started
    std::fs::write(
        home.path().join(".config/systemd/user").join(HUB_UNIT),
        "old",
    )
    .unwrap();
    let r2 = install(&h2, &s, true, false, true);
    assert!(r2.ok && r2.notes.len() == 1);
    assert!(fake2.called("systemctl --user restart majordomus-hub.service"));
    assert!(!fake2.called("systemctl --user enable"));
}

#[test]
fn a_failed_step_stops_the_rest_and_fails_the_install() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let fake = Fake::new();
    fake.answer(GIT, false, "")
        .answer(
            "launchctl print-disabled",
            true,
            "\"dev.majordomus.hub\" => disabled",
        )
        .answer("launchctl print gui", false, "")
        .answer("launchctl enable", false, "denied");
    with_fake!(fake, Manager::Launchd, false, h);
    let r = install(&h, &s, true, false, true);
    assert!(!r.ok);
    let failed = r.steps.iter().position(|st| st.ok == Some(false)).unwrap();
    assert_eq!(r.steps[failed].output, "denied");
    assert!(r.steps[failed + 1..].iter().all(|st| !st.ran));
    assert!(render_text(&serde_json::to_value(&r).unwrap()).contains("FAILED"));
}

#[test]
fn install_refuses_what_must_not_be_a_hub() {
    let home = tempfile::tempdir().unwrap();
    let fake = Fake::new();
    with_fake!(fake, Manager::Launchd, false, h);
    let r = install(
        &h,
        &spec(Manager::Unsupported, home.path()),
        true,
        false,
        true,
    );
    assert!(
        !r.ok
            && r.refusal
                .as_deref()
                .unwrap()
                .contains("neither launchd nor systemd")
    );
    let r = install(&h, &spec(Manager::Launchd, home.path()), false, false, true);
    assert!(r.refusal.as_deref().unwrap().contains("not a declared hub"));
    let mut s = spec(Manager::Launchd, home.path());
    s.executable = home.path().join("target/debug/majordomus");
    let r = install(&h, &s, true, false, true);
    assert!(r
        .refusal
        .as_deref()
        .unwrap()
        .contains("not the installer's launcher"));
    // named on purpose, it is the operator's choice; and `git symbolic-ref` succeeds: a branch
    let r = install(&h, &s, true, true, true);
    assert!(r.refusal.as_deref().unwrap().contains("is on a branch"));
    assert!(r.units.is_empty() && r.steps.is_empty());
    assert!(render_text(&serde_json::to_value(&r).unwrap()).contains("refused"));
}

#[test]
fn a_staging_root_writes_files_and_runs_nothing() {
    let home = tempfile::tempdir().unwrap();
    let fake = Fake::new();
    fake.answer(GIT, false, "");
    with_fake!(fake, Manager::SystemdUser, true, h);
    let s = spec(Manager::SystemdUser, home.path());
    let r = install(&h, &s, true, false, true);
    assert!(r.ok && r.mode == "staging" && r.notes.is_empty());
    assert!(home
        .path()
        .join(".config/systemd/user")
        .join(UPDATE_TIMER)
        .exists());
    assert!(r
        .steps
        .iter()
        .filter(|st| st.line.starts_with("systemctl"))
        .all(|st| !st.ran));
    assert!(!fake.called("systemctl"));
    let gone = uninstall(&h, &s, true);
    assert!(gone.ok);
    assert!(!home
        .path()
        .join(".config/systemd/user")
        .join(HUB_UNIT)
        .exists());
}

#[test]
fn uninstall_stops_and_removes_what_install_wrote() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    for r in render(&s) {
        write_file(&r.path, &r.content).unwrap();
    }
    let fake = Fake::new();
    fake.answer("launchctl print gui", true, "state = running");
    with_fake!(fake, Manager::Launchd, false, h);
    let planned = uninstall(&h, &s, false);
    assert!(planned.ok && planned.steps.len() == 4 && planned.steps.iter().all(|st| !st.ran));
    let r = uninstall(&h, &s, true);
    assert!(r.ok);
    assert!(fake.called("launchctl bootout gui/501/dev.majordomus.hub-update"));
    assert!(render(&s).iter().all(|u| !u.path.exists()));

    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::SystemdUser, home.path());
    for r in render(&s) {
        write_file(&r.path, &r.content).unwrap();
    }
    let fake = Fake::new();
    fake.answer("systemctl --user is-enabled", true, "enabled")
        .answer("systemctl --user is-active", true, "active");
    with_fake!(fake, Manager::SystemdUser, false, h);
    assert!(uninstall(&h, &s, true).ok);
    assert!(fake.called("systemctl --user disable --now majordomus-hub.service"));
    assert!(!fake.called("disable --now majordomus-hub-update.service"));
    assert!(fake.called("systemctl --user daemon-reload"));
    // removing a file that is already gone holds
    let (steps, held) = carry_out(&h, vec![Act::Remove(home.path().join("none"))], true);
    assert!(held && steps[0].ok == Some(true));
    // a file that cannot be removed or written fails its step
    let dir = home.path().join("a-dir");
    std::fs::create_dir_all(dir.join("x")).unwrap();
    let (_, held) = carry_out(&h, vec![Act::Remove(dir.clone())], true);
    assert!(!held);
    let (_, held) = carry_out(&h, vec![Act::Write(dir, "x".into())], true);
    assert!(!held);
    // a retried command that never holds fails after its last try
    let fake = Fake::new();
    fake.answer("no", false, "never");
    with_fake!(fake, Manager::Launchd, false, h);
    let (steps, held) = carry_out(&h, vec![Act::Retry(vec!["no".into()], 3)], true);
    assert!(!held && steps[0].output == "never");
    assert_eq!(fake.calls.borrow().len(), 3);
}

fn v<'a>(
    published: Option<&'a str>,
    installed: Option<&'a str>,
    served: Option<&'a str>,
    checkout: Option<&'a str>,
) -> Versions<'a> {
    Versions {
        published,
        installed,
        served,
        checkout,
        attached: 0,
        deferred_too_long: false,
    }
}

#[test]
fn the_decision_is_a_function_of_the_versions() {
    let none: Vec<String> = Vec::new();
    let failed = vec!["1.4.0".to_string()];
    let at = Some("1.3.0");
    assert!(matches!(
        decide(&v(at, None, at, at), &none),
        Decision::Unknown { .. }
    ));
    assert_eq!(
        decide(&v(at, Some("1.2.0"), Some("1.2.0"), Some("1.2.0")), &none),
        Decision::Install {
            version: "1.3.0".into()
        }
    );
    assert_eq!(
        decide(&v(Some("1.4.0"), at, at, at), &failed),
        Decision::Failed {
            version: "1.4.0".into()
        }
    );
    assert!(matches!(
        decide(&v(at, at, None, at), &none),
        Decision::Unknown { .. }
    ));
    assert_eq!(decide(&v(at, at, at, at), &none), Decision::Current);
    assert_eq!(decide(&v(None, at, at, at), &none), Decision::Current);
    // the hub restarted onto the new executable on its old checkout: the checkout moves
    assert_eq!(
        decide(&v(at, at, at, Some("1.2.0")), &none),
        Decision::Activate {
            version: "1.3.0".into()
        }
    );
    // a checkout at no tag is behind, and a hub that does not answer is restarted onto it
    assert_eq!(
        decide(&v(at, at, at, None), &none),
        Decision::Activate {
            version: "1.3.0".into()
        }
    );
    assert_eq!(
        decide(&v(at, at, None, Some("1.2.0")), &none),
        Decision::Activate {
            version: "1.3.0".into()
        }
    );
    assert_eq!(
        decide(&v(None, Some("1.4.0"), at, at), &failed),
        Decision::Failed {
            version: "1.4.0".into()
        }
    );
    let mut busy = v(None, at, Some("1.2.0"), Some("1.2.0"));
    busy.attached = 2;
    assert_eq!(decide(&busy, &none), Decision::Deferred { attached: 2 });
    busy.deferred_too_long = true;
    assert_eq!(
        decide(&busy, &none),
        Decision::Activate {
            version: "1.3.0".into()
        }
    );
}

fn latest(v: &str) -> String {
    format!("{{\"version\":\"{v}\"}}")
}

/// A host on 1.2.0 everywhere with 1.3.0 published.
fn behind() -> Fake {
    let fake = Fake::new();
    fake.serving(Some("1.2.0"))
        .installed("1.2.0")
        .answer("curl -fsS -m", true, &latest("1.3.0"))
        .answer(&format!("{GIT} describe"), true, "v1.2.0\n")
        .answer(&format!("{GIT} rev-parse"), true, "abc\n");
    fake
}

#[test]
fn an_update_installs_moves_the_checkout_restarts_rerenders_and_records_it() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let fake = behind();
    fake.restarts_onto(&[Some("1.3.0")]);
    with_fake!(fake, Manager::Launchd, false, h);

    let planned = update(&h, &s, RELEASE_BASE, false, false);
    assert_eq!(
        planned.decision,
        Decision::Install {
            version: "1.3.0".into()
        }
    );
    assert!(planned.ok && planned.steps.is_empty() && planned.result == "planned");
    assert_eq!(planned.checkout.as_deref(), Some("1.2.0"));

    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert_eq!(r.result, "updated", "{r:?}");
    assert!(r.ok);
    let dir = home.path().join(".local/bin").display().to_string();
    assert!(fake.called(&format!("--version 1.3.0 --install-dir {dir}")));
    assert!(fake.called(&format!("{GIT} checkout --quiet --detach v1.3.0")));
    assert!(fake.called("launchctl kickstart -k gui/501/dev.majordomus.hub"));
    assert!(fake.called("mesh hub install --apply --port 8791 --repo /srv/mj/hub repo"));
    assert!(!fake.called("MAJORDOMUS_RELEASE_BASE_URL"));
    assert!(!fake.called("--force"));
    let state = read_state(home.path());
    assert_eq!(state.last.unwrap().result, "updated");
    assert!(!lock_path(home.path()).exists(), "the lock is let go");
    assert!(render_text(&serde_json::to_value(&r).unwrap()).contains("decision"));
}

#[test]
fn a_hub_that_answers_nothing_after_the_restart_is_rolled_back_and_its_version_remembered() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::SystemdUser, home.path());
    let fake = behind();
    fake.installed("1.3.0")
        .restarts_onto(&[None, Some("1.2.0")]);
    with_fake!(fake, Manager::SystemdUser, false, h);
    let r = update(&h, &s, "http://127.0.0.1:9", true, false);
    assert_eq!(r.result, "rolled_back", "{r:?}");
    assert!(!r.ok);
    assert!(fake.called("MAJORDOMUS_RELEASE_BASE_URL=http://127.0.0.1:9 sh"));
    assert!(fake.called("--version 1.2.0"));
    assert!(fake.called("--force"));
    assert!(fake.called(&format!("{GIT} checkout --quiet --detach abc")));
    let state = read_state(home.path());
    assert_eq!(state.failed, ["1.3.0"]);
    assert!(state.last.unwrap().detail.contains("serves 1.2.0 again"));
    // the next run skips the failed version, and --retry tries it again
    fake.installed("1.3.0");
    assert_eq!(
        update(&h, &s, "http://127.0.0.1:9", true, false).result,
        "skipped"
    );
    fake.restarts_onto(&[Some("1.3.0")]);
    assert_eq!(
        update(&h, &s, "http://127.0.0.1:9", true, true).result,
        "updated"
    );
}

#[test]
fn a_rollback_that_cannot_reinstall_keeps_the_checkout_with_the_executable() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::SystemdUser, home.path());
    let fake = behind();
    fake.installed("1.3.0").restarts_onto(&[None, None]);
    fake.answer("env sh", false, "offline");
    with_fake!(fake, Manager::SystemdUser, false, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert_eq!(r.result, "failed");
    assert!(r.detail.contains("could not be installed again"));
    assert!(
        !fake.called("--detach abc"),
        "the checkout stays at the new tag"
    );

    // with the previous release back and still no answer, the report says so
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::SystemdUser, home.path());
    let fake = behind();
    fake.installed("1.3.0").restarts_onto(&[None, None]);
    with_fake!(fake, Manager::SystemdUser, false, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert_eq!(r.result, "rolled_back");
    assert!(r.detail.contains("does not answer with the previous"));

    // and with no previous release known, there is nothing to put back
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::SystemdUser, home.path());
    let fake = behind();
    fake.installed("1.3.0").serving(None).restarts_onto(&[None]);
    with_fake!(fake, Manager::SystemdUser, false, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(r.detail.contains("no previous release"), "{r:?}");
    assert_eq!(read_state(home.path()).failed, ["1.3.0"]);
}

#[test]
fn a_restart_that_does_not_take_blames_no_release() {
    for (name, restart_ok, after, needle) in [
        ("refused", false, None, "did not restart"),
        (
            "old answer",
            true,
            Some("1.2.0"),
            "another process serves it",
        ),
    ] {
        let home = tempfile::tempdir().unwrap();
        let s = spec(Manager::Launchd, home.path());
        let fake = behind();
        fake.installed("1.3.0").restarts_onto(&[after]);
        if !restart_ok {
            fake.answer("launchctl kickstart", false, "no such service");
        }
        with_fake!(fake, Manager::Launchd, false, h);
        let r = update(&h, &s, RELEASE_BASE, true, false);
        assert_eq!(r.result, "failed", "{name}");
        assert!(r.detail.contains(needle), "{name}: {}", r.detail);
        assert!(fake.called("--detach abc"), "{name}: the checkout is back");
        assert!(read_state(home.path()).failed.is_empty(), "{name}");
    }
}

#[test]
fn an_update_refuses_what_it_cannot_do_safely() {
    let cases = [
        ("dirty", "status", true, " M x\n", "uncommitted"),
        ("no git", "rev-parse", false, "", "not a git checkout"),
        ("no tag", "checkout", false, "", "no tag v1.3.0"),
    ];
    for (name, verb, ok, out, needle) in cases {
        let home = tempfile::tempdir().unwrap();
        let s = spec(Manager::Launchd, home.path());
        let fake = Fake::new();
        fake.answer(&format!("{GIT} {verb}"), ok, out);
        fake.serving(Some("1.2.0"))
            .installed("1.3.0")
            .answer("curl -fsS -m", true, &latest("1.3.0"))
            .answer(&format!("{GIT} rev-parse"), true, "abc\n");
        with_fake!(fake, Manager::Launchd, false, h);
        let r = update(&h, &s, RELEASE_BASE, true, false);
        assert_eq!(r.result, "failed", "{name}");
        assert!(r.detail.contains(needle), "{name}: {}", r.detail);
        assert!(!fake.called("kickstart"), "{name}");
    }
}

#[test]
fn an_update_reports_current_deferred_unknown_busy_and_a_failed_installer() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());

    let fake = Fake::new();
    fake.serving(Some("1.3.0"))
        .installed("1.3.0")
        .answer("curl -fsS -m", true, &latest("1.3.0"))
        .answer(&format!("{GIT} describe"), true, "v1.3.0\n");
    with_fake!(fake, Manager::Launchd, false, h);
    assert_eq!(update(&h, &s, RELEASE_BASE, true, false).result, "current");

    // sessions hold the restart back, a day at most
    let mut fake = behind();
    fake.attached = 1;
    fake.installed("1.3.0");
    with_fake!(fake, Manager::Launchd, false, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(r.ok && r.result == "deferred");
    assert_eq!(read_state(home.path()).deferred_since, Some(1_000_000));
    let mut state = read_state(home.path());
    state.deferred_since = Some(1);
    write_state(home.path(), &state).unwrap();
    fake.restarts_onto(&[Some("1.3.0")]);
    assert_eq!(update(&h, &s, RELEASE_BASE, true, false).result, "updated");
    assert_eq!(read_state(home.path()).deferred_since, None);

    let fake = Fake::new();
    with_fake!(fake, Manager::Launchd, false, h);
    let r = update(&h, &s, RELEASE_BASE, false, false);
    assert!(!r.ok && matches!(r.decision, Decision::Unknown { .. }));
    assert_eq!(update(&h, &s, RELEASE_BASE, true, false).result, "unknown");

    // another update holds the lock; a lock whose holder is gone is taken over
    let fake = behind();
    with_fake!(fake, Manager::Launchd, false, h);
    write_file(&lock_path(home.path()), "7\n").unwrap();
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(r.ok && r.result == "busy" && r.detail.contains("pid 7"));
    write_file(&lock_path(home.path()), "8\n").unwrap();
    assert_ne!(update(&h, &s, RELEASE_BASE, true, false).result, "busy");

    for (fails, needle) in [
        ("curl -fsSL", "did not install"),
        ("env sh", "did not install"),
    ] {
        let fake = behind();
        fake.answer(fails, false, "");
        with_fake!(fake, Manager::Launchd, false, h);
        let r = update(&h, &s, RELEASE_BASE, true, false);
        assert_eq!(r.result, "failed");
        assert!(r.detail.contains(needle));
    }
    // the installer ran and the executable is still the old one
    let fake = behind();
    fake.answer("env sh", true, "");
    with_fake!(fake, Manager::Launchd, false, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(r.detail.contains("executable is 1.2.0"), "{}", r.detail);
}

#[test]
fn an_update_through_anything_but_the_launcher_is_skipped() {
    let home = tempfile::tempdir().unwrap();
    let mut s = spec(Manager::Launchd, home.path());
    s.executable = home.path().join("target/debug/majordomus");
    let fake = behind();
    with_fake!(fake, Manager::Launchd, false, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(!r.ok && r.result == "skipped" && r.detail.contains("not the installer's launcher"));
    assert!(!fake.called("curl -fsSL"));
}

#[test]
fn an_update_rotates_a_log_past_its_bound_and_says_when_it_cannot_record() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let log = s.log(UnitRole::Hub);
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    let f = std::fs::File::create(&log).unwrap();
    f.set_len(LOG_BOUND + 1).unwrap();
    let fake = Fake::new();
    fake.serving(Some("1.3.0")).installed("1.3.0").answer(
        &format!("{GIT} describe"),
        true,
        "v1.3.0\n",
    );
    with_fake!(fake, Manager::Launchd, false, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(r
        .steps
        .iter()
        .any(|st| st.line.starts_with("rotate ") && st.ok == Some(true)));
    assert_eq!(std::fs::metadata(&log).unwrap().len(), 0);
    assert_eq!(
        std::fs::metadata(log.with_extension("log.1"))
            .unwrap()
            .len(),
        LOG_BOUND + 1
    );
    assert!(rotate(&home.path().join("absent.log")).is_none());

    // the state file is a directory: the run cannot be recorded, and fails for it
    let state = state_path(home.path());
    let _ = std::fs::remove_file(&state);
    std::fs::create_dir_all(state.join("in-the-way")).unwrap();
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(
        r.steps
            .iter()
            .any(|st| st.line.starts_with("record ") && st.ok == Some(false)),
        "{r:?}"
    );
    assert!(!r.ok);
}

#[test]
fn status_checks_units_hub_versions_update_and_fleet() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    for r in render(&s) {
        write_file(&r.path, &r.content).unwrap();
    }
    let run = |at, result: &str| UpdateState {
        last: Some(UpdateRun {
            at,
            result: result.into(),
            detail: "d".into(),
        }),
        ..UpdateState::default()
    };
    write_state(home.path(), &run(999_000, "current")).unwrap();
    let mut fake = Fake::new();
    fake.linked = vec![("25c9758f", "1.3.0"), ("aaba18ea", "1.2.0")];
    fake.serving(Some("1.3.0"))
        .installed("1.3.0")
        .answer("launchctl print gui", true, "state = running")
        .answer("curl -fsS -m", true, &latest("1.3.0"));
    with_fake!(fake, Manager::Launchd, false, h);
    let r = status(&h, &s, true, RELEASE_BASE);
    assert!(r.ok, "{:?}", r.checks);
    let level =
        |r: &HubStatusReport, n: &str| r.checks.iter().find(|c| c.name == n).map(|c| c.level);
    assert_eq!(level(&r, "fleet"), Some(Level::Warn));
    assert_eq!(level(&r, "current"), Some(Level::Pass));
    assert_eq!(level(&r, "launcher"), Some(Level::Pass));
    assert_eq!(r.hub.linked.len(), 2);
    let text = render_text(&serde_json::to_value(&r).unwrap());
    assert!(text.contains("aaba18ea 1.2.0") && text.contains("ok          true"));

    // behind the release: a warning while the restart is deferred, a failure otherwise
    let fake = Fake::new();
    fake.serving(Some("1.2.0"))
        .installed("1.3.0")
        .answer("launchctl print gui", true, "state = running")
        .answer("curl -fsS -m", true, &latest("1.3.0"));
    with_fake!(fake, Manager::Launchd, false, h);
    write_state(home.path(), &run(999_000, "deferred")).unwrap();
    assert_eq!(
        level(&status(&h, &s, true, RELEASE_BASE), "current"),
        Some(Level::Warn)
    );
    write_state(home.path(), &run(999_000, "failed")).unwrap();
    let r = status(&h, &s, true, RELEASE_BASE);
    assert!(!r.ok);
    assert_eq!(level(&r, "current"), Some(Level::Fail));
    assert_eq!(level(&r, "update"), Some(Level::Fail));
    let mut skipped = run(999_000, "skipped");
    skipped.failed = vec!["1.3.0".into()];
    write_state(home.path(), &skipped).unwrap();
    let r = status(&h, &s, true, RELEASE_BASE);
    assert_eq!(level(&r, "update"), Some(Level::Warn));
    assert_eq!(level(&r, "failed_versions"), Some(Level::Warn));
    write_state(home.path(), &run(1, "current")).unwrap();
    assert_eq!(
        level(&status(&h, &s, true, RELEASE_BASE), "update"),
        Some(Level::Fail)
    );
}

#[test]
fn status_names_every_missing_piece() {
    let home = tempfile::tempdir().unwrap();
    let mut s = spec(Manager::Launchd, home.path());
    let hub = render(&s)[0].clone();
    write_file(&hub.path, &hub.content).unwrap();
    s.executable = home.path().join("elsewhere");
    let mut fake = Fake::new();
    fake.linked = vec![];
    fake.serving(Some("1.3.0"))
        .answer("curl -fsS -m", false, "")
        .answer("launchctl print gui", false, "");
    with_fake!(fake, Manager::Launchd, false, h);
    let r = status(&h, &s, false, RELEASE_BASE);
    let level = |n: &str| r.checks.iter().find(|c| c.name == n).map(|c| c.level);
    assert_eq!(level("declared"), Some(Level::Warn));
    assert_eq!(level("launcher"), Some(Level::Warn));
    assert_eq!(level("unit:dev.majordomus.hub-update"), Some(Level::Fail)); // missing
    assert_eq!(level("linked"), Some(Level::Fail));
    assert_eq!(level("current"), Some(Level::Warn));
    assert_eq!(level("update"), Some(Level::Fail));
    assert_eq!(level("fleet"), None);

    let s = spec(Manager::Launchd, home.path());
    let fake = Fake::new();
    with_fake!(fake, Manager::Unsupported, true, h);
    let r = status(
        &h,
        &spec(Manager::Unsupported, home.path()),
        true,
        RELEASE_BASE,
    );
    assert_eq!(r.checks[0].level, Level::Fail);
    assert_eq!(
        r.checks
            .iter()
            .find(|c| c.name == "answering")
            .unwrap()
            .level,
        Level::Fail
    );
    std::fs::write(&hub.path, "drifted").unwrap();
    let fake = Fake::new();
    with_fake!(fake, Manager::Launchd, true, h);
    let r = status(&h, &s, true, RELEASE_BASE);
    assert!(r.checks.iter().any(|c| c.detail.contains("differs")));
    // current but not loaded
    write_file(&hub.path, &hub.content).unwrap();
    let r = status(&h, &s, true, RELEASE_BASE);
    assert!(r
        .checks
        .iter()
        .any(|c| c.detail.contains("not loaded or disabled")));
    assert!(render_text(&serde_json::to_value(&r).unwrap()).contains("remedy"));
}

#[test]
fn what_the_hub_answers_is_read_loosely() {
    let linked = linked_runtimes(&serde_json::json!({ "peers": [
        { "name": "a", "runtime": "a-1", "version": "1.3.0", "state": "connected" },
        { "name": "b", "state": "expired" },
        { "state": "connected" },
    ]}));
    assert_eq!(linked.len(), 2);
    assert_eq!(linked[1].machine, "");
    assert!(linked_runtimes(&serde_json::json!({})).is_empty());
    let home = tempfile::tempdir().unwrap();
    let fake = Fake::new();
    with_fake!(fake, Manager::Launchd, false, h);
    assert!(!ask_hub(&h, 8791).answering);
    assert_eq!(version_of(&h, Path::new("/x")), None);
    assert_eq!(checkout_version(&h, Path::new("/x")), None);
    fake.answer("/x --version", true, "\n");
    assert_eq!(version_of(&h, Path::new("/x")), None);
    fake.answer("curl", true, "not json");
    assert_eq!(published(&h, "https://x/"), None);
    assert_eq!(read_state(home.path()).failed.len(), 0);
    // a lock that cannot be made is a refusal, not a hang
    let blocked = home.path().join("blocked");
    std::fs::write(&blocked, "a file where the state directory goes").unwrap();
    assert!(take_lock(&h, &blocked).is_err());
}

#[test]
fn an_unsupported_host_owes_no_unit_command() {
    let home = tempfile::tempdir().unwrap();
    let fake = Fake::new();
    fake.answer(GIT, false, "");
    with_fake!(fake, Manager::Unsupported, false, h);
    let r = install(&h, &spec(Manager::Launchd, home.path()), true, false, false);
    assert!(r.units.iter().all(|u| !u.loaded && !u.enabled && !u.active));
    assert!(
        r.steps.iter().all(|st| st.line.starts_with("write ")),
        "{:?}",
        r.steps
    );
    assert_eq!(r.notes.len(), 1, "only the firewall: {:?}", r.notes);
    assert!(!fake.called("launchctl") && !fake.called("systemctl"));
}

#[test]
fn a_hub_without_a_board_or_links_is_still_an_answer() {
    let exec = |_: &[String]| (true, String::new());
    let get = |_: &str, p: &str| {
        (p == "/")
            .then(|| "{\"version\":\"1.3.0\"}".to_string())
            .or(Some("{}".into()))
    };
    let h = Host {
        manager: Manager::Launchd,
        uid: 1,
        user: "o".into(),
        pid: 1,
        staging: false,
        exec: &exec,
        get: &get,
        alive: &|_| false,
        now: &|| 0,
        sleep: &|_| {},
    };
    let a = ask_hub(&h, 8791);
    assert!(a.answering && a.version.as_deref() == Some("1.3.0"));
    assert_eq!(a.attached, 0);
    assert!(a.linked.is_empty());
}

#[test]
fn a_log_that_cannot_rotate_says_why() {
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("hub.log");
    std::fs::File::create(&log)
        .unwrap()
        .set_len(LOG_BOUND + 1)
        .unwrap();
    std::fs::create_dir_all(log.with_extension("log.1")).unwrap();
    let step = rotate(&log).unwrap();
    assert_eq!(step.ok, Some(false));
    assert!(!step.output.is_empty());
    // and a file whose directory cannot be made is not written
    let blocked = home.path().join("blocked");
    std::fs::write(&blocked, "a file").unwrap();
    assert!(write_file(&blocked.join("x"), "").is_err());
}

#[test]
fn a_staging_update_asks_nothing_and_decides_nothing() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let fake = behind();
    with_fake!(fake, Manager::Launchd, true, h);
    let r = update(&h, &s, RELEASE_BASE, true, false);
    assert!(matches!(r.decision, Decision::Unknown { .. }) && r.result == "unknown");
    assert!(!fake.called("curl") && !fake.called("git"));
    assert!(r.published.is_none() && r.served.is_none());
}

#[test]
fn a_hub_behind_the_release_with_no_update_ever_says_never() {
    let home = tempfile::tempdir().unwrap();
    let s = spec(Manager::Launchd, home.path());
    let fake = Fake::new();
    fake.serving(Some("1.2.0"))
        .installed("1.2.0")
        .answer("launchctl print gui", true, "state = running")
        .answer("curl -fsS -m", true, &latest("1.3.0"));
    with_fake!(fake, Manager::Launchd, false, h);
    let r = status(&h, &s, true, RELEASE_BASE);
    let current = r.checks.iter().find(|c| c.name == "current").unwrap();
    assert_eq!(current.level, Level::Fail);
    assert!(current.detail.contains("ended never"), "{}", current.detail);
}
