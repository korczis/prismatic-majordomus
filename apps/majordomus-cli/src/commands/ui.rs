//! `majordomus ui`: the primary surface in one step (ADR 0135, I2484).
//!
//! The command is a projection of what already exists and nothing more: the screen is
//! resolved against [`SCREENS`], the list the navigation is drawn from; the server is the
//! one [`serve::converge_with`] converges on, the whole of `serve ensure`, so a server
//! already answering is used and a second one is never started; and the address is the
//! one the lease publishes. There is no `ui status` and no `ui stop`, because `serve
//! status` and `serve stop` already answer for that server.
//!
//! A screen is refused before anything is started: a typo must not cost a server. The
//! browser is the platform's own opener; when it cannot be launched the command says so
//! and still prints the address, because the address is the answer and the browser only a
//! convenience.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;

use crate::capability::builtin::server::ServerStanding;
use crate::cli::{OutputFormat, UiArgs};
use crate::error::{Error, Result};
use crate::lease;
use crate::ui::{PREFIX, SCREENS};

use super::serve;

/// What `majordomus ui --format json` prints.
#[derive(Debug, Serialize)]
struct UiReport {
    /// Where this checkout's server stands.
    standing: ServerStanding,
    /// The address of the screen, when a server is there.
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    /// Whether this call started the server that answers.
    started: bool,
    /// Whether a browser was opened on the address.
    opened: bool,
    /// Why the standing is not `ready`, or why no browser was opened.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// Run `majordomus ui`.
pub fn run(args: UiArgs) -> Result<u8> {
    run_to(
        args,
        open_browser,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    )
}

/// [`run`], with the browser opened by `open` and the report written into `out` and `err`.
fn run_to(
    args: UiArgs,
    open: impl FnOnce(&str) -> std::result::Result<(), String>,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<u8> {
    // refused before the repository is read or a server started: a typo costs nothing
    let route = screen_route(args.screen.as_deref())?;
    let repo = serve::discover(&args.repo)?;
    let layer = serve::layer_args(&args.repo)?;
    // the declared server timings, the idle life among them, as `serve ensure` reads them
    let _ = crate::policy::LoadedPolicy::load(&repo);
    let wait = Duration::from_secs(args.wait);
    let c = serve::converge_with(
        &repo,
        args.port
            .unwrap_or_else(|| crate::cli::checkout_port(repo.root())),
        args.idle.unwrap_or_else(|| lease::timings().idle.as_secs()),
        wait,
        &layer,
    )?;
    let (report, code) = conclude(&c, route, wait, args.no_open, open);
    write_report(&report, args.format, out, err)?;
    Ok(code)
}

/// What a convergence comes to: the report and the exit code, after `open` was asked to
/// open the screen's address when a server is ready there and opening was not declined.
fn conclude(
    c: &serve::Convergence,
    route: &str,
    wait: Duration,
    no_open: bool,
    open: impl FnOnce(&str) -> std::result::Result<(), String>,
) -> (UiReport, u8) {
    let url = c
        .document
        .as_ref()
        .and_then(|d| d.url.as_deref())
        .map(|base| format!("{}{route}", base.trim_end_matches('/')));
    let mut report = UiReport {
        standing: c.standing,
        url: url.clone(),
        started: c.started,
        opened: false,
        reason: serve::not_ready(c, wait),
    };
    let ready = c.ready() && url.is_some();
    if let (true, Some(url), false) = (ready, &url, no_open) {
        match open(url) {
            Ok(()) => report.opened = true,
            Err(why) => report.reason = Some(format!("no browser was opened: {why}")),
        }
    }
    (report, if ready { 0 } else { 10 })
}

/// The route of the named screen, the dashboard's when none is named; an unknown name is
/// refused with every name there is, in the navigation's order.
///
/// A screen is named by the last word of its route, which is what a person reads in the
/// address bar: `roadmap` is `/ui/roadmap`.
fn screen_route(name: Option<&str>) -> Result<&'static str> {
    let Some(name) = name else {
        return Ok(PREFIX);
    };
    let named = |route: &'static str| {
        route
            .strip_prefix(PREFIX)
            .and_then(|rest| rest.strip_prefix('/'))
    };
    if let Some(screen) = SCREENS.iter().find(|s| named(s.route) == Some(name)) {
        return Ok(screen.route);
    }
    let known: Vec<&str> = SCREENS.iter().filter_map(|s| named(s.route)).collect();
    Err(Error::Refused {
        code: 2,
        reason: format!(
            "ui: no screen is called `{name}`; the screens are {} (none names the dashboard)",
            known.join(", ")
        ),
    })
}

