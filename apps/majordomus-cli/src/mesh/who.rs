//! Who runs this: the session behind a process, a path, an issue or an offloaded run (I3956).
//!
//! On 2026-10-22 a 33-minute remote `cargo test --doc` kept lundra at load 21, and naming the
//! session that started it took a trace by hand: the ssh client, its parent chain up to the
//! provider's process, that process's messaging socket, and a start time matched against the
//! session list. Slice 1 put the address on every board row; slice 2 made every offloaded run
//! carry its owner in its environment and in a run record on its host. This module joins
//! those facts into one answer, `majordomus mesh who <subject>`:
//!
//! - a **pid** (or `host:pid` on a declared offload host) is read where it runs — its parent
//!   chain, process group, start, command, working directory, the owner variables in its
//!   environment, a provider socket beside any process of the chain — and joined with the run
//!   records of that host, the board and the mesh claims;
//! - a **run tag** (or `host:tag`) is the run record its host keeps;
//! - an **issue** is every live mesh claim, announcement and run record that names it;
//! - a **path** is every board row, announcement, mesh claim and run record that covers it.
//!
//! A process whose parent is init and whose working directory is gone is a **leftover**, and
//! is named with its owner when its environment or a run record knows it. Nothing is guessed
//! from a name: a command line is reported, never used to decide whose a process is, and an
//! answer that cannot name an owner says `known: false` with every piece of evidence it
//! lacked. A remote host is read only through the offload path (its declared ssh address,
//! [`crate::offload::host::remote_of`]); nothing is signalled anywhere.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::capability::handler::Context;
use crate::mesh::state::ClaimView;
use crate::offload::owner::{owner_value, RunRecord};
use crate::offload::remote::Remote;
use crate::peers::{claims_meet, Peer, PeerAddress};

/// Where a provider keeps the sockets other sessions message it on, one per provider pid.
pub(crate) const SOCKET_DIR: &str = "/tmp/cc-socks";

/// The variable that moves [`SOCKET_DIR`] (a provider that keeps its sockets elsewhere, or a
/// test that cannot write the shared directory).
pub(crate) const SOCKET_DIR_ENV: &str = "MAJORDOMUS_WHO_SOCKET_DIR";

/// The variable that moves the provider's session registry from `~/.claude/sessions`, where
/// it keeps one `<pid>.json` per live session (its id, its name, its socket and the start of
/// its process). Only that file is read, never the `.key` beside it.
pub(crate) const SESSION_DIR_ENV: &str = "MAJORDOMUS_WHO_SESSION_DIR";

/// The longest parent chain walked; a chain is a handful of processes deep.
const MAX_CHAIN: usize = 40;

