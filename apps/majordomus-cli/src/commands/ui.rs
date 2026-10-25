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
        reason: serve::not_ready(&c, wait),
    };
    let ready = c.ready() && url.is_some();
    if let (true, Some(url), false) = (ready, &url, args.no_open) {
        match open_browser(url) {
            Ok(()) => report.opened = true,
            Err(why) => report.reason = Some(format!("no browser was opened: {why}")),
        }
    }
    print(&report, args.format)?;
    Ok(if ready { 0 } else { 10 })
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
    let named = |route: &'static str| route.strip_prefix(PREFIX)?.strip_prefix('/');
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
    let mut cmd = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        Command::new("xdg-open")
    };
    let program = format!("{:?}", cmd.get_program());
    let status = cmd
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("{program} could not be run: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited {status}"))
    }
}

/// The address on standard output, alone, so that `$(majordomus ui --no-open)` is it; what
/// else there is to say goes to standard error.
fn print(r: &UiReport, format: OutputFormat) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
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
            let mut err = std::io::stderr().lock();
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
}
