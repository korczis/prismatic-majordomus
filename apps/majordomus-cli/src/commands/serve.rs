//! `majordomus serve`: the HTTP projection on a loopback socket, without a stdio session
//! of its own. It is the same shared server `majordomus mcp` starts: when one is already
//! running for the repository, `serve` says where and exits 0 rather than starting a
//! second one.
//!
//! Lifecycle: when stdin is a pipe or a socket, the server lives as long as it: end of
//! file (the parent closed its end) starts the shutdown, which waits for attached peers to
//! leave. When stdin is anything else (a terminal, `/dev/null` under nohup or a service
//! manager, a file) nothing is watched and the process runs until it is stopped — or, with
//! `--idle`, until no peer has been attached for that long. There is no daemon mode either
//! way: whoever started the process owns it.
//!
//! # `status`, `ensure`, `stop`
//!
//! Three subcommands make the server's lifecycle something a person or a hook can converge
//! on rather than remember. `status` is the command-line projection of `server.status`.
//! `ensure` reads the lease, probes the server it names, and starts one as a process of its
//! own when none answers — with `--fallback`, so a taken port is never a failure, and with
//! `--idle`, so a server no client owns ends by itself — then waits until it is ready and
//! prints where it stands. Run twice, it starts nothing the second time; run by three
//! shells at once, the election lets one of the three servers bind and the others defer.
//! `stop` signals the server this checkout's lease names, when it answers for this
//! checkout, and waits for the lease to go. Nothing here kills a server of another checkout,
//! and nothing kills a server that was not asked for by name.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::app::App;
use crate::capability::builtin::server::{standing_of, ServerStanding};
use crate::cli::{OutputFormat, ServeArgs, ServeCommand};
use crate::deploy::{Deployment, Listen, KIND};
use crate::error::{Error, Result};
use crate::http::server::stdin_is_a_pipe;
use crate::lease::{self, LeaseDocument, LeaseFile, Role};
use crate::repository::Repository;
use crate::shared::SharedServer;

use super::executions::{ask_server, cli_capability, map};

/// Run `majordomus serve`.
pub fn run(args: ServeArgs) -> Result<u8> {
    let start = match &args.repo.repo {
        Some(p) => p.clone(),
        None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
    };
    let repo = Repository::discover(&start)?;
    match &args.command {
        None => serve(&args, &repo),
        Some(ServeCommand::Status { format }) => status(&args, *format),
        Some(ServeCommand::Ensure {
            port,
            idle,
            wait,
            format,
        }) => ensure(&repo, *port, *idle, Duration::from_secs(*wait), *format),
        Some(ServeCommand::Stop { wait }) => stop(&repo, Duration::from_secs(*wait)),
    }
}

/// Become the server, or say who already is.
fn serve(args: &ServeArgs, repo: &Repository) -> Result<u8> {
    let lease = match lease::elect(repo)? {
        Role::Peer { url } => {
            tracing::info!(
                url = %url,
                "a shared server for this repository is already running at {url} (its home page lists what it serves); not starting a second one"
            );
            return Ok(0);
        }
        Role::Server(lease) => lease,
    };
    // the lease is kept young while the layer loads, so that a peer waiting on it never
    // mistakes a slow start for an abandoned one
    lease.keep_alive();
    let app = App::load(&args.repo)?;
    // The address: the local default, or the one a deployment object declares. The port is
    // never typed twice — the object states it once and the process, the image and the
    // provider configuration all read that one.
    let declared = match &args.deployment {
        Some(id) => Some(deployment(&app, id)?),
        None => None,
    };
    let (host, port) = match &declared {
        Some((_, listen)) => (listen.interface.host().to_string(), listen.port.get()),
        None => (args.host.clone(), args.port),
    };
    let shared = SharedServer::start(
        app.context.clone(),
        crate::VERSION,
        &host,
        port,
        args.fallback && declared.is_none(),
        declared.as_ref().map(|(file, _)| file.as_str()),
        lease,
        Some(app.share.dir()),
    )?;
    if stdin_is_a_pipe() {
        tracing::info!(
            "stdin is a pipe; the server stops when it closes and the last peer has left"
        );
        let mut sink = Vec::new();
        let _ = std::io::stdin().lock().read_to_end(&mut sink);
        tracing::info!("stdin closed; stopping");
    } else if args.idle > 0 {
        let idle = Duration::from_secs(args.idle);
        tracing::info!(
            idle_seconds = args.idle,
            "stdin is not a pipe; the server stops when no peer has been attached for {} second(s)",
            args.idle
        );
        let mut idle_since = Instant::now();
        loop {
            std::thread::sleep(Duration::from_secs(1));
            if shared.peers_attached() > 0 {
                idle_since = Instant::now();
            } else if idle_since.elapsed() >= idle {
                tracing::info!(
                    "no peer for {} second(s); stopping",
                    idle_since.elapsed().as_secs()
                );
                break;
            }
        }
    } else {
        tracing::info!("stdin is not a pipe; the server runs until the process is stopped");
        loop {
            std::thread::sleep(Duration::from_secs(1));
            shared.endpoint().reap();
        }
    }
    shared.wait_until_peers_leave();
    shared.stop();
    Ok(0)
}

