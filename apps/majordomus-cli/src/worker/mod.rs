//! Workers: what carries out an issue's work for an execution (ADR 0225, I3982).
//!
//! A worker is an adapter, never a requirement. Each [`Adapter`] says whether it can run here
//! and how its provider is invoked headless; [`choose`] takes the first available one and
//! answers, when none is, every adapter's reason, which the execution shows while it stays
//! queued. [`run`] starts the chosen adapter's process in the execution's worktree, as the
//! leader of a process group of its own, and folds every line it writes into the execution's
//! events through [`crate::execution::Progress`], in order. A deadline ends it, and so does a
//! cancellation the execution receives: in both cases the whole group is killed, so nothing
//! it started outlives the execution.
//!
//! ```text
//!   choose(adapters) ──► run(adapter, assignment, progress)
//!                              │ spawn in the worktree, own process group
//!                              │ stdout / stderr ──► Progress::stream ──► ExecutionEvent
//!                              ▼
//!                           Outcome: succeeded · failed(exit) · timed out · cancelled
//! ```
//!
//! Nothing here picks an issue, claims a scope or creates a worktree: `issue.implement`
//! (I3983) does those as earlier steps of the same execution and hands this module the
//! assignment.
//!
//! ```
//! use majordomus_cli::worker::{choose, Adapter, Assignment, Command};
//! struct Absent;
//! impl Adapter for Absent {
//!     fn id(&self) -> &str { "absent" }
//!     fn available(&self) -> Result<(), String> { Err("not installed here".into()) }
//!     fn command(&self, _: &Assignment) -> Command { Command::new("false") }
//! }
//! let adapters: Vec<Box<dyn Adapter>> = vec![Box::new(Absent)];
//! let refused = choose(&adapters).unwrap_err();
//! assert_eq!(refused, vec![("absent".to_string(), "not installed here".to_string())]);
//! ```

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

pub use std::process::Command;

use crate::execution::{LogStream, Progress};

/// How often a run looks at its process, its output, its deadline and its cancellation.
const POLL: Duration = Duration::from_millis(50);

/// What a worker is given: the execution it works for, where, on what, and for how long.
///
/// ```
/// use majordomus_cli::worker::Assignment;
/// let a = Assignment::new("x-20261023T000000Z-0a1b2c3d", "/tmp/wt", "Implement I3982")
///     .within(std::time::Duration::from_secs(60));
/// assert_eq!(a.deadline.as_secs(), 60);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// The execution's id, passed to the worker so its own logs can name it.
    pub execution: String,
    /// The worktree the worker runs in; never the primary checkout.
    pub worktree: PathBuf,
    /// What it is asked to do: the issue's objective and compiled context.
    pub prompt: String,
    /// How long it may run before its whole process group is killed.
    pub deadline: Duration,
}

impl Assignment {
    /// An assignment with the default deadline of one hour: long enough for a worker to
    /// implement and test one issue, short enough that a hung one does not hold its lease
    /// through the night.
    ///
    /// ```
    /// use majordomus_cli::worker::Assignment;
    /// let a = Assignment::new("x-20261023T000000Z-0a1b2c3d", "/tmp/wt", "Implement I3982");
    /// assert_eq!((a.deadline.as_secs(), a.prompt.as_str()), (3600, "Implement I3982"));
    /// ```
    pub fn new(
        execution: impl Into<String>,
        worktree: impl Into<PathBuf>,
        prompt: impl Into<String>,
    ) -> Self {
        Assignment {
            execution: execution.into(),
            worktree: worktree.into(),
            prompt: prompt.into(),
            deadline: Duration::from_secs(3600),
        }
    }

    /// The same assignment with another deadline, after which [`run`] kills the worker's
    /// whole process group and answers [`Outcome::TimedOut`].
    ///
    /// ```
    /// use majordomus_cli::worker::Assignment;
    /// use std::time::Duration;
    /// let a = Assignment::new("x", "/tmp/wt", "p").within(Duration::from_secs(90));
    /// assert_eq!(a.deadline, Duration::from_secs(90));
    /// ```
    pub fn within(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }
}

