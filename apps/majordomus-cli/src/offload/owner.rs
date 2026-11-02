//! Who a remote run belongs to, and the record it leaves on its host (I3956).
//!
//! On 2026-10-22 a 33-minute `cargo test --doc` kept lundra at load 21, and the only way to
//! name the session that started it was a process-tree trace by hand; when the owner killed
//! its cargo and its shell, a `rustc` under them survived for 50 minutes with init as its
//! parent and a deleted worktree as its working directory, because nothing stopped the group.
//!
//! So every offloaded run now carries its owner — the session (its id on the mesh), the board
//! peer, the issue, the machine and process it was started from — in two places: in the
//! environment of every process the run starts (`MAJORDOMUS_OWNER_*`, inherited by every
//! `cargo` and `rustc` under it), and in a run record beside the host's queue
//! (`<queue directory>.runs/<tag>.run`) that names the run's process group. The run is started
//! in a process group of its own and stopped as one ([`super::stop`]), and the record is
//! removed when the run ends; a record that outlives its run is the evidence a leftover is
//! named by.
//!
//! The owner is derived from the slice-1 address model ([`PeerAddress`]: the provider's
//! session, its pid, this machine) and the board row that address matches; nothing is asked
//! of a hook or of the model. Every value is validated before it travels: a value that is not
//! one short word of the characters an id is made of is dropped, never mangled into one, so
//! no value can become a second line of the record or a word of the host's shell. The
//! provider's messaging token is a credential and is never read.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;

use crate::peers::{Peer, PeerAddress};

/// The variable naming the run itself (its tag) in every process of it: what tells a process
/// group that is this run's from one whose id the host has since handed to someone else.
pub const RUN_ENV: &str = "MAJORDOMUS_OWNER_RUN";

/// The first line of a run record.
pub const RUN_SCHEMA: &str = "majordomus-offload-run/v1";

/// The longest value an owner field may have.
const MAX_FIELD: usize = 128;

/// One owner value, validated: trimmed, at most 128 characters, every one of them a letter,
/// a digit or one of `-_.:@/`. Anything else is `None`, so a value carrying a newline, a
/// space or a quote never reaches a record line or a host's shell.
pub fn owner_value(v: &str) -> Option<String> {
    let v = v.trim();
    let ok = !v.is_empty()
        && v.chars().count() <= MAX_FIELD
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:@/".contains(c));
    ok.then(|| v.to_string())
}

/// Who a run belongs to: what a person needs to reach the worker that started it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RunOwner {
    /// The session's id on the mesh (`claude-code-<id>`), or the one this process inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// The board peer whose address is this worker's, or the one this process inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,
    /// The issue the work is for: the active task's, or the one this process inherited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The machine the run was started from.
    pub host: String,
    /// The worker's process on that machine.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

impl RunOwner {
    /// The owner an environment describes. What this process inherited as an owner wins — a
    /// run started from inside another run belongs to the session that started the first —
    /// and otherwise the address the environment describes ([`PeerAddress::from_env`]: the
    /// provider's session on the mesh, the worker's pid, `parent` when the provider names
    /// none), `host`, the active task's issue and the board peer the address matched.
    pub fn derive(
        env: impl Fn(&str) -> Option<String>,
        parent: Option<u32>,
        host: &str,
        task_issue: Option<String>,
        board_peer: Option<String>,
    ) -> RunOwner {
        let read = |k: &str| env(k).and_then(|v| owner_value(&v));
        let address = PeerAddress::from_env(&env, parent, host);
        let inherited_pid = read("MAJORDOMUS_OWNER_PID").and_then(|p| p.parse::<u32>().ok());
        RunOwner {
            session: read("MAJORDOMUS_OWNER_SESSION").or_else(|| {
                address
                    .as_ref()
                    .and_then(PeerAddress::mesh_session)
                    .and_then(|s| owner_value(&s))
            }),
            peer: read("MAJORDOMUS_OWNER_PEER")
                .or_else(|| board_peer.and_then(|p| owner_value(&p))),
            issue: read("MAJORDOMUS_OWNER_ISSUE")
                .or_else(|| task_issue.and_then(|i| owner_value(&i))),
            host: read("MAJORDOMUS_OWNER_HOST")
                .or_else(|| owner_value(host))
                .unwrap_or_default(),
            pid: inherited_pid.or_else(|| address.and_then(|a| a.pid)),
        }
    }