/// Open `url` with the platform's own opener and wait for the opener, which returns as
/// soon as it has handed the address on.
fn open_browser(url: &str) -> std::result::Result<(), String> {
    launch(opener(url))
}

/// The platform's opener, given `url` and nothing to read or write: built here and run by
/// [`launch`], so what is run can be read without running it.
fn opener(url: &str) -> Command {
    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let mut cmd = Command::new("xdg-open");
    cmd.arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd
}

/// Run `cmd` to its end: an opener that cannot be run, or that exits unsuccessfully, is
/// the reason no browser was opened.
fn launch(mut cmd: Command) -> std::result::Result<(), String> {
    let program = format!("{:?}", cmd.get_program());
    let status = cmd
        .status()
        .map_err(|e| format!("{program} could not be run: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited {status}"))
    }
}

/// The report, into `out` and `err`: the address on `out`, alone, so that
/// `$(majordomus ui --no-open)` is it; what else there is to say goes to `err`.
fn write_report(
    r: &UiReport,
    format: OutputFormat,
    out: &mut impl Write,
    err: &mut impl Write,
) -> Result<()> {
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(r).unwrap_or_default()
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            if r.standing == ServerStanding::Ready {
                if let Some(url) = &r.url {
                    writeln!(out, "{url}").map_err(Error::Transport)?;
                }
            }
            if r.standing != ServerStanding::Ready {
                let why = r.reason.as_deref().unwrap_or("no server answers");
                let _ = writeln!(err, "majordomus ui: {}: {why}", r.standing.as_str());
            } else if let Some(why) = &r.reason {
                let _ = writeln!(err, "majordomus ui: {why}; open the address yourself");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_screen_is_the_dashboard() {
        assert_eq!(screen_route(None).unwrap(), PREFIX);
    }

    #[test]
    fn every_screen_but_the_dashboard_is_named_by_the_last_word_of_its_route() {
        for screen in SCREENS.iter().filter(|s| s.route != PREFIX) {
            let name = screen.route.rsplit('/').next().unwrap();
            assert_eq!(screen_route(Some(name)).unwrap(), screen.route);
        }
    }

    #[test]
    fn an_unknown_screen_is_refused_with_every_name_there_is() {
        let err = screen_route(Some("nowhere")).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        let msg = err.to_string();
        assert!(msg.contains("`nowhere`"), "{msg}");
        assert!(msg.contains("roadmap") && msg.contains("mesh"), "{msg}");
    }

    #[test]
    fn a_route_word_is_not_a_prefix_match() {
        // `road` is a prefix of `roadmap` and must not open it
        assert!(screen_route(Some("road")).is_err());
        assert!(screen_route(Some("ui")).is_err());
        assert!(screen_route(Some("")).is_err());
    }

    /// A convergence standing as given, on a lease naming `url`, as `serve` would answer.
    fn converged(standing: ServerStanding, url: Option<&str>) -> serve::Convergence {
        serve::Convergence {
            standing,
            document: url.map(|u| {
                serde_json::from_value(serde_json::json!({ "schema": "lease", "url": u })).unwrap()
            }),
            reason: None,
            started: false,
            timed_out: false,
            log: std::path::PathBuf::from("server.log"),
        }
    }

    #[test]
    fn a_ready_server_opens_the_screen_once_and_answers_zero() {
        let c = converged(ServerStanding::Ready, Some("http://127.0.0.1:9/"));
        let mut asked = Vec::new();
        let (report, code) = conclude(&c, "/ui/roadmap", Duration::ZERO, false, |u| {
            asked.push(u.to_string());
            Ok(())
        });
        assert_eq!(code, 0);
        assert_eq!(
            asked,
            ["http://127.0.0.1:9/ui/roadmap"],
            "one slash between them"
        );
        assert!(report.opened && report.reason.is_none(), "{report:?}");
        assert_eq!(report.url.as_deref(), Some("http://127.0.0.1:9/ui/roadmap"));
    }

    #[test]
    fn an_opener_that_fails_is_the_reason_and_the_address_still_stands() {
        let c = converged(ServerStanding::Ready, Some("http://127.0.0.1:9"));
        let (report, code) = conclude(&c, "/ui", Duration::ZERO, false, |_| {
            Err("xdg-open exited 3".into())
        });
        assert_eq!(
            code, 0,
            "the address is the answer; the browser only a convenience"
        );
        assert!(!report.opened);
        assert_eq!(
            report.reason.as_deref(),
            Some("no browser was opened: xdg-open exited 3")
        );
        assert_eq!(report.url.as_deref(), Some("http://127.0.0.1:9/ui"));
    }

    #[test]
    fn no_open_and_no_server_open_nothing() {
        let ready = converged(ServerStanding::Ready, Some("http://127.0.0.1:9/"));
        let (report, code) = conclude(&ready, "/ui", Duration::ZERO, true, |_| {
            panic!("--no-open must not open")
        });
        assert_eq!((code, report.opened), (0, false));
        let absent = converged(ServerStanding::Absent, None);
        let (report, code) = conclude(&absent, "/ui", Duration::ZERO, false, |_| {
            panic!("there is nothing to open")
        });
        assert_eq!(code, 10);
        assert!(report.url.is_none() && !report.opened, "{report:?}");
    }

    #[test]
    fn the_opener_is_the_platforms_own_given_the_address_and_no_terminal() {
        let cmd = opener("http://127.0.0.1:9/ui");
        let program = cmd.get_program().to_string_lossy().into_owned();
        assert!(
            ["open", "xdg-open", "cmd"].contains(&program.as_str()),
            "{program}"
        );
        assert_eq!(
            cmd.get_args()
                .last()
                .map(|a| a.to_string_lossy().into_owned()),
            Some("http://127.0.0.1:9/ui".to_string())
        );
    }

    #[test]
    fn launching_says_whether_the_opener_ran_and_how_it_ended() {
        assert_eq!(launch(Command::new("true")), Ok(()));
        let failed = launch(Command::new("false")).unwrap_err();
        assert!(failed.contains("\"false\" exited"), "{failed}");
        let missing = launch(Command::new("majordomus-no-such-opener")).unwrap_err();
        assert!(missing.contains("could not be run"), "{missing}");
    }

    /// The real opener is run only where it cannot open anything a person sees: on Linux
    /// without a display, as CI's coverage job runs, `xdg-open` answers at once. On a
    /// desktop it would open a browser, so it is not run there.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_platforms_opener_is_run_and_its_answer_returned() {
        if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
            return;
        }
        match open_browser("about:blank") {
            Ok(()) => {}
            Err(why) => assert!(why.contains("xdg-open"), "{why}"),
        }
    }

    /// What [`write_report`] writes for `r`, as (standard output, standard error).
    fn written(r: &UiReport, format: OutputFormat) -> (String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        write_report(r, format, &mut out, &mut err).unwrap();
        (
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn report(standing: ServerStanding, url: Option<&str>, reason: Option<&str>) -> UiReport {
        UiReport {
            standing,
            url: url.map(str::to_string),
            started: false,
            opened: false,
            reason: reason.map(str::to_string),
        }
    }

    #[test]
    fn the_address_alone_goes_to_standard_output_and_the_rest_to_standard_error() {
        let ready = report(ServerStanding::Ready, Some("http://h/ui"), None);
        assert_eq!(
            written(&ready, OutputFormat::Text),
            ("http://h/ui\n".into(), String::new())
        );
        let unopened = report(
            ServerStanding::Ready,
            Some("http://h/ui"),
            Some("no browser"),
        );
        assert_eq!(
            written(&unopened, OutputFormat::Text),
            (
                "http://h/ui\n".into(),
                "majordomus ui: no browser; open the address yourself\n".into()
            )
        );
        let (out, err) = written(
            &report(ServerStanding::Absent, None, None),
            OutputFormat::Text,
        );
        assert_eq!(out, "", "no address without a server");
        assert!(err.ends_with(": no server answers\n"), "{err}");
        let (out, err) = written(
            &report(ServerStanding::Stale, None, Some("its process is gone")),
            OutputFormat::Text,
        );
        assert_eq!(out, "");
        assert!(err.ends_with(": its process is gone\n"), "{err}");
    }

    #[test]
    fn the_json_report_is_the_whole_answer_on_standard_output() {
        let r = report(ServerStanding::Ready, Some("http://h/ui"), None);
        let (out, err) = written(&r, OutputFormat::Json);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["url"], "http://h/ui");
        assert_eq!(v["opened"], false);
        assert!(v.get("reason").is_none(), "{v}");
        assert_eq!(err, "");
    }

    /// A stream that takes nothing: every write is refused, as a closed pipe would.
    struct Closed;

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn an_address_that_cannot_be_written_is_the_error() {
        let r = report(ServerStanding::Ready, Some("http://h/ui"), None);
        for format in [OutputFormat::Json, OutputFormat::Text] {
            let refused = write_report(&r, format, &mut Closed, &mut Vec::new()).unwrap_err();
            assert!(matches!(refused, Error::Transport(_)), "{refused:?}");
        }
    }

    #[test]
    fn a_ready_server_without_an_address_prints_nothing() {
        let r = report(ServerStanding::Ready, None, None);
        assert_eq!(
            written(&r, OutputFormat::Text),
            (String::new(), String::new())
        );
    }

    /// `majordomus ui` parsed from `argv` after the command's name.
    fn ui_args(argv: &[&str]) -> UiArgs {
        use clap::Parser;
        let mut all = vec!["majordomus", "ui"];
        all.extend_from_slice(argv);
        let crate::cli::Command::Ui(args) = crate::cli::Cli::try_parse_from(all).unwrap().command
        else {
            panic!("ui");
        };
        args
    }

    /// [`run_to`] on `args`, with an opener that must not be reached and the report kept.
    fn run_kept(args: UiArgs) -> Result<u8> {
        run_to(
            args,
            |_| panic!("nothing is ready to open"),
            &mut Vec::new(),
            &mut Vec::new(),
        )
    }

    #[test]
    fn a_place_that_is_no_repository_is_refused_before_anything_starts() {
        let args = ui_args(&["--repo", "/nonexistent/majordomus-ui", "--wait", "0"]);
        assert!(run_kept(args).is_err(), "no repository is found there");
    }

    #[test]
    fn a_share_that_cannot_be_made_absolute_is_refused() {
        let fixture = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = fixture.root().canonicalize().unwrap();
        let mut args = ui_args(&["--repo", root.to_str().unwrap(), "--wait", "0"]);
        // the command line cannot say it; an empty path is the one no platform makes absolute
        args.repo.share = Some(std::path::PathBuf::new());
        assert!(run_kept(args).is_err(), "an empty share is refused");
    }

    #[test]
    fn a_server_that_cannot_be_claimed_is_the_error() {
        let fixture = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = fixture.root().canonicalize().unwrap();
        let repo = crate::repository::Repository::discover(&root).unwrap();
        // the directory the start is claimed in is a file: no claim can be made there
        let state = lease::lease_path(&repo).parent().unwrap().to_path_buf();
        if state.is_dir() {
            std::fs::remove_dir_all(&state).unwrap();
        }
        std::fs::create_dir_all(state.parent().unwrap()).unwrap();
        std::fs::write(&state, "").unwrap();
        let args = ui_args(&["--repo", root.to_str().unwrap(), "--wait", "0"]);
        assert!(run_kept(args).is_err(), "a claim in a file cannot be made");
    }

    #[test]
    fn a_report_that_cannot_be_written_is_the_error() {
        let fixture = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = fixture.root().canonicalize().unwrap();
        let repo = crate::repository::Repository::discover(&root).unwrap();
        let claim = lease::lease_path(&repo).with_file_name("server.spawn");
        std::fs::create_dir_all(claim.parent().unwrap()).unwrap();
        std::fs::write(&claim, std::process::id().to_string()).unwrap();
        let args = ui_args(&[
            "--repo",
            root.to_str().unwrap(),
            "--wait",
            "0",
            "--format",
            "json",
        ]);
        let refused = run_to(args, |_| Ok(()), &mut Closed, &mut Vec::new()).unwrap_err();
        assert!(matches!(refused, Error::Transport(_)), "{refused:?}");
    }

    #[test]
    fn ui_on_a_checkout_whose_start_is_claimed_starts_nothing_and_answers_ten() {
        use clap::Parser;
        let fixture = crate::synthetic::SyntheticRepository::small().unwrap();
        let root = fixture.root().canonicalize().unwrap();
        let repo = crate::repository::Repository::discover(&root).unwrap();
        // another process has claimed the start, as in serve's own test: nothing is started
        let claim = lease::lease_path(&repo).with_file_name("server.spawn");
        std::fs::create_dir_all(claim.parent().unwrap()).unwrap();
        std::fs::write(&claim, std::process::id().to_string()).unwrap();
        let parse = |extra: &[&str]| {
            let mut argv = vec!["majordomus", "ui", "--repo", root.to_str().unwrap()];
            argv.extend_from_slice(extra);
            let crate::cli::Command::Ui(args) =
                crate::cli::Cli::try_parse_from(argv).unwrap().command
            else {
                panic!("ui");
            };
            args
        };
        let code = run(parse(&["--wait", "0", "--format", "json"])).unwrap();
        assert_eq!(code, 10, "no server answers, so the contract is unmet");
        assert!(!lease::lease_path(&repo).exists(), "nothing was started");
        let refused = run(parse(&["nowhere"])).unwrap_err();
        assert_eq!(refused.exit_code(), 2);
    }
}