/// One way of running a provider headless. Implemented once per provider, beside nothing
/// else: the adapter decides only whether it can run and what process it starts.
///
/// ```
/// use majordomus_cli::worker::{Adapter, Assignment, Command};
/// struct Echo;
/// impl Adapter for Echo {
///     fn id(&self) -> &str { "echo" }
///     fn available(&self) -> Result<(), String> { Ok(()) }
///     fn command(&self, a: &Assignment) -> Command {
///         let mut c = Command::new("echo");
///         c.arg(&a.prompt);
///         c
///     }
/// }
/// let adapter: &dyn Adapter = &Echo;
/// assert_eq!(adapter.id(), "echo");
/// assert!(adapter.available().is_ok());
/// let cmd = adapter.command(&Assignment::new("x", "/tmp", "hello"));
/// assert_eq!(cmd.get_program(), "echo");
/// ```
pub trait Adapter: Send + Sync {
    /// The adapter's id, as an execution names it and as a refusal lists it when no
    /// adapter can run.
    ///
    /// ```
    /// use majordomus_cli::worker::{Adapter, ClaudeCode};
    /// assert_eq!(ClaudeCode::default().id(), "claude-code");
    /// ```
    fn id(&self) -> &str;
    /// Whether it can run on this machine now; the reason when it cannot, which the
    /// execution shows while it stays queued.
    ///
    /// ```
    /// use majordomus_cli::worker::{Adapter, Program};
    /// let missing = Program::new("x", "no-such-program-anywhere", &[]).available();
    /// assert!(missing.unwrap_err().contains("not on PATH"));
    /// ```
    fn available(&self) -> Result<(), String>;
    /// The process to start for an assignment. [`run`] sets the working directory, the
    /// standard streams and the process group; the adapter sets the program and arguments.
    ///
    /// ```
    /// use majordomus_cli::worker::{Adapter, Assignment, Program};
    /// let cmd = Program::new("p", "sh", &["-c"]).command(&Assignment::new("x", "/tmp", "true"));
    /// let args: Vec<_> = cmd.get_args().collect();
    /// assert_eq!(args, ["-c", "true"], "the prompt comes last");
    /// ```
    fn command(&self, assignment: &Assignment) -> Command;
}

/// An adapter is shown by its id: what an execution, a log line and a test failure name.
impl std::fmt::Debug for dyn Adapter + '_ {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Adapter({})", self.id())
    }
}

/// The first available adapter, or every adapter's reason when none is.
///
/// ```
/// use majordomus_cli::worker::{choose, Adapter, Assignment, Command, Program};
/// let adapters: Vec<Box<dyn Adapter>> = vec![Box::new(Program::new("echo", "echo", &["hi"]))];
/// assert_eq!(choose(&adapters).unwrap().id(), "echo");
/// assert!(choose(&[]).unwrap_err().is_empty(), "no adapter declared, no reason to give");
/// ```
pub fn choose(adapters: &[Box<dyn Adapter>]) -> Result<&dyn Adapter, Vec<(String, String)>> {
    let mut reasons = Vec::new();
    for adapter in adapters {
        match adapter.available() {
            Ok(()) => return Ok(adapter.as_ref()),
            Err(reason) => reasons.push((adapter.id().to_string(), reason)),
        }
    }
    Err(reasons)
}

/// How a worker's run ended: the step's outcome in the execution, and the only thing a
/// later step reads about the worker — whether its work is then verified is not decided
/// here (I3986).
///
/// ```
/// use majordomus_cli::worker::Outcome;
/// assert!(Outcome::Succeeded.ok());
/// assert!(!Outcome::TimedOut.ok());
/// assert_eq!(Outcome::Failed { exit: Some(2) }.to_string(), "the worker exited 2");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// It exited 0.
    Succeeded,
    /// It exited otherwise; `None` when a signal ended it.
    Failed {
        /// Its exit code.
        exit: Option<i32>,
    },
    /// The deadline ended it, and its process group was killed.
    TimedOut,
    /// The execution was cancelled, and its process group was killed.
    Cancelled,
}

impl Outcome {
    /// Did the worker exit 0? A timeout, a cancellation and a signal are never success,
    /// whatever the worker had written before it was stopped.
    ///
    /// ```
    /// use majordomus_cli::worker::Outcome;
    /// assert!(Outcome::Succeeded.ok());
    /// assert!(!Outcome::Cancelled.ok());
    /// assert!(!Outcome::Failed { exit: None }.ok());
    /// ```
    pub fn ok(&self) -> bool {
        matches!(self, Outcome::Succeeded)
    }
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::Succeeded => write!(f, "the worker succeeded"),
            Outcome::Failed { exit: Some(code) } => write!(f, "the worker exited {code}"),
            Outcome::Failed { exit: None } => write!(f, "the worker was ended by a signal"),
            Outcome::TimedOut => write!(f, "the worker ran past its deadline and was stopped"),
            Outcome::Cancelled => write!(f, "the execution was cancelled and the worker stopped"),
        }
    }
}