    /// The owner of a run this process starts for the checkout at `worktree`: its
    /// environment, its parent, this machine, the checkout's active task and the board row
    /// of the checkout's server whose address is this worker's.
    pub(crate) fn of_this_process(worktree: &Path) -> RunOwner {
        let issue = crate::capability::builtin::continuity::read_task(
            &worktree.join(".ai/local/state/current.yaml"),
        )
        .filter(|t| t.outcome == "active")
        .map(|t| t.issue);
        let env = |k: &str| std::env::var(k).ok();
        let parent = Some(std::os::unix::process::parent_id());
        let host = crate::integration::drain::host();
        let peer = PeerAddress::from_env(env, parent, &host)
            .and_then(|a| peer_on_board(&board_of_checkout(worktree), &a));
        Self::derive(env, parent, &host, issue, peer)
    }

    /// The five script arguments, in the order the host exports them —
    /// `MAJORDOMUS_OWNER_SESSION`, `_PEER`, `_ISSUE`, `_HOST`, `_PID`; an unknown value is
    /// empty.
    pub fn args(&self) -> [String; 5] {
        [
            self.session.clone().unwrap_or_default(),
            self.peer.clone().unwrap_or_default(),
            self.issue.clone().unwrap_or_default(),
            self.host.clone(),
            self.pid.map(|p| p.to_string()).unwrap_or_default(),
        ]
    }
}

/// The board peer whose address is `address`: the row carrying the same provider session,
/// or — for a client that exposes none — the same process on the same machine. An attached
/// row is preferred over one whose connection has gone. `None` when no row matches.
pub fn peer_on_board(peers: &[Peer], address: &PeerAddress) -> Option<String> {
    let same = |p: &Peer| {
        p.address
            .as_ref()
            .is_some_and(|a| match (&a.session, &address.session) {
                (Some(theirs), Some(ours)) => theirs == ours && a.provider == address.provider,
                _ => a.host == address.host && a.pid.is_some() && a.pid == address.pid,
            })
    };
    let matching: Vec<&Peer> = peers.iter().filter(|p| same(p)).collect();
    matching
        .iter()
        .find(|p| p.attached)
        .or_else(|| matching.first())
        .map(|p| p.id.to_string())
}

/// This checkout's own board, as the server its lease names answers it; empty when there is
/// no lease, no address, no answer or no board — the owner is then simply unnamed by a peer.
/// One request, bounded by the declared probe timeout, through the route the registry
/// declares for `peers.list` (never a path typed here).
pub(crate) fn board_of_checkout(worktree: &Path) -> Vec<Peer> {
    let half = crate::capability::builtin::server::local_half(worktree);
    let lease = crate::lease::LeaseFile::read(&crate::lease::lease_file(worktree, &half));
    let Some(url) = lease.document().and_then(|d| d.url.clone()) else {
        return Vec::new();
    };
    let Some(path) = crate::command_graph::load::registry()
        .and_then(|r| r.get("peers.list").and_then(|c| c.exposure.http.clone()))
        .map(|h| h.path)
    else {
        return Vec::new();
    };
    crate::mcp::bridge::request(
        &url,
        "GET",
        &format!("{path}?checkouts=this"),
        &[],
        None,
        crate::lease::timings().probe_timeout,
    )
    .ok()
    .filter(|r| r.status == 200)
    .and_then(|r| serde_json::from_str::<serde_json::Value>(&r.body).ok())
    .and_then(|v| serde_json::from_value(v["peers"].clone()).ok())
    .unwrap_or_default()
}

/// A run record as the host keeps it while the run lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RunRecord {
    /// The offload tag: the first ten characters of the commit that travelled.
    pub tag: String,
    /// The run's shell on the host.
    pub pid: u32,
    /// The run's process group; absent where the host could not start one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pgid: Option<u32>,
    /// When it started, seconds since the epoch.
    pub started_at: u64,
    /// The host it runs on.
    pub host: String,
    /// The worktree it runs in.
    pub cwd: String,
    /// Who it belongs to.
    pub owner: RunOwner,
}