/// `serve status`: the projection of `server.status`, asked of the running server when
/// there is one — so that the answer carries the lease that process holds — and answered
/// locally otherwise.
fn status(args: &ServeArgs, format: OutputFormat) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = cli_capability(ctx, &["serve", "status"])?;
    let answer = match lease::serving(&app.repository) {
        Some(url) => ask_server(ctx, &url, id, &json!({}))?,
        None => ctx.execute(id, json!({})).map_err(map)?,
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => writeln!(out, "{}", pretty(&answer)).map_err(Error::Transport)?,
        OutputFormat::Text => render_status(&mut out, &answer)?,
    }
    Ok(0)
}

fn render_status(out: &mut impl Write, s: &Value) -> Result<()> {
    let w = |out: &mut dyn Write, line: String| -> Result<()> {
        writeln!(out, "{line}").map_err(Error::Transport)
    };
    w(
        out,
        format!("standing   {}", s["standing"].as_str().unwrap_or("?")),
    )?;
    if let Some(p) = s.get("this_process") {
        w(
            out,
            format!(
                "this       {}  pid {}  version {}  since {}",
                p["url"].as_str().unwrap_or("-"),
                p["pid"],
                p["version"].as_str().unwrap_or("-"),
                p["started_at"].as_str().unwrap_or("-")
            ),
        )?;
    }
    w(
        out,
        format!(
            "desired    {}:{}  version {}",
            s["desired"]["host"].as_str().unwrap_or("-"),
            s["desired"]["port"],
            s["desired"]["version"].as_str().unwrap_or("-")
        ),
    )?;
    match s["git"]["id"].as_str() {
        Some(id) => w(
            out,
            format!(
                "repository {}{}",
                id,
                if s["git"]["linked"] == true {
                    "  (this is a linked worktree)"
                } else {
                    ""
                }
            ),
        )?,
        None => w(
            out,
            "repository not a git repository: this checkout alone".into(),
        )?,
    }
    w(out, "servers:".into())?;
    for v in s["servers"].as_array().into_iter().flatten() {
        let mut line = format!(
            "  {:<9} {}",
            v["standing"].as_str().unwrap_or("?"),
            v["worktree"].as_str().unwrap_or("?")
        );
        if let Some(b) = v["branch"].as_str() {
            line.push_str(&format!(" ({b})"));
        }
        if let Some(url) = v["lease"]["url"].as_str() {
            line.push_str(&format!("  {url}"));
        }
        if let Some(n) = v["peers"].as_u64() {
            line.push_str(&format!("  peers {n}"));
        }
        if v["this_checkout"] == true {
            line.push_str("  <- this checkout");
        }
        w(out, line)?;
        if let Some(r) = v["reason"].as_str() {
            w(out, format!("            {r}"))?;
        }
    }
    Ok(())
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// What `serve ensure` answers: where the server stands after this call, and whether this
/// call is what started it.
#[derive(Debug, Serialize)]
struct EnsureReport {
    /// Where this checkout's server stands now.
    standing: ServerStanding,
    /// The address, when a server is there.
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    /// Its pid, from the lease.
    #[serde(skip_serializing_if = "Option::is_none")]
    pid: Option<u32>,
    /// Its version, from the lease.
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// Whether this call started the server that now answers.
    started: bool,
    /// Why the standing is not `ready`, when it is not.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    /// Where a server this command starts writes its log.
    log: PathBuf,
}

/// Where a server `ensure` starts writes its log: beside the lease, in the local half.
fn server_log(repo: &Repository) -> PathBuf {
    lease::lease_path(repo).with_file_name("server.log")
}

/// `serve ensure`: converge on a ready server for this checkout.
///
/// The loop reads the lease and probes the server it names on every round, exactly as the
/// election does, and decides from the standing: `ready` ends it; `starting` waits;
/// `absent` and `stale` start a server once and then wait for it, because the election in
/// that process takes a stale lease over itself; `outdated` starts one only when the
/// election would take the lease over — the same executable, replaced on disk — and is
/// otherwise reported with the remedy, because a server of another build that answers is
/// not this command's to end. The whole call is bounded by `wait`.
fn ensure(
    repo: &Repository,
    port: u16,
    idle: u64,
    wait: Duration,
    format: OutputFormat,
) -> Result<u8> {
    let path = lease::lease_path(repo);
    let log = server_log(repo);
    let deadline = Instant::now() + wait;
    let mut started = false;
    loop {
        let file = LeaseFile::read(&path);
        let (standing, reason) = standing_of(
            &file,
            lease::file_age(&path),
            |url| lease::probe(url, repo.root()),
            crate::VERSION,
        );
        let doc = file.document().cloned();
        match standing {
            ServerStanding::Ready => return report(standing, doc, None, started, &log, format),
            ServerStanding::Starting => {}
            ServerStanding::Absent | ServerStanding::Stale if !started => {
                spawn_server(repo, port, idle, &log)?;
                started = true;
            }
            ServerStanding::Absent | ServerStanding::Stale => {}
            ServerStanding::Outdated => {
                let takes_over = doc
                    .as_ref()
                    .and_then(|d| d.executable.as_ref())
                    .is_some_and(|e| {
                        e.replaced().is_some()
                            && std::env::current_exe().ok().as_deref() == Some(e.path.as_path())
                    });
                if takes_over && !started {
                    spawn_server(repo, port, idle, &log)?;
                    started = true;
                } else if !started || Instant::now() >= deadline {
                    let remedy = format!(
                        "{}; `majordomus serve stop` ends it, and `serve ensure` then starts one from this executable",
                        reason.unwrap_or_default()
                    );
                    return report(standing, doc, Some(remedy), started, &log, format);
                }
            }
        }
        if Instant::now() >= deadline {
            let why = format!(
                "{}no server became ready within {} second(s); the log is {}",
                reason.map(|r| format!("{r}; ")).unwrap_or_default(),
                wait.as_secs(),
                log.display()
            );
            return report(standing, doc, Some(why), started, &log, format);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Print the report and decide the exit code: 0 for a ready server, 10 for anything else,
/// which is the code for a contract unmet everywhere in this executable.
fn report(
    standing: ServerStanding,
    doc: Option<LeaseDocument>,
    reason: Option<String>,
    started: bool,
    log: &Path,
    format: OutputFormat,
) -> Result<u8> {
    let r = EnsureReport {
        standing,
        url: doc.as_ref().and_then(|d| d.url.clone()),
        pid: doc.as_ref().map(|d| d.pid),
        version: doc.as_ref().and_then(|d| d.version.clone()),
        started,
        reason,
        log: log.to_path_buf(),
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&r).unwrap_or_default()
        )
        .map_err(Error::Transport)?,
        OutputFormat::Text => {
            let mut line = r.standing.as_str().to_string();
            if let Some(url) = &r.url {
                line.push_str(&format!(" {url}"));
            }
            if let Some(pid) = r.pid {
                line.push_str(&format!(" pid {pid}"));
            }
            if let Some(v) = &r.version {
                line.push_str(&format!(" version {v}"));
            }
            if r.started {
                line.push_str(" (started by this call)");
            }
            if let Some(why) = &r.reason {
                line.push_str(&format!(": {why}"));
            }
            writeln!(out, "{line}").map_err(Error::Transport)?;
        }
    }
    Ok(if standing == ServerStanding::Ready {
        0
    } else {
        10
    })
}

/// Start a server for this checkout as a process of its own: this executable, `serve`, the
/// port asked for with a free one as the fallback, the idle life it was given, its log
/// beside the lease, in its own process group so that it outlives the shell that asked. The
/// election in that process decides whether it serves or defers; this only starts it.
fn spawn_server(repo: &Repository, port: u16, idle: u64, log: &Path) -> Result<()> {
    let exe = std::env::current_exe().map_err(|e| Error::io("the executable", e))?;
    if let Some(dir) = log.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .map_err(|e| Error::io(log, e))?;
    let mut cmd = Command::new(exe);
    cmd.arg("serve")
        .arg("--repo")
        .arg(repo.root())
        .arg("--port")
        .arg(port.to_string())
        .arg("--idle")
        .arg(idle.to_string())
        .arg("--fallback")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(file));
    if std::env::var_os("MAJORDOMUS_LOG").is_none() {
        cmd.env("MAJORDOMUS_LOG", "info");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn().map_err(|e| Error::io(log, e))?;
    tracing::info!(pid = child.id(), log = %log.display(), "started a server for this checkout");
    Ok(())
}

/// `serve stop`: end the server this checkout's lease names, when it answers for this
/// checkout, and wait for the lease to go.
fn stop(repo: &Repository, wait: Duration) -> Result<u8> {
    let path = lease::lease_path(repo);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let say = |out: &mut dyn Write, line: String| -> Result<()> {
        writeln!(out, "{line}").map_err(Error::Transport)
    };
    let doc = match LeaseFile::read(&path) {
        LeaseFile::Absent => {
            say(
                &mut out,
                "no server: this checkout has no lease, nothing to stop".into(),
            )?;
            return Ok(0);
        }
        LeaseFile::Empty | LeaseFile::Corrupt(_) => {
            say(
                &mut out,
                "the lease is not a lease document; no server answers it, and the next start takes it over".into(),
            )?;
            return Ok(0);
        }
        LeaseFile::Document(doc) => doc,
    };
    let Some(url) = doc.url.clone() else {
        say(
            &mut out,
            "the server is still binding; nothing to stop yet".into(),
        )?;
        return Ok(10);
    };
    if !lease::probe(&url, repo.root()) {
        say(
            &mut out,
            format!(
                "stale lease: the server it names at {url} does not answer for this checkout; the next start takes it over"
            ),
        )?;
        return Ok(0);
    }
    if doc.pid == 0 {
        return Err(Error::Lease {
            reason: format!("the lease at {} names no pid to signal", path.display()),
        });
    }
    signal_stop(doc.pid)?;
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        if matches!(LeaseFile::read(&path), LeaseFile::Absent) {
            say(&mut out, format!("stopped {url} (pid {})", doc.pid))?;
            return Ok(0);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    say(
        &mut out,
        format!(
            "asked pid {} at {url} to stop; the lease is still there after {} second(s)",
            doc.pid,
            wait.as_secs()
        ),
    )?;
    Ok(10)
}

#[cfg(unix)]
fn signal_stop(pid: u32) -> Result<()> {
    // SAFETY: kill(2) with a pid this process read from the lease of its own checkout and
    // the signal the server removes its lease on; no memory is touched.
    let rc = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    if rc != 0 {
        return Err(Error::Lease {
            reason: format!(
                "cannot signal pid {pid}: {}",
                std::io::Error::last_os_error()
            ),
        });
    }
    Ok(())
}

#[cfg(not(unix))]
fn signal_stop(_: u32) -> Result<()> {
    Err(Error::Lease {
        reason: "stopping a server by signal is not supported on this platform".into(),
    })
}

/// The listen address one deployment object declares, with the file that declares it. An
/// id the layer does not have is refused by name rather than falling back to a default: a
/// hosted process that quietly bound loopback would pass every check here and be
/// unreachable in production.
fn deployment(app: &App, id: &str) -> Result<(String, Listen)> {
    let object = app
        .context
        .index
        .objects
        .iter()
        .find(|o| o.kind == KIND && o.identity == id)
        .ok_or_else(|| Error::DeploymentNotFound { id: id.into() })?;
    let parsed = Deployment::parse(object).map_err(|refusal| Error::InvalidDeployment {
        reason: refusal.to_string(),
    })?;
    Ok((object.provenance.path.clone(), parsed.listen))
}