/// Run an adapter for an assignment, its output folded into the execution's events.
///
/// The process starts in the assignment's worktree with no standard input, as the leader of
/// a process group of its own. Each line it writes to standard output or standard error is
/// one log event of that stream, in the order it was read. The run ends when the process
/// exits, when the deadline passes or when the execution is cancelled; in the last two cases
/// the whole group is killed and reaped first. An error is a process that could not start.
///
/// ```
/// use majordomus_cli::execution::Progress;
/// use majordomus_cli::worker::{run, Assignment, Outcome, Program};
/// let dir = tempfile::tempdir().unwrap();
/// let echo = Program::new("echo", "echo", &["one"]);
/// let outcome = run(&echo, &Assignment::new("x", dir.path(), "say one"), &Progress::silent()).unwrap();
/// assert_eq!(outcome, Outcome::Succeeded);
/// ```
pub fn run(
    adapter: &dyn Adapter,
    assignment: &Assignment,
    progress: &Progress,
) -> Result<Outcome, String> {
    let mut cmd = adapter.command(assignment);
    cmd.current_dir(&assignment.worktree)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("MAJORDOMUS_EXECUTION_ID", &assignment.execution);
    let (mut child, _group) = crate::economics::runner::spawn_group(&mut cmd)
        .map_err(|e| format!("{}: {e}", adapter.id()))?;
    let (tx, rx) = std::sync::mpsc::channel::<(LogStream, String)>();
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|s| (LogStream::Stdout, Box::new(s) as Box<dyn Read + Send>)),
        child
            .stderr
            .take()
            .map(|s| (LogStream::Stderr, Box::new(s) as Box<dyn Read + Send>)),
    ]
    .into_iter()
    .flatten()
    .map(|(stream, reader)| {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines().map_while(Result::ok) {
                if tx.send((stream, line)).is_err() {
                    break;
                }
            }
        })
    })
    .collect();
    drop(tx);
    let started = Instant::now();
    let ended = loop {
        while let Ok((stream, line)) = rx.try_recv() {
            progress.stream(stream, line);
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break if status.success() {
                Outcome::Succeeded
            } else {
                Outcome::Failed {
                    exit: status.code(),
                }
            };
        }
        let stop = if progress.cancelled() {
            Some(Outcome::Cancelled)
        } else if started.elapsed() >= assignment.deadline {
            Some(Outcome::TimedOut)
        } else {
            None
        };
        if let Some(outcome) = stop {
            kill_group(&mut child);
            break outcome;
        }
        std::thread::sleep(POLL);
    };
    for reader in readers {
        let _ = reader.join();
    }
    // what the readers took after the last look
    while let Ok((stream, line)) = rx.try_recv() {
        progress.stream(stream, line);
    }
    Ok(ended)
}

/// Kill the child's whole process group and reap the child.
fn kill_group(child: &mut std::process::Child) {
    #[cfg(unix)]
    // SAFETY: the child leads its own group (`spawn_group`) and is not reaped yet, so the
    // group id is still its own and names no other process's group.
    unsafe {
        libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// An adapter that runs one program with fixed arguments, the prompt appended last: what a
/// test and a provider without its own invocation use. Available when the program resolves
/// on `PATH` or as a path.
///
/// ```
/// use majordomus_cli::worker::{Adapter, Program};
/// assert!(Program::new("sh", "sh", &["-c", "true"]).available().is_ok());
/// assert!(Program::new("none", "no-such-program-here", &[]).available().is_err());
/// ```
#[derive(Debug, Clone)]
pub struct Program {
    id: String,
    program: String,
    args: Vec<String>,
}

impl Program {
    /// An adapter called `id` that runs `program` with `args`, then the prompt.
    ///
    /// ```
    /// use majordomus_cli::worker::{Adapter, Program};
    /// let p = Program::new("fixture", "sh", &["-c", "echo done"]);
    /// assert_eq!(p.id(), "fixture");
    /// ```
    pub fn new(id: &str, program: &str, args: &[&str]) -> Self {
        Program {
            id: id.into(),
            program: program.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }
}

impl Adapter for Program {
    fn id(&self) -> &str {
        &self.id
    }

    fn available(&self) -> Result<(), String> {
        resolve(&self.program)
            .map(|_| ())
            .ok_or_else(|| format!("`{}` is not on PATH here", self.program))
    }

    fn command(&self, assignment: &Assignment) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args).arg(&assignment.prompt);
        cmd
    }
}

/// Claude Code, headless: `claude -p <prompt>` streaming JSON, in the worktree, with edits
/// accepted and nothing else assumed. Declared, never required: an execution with no
/// `claude` on this machine is told so and stays queued.
///
/// ```
/// use majordomus_cli::worker::{Adapter, Assignment, ClaudeCode};
/// let a = ClaudeCode::default().command(&Assignment::new("x", "/tmp", "do it"));
/// let args: Vec<_> = a.get_args().map(|s| s.to_string_lossy().into_owned()).collect();
/// assert_eq!(&args[..2], ["-p", "do it"]);
/// assert!(args.contains(&"stream-json".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct ClaudeCode {
    program: String,
}

impl Default for ClaudeCode {
    fn default() -> Self {
        ClaudeCode {
            program: "claude".into(),
        }
    }
}

impl Adapter for ClaudeCode {
    fn id(&self) -> &str {
        "claude-code"
    }

    fn available(&self) -> Result<(), String> {
        resolve(&self.program)
            .map(|_| ())
            .ok_or_else(|| "Claude Code (`claude`) is not installed on this machine".into())
    }

    fn command(&self, assignment: &Assignment) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.arg("-p")
            .arg(&assignment.prompt)
            .args(["--output-format", "stream-json", "--verbose"])
            .args(["--permission-mode", "acceptEdits"])
            .arg("--no-session-persistence");
        cmd
    }
}

/// The program's path: itself when it names a path, else the first match on `PATH`.
fn resolve(program: &str) -> Option<PathBuf> {
    let candidate = std::path::Path::new(program);
    if candidate.components().count() > 1 {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|p| p.is_file())
    })
}
