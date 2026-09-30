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
//! shells at once, one of the three starts a process and the other two wait for its lease
//! (`server.spawn` beside the lease records the start until that process has elected).
//! `stop` signals the server this checkout's lease names, when it answers for this
//! checkout, and waits for *that server's* lease to go — the lease it read, by its token,
//! and not merely the path, which another process may occupy in the same instant.
//! Nothing here kills a server of another checkout, and nothing kills a server that was not
//! asked for by name.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::app::App;
use crate::capability::builtin::server::{standing_of, Checkouts, ServerStanding};
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
        Some(ServeCommand::Status { checkouts, format }) => status(&args, *checkouts, *format),
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
    let elected = lease::elect(repo);
    // Whatever the election decided, the process `ensure` started to hold it has now
    // decided: the claim it was started under is spent, and the next `ensure` reads the
    // lease rather than waiting on this process.
    release_spawn_claim(repo);
    let lease = match elected? {
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
    // The server outlives many commits. It follows the repository rather than freezing
    // the picture it started with: `crate::live` says what that costs and why.
    let live = std::sync::Arc::new(crate::live::Live::watching(
        args.repo.clone(),
        app.context.clone(),
    ));
    let shared = SharedServer::start(
        live,
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
///
/// The flag is the capability's input field and nothing else: it is serialised into the
/// same JSON whether the question travels over the loopback socket or is executed here, so
/// `--checkouts this` costs one lease read and one probe on either path.
fn status(args: &ServeArgs, checkouts: Checkouts, format: OutputFormat) -> Result<u8> {
    let app = App::load(&args.repo)?;
    let ctx = &app.context;
    let id = cli_capability(ctx, &["serve", "status"])?;
    let input = json!({ "checkouts": checkouts });
    let answer = match lease::serving(&app.repository) {
        Some(url) => ask_server(ctx, &url, id, &input)?,
        None => ctx.execute(id, input).map_err(map)?,
    };
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    match format {
        OutputFormat::Json => writeln!(out, "{}", pretty(&answer)).map_err(Error::Transport)?,
        OutputFormat::Text => render_status(&mut out, &answer, checkouts)?,
    }
    Ok(0)
}

fn render_status(out: &mut impl Write, s: &Value, checkouts: Checkouts) -> Result<()> {
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
    w(
        out,
        match checkouts {
            Checkouts::Repository => "servers:".into(),
            Checkouts::This => {
                "servers: (this checkout alone; --checkouts repository lists every checkout)"
                    .to_string()
            }
        },
    )?;
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

/// Where `ensure` records the process it started and that has not yet decided whether it
/// serves: beside the lease, in the local half.
///
/// # Why it exists
///
/// The election decides who serves, and it decides correctly however many processes stand
/// for it. What it cannot decide is *when* they stand. Three `ensure` calls that read an
/// absent lease in the same instant each started a process of their own (measured: three of
/// three, every round), one of them bound, and each call reported that it had started the
/// server. The two losers are harmless once they reach the election and defer. Until they
/// reach it, they are processes that will elect later — and on a loaded machine (the
/// instrumented coverage run is one) "later" came after `serve stop` had removed the
/// winner's lease: a loser then found no lease, became the server, and `stop` watched a
/// lease that would not go away and exited 10. So a stopped checkout did not stay stopped,
/// by a process nobody had asked for.
///
/// The claim makes the start itself single: the call that creates this file is the one that
/// starts a process, and every other call sees a start in progress and waits for its lease
/// instead of making another. The started process removes the claim once its election has
/// decided ([`release_spawn_claim`]); a claim whose process is gone, or that is older than a
/// process can take to decide, is taken over ([`spawn_claim_standing`]).
fn spawn_claim_path(repo: &Repository) -> PathBuf {
    lease::lease_path(repo).with_file_name("server.spawn")
}

/// The longest a started process can legitimately take between being started and removing
/// its claim: the election is bounded by [`lease::JOIN_TIMEOUT`], and a lease may be
/// binding for [`lease::BIND_GRACE`] before anybody may count it as abandoned. A claim older
/// than both no longer describes a process that is about to decide.
const SPAWN_CLAIM_BOUND: Duration =
    Duration::from_secs(lease::JOIN_TIMEOUT.as_secs() + lease::BIND_GRACE.as_secs());

/// What an existing start claim says about the process it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpawnClaim {
    /// A process was started and has not decided yet: wait for the lease it will write.
    Pending,
    /// Nothing is going to decide under this claim: take it over.
    Abandoned,
}

/// Classify a start claim from what it holds, how old it is and whether the process it
/// names is alive. Pure, so each branch is a unit test rather than a race.
///
/// The claim is written in two steps — created, then filled with the pid once the process
/// exists — so an empty or unreadable claim is a start in progress while it is young and an
/// abandoned one once it is old. A named process that is gone has decided nothing and never
/// will; one that is alive is waited on only while it could still be deciding, because a pid
/// can be reused by an unrelated process and an age bound is what keeps that from holding
/// every later start off for good.
fn spawn_claim_standing(content: &str, age: Duration, alive: impl Fn(u32) -> bool) -> SpawnClaim {
    if age > SPAWN_CLAIM_BOUND {
        return SpawnClaim::Abandoned;
    }
    match content.trim().parse::<u32>() {
        Ok(pid) if pid > 0 && alive(pid) => SpawnClaim::Pending,
        Ok(_) => SpawnClaim::Abandoned,
        Err(_) if age > lease::BIND_GRACE => SpawnClaim::Abandoned,
        Err(_) => SpawnClaim::Pending,
    }
}

/// Is a process with this pid alive? `kill(pid, 0)` delivers nothing and answers whether
/// the process exists; a process of another user answers `EPERM`, which is alive too.
#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: kill(2) with signal 0 performs the existence and permission checks only.
    let rc = unsafe { libc::kill(pid as libc::pid_t, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn process_alive(_: u32) -> bool {
    false
}

/// The outcome of trying to become the one call that starts a server.
enum SpawnTurn {
    /// This call holds the claim and must start the process.
    Mine(fs::File),
    /// Another call's process is starting; wait for its lease.
    Pending,
    /// The claim was taken over or changed under us; ask again on the next round.
    Retry,
}

/// Try to become the one call that starts a server for this checkout.
fn claim_spawn(repo: &Repository) -> Result<SpawnTurn> {
    let path = spawn_claim_path(repo);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => Ok(SpawnTurn::Mine(file)),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let content = fs::read_to_string(&path).unwrap_or_default();
            match spawn_claim_standing(&content, lease::file_age(&path), process_alive) {
                SpawnClaim::Pending => Ok(SpawnTurn::Pending),
                SpawnClaim::Abandoned => {
                    // remove only the claim that was judged: one written since is somebody's
                    // fresh start, and the next round reads that one
                    if fs::read_to_string(&path).unwrap_or_default() == content {
                        match fs::remove_file(&path) {
                            Ok(()) => {}
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                            Err(e) => return Err(Error::io(&path, e)),
                        }
                    }
                    Ok(SpawnTurn::Retry)
                }
            }
        }
        Err(e) => Err(Error::io(&path, e)),
    }
}

/// Remove the start claim when it names this process: called by a started server once its
/// election has decided. A claim naming anybody else is theirs, and is left alone.
fn release_spawn_claim(repo: &Repository) {
    let path = spawn_claim_path(repo);
    let mine = std::process::id().to_string();
    if fs::read_to_string(&path).is_ok_and(|c| c.trim() == mine) {
        let _ = fs::remove_file(&path);
    }
}

/// Where a checkout's runtime stands after a call that tried to make it stand somewhere,
/// and which arm of the loop the call left by.
///
/// This is the value both entry paths share. `serve ensure` renders it for a person and
/// turns it into an exit code; `majordomus env enter` reads it on the shell-prompt path
/// and says at most one line about it. Neither of them re-decides any of it, because two
/// readings of one state are two answers waiting to disagree — the claim
/// `project.interfaces-are-projections` makes of every other surface, and the reason the
/// convergence itself is one function rather than one per caller.
#[derive(Debug)]
pub struct Convergence {
    /// Where this checkout's server stands now.
    pub standing: ServerStanding,
    /// The lease it stands on, when there is one.
    pub document: Option<LeaseDocument>,
    /// Why the standing is not `ready`, in the words the standing itself gave.
    pub reason: Option<String>,
    /// Whether this call is what started the server that now stands there.
    pub started: bool,
    /// Whether the call ran out of `wait` rather than reaching a decision.
    pub timed_out: bool,
    /// Where a server this call started writes its log.
    pub log: PathBuf,
}

impl Convergence {
    /// Is the runtime there and answering for this checkout?
    pub fn ready(&self) -> bool {
        self.standing == ServerStanding::Ready
    }
}

/// Converge on a ready server for this checkout: the whole of what `serve ensure` does,
/// as a function, so that every entry path runs this one and not a copy of it.
///
/// The loop reads the lease and probes the server it names on every round, exactly as the
/// election does, and decides from the standing: `ready` ends it; `starting` waits;
/// `absent` and `stale` start a server once and then wait for it, because the election in
/// that process takes a stale lease over itself; `outdated` starts one only when the
/// election would take the lease over — the same executable, replaced on disk — and is
/// otherwise reported with the remedy, because a server of another build that answers is
/// not this command's to end. The whole call is bounded by `wait`
/// (`project.every-wait-is-bounded`).
///
/// `wait` of zero is a real value and not a degenerate one: it means *start what must be
/// started and return*, which is what a shell prompt asks for. The server then becomes
/// ready beside the shell rather than in front of it, and the entry file's `watch_file`
/// over the lease is what brings the published address into the environment a moment
/// later. Nothing is lost by not waiting: the election in the started process is what
/// decides who serves, and it decides that whether or not anybody is watching.
pub fn converge(repo: &Repository, port: u16, idle: u64, wait: Duration) -> Result<Convergence> {
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
        // A function of `started` rather than a closure over it: the loop assigns to
        // `started` in the arms below, and a closure that borrowed it would make that
        // assignment a borrow-check error.
        let settle = |started: bool, timed_out: bool| Convergence {
            standing,
            document: doc.clone(),
            reason: reason.clone(),
            started,
            timed_out,
            log: log.clone(),
        };
        match standing {
            ServerStanding::Ready => return Ok(settle(started, false)),
            ServerStanding::Starting => {}
            ServerStanding::Absent | ServerStanding::Stale if !started => {
                match claim_spawn(repo)? {
                    SpawnTurn::Mine(mut claim) => {
                        match spawn_server(repo, port, idle, &log) {
                            Ok(pid) => {
                                // the pid fills the claim in: from here on another call can
                                // tell a process that is deciding from one that is gone
                                claim
                                    .write_all(pid.to_string().as_bytes())
                                    .map_err(|e| Error::io(spawn_claim_path(repo), e))?;
                                started = true;
                            }
                            Err(e) => {
                                let _ = fs::remove_file(spawn_claim_path(repo));
                                return Err(e);
                            }
                        }
                    }
                    SpawnTurn::Pending | SpawnTurn::Retry => {}
                }
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
                    return Ok(settle(started, false));
                }
            }
        }
        if Instant::now() >= deadline {
            return Ok(settle(started, true));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// `serve ensure`: converge on a ready server for this checkout, and say where it stands.
///
/// The convergence is [`converge`]'s; what belongs to the command is the phrasing of a
/// standing that is not `ready` and the exit code that follows from it.
fn ensure(
    repo: &Repository,
    port: u16,
    idle: u64,
    wait: Duration,
    format: OutputFormat,
) -> Result<u8> {
    let c = converge(repo, port, idle, wait)?;
    let reason = if c.ready() {
        None
    } else if c.timed_out {
        Some(format!(
            "{}no server became ready within {} second(s); the log is {}",
            c.reason
                .as_ref()
                .map(|r| format!("{r}; "))
                .unwrap_or_default(),
            wait.as_secs(),
            c.log.display()
        ))
    } else if c.standing == ServerStanding::Outdated {
        Some(format!(
            "{}; `majordomus serve stop` ends it, and `serve ensure` then starts one from this executable",
            c.reason.clone().unwrap_or_default()
        ))
    } else {
        c.reason.clone()
    };
    report(c.standing, c.document, reason, c.started, &c.log, format)
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
/// beside the lease, in its own process group so that it outlives the shell that asked, and
/// carrying none of its parent's open file descriptors. The election in that process decides
/// whether it serves or defers; this only starts it.
///
/// # Why the descriptors are closed, and what happened when they were not
///
/// Standard input, output and error are redirected here, and for a long time that looked
/// like enough. It is not: `Command` only replaces those three, and **every other descriptor
/// the parent had open is inherited by the child**, which then holds it for its whole life —
/// fifteen minutes, by `DEFAULT_IDLE_SECONDS`.
///
/// direnv is where that stops being theoretical. It hands the file it evaluates an extra
/// descriptor of its own (fd 6 here), and it does not return until that pipe closes. On
/// 2026-09-11, the first real `cd` into a checkout after entry learnt to start a server hung
/// for **481 seconds** and only returned when the server was killed by hand: `lsof` showed
/// the server holding fd 3 on the other end of direnv's own pipe. Nothing in this
/// repository's tests could see it, because a test that captures a command's output in a
/// command substitution passes it no descriptor above two — the very thing direnv does.
///
/// So the child closes everything above the three it was given, between `fork` and `exec`.
/// The limit is read in the parent, because `pre_exec` runs in a forked child where only
/// async-signal-safe calls are allowed and `close(2)` is one while `sysconf(3)` is not
/// promised to be. `test/cases/190` holds it: an entry made with a descriptor open on a pipe
/// must leave that pipe closed when it returns.
fn spawn_server(repo: &Repository, port: u16, idle: u64, log: &Path) -> Result<u32> {
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
        // Read here, in the parent: see the note above on what `pre_exec` may call.
        // SAFETY: sysconf(3) with a standard name; it reads a limit and touches no memory
        // of ours.
        let limit = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
        // A machine with no answer, or an absurd one, still gets a bounded sweep: the cost
        // is one `close(2)` per descriptor on a path that is already forking a process.
        let highest: i32 = if limit < 4 {
            1024
        } else {
            limit.min(65_536) as i32
        };
        // SAFETY: the closure runs in the forked child before `exec`, and calls nothing but
        // `close(2)`, which is async-signal-safe. It touches no descriptor the child was
        // given — standard input, output and error are set above and are 0, 1 and 2 — and a
        // descriptor that was not open makes `close` fail harmlessly, which is why the
        // result is ignored.
        unsafe {
            cmd.pre_exec(move || {
                for fd in 3..highest {
                    libc::close(fd);
                }
                Ok(())
            });
        }
    }
    let child = cmd.spawn().map_err(|e| Error::io(log, e))?;
    tracing::info!(pid = child.id(), log = %log.display(), "started a server for this checkout");
    Ok(child.id())
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
        // What this command ends is *the server the lease named*, and what it waits for is
        // that server's lease — not the path. The two are the same thing right up until
        // another process takes the checkout over in the same instant, and then they are
        // not: a `serve` started a moment ago by a concurrent `serve ensure` may still be
        // in `lease::elect`, waiting for the very server this command just signalled, and
        // the file it finds freed is a file it creates again within milliseconds under a
        // token of its own. The path is then occupied without pause and this loop, reading
        // the path, waited out its whole bound and reported a server that would not stop —
        // of a server that had stopped before the first tick. So the token decides.
        match LeaseFile::read(&path).document() {
            None => {
                say(&mut out, format!("stopped {url} (pid {})", doc.pid))?;
                return Ok(0);
            }
            Some(d) if d.token != doc.token => {
                say(
                    &mut out,
                    format!(
                        "stopped {url} (pid {}); another process has taken this checkout's \
                         lease since (pid {}{})",
                        doc.pid,
                        d.pid,
                        d.url
                            .as_deref()
                            .map(|u| format!(" at {u}"))
                            .unwrap_or_else(|| ", still binding".into())
                    ),
                )?;
                return Ok(0);
            }
            Some(_) => {}
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    say(
        &mut out,
        format!(
            "asked pid {} at {url} to stop; its lease is still there after {} second(s)",
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

#[cfg(test)]
mod tests {
    use super::*;

    const YOUNG: Duration = Duration::ZERO;

    #[test]
    fn a_claim_naming_a_live_process_is_a_start_in_progress() {
        assert_eq!(
            spawn_claim_standing("4242", YOUNG, |_| true),
            SpawnClaim::Pending
        );
    }

    #[test]
    fn a_claim_naming_a_process_that_is_gone_is_taken_over() {
        assert_eq!(
            spawn_claim_standing("4242\n", YOUNG, |_| false),
            SpawnClaim::Abandoned
        );
        assert_eq!(
            spawn_claim_standing("0", YOUNG, |_| true),
            SpawnClaim::Abandoned,
            "pid 0 names no process a start could have made"
        );
    }

    #[test]
    fn an_unfilled_claim_waits_only_while_it_is_young() {
        assert_eq!(
            spawn_claim_standing("", YOUNG, |_| true),
            SpawnClaim::Pending
        );
        assert_eq!(
            spawn_claim_standing("", lease::BIND_GRACE + Duration::from_secs(1), |_| true),
            SpawnClaim::Abandoned
        );
    }

    #[test]
    fn a_live_pid_does_not_hold_a_start_off_past_the_bound() {
        // a pid reused by an unrelated process must not block every later start for good
        assert_eq!(
            spawn_claim_standing("4242", SPAWN_CLAIM_BOUND + Duration::from_secs(1), |_| true),
            SpawnClaim::Abandoned
        );
    }

    #[test]
    fn this_process_is_alive_and_an_unused_pid_is_not() {
        assert!(process_alive(std::process::id()));
        // the highest pid a kernel hands out is far below this on every supported platform
        assert!(!process_alive(i32::MAX as u32 - 1));
    }
}