impl RunRecord {
    /// Read a record; an error says what is missing.
    pub fn parse(text: &str) -> Result<RunRecord, String> {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(RUN_SCHEMA) {
            return Err(format!(
                "not a run record: the first line is not {RUN_SCHEMA}"
            ));
        }
        let mut f: BTreeMap<&str, &str> = BTreeMap::new();
        for line in lines {
            let (k, v) = line.split_once(' ').unwrap_or((line, ""));
            f.insert(k.trim(), v.trim());
        }
        let text_of = |k: &str| {
            f.get(k)
                .copied()
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        let num = |k: &str| text_of(k).and_then(|v| v.parse::<u32>().ok());
        let need = |k: &str| text_of(k).ok_or_else(|| format!("the run record has no {k}"));
        Ok(RunRecord {
            tag: need("tag")?,
            pid: num("pid").ok_or("the run record has no pid")?,
            pgid: num("pgid"),
            started_at: text_of("started_at")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            host: need("host")?,
            cwd: text_of("cwd").unwrap_or_default(),
            owner: RunOwner {
                session: text_of("owner_session"),
                peer: text_of("owner_peer"),
                issue: text_of("owner_issue"),
                host: text_of("owner_host").unwrap_or_default(),
                pid: num("owner_pid"),
            },
        })
    }
}

/// Where a host keeps its run records: beside the queue's ticket directory, never inside it,
/// because everything in that directory is read as a ticket.
pub fn runs_directory(queue_directory: &str) -> String {
    format!("{}.runs", queue_directory.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::{PeerBoard, Transport};

    /// An owner this process inherited wins over the provider's own session, so a run started
    /// from inside another run belongs to the session that started the first one.
    #[test]
    fn an_inherited_owner_wins() {
        let env = |k: &str| match k {
            "CLAUDE_CODE_SESSION_ID" => Some("inner".to_string()),
            "CLAUDE_PID" => Some("77".to_string()),
            "MAJORDOMUS_OWNER_SESSION" => Some("claude-code-outer".to_string()),
            "MAJORDOMUS_OWNER_PEER" => Some("p9".to_string()),
            "MAJORDOMUS_OWNER_ISSUE" => Some("I7".to_string()),
            "MAJORDOMUS_OWNER_HOST" => Some("mac".to_string()),
            "MAJORDOMUS_OWNER_PID" => Some("24025".to_string()),
            _ => None,
        };
        let o = RunOwner::derive(env, Some(5), "lundra", Some("I1".into()), Some("p1".into()));
        assert_eq!(o.session.as_deref(), Some("claude-code-outer"));
        assert_eq!(
            (o.peer.as_deref(), o.issue.as_deref()),
            (Some("p9"), Some("I7"))
        );
        assert_eq!((o.host.as_str(), o.pid), ("mac", Some(24025)));
    }

    /// Without an inherited owner the address, the task and the board name it; a value that
    /// is not one word of id characters is dropped rather than mangled, and an unknown value
    /// is an empty argument.
    #[test]
    fn an_owner_is_derived_from_the_address_and_bad_values_are_dropped() {
        let env = |k: &str| match k {
            "CLAUDE_CODE_SESSION_ID" => Some("s-1".to_string()),
            "CLAUDE_PID" => Some("4242".to_string()),
            "MAJORDOMUS_OWNER_SESSION" => Some("forged\nowner_issue I0".to_string()),
            "MAJORDOMUS_OWNER_PID" => Some("not-a-pid".to_string()),
            _ => None,
        };
        let o = RunOwner::derive(
            env,
            Some(5),
            "mac",
            Some("I3956".into()),
            Some("p 1".into()),
        );
        assert_eq!(
            o.session.as_deref(),
            Some("claude-code-s-1"),
            "the forged one is dropped"
        );
        assert_eq!((o.pid, o.peer.as_deref()), (Some(4242), None));
        assert_eq!(o.issue.as_deref(), Some("I3956"));
        let bare = RunOwner::derive(|_| None, Some(5), "lundra", Some("  ".into()), None);
        assert_eq!(bare.args(), ["", "", "", "lundra", "5"].map(String::from));
        let nohost = RunOwner::derive(|_| None, None, "bad host", None, None);
        assert_eq!((nohost.host.as_str(), nohost.pid), ("", None));
    }

    fn board_with(addresses: &[(PeerAddress, bool)]) -> Vec<Peer> {
        let board = PeerBoard::new();
        for (a, attached) in addresses {
            let id = board.attach(Transport::Http);
            board.locate(&id, a.clone());
            if !attached {
                // a peer that announced something is kept, detached, when it goes
                board.announce(&id, "gone", Vec::new());
                board.detach(&id);
            }
        }
        board.list()
    }

    /// The peer is the row carrying this worker's provider session, else the row of the same
    /// process on the same machine; an attached row wins over a departed one.
    #[test]
    fn the_board_peer_is_the_row_with_this_workers_address() {
        let env =
            |s: &'static str| move |k: &str| (k == "CLAUDE_CODE_SESSION_ID").then(|| s.to_string());
        let mine = PeerAddress::from_env(env("s1"), Some(7), "mac").unwrap();
        let other = PeerAddress::from_env(env("s2"), Some(7), "mac").unwrap();
        let bare = PeerAddress::from_env(|_| None, Some(7), "mac").unwrap();
        let elsewhere = PeerAddress::from_env(|_| None, Some(7), "lundra").unwrap();
        let peers = board_with(&[
            (other.clone(), true),
            (mine.clone(), false),
            (mine.clone(), true),
        ]);
        let departed: Vec<&Peer> = peers.iter().filter(|p| !p.attached).collect();
        assert_eq!(departed.len(), 1, "{peers:?}");
        assert_eq!(
            peer_on_board(&peers, &mine).as_deref(),
            Some("p3"),
            "attached wins"
        );
        let peers = board_with(&[(other.clone(), true), (mine.clone(), false)]);
        assert_eq!(
            peer_on_board(&peers, &mine).as_deref(),
            Some("p2"),
            "departed, still named"
        );
        let peers = board_with(&[(elsewhere, true), (bare.clone(), true)]);
        assert_eq!(
            peer_on_board(&peers, &bare).as_deref(),
            Some("p2"),
            "same host and pid"
        );
        assert_eq!(peer_on_board(&board_with(&[(other, true)]), &mine), None);
    }

    /// A record missing what a run cannot be without is an error naming it; what a run may
    /// lack (a group, an owner's peer) is absent rather than empty.
    #[test]
    fn a_run_record_names_what_it_lacks() {
        let full = format!("{RUN_SCHEMA}\ntag t\npid 1\npgid\nhost h\nowner_session s\n");
        let r = RunRecord::parse(&full).unwrap();
        assert_eq!((r.pgid, r.started_at, r.cwd.as_str()), (None, 0, ""));
        assert_eq!(r.owner.session.as_deref(), Some("s"));
        for (missing, text) in [
            ("tag", format!("{RUN_SCHEMA}\npid 1\nhost h\n")),
            ("pid", format!("{RUN_SCHEMA}\ntag t\npid x\nhost h\n")),
            ("host", format!("{RUN_SCHEMA}\ntag t\npid 1\n")),
        ] {
            let e = RunRecord::parse(&text).expect_err(missing);
            assert!(e.contains(missing), "{e}");
        }
    }

    /// The owner of this process is derived without failing, in a checkout with no server
    /// and no active task; with an active task, its issue is the owner's.
    #[test]
    fn this_process_has_an_owner() {
        let dir = tempfile::tempdir().unwrap();
        let inherited = std::env::var("MAJORDOMUS_OWNER_ISSUE").ok();
        let o = RunOwner::of_this_process(dir.path());
        if inherited.is_none() {
            assert!(o.issue.is_none(), "no active task, no issue");
        }
        let state = dir.path().join(".ai/local/state");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(
            state.join("current.yaml"),
            "id: t-1\ntask: x\noutcome: active\nissue: I3956\n",
        )
        .unwrap();
        let o = RunOwner::of_this_process(dir.path());
        if inherited.is_none() {
            assert_eq!(o.issue.as_deref(), Some("I3956"));
        }
    }

    /// A checkout whose lease names an address nobody answers on has an empty board.
    #[test]
    fn a_checkout_whose_server_is_gone_has_an_empty_board() {
        let dir = tempfile::tempdir().unwrap();
        assert!(board_of_checkout(dir.path()).is_empty(), "no lease");
        let half = crate::capability::builtin::server::local_half(dir.path());
        let file = crate::lease::lease_file(dir.path(), &half);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        // a port that was bound a moment ago and is closed now
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        std::fs::write(
            &file,
            format!(
                r#"{{"schema":"{}","token":"t","root":"/r","url":"http://127.0.0.1:{port}"}}"#,
                crate::lease::SCHEMA
            ),
        )
        .unwrap();
        assert!(board_of_checkout(dir.path()).is_empty(), "nobody answers");
    }

    /// A value is one word of id characters, at most 128 of them, or it is nothing.
    #[test]
    fn an_owner_value_is_one_word_of_id_characters() {
        assert_eq!(
            owner_value(" claude-code-877e4b89 ").as_deref(),
            Some("claude-code-877e4b89")
        );
        assert_eq!(
            owner_value("u@host.local:p/1_x").as_deref(),
            Some("u@host.local:p/1_x")
        );
        for bad in [
            "",
            "  ",
            "s\nowner_issue forged",
            "a b",
            "it's",
            "$(x)",
            "a;b",
        ] {
            assert_eq!(owner_value(bad), None, "{bad:?}");
        }
        assert_eq!(owner_value(&"x".repeat(128)).map(|v| v.len()), Some(128));
        assert_eq!(owner_value(&"x".repeat(129)), None);
    }

    #[test]
    fn the_run_records_live_beside_the_queue_never_in_it() {
        assert_eq!(
            runs_directory("/tmp/offload-queue/"),
            "/tmp/offload-queue.runs"
        );
        assert_eq!(runs_directory("q"), "q.runs");
    }

    /// The board the checkout's server answers is read through the route the registry
    /// declares for `peers.list`, and a row is named by its peer id; an answer that is not a
    /// board is an empty one.
    #[test]
    fn a_checkout_whose_server_answers_has_its_board() {
        use std::io::{Read, Write};
        let serve = |body: &'static str, status: &'static str| {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let handle = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap();
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(reply.as_bytes()).unwrap();
                request
            });
            (port, handle)
        };
        let dir = tempfile::tempdir().unwrap();
        let half = crate::capability::builtin::server::local_half(dir.path());
        let file = crate::lease::lease_file(dir.path(), &half);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let lease = |port: u16| {
            std::fs::write(
                &file,
                format!(
                    r#"{{"schema":"{}","token":"t","root":"/r","url":"http://127.0.0.1:{port}"}}"#,
                    crate::lease::SCHEMA
                ),
            )
            .unwrap()
        };
        let board = PeerBoard::new();
        let id = board.attach(Transport::Http);
        let env = |k: &str| (k == "CLAUDE_CODE_SESSION_ID").then(|| "s1".to_string());
        let me = PeerAddress::from_env(env, Some(7), "mac").unwrap();
        board.locate(&id, me.clone());
        let body: &'static str = Box::leak(
            serde_json::json!({ "peers": board.list() })
                .to_string()
                .into_boxed_str(),
        );
        let (port, handle) = serve(body, "200 OK");
        lease(port);
        let peers = board_of_checkout(dir.path());
        let request = handle.join().unwrap();
        assert!(request.contains("checkouts=this"), "{request}");
        assert_eq!(peer_on_board(&peers, &me).as_deref(), Some("p1"));
        let (port, handle) = serve("{}", "500 Internal Server Error");
        lease(port);
        assert!(
            board_of_checkout(dir.path()).is_empty(),
            "a refusal is no board"
        );
        handle.join().unwrap();
    }

    /// The host exports every owner value under its own name, and the run's tag beside them.
    #[test]
    fn the_host_exports_every_owner_value() {
        let script = super::super::run::derive_command(&super::super::run::DeriveArgs {
            lock: "l",
            worktrees: "w",
            tag: "t",
            shared: "s",
            derive: "d",
            spool: "sp",
            prefix: "",
            observe: "",
            held: false,
            runs: "r",
            owner: &["a", "b", "c", "d", "e"].map(String::from),
            touch: &["x".to_string()],
        });
        for name in [
            "MAJORDOMUS_OWNER_SESSION",
            "MAJORDOMUS_OWNER_PEER",
            "MAJORDOMUS_OWNER_ISSUE",
            "MAJORDOMUS_OWNER_HOST",
            "MAJORDOMUS_OWNER_PID",
            RUN_ENV,
        ] {
            assert!(script.0.contains(name), "{name}");
        }
        assert_eq!(
            script.1,
            ["l", "w", "t", "s", "d", "sp", "", "", "0", "r", "a", "b", "c", "d", "e", "x"]
                .map(String::from)
        );
    }
}