/// The facts a host reports about a process and the run records it keeps. Arguments: pid
/// (empty: no process), runs directory (empty: none), tag (empty: every record), socket
/// directory, the longest chain. Only the owner variables of an environment are printed, never a token.
const FACTS: &str = r#"pid=$1; runs=$2; tag=$3; socks=$4; max=$5; sessions=${6:-$HOME/.claude/sessions}
case $runs in ''|/*) ;; *) runs=$HOME/$runs ;; esac
keys='^(CLAUDE_CODE_SESSION_ID|CLAUDE_CODE_MESSAGING_SOCKET|CLAUDE_PID|MAJORDOMUS_OWNER_(SESSION|PEER|ISSUE|HOST|PID|RUN))='
p=$pid; n=0
while [ -n "$p" ] && [ "$n" -lt "$max" ]; do
  line=$(ps -o pid= -o ppid= -o pgid= -p "$p" 2>/dev/null | head -1)
  set -- $line
  [ "${1:-}" = "$p" ] || break
  pp=$2
  echo "proc $1 $2 $3"
  echo "start $p $(ps -o lstart= -p "$p" 2>/dev/null | head -1)"
  echo "cmd $p $(ps -ww -o command= -p "$p" 2>/dev/null | head -1)"
  if [ -r "/proc/$p/environ" ]; then tr '\0' '\n' < "/proc/$p/environ" 2>/dev/null
  else ps -E -ww -o command= -p "$p" 2>/dev/null | tr ' ' '\n'; fi | grep -E "$keys" | sed "s/^/env $p /"
  if [ -L "/proc/$p/cwd" ]; then c=$(readlink "/proc/$p/cwd" 2>/dev/null)
  else c=$(lsof -a -d cwd -p "$p" -Fn 2>/dev/null | sed -n 's/^n//p' | head -1); fi
  if [ -n "$c" ]; then echo "cwd $p $c"; [ -d "$c" ] || echo "gone $p"; fi
  if [ -S "$socks/$p.sock" ]; then echo "socket $p $socks/$p.sock"; fi
  if [ -f "$sessions/$p.json" ]; then
    echo "ustart $p $(TZ=UTC ps -o lstart= -p "$p" 2>/dev/null | head -1)"
    echo "session $p $(tr -d '\n' < "$sessions/$p.json")"
  fi
  [ "$pp" -gt 1 ] 2>/dev/null || break
  p=$pp; n=$((n+1))
done
if [ -n "$runs" ] && [ -d "$runs" ]; then
  echo "runs $runs"
  for f in "$runs"/*.run; do
    [ -f "$f" ] || continue
    if [ -n "$tag" ] && [ "$f" != "$runs/$tag.run" ]; then continue; fi
    echo "record $f"
    sed 's/^/| /' "$f"
  done
fi"#;

// ---------------------------------------------------------------- the subject

/// What `mesh who` is asked about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A process: on this machine, or on a declared offload host.
    Pid {
        /// The declared host, `None` for this machine.
        host: Option<String>,
        /// The process.
        pid: u32,
    },
    /// An offloaded run, by the tag its derive printed.
    Tag {
        /// The declared host; `None` is the declaration's only host.
        host: Option<String>,
        /// The tag.
        tag: String,
    },
    /// An issue id (`I3956`).
    Issue(String),
    /// A path: a checkout, or a file or directory of the repository.
    Path(String),
}

fn is_issue(s: &str) -> bool {
    s.len() > 1 && s.starts_with('I') && s[1..].chars().all(|c| c.is_ascii_digit())
}

fn pid_of(s: &str) -> Option<u32> {
    (!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .then(|| s.parse::<u32>().ok())
        .flatten()
        .filter(|p| *p > 0)
}

impl Subject {
    /// Read a subject: `I<digits>` is an issue; a path starting with `/` or `.`, or holding a
    /// `/`, is a path; `<host>:<pid>` and `<host>:<tag>` name a declared host; digits are a
    /// pid; ten to forty letters and digits are a run tag. Anything else is refused, naming
    /// the forms.
    pub fn parse(text: &str) -> Result<Subject, String> {
        let s = text.trim();
        let forms = "a pid, host:pid, run tag, host:tag, issue (I<digits>) or path";
        if s.is_empty() {
            return Err(format!("no subject: name {forms}"));
        }
        if is_issue(s) {
            return Ok(Subject::Issue(s.to_string()));
        }
        if s.starts_with('/') || s.starts_with('.') {
            return Ok(Subject::Path(s.to_string()));
        }
        if let Some((host, rest)) = s.split_once(':') {
            if !host.is_empty() && !host.contains('/') && owner_value(host).is_some() {
                if let Some(pid) = pid_of(rest) {
                    return Ok(Subject::Pid {
                        host: Some(host.to_string()),
                        pid,
                    });
                }
                if crate::offload::stop::valid_tag(rest) {
                    return Ok(Subject::Tag {
                        host: Some(host.to_string()),
                        tag: rest.to_string(),
                    });
                }
            }
            return Err(format!("'{s}' is not {forms}"));
        }
        if let Some(pid) = pid_of(s) {
            return Ok(Subject::Pid { host: None, pid });
        }
        if crate::offload::stop::valid_tag(s) {
            return Ok(Subject::Tag {
                host: None,
                tag: s.to_string(),
            });
        }
        if s.contains('/') {
            return Ok(Subject::Path(s.to_string()));
        }
        Err(format!("'{s}' is not {forms}"))
    }

    /// The kind, as the report names it.
    pub fn kind(&self) -> &'static str {
        match self {
            Subject::Pid { .. } => "pid",
            Subject::Tag { .. } => "tag",
            Subject::Issue(_) => "issue",
            Subject::Path(_) => "path",
        }
    }
}

// ---------------------------------------------------------------- the facts

/// One process of a parent chain, as its host reports it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ProcessFact {
    /// The process.
    pub pid: u32,
    /// Its parent; 1 is init, which a process whose parent died is handed to.
    pub ppid: u32,
    /// Its process group.
    pub pgid: u32,
    /// When it started, as the host's `ps` says it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    /// Its command line: reported, never used to decide whose it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Its working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Whether that directory no longer exists (a removed worktree).
    pub cwd_gone: bool,
    /// The provider's messaging socket for this pid, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    /// The owner variables of its environment (never a token).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// The provider's session registry entry for this pid, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session: Option<ProviderSession>,
    /// When it started, in UTC, as the provider's registry spells it.
    #[serde(skip)]
    pub started_utc: Option<String>,
}

/// What a provider's session registry says about one of its processes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ProviderSession {
    /// The provider's id for the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The session's name, as the provider shows it in its session list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The socket other sessions message it on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    /// When the registry says its process started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    /// Whether that start is this process's: a pid the host handed on since is not the
    /// session's, and its entry is then not used.
    pub verified: bool,
}

impl ProviderSession {
    /// Read the registry entry: its id, name, socket and start, each validated; nothing
    /// else of the file is kept.
    fn parse(json: &str, started_utc: Option<&str>) -> Option<ProviderSession> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        let text = |k: &str| v[k].as_str().map(str::trim).map(str::to_string);
        let words = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
        let started = text("procStart");
        let verified = match (&started, started_utc) {
            (Some(a), Some(b)) => !b.trim().is_empty() && words(a) == words(b),
            _ => false,
        };
        Some(ProviderSession {
            session: text("sessionId").and_then(|s| owner_value(&s)),
            name: text("name").and_then(|s| owner_value(&s)),
            socket: text("messagingSocketPath").and_then(|s| owner_value(&s)),
            started,
            verified,
        })
    }
}

/// A run record as it was read, or why it could not be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordRead {
    /// The record's file on its host.
    pub file: String,
    /// The record.
    pub record: Result<RunRecord, String>,
}

/// What a host reported: the chain from the process up, and its run records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// The process first, then its parent, up to the last process below init.
    pub chain: Vec<ProcessFact>,
    /// The runs directory, when the host has one.
    pub runs: Option<String>,
    /// Every record read there (only the tag's, when one was asked for).
    pub records: Vec<RecordRead>,
}

/// Read what [`FACTS`] printed.
pub fn parse_facts(text: &str) -> Facts {
    let mut facts = Facts::default();
    let mut record: Option<(String, String)> = None;
    let close = |facts: &mut Facts, record: &mut Option<(String, String)>| {
        if let Some((file, body)) = record.take() {
            facts.records.push(RecordRead {
                file,
                record: RunRecord::parse(&body),
            });
        }
    };
    for line in text.lines() {
        if let Some(body) = line
            .strip_prefix("| ")
            .or_else(|| (line == "|").then_some(""))
        {
            if let Some((_, text)) = record.as_mut() {
                text.push_str(body);
                text.push('\n');
            }
            continue;
        }
        close(&mut facts, &mut record);
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        match word {
            "proc" => {
                let n: Vec<u32> = rest
                    .split_whitespace()
                    .filter_map(|w| w.parse().ok())
                    .collect();
                if let [pid, ppid, pgid] = n[..] {
                    facts.chain.push(ProcessFact {
                        pid,
                        ppid,
                        pgid,
                        ..ProcessFact::default()
                    });
                }
            }
            "runs" => facts.runs = Some(rest.trim().to_string()),
            "record" => record = Some((rest.trim().to_string(), String::new())),
            _ => {
                let (pid, value) = rest.split_once(' ').unwrap_or((rest, ""));
                let Some(p) = pid
                    .parse::<u32>()
                    .ok()
                    .and_then(|pid| facts.chain.iter_mut().rfind(|p| p.pid == pid))
                else {
                    continue;
                };
                let value = value.trim();
                let some = || (!value.is_empty()).then(|| value.to_string());
                match word {
                    "start" => p.started = some(),
                    "cmd" => p.command = some(),
                    "cwd" => p.cwd = some().map(|c| c.trim_end_matches(" (deleted)").to_string()),
                    "gone" => p.cwd_gone = true,
                    "socket" => p.socket = some(),
                    "ustart" => p.started_utc = some(),
                    "session" => {
                        p.provider_session = ProviderSession::parse(value, p.started_utc.as_deref())
                    }
                    "env" => {
                        if let Some((k, v)) = value.split_once('=') {
                            p.env.insert(k.to_string(), v.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    close(&mut facts, &mut record);
    facts
}

// ---------------------------------------------------------------- the answer

/// One worker the answer names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WhoOwner {
    /// The session's id on the mesh (`claude-code-<id>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The board peer, when a board row is this worker's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    /// The checkout that board row is attached to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checkout: Option<PathBuf>,
    /// The session's name, as its provider shows it in its session list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The issue the work is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// Where the worker is reached: its machine, process, provider session and socket.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<PeerAddress>,
    /// What named each of these, one line per fact used.
    pub basis: Vec<String>,
}

/// One source of evidence, and what it gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WhoEvidence {
    /// `process`, `run_records`, `board` or `mesh_claims`.
    pub source: String,
    /// Whether the answer used it.
    pub used: bool,
    /// What it said, or why it said nothing.
    pub detail: String,
}

/// The answer of `mesh.who`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct WhoReport {
    /// The subject as asked.
    pub subject: String,
    /// `pid`, `tag`, `issue` or `path`.
    pub kind: String,
    /// The machine the subject was read on.
    pub host: String,
    /// Whether an owner was named; `false` is "unknown", with `missing` saying why.
    pub known: bool,
    /// Whether the process is a leftover: init is its parent and its directory is gone.
    pub leftover: bool,
    /// Every worker the evidence names.
    pub owners: Vec<WhoOwner>,
    /// The process and its parents, for a pid.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chain: Vec<ProcessFact>,
    /// The run record the subject belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<RunRecord>,
    /// Every source consulted.
    pub evidence: Vec<WhoEvidence>,
    /// What could not be named, and the evidence that would have named it.
    pub missing: Vec<String>,
}

/// Everything an answer is made from, gathered before any decision.
#[derive(Debug, Clone)]
pub struct Sources {
    /// The machine the subject is read on.
    pub host: String,
    /// What that host reported, or why it could not be asked.
    pub facts: Result<Facts, String>,
    /// The repository's board.
    pub board: Result<Vec<Peer>, String>,
    /// The mesh claims, live or not.
    pub claims: Result<Vec<ClaimView>, String>,
    /// The repository root, for paths.
    pub root: PathBuf,
}

fn ev(source: &str, used: bool, detail: impl Into<String>) -> WhoEvidence {
    WhoEvidence {
        source: source.into(),
        used,
        detail: detail.into(),
    }
}

/// The board row of a worker: the one carrying its mesh session, else the one whose address
/// is one of `pids` on `host`. An attached row wins over a departed one.
fn board_row<'a>(
    board: &'a [Peer],
    session: Option<&str>,
    host: &str,
    pids: &[u32],
) -> Option<&'a Peer> {
    let same = |p: &&Peer| {
        p.address.as_ref().is_some_and(|a| {
            let by_session = session.is_some() && a.mesh_session().as_deref() == session;
            let by_pid = a.host == host && a.pid.is_some_and(|pid| pids.contains(&pid));
            by_session || by_pid
        })
    };
    board
        .iter()
        .filter(same)
        .find(|p| p.attached)
        .or_else(|| board.iter().find(same))
}

/// Fill an owner from the board and the mesh claims, recording what each gave.
fn join(
    owner: &mut WhoOwner,
    src: &Sources,
    host: &str,
    pids: &[u32],
    used: &mut Vec<WhoEvidence>,
) {
    match &src.board {
        Ok(board) => match board_row(board, owner.session.as_deref(), host, pids) {
            Some(row) => {
                owner.peer.get_or_insert_with(|| row.id.to_string());
                owner.checkout = row.checkout.as_ref().map(|c| c.worktree.clone());
                if let Some(a) = &row.address {
                    owner.address = Some(a.clone());
                    if owner.session.is_none() {
                        owner.session = a.mesh_session();
                    }
                }
                owner.basis.push(format!(
                    "board row {}{} carries the address",
                    row.id,
                    row.checkout
                        .as_ref()
                        .map(|c| format!(" of {}", c.worktree.display()))
                        .unwrap_or_default()
                ));
                used.push(ev(
                    "board",
                    true,
                    format!("row {} is this worker's", row.id),
                ));
            }
            None => used.push(ev(
                "board",
                false,
                format!("{} rows, none whose address is this worker's", board.len()),
            )),
        },
        Err(e) => used.push(ev("board", false, format!("not read: {e}"))),
    }
    match &src.claims {
        Ok(claims) => {
            let live: Vec<&ClaimView> = claims
                .iter()
                .filter(|c| {
                    c.state.is_live()
                        && owner.session.is_some()
                        && claim_holder(&c.session).0 == owner.session
                })
                .collect();
            if let Some(c) = live.iter().find(|c| c.issue.is_some()) {
                if owner.issue.is_none() {
                    owner.issue = c.issue.clone();
                    owner
                        .basis
                        .push(format!("mesh claim {} names the issue", c.key));
                }
            }
            used.push(ev(
                "mesh_claims",
                !live.is_empty(),
                format!("{} live claims of this session", live.len()),
            ));
        }
        Err(e) => used.push(ev("mesh_claims", false, format!("not read: {e}"))),
    }
}

/// The worker a mesh claim's session names. A claim's session is `<stream>/<local>`, and the
/// local part is a provider session (`claude-code-<id>`), a task (`task-t-…`) or a board
/// peer of that runtime's server (`board-p9`): `(session, board peer)`.
fn claim_holder(session: &str) -> (Option<String>, Option<String>) {
    let local = session.rsplit_once('/').map_or(session, |(_, l)| l);
    match local.strip_prefix("board-") {
        Some(peer) => (None, Some(peer.to_string())),
        None => (Some(local.to_string()), None),
    }
}

/// What a field still lacks, for `missing`.
fn missing_of(owner: &WhoOwner, why_session: &str) -> Vec<String> {
    let mut out = Vec::new();
    if owner.session.is_none() {
        out.push(format!("session: {why_session}"));
    }
    if owner.peer.is_none() {
        out.push("peer: no board row carries this worker's session or one of its pids".to_string());
    }
    if owner.issue.is_none() {
        out.push(
            "issue: no MAJORDOMUS_OWNER_ISSUE, run record issue or live mesh claim names one"
                .to_string(),
        );
    }
    if owner.address.is_none() {
        out.push(
            "address: no provider process in the chain (no socket, no verified session \
             registry entry, no CLAUDE_PID among it) and no board row"
                .to_string(),
        );
    }
    out
}

fn fill_from_record(owner: &mut WhoOwner, r: &RunRecord, how: &str) {
    let o = &r.owner;
    let mut took = false;
    for (field, value) in [
        (&mut owner.session, &o.session),
        (&mut owner.peer, &o.peer),
        (&mut owner.issue, &o.issue),
    ] {
        if field.is_none() && value.is_some() {
            *field = value.clone();
            took = true;
        }
    }
    if owner.address.is_none() && !o.host.is_empty() {
        owner.address = Some(PeerAddress {
            provider: o
                .session
                .as_ref()
                .filter(|s| s.starts_with("claude-code-"))
                .map(|_| "claude-code".into()),
            host: o.host.clone(),
            pid: o.pid,
            session: o
                .session
                .as_deref()
                .and_then(|s| s.strip_prefix("claude-code-"))
                .map(str::to_string),
            socket: None,
        });
        took = true;
    }
    if took {
        owner
            .basis
            .push(format!("run record {} ({how}) names its owner", r.tag));
    }
}

/// The answer for a process, from its chain and its host's run records.
fn for_process(subject: &str, src: &Sources) -> WhoReport {
    let mut report = WhoReport {
        subject: subject.into(),
        kind: "pid".into(),
        host: src.host.clone(),
        known: false,
        leftover: false,
        owners: Vec::new(),
        chain: Vec::new(),
        run: None,
        evidence: Vec::new(),
        missing: Vec::new(),
    };
    let facts = match &src.facts {
        Ok(f) if !f.chain.is_empty() => f,
        Ok(_) => {
            report.evidence.push(ev(
                "process",
                false,
                format!("no process {subject} on {}", src.host),
            ));
            report
                .missing
                .push(format!("process: {} reports no such process", src.host));
            return report;
        }
        Err(e) => {
            report.evidence.push(ev(
                "process",
                false,
                format!("{} could not be asked: {e}", src.host),
            ));
            report.missing.push(format!("process: {e}"));
            return report;
        }
    };
    let chain = &facts.chain;
    let me = &chain[0];
    report.leftover = me.ppid == 1 && me.cwd_gone;
    let pids: Vec<u32> = chain.iter().map(|p| p.pid).collect();
    let mut owner = WhoOwner::default();
    let env = |k: &str| {
        chain.iter().find_map(|p| {
            p.env
                .get(k)
                .and_then(|v| owner_value(v))
                .map(|v| (p.pid, v))
        })
    };
    if let Some((pid, s)) = env("MAJORDOMUS_OWNER_SESSION") {
        owner.session = Some(s);
        owner.basis.push(format!(
            "MAJORDOMUS_OWNER_SESSION in the environment of pid {pid}"
        ));
    } else if let Some((pid, s)) = env("CLAUDE_CODE_SESSION_ID") {
        owner.session = Some(format!("claude-code-{s}"));
        owner.basis.push(format!(
            "CLAUDE_CODE_SESSION_ID in the environment of pid {pid}"
        ));
    }
    for (key, field) in [
        ("MAJORDOMUS_OWNER_PEER", &mut owner.peer),
        ("MAJORDOMUS_OWNER_ISSUE", &mut owner.issue),
    ] {
        if let Some((pid, v)) = env(key) {
            *field = Some(v);
            owner
                .basis
                .push(format!("{key} in the environment of pid {pid}"));
        }
    }
    // the provider's registry entry for a process of the chain whose start it matches
    let registered = chain.iter().find_map(|p| {
        p.provider_session
            .as_ref()
            .filter(|s| s.verified)
            .map(|s| (p.pid, s))
    });
    if let Some((pid, s)) = registered {
        if owner.session.is_none() {
            if let Some(id) = &s.session {
                owner.session = Some(format!("claude-code-{id}"));
            }
        }
        owner.name = s.name.clone();
        owner.basis.push(format!(
            "the provider's session registry entry for pid {pid}, whose start matches"
        ));
    }
    for p in chain {
        if let Some(s) = p.provider_session.as_ref().filter(|s| !s.verified) {
            owner.basis.push(format!(
                "not used: the registry entry for pid {} is from an earlier process (started {})",
                p.pid,
                s.started.as_deref().unwrap_or("?")
            ));
        }
    }
    // the provider's own process: one with a socket or a verified registry entry, or the pid
    // CLAUDE_PID names, among the chain on this host — a fact, never a command name
    let provider = chain
        .iter()
        .find(|p| p.socket.is_some())
        .map(|p| (p.pid, p.socket.clone(), "its socket exists"))
        .or_else(|| registered.map(|(pid, s)| (pid, s.socket.clone(), "the registry names it")))
        .or_else(|| {
            env("CLAUDE_PID")
                .and_then(|(_, v)| v.parse::<u32>().ok())
                .filter(|p| pids.contains(p))
                .map(|p| {
                    (
                        p,
                        env("CLAUDE_CODE_MESSAGING_SOCKET").map(|(_, s)| s),
                        "CLAUDE_PID names it",
                    )
                })
        });
    if let Some((pid, socket, why)) = &provider {
        owner.address = Some(PeerAddress {
            provider: Some("claude-code".into()),
            host: src.host.clone(),
            pid: Some(*pid),
            session: owner
                .session
                .as_deref()
                .and_then(|s| s.strip_prefix("claude-code-"))
                .map(str::to_string),
            socket: socket.clone(),
        });
        owner
            .basis
            .push(format!("provider process {pid} in the chain ({why})"));
    }
    report.evidence.push(ev(
        "process",
        true,
        format!(
            "pid {} (parent {}, group {}{}), a chain of {} up to {}",
            me.pid,
            me.ppid,
            me.pgid,
            me.cwd
                .as_deref()
                .map(|c| format!(", cwd {c}{}", if me.cwd_gone { " (gone)" } else { "" }))
                .unwrap_or_default(),
            chain.len(),
            provider
                .as_ref()
                .map(|(p, _, _)| format!("provider pid {p}"))
                .unwrap_or_else(|| "no provider process".into())
        ),
    ));
    // the run record: by the tag the process carries, else by its group, else by its shell
    let tag = env("MAJORDOMUS_OWNER_RUN").map(|(_, t)| t);
    let records: Vec<&RunRecord> = facts
        .records
        .iter()
        .filter_map(|r| r.record.as_ref().ok())
        .collect();
    let matched = tag
        .as_ref()
        .and_then(|t| {
            records
                .iter()
                .find(|r| &r.tag == t)
                .map(|r| (*r, "its tag"))
        })
        .or_else(|| {
            records
                .iter()
                .find(|r| r.pgid == Some(me.pgid))
                .map(|r| (*r, "its process group"))
        })
        .or_else(|| {
            records
                .iter()
                .find(|r| pids.contains(&r.pid))
                .map(|r| (*r, "its shell"))
        });
    match (&facts.runs, matched) {
        (_, Some((r, how))) => {
            fill_from_record(&mut owner, r, how);
            report.evidence.push(ev(
                "run_records",
                true,
                format!("run {} matched by {how}", r.tag),
            ));
            report.run = Some(r.clone());
        }
        (Some(dir), None) => report.evidence.push(ev(
            "run_records",
            false,
            format!(
                "{} records under {dir}, none with its tag, group or shell",
                records.len()
            ),
        )),
        (None, None) => report.evidence.push(ev(
            "run_records",
            false,
            format!(
                "{} has no run records directory (it runs no offloaded derive)",
                src.host
            ),
        )),
    }
    let mut joins = Vec::new();
    let mut host_pids = pids.clone();
    if let Some(p) = owner.address.as_ref().and_then(|a| a.pid) {
        host_pids.push(p);
    }
    let addr_host = owner
        .address
        .as_ref()
        .map(|a| a.host.clone())
        .unwrap_or_else(|| src.host.clone());
    join(&mut owner, src, &addr_host, &host_pids, &mut joins);
    report.evidence.extend(joins);
    report.missing = missing_of(
        &owner,
        "no MAJORDOMUS_OWNER_SESSION or CLAUDE_CODE_SESSION_ID in the environment of its chain, \
         no run record and no board row names one",
    );
    report.known = owner.session.is_some() || owner.peer.is_some();
    if report.leftover {
        owner.basis.push(format!(
            "a leftover: init is its parent and its directory {} is gone",
            me.cwd.as_deref().unwrap_or("?")
        ));
    }
    report.chain = chain.clone();
    if report.known {
        report.owners.push(owner);
    }
    report
}

/// The answer for a run tag, from the record its host keeps.
fn for_tag(subject: &str, tag: &str, src: &Sources) -> WhoReport {
    let mut report = empty(subject, "tag", src);
    let found = match &src.facts {
        Ok(f) => f
            .records
            .iter()
            .find(|r| r.file.ends_with(&format!("/{tag}.run")))
            .map(|r| r.record.clone())
            .ok_or_else(|| match &f.runs {
                Some(dir) => format!(
                    "{} holds no {dir}/{tag}.run: the run ended, or never ran there",
                    src.host
                ),
                None => format!("{} keeps no run records", src.host),
            })
            .and_then(|r| r),
        Err(e) => Err(format!("{} could not be asked: {e}", src.host)),
    };
    match found {
        Ok(r) => {
            let mut owner = WhoOwner::default();
            fill_from_record(&mut owner, &r, "its tag");
            report.evidence.push(ev(
                "run_records",
                true,
                format!("run {tag}, group {:?}, cwd {}", r.pgid, r.cwd),
            ));
            let pids: Vec<u32> = r.owner.pid.into_iter().collect();
            let mut joins = Vec::new();
            let host = r.owner.host.clone();
            join(&mut owner, src, &host, &pids, &mut joins);
            report.evidence.extend(joins);
            report.missing = missing_of(&owner, "the run record names no session");
            report.known = owner.session.is_some() || owner.peer.is_some();
            report.run = Some(r);
            if report.known {
                report.owners.push(owner);
            }
        }
        Err(e) => {
            report.evidence.push(ev("run_records", false, e.clone()));
            report.missing.push(format!("run record: {e}"));
        }
    }
    report
}

fn empty(subject: &str, kind: &str, src: &Sources) -> WhoReport {
    WhoReport {
        subject: subject.into(),
        kind: kind.into(),
        host: src.host.clone(),
        known: false,
        leftover: false,
        owners: Vec::new(),
        chain: Vec::new(),
        run: None,
        evidence: Vec::new(),
        missing: Vec::new(),
    }
}

/// Whether an announcement's own words name the issue as a word of their own.
fn names_issue(text: &str, issue: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == issue)
}

/// Gather owners from many sources (an issue or a path): one per session or peer, every
/// basis kept, each joined with the board for its row and address.
struct Gathered {
    by_key: BTreeMap<String, WhoOwner>,
}

impl Gathered {
    fn add(
        &mut self,
        session: Option<String>,
        peer: Option<&Peer>,
        issue: Option<String>,
        basis: String,
    ) {
        let session = session.or_else(|| {
            peer.and_then(|p| p.address.as_ref())
                .and_then(PeerAddress::mesh_session)
        });
        let key = session
            .clone()
            .or_else(|| {
                peer.map(|p| {
                    format!(
                        "{}@{}",
                        p.id,
                        p.checkout
                            .as_ref()
                            .map(|c| c.id.clone())
                            .unwrap_or_default()
                    )
                })
            })
            .unwrap_or_else(|| basis.clone());
        let o = self.by_key.entry(key).or_default();
        if o.session.is_none() {
            o.session = session;
        }
        if let Some(p) = peer {
            o.peer.get_or_insert_with(|| p.id.to_string());
            if o.checkout.is_none() {
                o.checkout = p.checkout.as_ref().map(|c| c.worktree.clone());
            }
            if o.address.is_none() {
                o.address = p.address.clone();
            }
        }
        if o.issue.is_none() {
            o.issue = issue;
        }
        o.basis.push(basis);
    }

    fn add_claim(&mut self, c: &ClaimView, basis: String) {
        let (session, peer) = claim_holder(&c.session);
        let o = self
            .by_key
            .entry(session.clone().unwrap_or_else(|| c.session.clone()))
            .or_default();
        if o.session.is_none() {
            o.session = session;
        }
        if o.peer.is_none() {
            o.peer = peer;
        }
        if o.issue.is_none() {
            o.issue = c.issue.clone();
        }
        o.basis.push(basis);
    }

    fn finish(self, src: &Sources, report: &mut WhoReport) {
        for (_, mut o) in self.by_key {
            if o.peer.is_none() {
                if let (Ok(board), Some(s)) = (&src.board, o.session.as_deref()) {
                    if let Some(row) = board_row(board, Some(s), "", &[]) {
                        o.peer = Some(row.id.to_string());
                        o.checkout = row.checkout.as_ref().map(|c| c.worktree.clone());
                        o.address = row.address.clone();
                        o.basis
                            .push(format!("board row {} carries the session", row.id));
                    }
                }
            }
            report.owners.push(o);
        }
        report.known = !report.owners.is_empty();
    }
}

fn source_detail(name: &str, r: &Result<usize, &String>) -> WhoEvidence {
    match r {
        Ok(n) => ev(name, *n > 0, format!("{n} matched")),
        Err(e) => ev(name, false, format!("not read: {e}")),
    }
}

fn records_of(src: &Sources) -> Vec<RunRecord> {
    src.facts
        .as_ref()
        .map(|f| {
            f.records
                .iter()
                .filter_map(|r| r.record.clone().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn runs_detail(src: &Sources, n: usize) -> WhoEvidence {
    match &src.facts {
        Ok(Facts {
            runs: Some(dir), ..
        }) => ev(
            "run_records",
            n > 0,
            format!("{n} of the records under {dir} matched"),
        ),
        Ok(_) => ev(
            "run_records",
            false,
            format!("{} keeps no run records", src.host),
        ),
        Err(e) => ev("run_records", false, format!("not read: {e}")),
    }
}

/// The answer for an issue: every live mesh claim, announcement and run record naming it.
fn for_issue(issue: &str, src: &Sources) -> WhoReport {
    let mut report = empty(issue, "issue", src);
    let mut g = Gathered {
        by_key: BTreeMap::new(),
    };
    let claims = src.claims.as_ref().map(|claims| {
        let mut n = 0;
        for c in claims
            .iter()
            .filter(|c| c.state.is_live() && c.issue.as_deref() == Some(issue))
        {
            n += 1;
            g.add_claim(c, format!("mesh claim {} names {issue}", c.key));
        }
        n
    });
    let board = src.board.as_ref().map(|board| {
        let mut n = 0;
        for p in board {
            for a in p.claims.iter().chain(p.announcement.iter()) {
                if names_issue(&a.intent, issue) {
                    n += 1;
                    g.add(
                        None,
                        Some(p),
                        Some(issue.to_string()),
                        format!(
                            "{} announced {issue}{}",
                            p.id,
                            a.name
                                .as_deref()
                                .map(|n| format!(" (claim {n})"))
                                .unwrap_or_default()
                        ),
                    );
                    break;
                }
            }
        }
        n
    });
    let records = records_of(src);
    let mut runs = 0;
    for r in records
        .iter()
        .filter(|r| r.owner.issue.as_deref() == Some(issue))
    {
        runs += 1;
        g.add(
            r.owner.session.clone(),
            None,
            Some(issue.into()),
            format!("run record {} on {} names {issue}", r.tag, r.host),
        );
    }
    g.finish(src, &mut report);
    report.evidence = vec![
        source_detail("mesh_claims", &claims),
        source_detail("board", &board),
        runs_detail(src, runs),
    ];
    if !report.known {
        report.missing.push(format!(
            "owner: no live mesh claim, board announcement or run record names {issue}"
        ));
    }
    report
}

/// A path relative to `base`, when it lies under it; a relative path is taken as is.
fn relative(path: &str, base: &Path) -> Option<String> {
    let p = Path::new(path);
    if p.is_relative() {
        return Some(
            path.trim_start_matches("./")
                .trim_end_matches('/')
                .to_string(),
        );
    }
    let canon = |q: &Path| q.canonicalize().unwrap_or_else(|_| q.to_path_buf());
    p.strip_prefix(base)
        .ok()
        .map(Path::to_path_buf)
        .or_else(|| {
            canon(p)
                .strip_prefix(canon(base))
                .ok()
                .map(Path::to_path_buf)
        })
        .map(|r| r.to_string_lossy().into_owned())
}

/// The answer for a path: every board row, announcement, mesh claim and run record over it.
fn for_path(path: &str, src: &Sources) -> WhoReport {
    let mut report = empty(path, "path", src);
    let mut g = Gathered {
        by_key: BTreeMap::new(),
    };
    let meets = |rel: &str, scope: &str| {
        rel.is_empty() || scope == "." || claims_meet(rel, scope.trim_end_matches('/'))
    };
    let board = src.board.as_ref().map(|board| {
        let mut n = 0;
        for p in board {
            let worktree = p
                .checkout
                .as_ref()
                .map(|c| c.worktree.clone())
                .unwrap_or_else(|| src.root.clone());
            let Some(rel) = relative(path, &worktree) else {
                continue;
            };
            for a in p.claims.iter().chain(p.announcement.iter()) {
                if let Some(s) = a.scope.iter().find(|s| meets(&rel, s)) {
                    n += 1;
                    g.add(None, Some(p), None, format!("{} announced scope {s}", p.id));
                    break;
                }
            }
        }
        n
    });
    let rel_root = relative(path, &src.root);
    let claims = src.claims.as_ref().map(|claims| {
        let mut n = 0;
        if let Some(rel) = &rel_root {
            for c in claims.iter().filter(|c| c.state.is_live()) {
                if let Some(s) = c.scope.iter().find(|s| meets(rel, s)) {
                    n += 1;
                    g.add_claim(c, format!("mesh claim {} covers {s}", c.key));
                }
            }
        }
        n
    });
    let records = records_of(src);
    let mut runs = 0;
    for r in records.iter().filter(|r| {
        !r.cwd.is_empty() && (r.cwd == path || path.starts_with(&format!("{}/", r.cwd)))
    }) {
        runs += 1;
        g.add(
            r.owner.session.clone(),
            None,
            r.owner.issue.clone(),
            format!("run record {} runs in {}", r.tag, r.cwd),
        );
    }
    g.finish(src, &mut report);
    report.evidence = vec![
        source_detail("board", &board),
        source_detail("mesh_claims", &claims),
        runs_detail(src, runs),
    ];
    if !report.known {
        report.missing.push(format!(
            "owner: no board announcement, live mesh claim or run record covers {path}"
        ));
    }
    report
}

/// The answer, from sources already gathered: no process is asked and nothing is read here.
pub fn answer(text: &str, subject: &Subject, src: &Sources) -> WhoReport {
    let mut report = match subject {
        Subject::Pid { .. } => for_process(text, src),
        Subject::Tag { tag, .. } => for_tag(text, tag, src),
        Subject::Issue(i) => for_issue(i, src),
        Subject::Path(p) => for_path(p, src),
    };
    report.subject = text.to_string();
    report.kind = subject.kind().to_string();
    report
}

// ---------------------------------------------------------------- gathering

/// The provider's session registry: [`SESSION_DIR_ENV`] when set, else empty, which the
/// host's script reads as its own `~/.claude/sessions`.
fn session_dir() -> String {
    std::env::var(SESSION_DIR_ENV)
        .ok()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_default()
}

/// The socket directory: [`SOCKET_DIR_ENV`] when set, else [`SOCKET_DIR`].
fn socket_dir() -> String {
    std::env::var(SOCKET_DIR_ENV)
        .ok()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| SOCKET_DIR.to_string())
}

/// Ask a host for its facts.
/// Where a host's provider keeps its sockets and its session registry.
struct Places {
    socks: String,
    sessions: String,
}

impl Places {
    /// The provider's own places, unless the environment moves them.
    fn of_env() -> Places {
        Places {
            socks: socket_dir(),
            sessions: session_dir(),
        }
    }
}

fn ask(
    remote: &Remote,
    pid: Option<u32>,
    runs: Option<&str>,
    tag: Option<&str>,
    places: &Places,
) -> Result<Facts, String> {
    let pid = pid.map(|p| p.to_string()).unwrap_or_default();
    let out = remote
        .run(
            FACTS,
            &[
                &pid,
                runs.unwrap_or(""),
                tag.unwrap_or(""),
                &places.socks,
                &MAX_CHAIN.to_string(),
                &places.sessions,
            ],
            None,
        )
        .map_err(|e| e.to_string())?;
    match out.code {
        Some(0) => Ok(parse_facts(&out.stdout)),
        other => Err(format!("ended with {other:?}: {}", out.stderr.trim())),
    }
}

/// Whether this process is the server of the checkout at `root`.
fn serving(root: &Path) -> bool {
    let half = crate::capability::builtin::server::local_half(root);
    crate::lease::LeaseFile::read(&crate::lease::lease_file(root, &half))
        .document()
        .is_some_and(|d| d.pid == std::process::id())
}

/// The server a mesh question goes to, from a `peers.list` answer: this checkout's when it is
/// ready, otherwise the first ready one in the order git lists the checkouts (the primary
/// first) — the mesh is the repository's, as `majordomus mesh` already reads it.
fn ready_server(list: &serde_json::Value) -> Option<String> {
    let boards = list["boards"].as_array()?;
    let ready = |b: &&serde_json::Value| b["standing"] == "ready" && b["url"].is_string();
    boards
        .iter()
        .filter(ready)
        .find(|b| b["this_checkout"] == true)
        .or_else(|| boards.iter().find(|b| ready(b)))
        .and_then(|b| b["url"].as_str().map(str::to_string))
}

/// The repository's board: every checkout's, this checkout's from its own server when this
/// process is not it; and the server mesh questions go to.
fn board(ctx: &Context, root: &Path) -> (Result<Vec<Peer>, String>, Option<String>) {
    let v = match ctx.execute(
        "peers.list",
        serde_json::json!({ "checkouts": "repository" }),
    ) {
        Ok(v) => v,
        Err(e) => return (Err(e.to_string()), None),
    };
    let url = ready_server(&v);
    (board_of_list(&v, root), url)
}

fn board_of_list(v: &serde_json::Value, root: &Path) -> Result<Vec<Peer>, String> {
    let mut peers: Vec<Peer> =
        serde_json::from_value(v["peers"].clone()).map_err(|e| e.to_string())?;
    if !serving(root) {
        let own = crate::offload::owner::board_of_checkout(root);
        let checkout = peers
            .iter()
            .find_map(|p| p.checkout.clone().filter(|c| c.this_checkout));
        peers.retain(|p| !p.checkout.as_ref().is_some_and(|c| c.this_checkout));
        peers.extend(own.into_iter().map(|mut p| {
            p.checkout = checkout.clone();
            p
        }));
    }
    Ok(peers)
}

/// The mesh claims: this process's own when it cooperates, else those of the server `url`.
fn claims(ctx: &Context, root: &Path, url: Option<String>) -> Result<Vec<ClaimView>, String> {
    if let Some(c) = ctx.mesh.cooperation() {
        return Ok(c.state().claims);
    }
    if serving(root) {
        return Err(ctx
            .mesh
            .cooperation_status()
            .reason
            .unwrap_or_else(|| "cooperation is not active".into()));
    }
    let url = url.ok_or("no server of this repository is ready to ask")?;
    let path = ctx
        .registry
        .get("mesh.state")
        .and_then(|c| c.exposure.http.as_ref())
        .map(|h| h.path.clone())
        .ok_or("mesh.state declares no HTTP route")?;
    let reply = crate::mcp::bridge::request(
        &url,
        "GET",
        &format!("{path}?view=full"),
        &[],
        None,
        crate::lease::timings().probe_timeout,
    )
    .map_err(|e| format!("{url} did not answer ({e})"))?;
    if reply.status != 200 {
        return Err(format!("{url} answered {}", reply.status));
    }
    let v: serde_json::Value = serde_json::from_str(&reply.body).map_err(|e| e.to_string())?;
    if v["active"] != serde_json::json!(true) {
        return Err(v["reason"]
            .as_str()
            .unwrap_or("cooperation is not active")
            .to_string());
    }
    serde_json::from_value(v["state"]["claims"].clone()).map_err(|e| e.to_string())
}

/// Where a subject is read: this machine, or a declared offload host through its ssh path;
/// and that host's runs directory, when an offload queue is declared.
fn reach(
    ctx: &Context,
    host: Option<&str>,
    need_host: bool,
) -> Result<(Remote, String, Option<String>), String> {
    let declared = crate::offload::from_objects(&ctx.index.objects)?;
    let runs = declared
        .as_ref()
        .map(|d| crate::offload::owner::runs_directory(&d.queue.directory));
    if host.is_none() && !need_host {
        return Ok((Remote::local(), crate::integration::drain::host(), runs));
    }
    let d = declared.ok_or(
        "this repository declares no offload host; a remote process is read through the \
         offload path only",
    )?;
    let h =
        match host {
            Some(name) => d.host(name).cloned().ok_or_else(|| {
                format!(
                "'{name}' is not a declared offload host ({}); a remote process is read through \
                 the offload path only",
                d.hosts.iter().map(|h| h.id.as_str()).collect::<Vec<_>>().join(", ")
            )
            })?,
            None if d.hosts.len() == 1 => d.hosts[0].clone(),
            None => {
                return Err(format!(
                    "{} offload hosts are declared; name the run's as host:tag",
                    d.hosts.len()
                ))
            }
        };
    Ok((crate::offload::host::remote_of(&h), h.id.clone(), runs))
}

/// Answer `mesh who` for `text`, gathering every source first.
pub(crate) fn who(ctx: &Context, text: &str, subject: &Subject) -> WhoReport {
    let root = PathBuf::from(&ctx.index.repository.root);
    let (host, pid, tag, need_host) = match subject {
        Subject::Pid { host, pid } => (host.as_deref(), Some(*pid), None, false),
        Subject::Tag { host, tag } => (host.as_deref(), None, Some(tag.as_str()), true),
        _ => (None, None, None, false),
    };
    let reached = reach(ctx, host, need_host);
    let (label, facts) = match reached {
        Ok((remote, label, runs)) => {
            let facts = ask(&remote, pid, runs.as_deref(), tag, &Places::of_env());
            (label, facts)
        }
        Err(e) => (host.unwrap_or("this machine").to_string(), Err(e)),
    };
    let (board, server) = board(ctx, &root);
    let src = Sources {
        host: label,
        facts,
        board,
        claims: claims(ctx, &root, server),
        root,
    };
    answer(text, subject, &src)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::journal::{ClaimMode, StreamId};
    use crate::mesh::state::ClaimState;
    use crate::offload::owner::{RunOwner, RUN_SCHEMA};
    use crate::peers::{PeerBoard, Transport};

    const SID: &str = "877e4b89";
    const STREAM: &str = "641bdb94dae9f62268abf5ca73c11e15-b10f19aa2cd888a6-8d2e04b2b18ebbdb";

    fn claim(session: &str, issue: Option<&str>, scope: &[&str], state: ClaimState) -> ClaimView {
        ClaimView {
            key: format!("k-{session}"),
            stream: StreamId::new(&"a".repeat(32), &"b".repeat(16), &"c".repeat(16)).unwrap(),
            runtime: "r".into(),
            session: session.into(),
            scope: scope.iter().map(|s| s.to_string()).collect(),
            intent: None,
            mode: ClaimMode::Advisory,
            issue: issue.map(str::to_string),
            acquired_lamport: 1,
            acquired_seq: 1,
            state,
        }
    }

    fn address(session: Option<&str>, pid: u32, host: &str) -> PeerAddress {
        PeerAddress {
            provider: session.map(|_| "claude-code".into()),
            host: host.into(),
            pid: Some(pid),
            session: session.map(str::to_string),
            socket: Some(format!("/tmp/cc-socks/{pid}.sock")),
        }
    }

    /// A board of `(address, announcement, scope, attached)` rows.
    /// One board row: its address, its announcement, the announced scope, and whether it
    /// is still attached.
    type Row<'a> = (Option<PeerAddress>, Option<&'a str>, &'a [&'a str], bool);

    fn board(rows: &[Row<'_>]) -> Vec<Peer> {
        let b = PeerBoard::new();
        for (a, intent, scope, attached) in rows {
            let id = b.attach(Transport::Http);
            if let Some(a) = a {
                b.locate(&id, a.clone());
            }
            if let Some(i) = intent {
                b.announce(&id, i, scope.iter().map(|s| s.to_string()).collect());
            }
            if !attached {
                b.detach(&id);
            }
        }
        b.list()
    }

    fn record(
        tag: &str,
        pgid: u32,
        pid: u32,
        session: Option<&str>,
        issue: Option<&str>,
    ) -> RunRecord {
        RunRecord {
            tag: tag.into(),
            pid,
            pgid: Some(pgid),
            started_at: 1,
            host: "lundra".into(),
            cwd: format!("/home/dev/offload-{tag}"),
            owner: RunOwner {
                session: session.map(str::to_string),
                peer: None,
                issue: issue.map(str::to_string),
                host: "mac".into(),
                pid: Some(24025),
            },
        }
    }

    fn proc(pid: u32, ppid: u32, pgid: u32) -> ProcessFact {
        ProcessFact {
            pid,
            ppid,
            pgid,
            ..ProcessFact::default()
        }
    }

    fn sources(
        facts: Result<Facts, String>,
        board: Result<Vec<Peer>, String>,
        claims: Result<Vec<ClaimView>, String>,
    ) -> Sources {
        Sources {
            host: "mac".into(),
            facts,
            board,
            claims,
            root: PathBuf::from("/repo"),
        }
    }

    fn used(r: &WhoReport, source: &str) -> bool {
        r.evidence.iter().any(|e| e.source == source && e.used)
    }

    #[test]
    fn every_subject_form_is_read_and_anything_else_is_refused() {
        let pid = |host: Option<&str>, pid| Subject::Pid {
            host: host.map(str::to_string),
            pid,
        };
        let tag = |host: Option<&str>, t: &str| Subject::Tag {
            host: host.map(str::to_string),
            tag: t.into(),
        };
        for (text, want) in [
            ("24025", pid(None, 24025)),
            (" lundra:24025 ", pid(Some("lundra"), 24025)),
            ("083ddafbbc", tag(None, "083ddafbbc")),
            ("lundra:083ddafbbc", tag(Some("lundra"), "083ddafbbc")),
            ("I3956", Subject::Issue("I3956".into())),
            ("/repo/apps", Subject::Path("/repo/apps".into())),
            ("./apps", Subject::Path("./apps".into())),
            (
                "apps/majordomus-cli",
                Subject::Path("apps/majordomus-cli".into()),
            ),
        ] {
            assert_eq!(Subject::parse(text).as_ref(), Ok(&want), "{text}");
        }
        assert_eq!(pid(None, 1).kind(), "pid");
        assert_eq!(tag(None, "x").kind(), "tag");
        assert_eq!(Subject::Issue("I1".into()).kind(), "issue");
        assert_eq!(Subject::Path("/".into()).kind(), "path");
        for bad in [
            "",
            "  ",
            "0",
            "lundra:",
            ":12",
            "lundra:x y",
            "bad host:12",
            "cargo",
            "I",
            "Ix1",
        ] {
            let e = Subject::parse(bad).expect_err(bad);
            assert!(e.contains("pid") && e.contains("path"), "{bad}: {e}");
        }
    }

    #[test]
    fn the_facts_a_host_prints_are_read_back() {
        let text = format!(
            "proc 30 20 30\nstart 30 Thu Oct 22 10:00:00 2026\ncmd 30 rustc --crate-name x\n\
             env 30 MAJORDOMUS_OWNER_RUN=083ddafbbc\nenv 30 NOEQUALS\ncwd 30 /home/w (deleted)\ngone 30\n\
             proc 20 1 20\ncmd 20\nsocket 20 /tmp/cc-socks/20.sock\nenv 99 CLAUDE_PID=1\nbogus 20 x\n\
             runs /q.runs\nrecord /q.runs/a.run\n| {RUN_SCHEMA}\n| tag a\n| pid 5\n| host h\n|\n\
             record /q.runs/b.run\n| not a record\nproc x y z\n"
        );
        let f = parse_facts(&text);
        assert_eq!(f.chain.len(), 2);
        let me = &f.chain[0];
        assert_eq!((me.pid, me.ppid, me.pgid), (30, 20, 30));
        assert_eq!(me.started.as_deref(), Some("Thu Oct 22 10:00:00 2026"));
        assert_eq!(me.command.as_deref(), Some("rustc --crate-name x"));
        assert_eq!((me.cwd.as_deref(), me.cwd_gone), (Some("/home/w"), true));
        assert_eq!(me.env.len(), 1, "a line without '=' is no variable");
        assert_eq!(f.chain[1].command, None, "an empty command is none");
        assert_eq!(f.chain[1].socket.as_deref(), Some("/tmp/cc-socks/20.sock"));
        assert_eq!(f.runs.as_deref(), Some("/q.runs"));
        assert_eq!(f.records.len(), 2);
        assert_eq!(f.records[0].record.as_ref().unwrap().tag, "a");
        assert!(
            f.records[1].record.is_err(),
            "a record that does not parse says why"
        );
        assert_eq!(parse_facts(""), Facts::default());
    }

    /// A local worker's process: its own environment names the session, the provider's
    /// socket in its chain is the address, the board row carries the peer, and a live mesh
    /// claim of that session names the issue.
    #[test]
    fn a_workers_process_is_named_by_its_environment_socket_board_and_claims() {
        let mut me = proc(30, 20, 30);
        me.env.insert("CLAUDE_CODE_SESSION_ID".into(), SID.into());
        let mut provider = proc(20, 10, 20);
        provider.socket = Some("/tmp/cc-socks/20.sock".into());
        let facts = Facts {
            chain: vec![me, provider, proc(10, 1, 10)],
            ..Facts::default()
        };
        let b = board(&[
            (Some(address(Some("other"), 99, "mac")), None, &[], true),
            (Some(address(Some(SID), 20, "mac")), Some("work"), &[], true),
        ]);
        let claims = vec![
            claim(
                &format!("claude-code-{SID}"),
                Some("I0001"),
                &["x"],
                ClaimState::Released,
            ),
            claim(
                &format!("{STREAM}/claude-code-{SID}"),
                Some("I3956"),
                &["apps"],
                ClaimState::Held,
            ),
        ];
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Ok(facts), Ok(b), Ok(claims)),
        );
        assert!(r.known && !r.leftover, "{r:?}");
        let o = &r.owners[0];
        assert_eq!(o.session.as_deref(), Some("claude-code-877e4b89"));
        assert_eq!(
            (o.peer.as_deref(), o.issue.as_deref()),
            (Some("p2"), Some("I3956"))
        );
        assert_eq!(o.address.as_ref().and_then(|a| a.pid), Some(20));
        assert!(used(&r, "process") && used(&r, "board") && used(&r, "mesh_claims"));
        assert!(!used(&r, "run_records"));
        assert!(r.missing.is_empty(), "{:?}", r.missing);
        assert_eq!(r.chain.len(), 3);
        let text = serde_json::to_string(&r).unwrap();
        assert!(text.contains("\"known\":true"), "{text}");
    }

    /// A leftover on a build host carries nothing in its environment that is read here; the
    /// run record its group matches names the owner, and the answer says it is a leftover.
    #[test]
    fn a_leftover_is_named_by_the_run_record_of_its_group() {
        let mut me = proc(30, 1, 7);
        me.cwd = Some("/home/dev/offload-083ddafbbc".into());
        me.cwd_gone = true;
        let facts = Facts {
            chain: vec![me],
            runs: Some("/q.runs".into()),
            records: vec![
                RecordRead {
                    file: "/q.runs/x.run".into(),
                    record: Err("bad".into()),
                },
                RecordRead {
                    file: "/q.runs/083ddafbbc.run".into(),
                    record: Ok(record(
                        "083ddafbbc",
                        7,
                        6,
                        Some("claude-code-s1"),
                        Some("I3956"),
                    )),
                },
            ],
        };
        let src = Sources {
            host: "lundra".into(),
            ..sources(Ok(facts), Err("no server".into()), Err("inactive".into()))
        };
        let r = answer("lundra:30", &Subject::parse("lundra:30").unwrap(), &src);
        assert!(r.known && r.leftover, "{r:?}");
        let o = &r.owners[0];
        assert_eq!(
            (o.session.as_deref(), o.issue.as_deref()),
            (Some("claude-code-s1"), Some("I3956"))
        );
        let a = o.address.as_ref().unwrap();
        assert_eq!(
            (a.host.as_str(), a.pid, a.session.as_deref()),
            ("mac", Some(24025), Some("s1"))
        );
        assert!(
            o.basis.iter().any(|b| b.contains("process group")),
            "{:?}",
            o.basis
        );
        assert!(
            o.basis.iter().any(|b| b.contains("leftover")),
            "{:?}",
            o.basis
        );
        assert_eq!(r.run.as_ref().map(|r| r.tag.as_str()), Some("083ddafbbc"));
        assert!(r
            .evidence
            .iter()
            .any(|e| e.source == "board" && e.detail.contains("no server")));
        assert!(
            r.missing.iter().any(|m| m.starts_with("peer:")),
            "{:?}",
            r.missing
        );
    }

    /// The tag a process carries wins over a group the host may have handed on, and a run
    /// shell among the chain is matched when neither is there; CLAUDE_PID names the provider
    /// when no socket does.
    #[test]
    fn the_run_is_matched_by_tag_then_group_then_shell_and_claude_pid_names_the_provider() {
        let records = || {
            vec![
                RecordRead {
                    file: "/q/a.run".into(),
                    record: Ok(record("aaaaaaaaaa", 7, 6, Some("claude-code-old"), None)),
                },
                RecordRead {
                    file: "/q/b.run".into(),
                    record: Ok(record("bbbbbbbbbb", 8, 20, Some("claude-code-new"), None)),
                },
            ]
        };
        let mut me = proc(30, 20, 7);
        me.env
            .insert("MAJORDOMUS_OWNER_RUN".into(), "bbbbbbbbbb".into());
        me.env.insert("MAJORDOMUS_OWNER_PEER".into(), "p4".into());
        me.env.insert("CLAUDE_PID".into(), "20".into());
        me.env
            .insert("CLAUDE_CODE_MESSAGING_SOCKET".into(), "/s/20.sock".into());
        let facts = Facts {
            chain: vec![me, proc(20, 1, 20)],
            runs: Some("/q".into()),
            records: records(),
        };
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Ok(facts), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert_eq!(
            r.run.as_ref().unwrap().tag,
            "bbbbbbbbbb",
            "its tag, not group 7's record"
        );
        let o = &r.owners[0];
        assert_eq!(
            (o.session.as_deref(), o.peer.as_deref()),
            (Some("claude-code-new"), Some("p4"))
        );
        let a = o.address.as_ref().unwrap();
        assert_eq!((a.pid, a.socket.as_deref()), (Some(20), Some("/s/20.sock")));
        assert!(
            o.basis.iter().any(|b| b.contains("CLAUDE_PID")),
            "{:?}",
            o.basis
        );

        let facts = Facts {
            chain: vec![proc(31, 20, 9), proc(20, 1, 20)],
            runs: Some("/q".into()),
            records: records(),
        };
        let r = answer(
            "31",
            &Subject::parse("31").unwrap(),
            &sources(Ok(facts), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert_eq!(
            r.run.as_ref().unwrap().tag,
            "bbbbbbbbbb",
            "its shell, pid 20"
        );
        assert!(r.owners[0].basis.iter().any(|b| b.contains("its shell")));
    }

    /// With no evidence the answer is unknown and lists what each field lacked; with no
    /// process, or a host that could not be asked, it says that.
    #[test]
    fn without_evidence_the_answer_is_unknown_with_what_is_missing() {
        let facts = Facts {
            chain: vec![proc(30, 20, 30)],
            ..Facts::default()
        };
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Ok(facts), Ok(Vec::new()), Err("off".into())),
        );
        assert!(!r.known && r.owners.is_empty());
        assert_eq!(r.missing.len(), 4, "{:?}", r.missing);
        assert!(r.missing[0].starts_with("session:"));
        assert!(r.evidence.iter().any(
            |e| e.source == "run_records" && e.detail.contains("has no run records directory")
        ));
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Ok(Facts::default()), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(
            !r.known && r.missing[0].contains("no such process"),
            "{r:?}"
        );
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Err("ssh: refused".into()), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(!r.known && r.missing[0].contains("ssh: refused"), "{r:?}");
        let facts = Facts {
            chain: vec![proc(30, 20, 30)],
            runs: Some("/q".into()),
            ..Facts::default()
        };
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Ok(facts), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(r
            .evidence
            .iter()
            .any(|e| e.source == "run_records" && e.detail.contains("none with its tag")));
    }

    /// The provider's session registry names the session, its name and its socket when the
    /// start it records is the process's; an entry from an earlier process of that pid is
    /// reported and not used. Nothing but the four fields is kept.
    #[test]
    fn the_providers_session_registry_names_a_session_whose_start_matches() {
        let entry = r#"{"pid":20,"sessionId":"877e4b89","name":"prismatic-majordomus-39",
            "messagingSocketPath":"/tmp/cc-socks/20.sock","procStart":"Thu Oct 22 15:21:51 2026",
            "status":"busy"}"#
            .replace('\n', "");
        let text = format!(
            "proc 30 20 30\nproc 20 1 20\nustart 20 Thu Oct 22  15:21:51 2026\nsession 20 {entry}\n"
        );
        let f = parse_facts(&text);
        let s = f.chain[1].provider_session.clone().unwrap();
        assert!(s.verified, "the start matches, whitespace aside: {s:?}");
        assert_eq!(s.name.as_deref(), Some("prismatic-majordomus-39"));
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("busy"),
            "only the named fields are kept: {json}"
        );
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Ok(f), Ok(Vec::new()), Ok(Vec::new())),
        );
        let o = &r.owners[0];
        assert_eq!(o.session.as_deref(), Some("claude-code-877e4b89"));
        assert_eq!(o.name.as_deref(), Some("prismatic-majordomus-39"));
        let a = o.address.as_ref().unwrap();
        assert_eq!(
            (a.pid, a.socket.as_deref()),
            (Some(20), Some("/tmp/cc-socks/20.sock"))
        );

        let stale = format!(
            "proc 30 20 30\nproc 20 1 20\nustart 20 Fri Oct 23 09:00:00 2026\nsession 20 {entry}\n"
        );
        let r = answer(
            "30",
            &Subject::parse("30").unwrap(),
            &sources(Ok(parse_facts(&stale)), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(!r.known, "a reused pid's entry names nobody: {r:?}");
        let unstarted = format!("proc 20 1 20\nsession 20 {entry}\n");
        let f = parse_facts(&unstarted);
        assert!(
            !f.chain[0].provider_session.as_ref().unwrap().verified,
            "no start, no match"
        );
        assert!(ProviderSession::parse("not json", None).is_none());
    }

    #[test]
    fn a_tag_is_the_record_its_host_keeps() {
        let rec = RecordRead {
            file: "/q/083ddafbbc.run".into(),
            record: Ok(record(
                "083ddafbbc",
                7,
                6,
                Some(&format!("claude-code-{SID}")),
                Some("I3956"),
            )),
        };
        let b = board(&[(Some(address(Some(SID), 24025, "mac")), None, &[], true)]);
        let facts = Facts {
            records: vec![rec],
            runs: Some("/q".into()),
            ..Facts::default()
        };
        let subject = Subject::parse("083ddafbbc").unwrap();
        let r = answer(
            "083ddafbbc",
            &subject,
            &sources(Ok(facts), Ok(b), Ok(Vec::new())),
        );
        assert!(r.known, "{r:?}");
        assert_eq!(r.owners[0].peer.as_deref(), Some("p1"));
        assert_eq!(
            r.owners[0]
                .address
                .as_ref()
                .and_then(|a| a.socket.as_deref()),
            Some("/tmp/cc-socks/24025.sock")
        );
        let none = Facts {
            runs: Some("/q".into()),
            ..Facts::default()
        };
        let r = answer(
            "083ddafbbc",
            &subject,
            &sources(Ok(none), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(
            !r.known && r.missing[0].contains("holds no /q/083ddafbbc.run"),
            "{r:?}"
        );
        let r = answer(
            "083ddafbbc",
            &subject,
            &sources(Ok(Facts::default()), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(r.missing[0].contains("keeps no run records"), "{r:?}");
        let r = answer(
            "083ddafbbc",
            &subject,
            &sources(Err("gone".into()), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(r.missing[0].contains("could not be asked"), "{r:?}");
        let bad = Facts {
            records: vec![RecordRead {
                file: "/q/083ddafbbc.run".into(),
                record: Err("not a run record".into()),
            }],
            ..Facts::default()
        };
        let r = answer(
            "083ddafbbc",
            &subject,
            &sources(Ok(bad), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(r.missing[0].contains("not a run record"), "{r:?}");
        let anonymous = Facts {
            records: vec![RecordRead {
                file: "/q/083ddafbbc.run".into(),
                record: Ok(record("083ddafbbc", 7, 6, None, None)),
            }],
            ..Facts::default()
        };
        let r = answer(
            "083ddafbbc",
            &subject,
            &sources(Ok(anonymous), Ok(Vec::new()), Ok(Vec::new())),
        );
        assert!(
            !r.known && r.missing[0].contains("names no session"),
            "{r:?}"
        );
    }

    /// An issue is every live claim, announcement and record that names it, one owner per
    /// session, each joined with its board row.
    #[test]
    fn an_issue_is_answered_by_claims_announcements_and_records() {
        let b = board(&[
            (
                Some(address(Some(SID), 20, "mac")),
                Some("T10c I3956 slice 3"),
                &[],
                true,
            ),
            (None, Some("I39560 is another issue"), &[], true),
        ]);
        let claims = vec![
            claim(
                &format!("{STREAM}/claude-code-{SID}"),
                Some("I3956"),
                &["apps"],
                ClaimState::Held,
            ),
            claim(
                &format!("{STREAM}/board-p9"),
                Some("I3956"),
                &["apps"],
                ClaimState::Held,
            ),
            claim(
                "claude-code-gone",
                Some("I3956"),
                &["apps"],
                ClaimState::Expired("x".into()),
            ),
        ];
        let rec = RecordRead {
            file: "/q/a.run".into(),
            record: Ok(record(
                "aaaaaaaaaa",
                7,
                6,
                Some("claude-code-remote"),
                Some("I3956"),
            )),
        };
        let facts = Facts {
            records: vec![rec],
            runs: Some("/q".into()),
            ..Facts::default()
        };
        let r = answer(
            "I3956",
            &Subject::parse("I3956").unwrap(),
            &sources(Ok(facts), Ok(b), Ok(claims)),
        );
        assert!(r.known);
        let sessions: Vec<Option<&str>> = r.owners.iter().map(|o| o.session.as_deref()).collect();
        assert_eq!(
            sessions,
            [
                None,
                Some("claude-code-877e4b89"),
                Some("claude-code-remote")
            ],
            "{r:?}"
        );
        assert_eq!(
            r.owners[0].peer.as_deref(),
            Some("p9"),
            "a board claim names its peer"
        );
        let mine = &r.owners[1];
        assert_eq!(mine.peer.as_deref(), Some("p1"));
        assert_eq!(
            mine.basis.len(),
            2,
            "the claim and the announcement: {:?}",
            mine.basis
        );
        assert!(used(&r, "mesh_claims") && used(&r, "board") && used(&r, "run_records"));
        let r = answer(
            "I1",
            &Subject::parse("I1").unwrap(),
            &sources(Err("x".into()), Err("y".into()), Err("z".into())),
        );
        assert!(!r.known && r.missing[0].contains("I1"));
        assert!(
            r.evidence
                .iter()
                .all(|e| !e.used && e.detail.contains("not read")),
            "{:?}",
            r.evidence
        );
        assert!(names_issue("T10 I3956: s3", "I3956") && !names_issue("I39560", "I3956"));
    }

    /// A path is every announcement, live mesh claim and run record that covers it.
    #[test]
    fn a_path_is_answered_by_announced_scopes_claims_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let b = board(&[
            (
                Some(address(Some(SID), 20, "mac")),
                Some("who"),
                &["apps/majordomus-cli/src/mesh/"],
                true,
            ),
            (None, Some("docs"), &["docs"], true),
        ]);
        let claims = vec![claim(
            "claude-code-x",
            Some("I7"),
            &["apps"],
            ClaimState::Held,
        )];
        let rec = RecordRead {
            file: "/q/a.run".into(),
            record: Ok(record("aaaaaaaaaa", 7, 6, Some("claude-code-r"), None)),
        };
        let facts = Facts {
            records: vec![rec],
            runs: Some("/q".into()),
            ..Facts::default()
        };
        let src = Sources {
            root: root.clone(),
            ..sources(Ok(facts), Ok(b), Ok(claims))
        };
        let path = format!("{}/apps/majordomus-cli/src/mesh/who.rs", root.display());
        let r = answer(&path, &Subject::parse(&path).unwrap(), &src);
        let sessions: Vec<Option<&str>> = r.owners.iter().map(|o| o.session.as_deref()).collect();
        assert_eq!(
            sessions,
            [Some("claude-code-877e4b89"), Some("claude-code-x")],
            "{r:?}"
        );
        assert_eq!(r.owners[1].issue.as_deref(), Some("I7"));
        let r = answer(
            "/home/dev/offload-aaaaaaaaaa/src",
            &Subject::parse("/home/dev/offload-aaaaaaaaaa/src").unwrap(),
            &src,
        );
        assert_eq!(r.owners.len(), 1, "{r:?}");
        assert_eq!(r.owners[0].session.as_deref(), Some("claude-code-r"));
        let r = answer("elsewhere/x", &Subject::parse("elsewhere/x").unwrap(), &src);
        assert!(!r.known && r.missing[0].contains("elsewhere/x"), "{r:?}");
        assert_eq!(relative("./apps/", &root).as_deref(), Some("apps"));
        assert_eq!(relative("/nowhere/at/all", &root), None);
    }

    #[test]
    fn a_board_row_is_matched_by_session_or_pid_and_attached_wins() {
        let b = board(&[
            (
                Some(address(Some(SID), 20, "mac")),
                Some("gone"),
                &[],
                false,
            ),
            (Some(address(Some(SID), 21, "mac")), None, &[], true),
            (Some(address(None, 30, "lundra")), None, &[], true),
        ]);
        let s = format!("claude-code-{SID}");
        assert_eq!(
            board_row(&b, Some(&s), "mac", &[])
                .map(|p| p.id.to_string())
                .as_deref(),
            Some("p2")
        );
        assert_eq!(
            board_row(&b, None, "lundra", &[30])
                .map(|p| p.id.to_string())
                .as_deref(),
            Some("p3")
        );
        assert_eq!(
            board_row(&b, None, "mac", &[30]),
            None,
            "a pid is a pid on its host"
        );
        let gone = board(&[(
            Some(address(Some(SID), 20, "mac")),
            Some("gone"),
            &[],
            false,
        )]);
        assert_eq!(
            board_row(&gone, Some(&s), "", &[])
                .map(|p| p.id.to_string())
                .as_deref(),
            Some("p1")
        );
    }

    /// The facts script, run for real on this machine: this test's own process and its
    /// parent, and the run records of a directory, all of them or only the tag's.
    #[test]
    fn the_facts_script_reads_a_real_process_and_its_records() {
        let dir = tempfile::tempdir().unwrap();
        let runs = dir.path().join("q.runs");
        std::fs::create_dir_all(&runs).unwrap();
        for tag in ["aaaaaaaaaa", "bbbbbbbbbb"] {
            std::fs::write(
                runs.join(format!("{tag}.run")),
                format!("{RUN_SCHEMA}\ntag {tag}\npid 5\nhost h\n"),
            )
            .unwrap();
        }
        let me = std::process::id();
        let runs_text = runs.to_string_lossy().into_owned();
        // this process's socket and registry entry, in places of the test's own
        let socks = dir.path().join("socks");
        let sessions = dir.path().join("sessions");
        std::fs::create_dir_all(&socks).unwrap();
        std::fs::create_dir_all(&sessions).unwrap();
        let _socket =
            std::os::unix::net::UnixListener::bind(socks.join(format!("{me}.sock"))).unwrap();
        let utc = std::process::Command::new("ps")
            .env("TZ", "UTC")
            .args(["-o", "lstart=", "-p", &me.to_string()])
            .output()
            .unwrap();
        let utc = String::from_utf8_lossy(&utc.stdout).trim().to_string();
        std::fs::write(
            sessions.join(format!("{me}.json")),
            serde_json::json!({ "pid": me, "sessionId": "s-test", "name": "unit",
                "procStart": utc, "messagingSocketPath": "/x.sock" })
            .to_string(),
        )
        .unwrap();
        let places = Places {
            socks: socks.to_string_lossy().into_owned(),
            sessions: sessions.to_string_lossy().into_owned(),
        };
        let none = Places {
            socks: "/nonexistent".into(),
            sessions: "/nonexistent".into(),
        };
        let f = ask(&Remote::local(), Some(me), Some(&runs_text), None, &places).unwrap();
        assert_eq!(
            f.chain[0].socket,
            Some(format!("{}/{me}.sock", places.socks)),
            "{f:?}"
        );
        let entry = f.chain[0]
            .provider_session
            .clone()
            .expect("the registry entry");
        assert!(entry.verified, "its start is this process's: {entry:?}");
        assert_eq!(entry.session.as_deref(), Some("s-test"));
        assert_eq!(f.chain[0].pid, me, "{f:?}");
        assert_eq!(f.chain[0].ppid, std::os::unix::process::parent_id());
        assert!(
            f.chain[0].command.is_some() && f.chain[0].started.is_some(),
            "{f:?}"
        );
        assert!(f.chain.len() >= 2, "a parent chain: {f:?}");
        assert_eq!(f.runs.as_deref(), Some(runs_text.as_str()));
        assert_eq!(f.records.len(), 2);
        let f = ask(
            &Remote::local(),
            None,
            Some(&runs_text),
            Some("bbbbbbbbbb"),
            &none,
        )
        .unwrap();
        assert!(f.chain.is_empty());
        assert_eq!(f.records.len(), 1);
        assert_eq!(f.records[0].record.as_ref().unwrap().tag, "bbbbbbbbbb");
        let f = ask(&Remote::local(), Some(u32::MAX), None, None, &none).unwrap();
        assert!(
            f.chain.is_empty() && f.runs.is_none(),
            "no such process, no runs: {f:?}"
        );
        let broken = Remote::through(Path::new("/nonexistent/ssh-stand-in"));
        assert!(ask(&broken, Some(1), None, None, &none).is_err());
        let stopped = Remote::through(Path::new("/usr/bin/false"));
        let e = ask(&stopped, Some(1), None, None, &none).unwrap_err();
        assert!(e.contains("ended with"), "{e}");
    }

    fn context(
        declared: Option<serde_json::Value>,
    ) -> (crate::synthetic::SyntheticRepository, Context) {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let mut index = repo.index().unwrap();
        if let Some(metadata) = declared {
            index.objects.push(crate::model::Object {
                kind: crate::offload::declaration::KIND.into(),
                identity: "derive".into(),
                uri: "majordomus://derive-offload/derive".into(),
                title: None,
                description: None,
                metadata,
                body: String::new(),
                content: String::new(),
                media_type: "application/yaml",
                provenance: crate::model::Provenance {
                    path: ".ai/repo/offload/derive.yaml".into(),
                    directory: ".ai/repo/offload".into(),
                    source_class: "derive-offload".into(),
                    section: None,
                    bytes: 0,
                    member: None,
                },
            });
        }
        let registry = crate::capability::CapabilityRegistry::builder()
            .with_modules(vec![
                crate::capability::builtin::mesh::module(),
                crate::capability::builtin::peers::module(),
            ])
            .with_index(&index)
            .build()
            .expect("the registry builds");
        let ctx = Context::new(std::sync::Arc::new(index), std::sync::Arc::new(registry));
        (repo, ctx)
    }

    fn declaration(queue: &Path, hosts: usize) -> serde_json::Value {
        let hosts: Vec<serde_json::Value> = (0..hosts)
            .map(|i| serde_json::json!({ "id": format!("h{i}"), "address": format!("u@h{i}") }))
            .collect();
        serde_json::json!({
            "schema": "derive-offload/v1", "kind": "derive-offload", "id": "derive",
            "queue": { "directory": queue.join("queue").to_string_lossy() },
            "hosts": hosts,
        })
    }

    /// The whole capability over a context: this process is found and read, the declared
    /// queue's records are this machine's, a server that is not running is a board and
    /// claims not read rather than a failure; a remote subject on an undeclared host, or a
    /// tag among several hosts, is refused before anything is reached.
    #[test]
    fn who_gathers_every_source_and_reaches_only_declared_hosts() {
        let dir = tempfile::tempdir().unwrap();
        let (_repo, ctx) = context(Some(declaration(dir.path(), 2)));
        let me = std::process::id().to_string();
        let r = who(&ctx, &me, &Subject::parse(&me).unwrap());
        assert_eq!(
            r.chain.first().map(|p| p.pid),
            Some(std::process::id()),
            "{r:?}"
        );
        assert!(used(&r, "process"));
        for (text, want) in [
            ("nowhere:12", "'nowhere' is not a declared offload host"),
            ("083ddafbbc", "2 offload hosts are declared"),
        ] {
            let r = who(&ctx, text, &Subject::parse(text).unwrap());
            assert!(!r.known && r.missing[0].contains(want), "{text}: {r:?}");
        }
        let (_repo, bare) = context(None);
        let r = who(&bare, "nowhere:12", &Subject::parse("nowhere:12").unwrap());
        assert!(r.missing[0].contains("declares no offload host"), "{r:?}");
        let r = who(&bare, "I1", &Subject::parse("I1").unwrap());
        assert!(!r.known);
        assert!(
            r.evidence
                .iter()
                .any(|e| e.source == "mesh_claims" && e.detail.contains("not read")),
            "{r:?}"
        );
        let (_repo, broken) = context(Some(serde_json::json!({ "schema": "derive-offload/v1" })));
        let r = who(&broken, "12", &Subject::parse("12").unwrap());
        assert!(!r.known && r.missing[0].starts_with("process:"), "{r:?}");
    }

    /// Mesh questions go to this checkout's server when it is ready, else to the first ready
    /// one; a list with no ready server names none.
    #[test]
    fn the_mesh_is_asked_of_this_checkouts_server_or_the_first_ready_one() {
        let list = |boards: serde_json::Value| serde_json::json!({ "boards": boards });
        let b = |url: &str, standing: &str, this: bool| serde_json::json!({ "url": url, "standing": standing, "this_checkout": this });
        let both = list(serde_json::json!([
            b("http://a", "ready", false),
            b("http://b", "ready", true)
        ]));
        assert_eq!(ready_server(&both).as_deref(), Some("http://b"));
        let other = list(serde_json::json!([
            b("http://x", "stale", true),
            b("http://a", "ready", false)
        ]));
        assert_eq!(ready_server(&other).as_deref(), Some("http://a"));
        assert_eq!(
            ready_server(&list(serde_json::json!([b("http://x", "absent", true)]))),
            None
        );
        assert_eq!(ready_server(&serde_json::json!({})), None);
        let peers = serde_json::json!({ "peers": "not a board" });
        assert!(board_of_list(&peers, Path::new("/nowhere")).is_err());
    }

    #[test]
    fn the_socket_directory_is_the_providers_unless_moved() {
        let d = socket_dir();
        match std::env::var(SOCKET_DIR_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            Some(v) => assert_eq!(d, v),
            None => assert_eq!(d, SOCKET_DIR),
        }
        let p = Places::of_env();
        assert_eq!(p.socks, d);
        match std::env::var(SESSION_DIR_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty())
        {
            Some(v) => assert_eq!(p.sessions, v),
            None => assert_eq!(p.sessions, "", "the host's own ~/.claude/sessions"),
        }
    }
}
