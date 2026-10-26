//! Stopping an offloaded run: its whole process group, on its host, through the offload path
//! and nowhere else (I3956).
//!
//! On 2026-10-22 the owner of a runaway remote `cargo test --doc` killed its cargo and its
//! shell by hand, and a `rustc` under them ran on for 50 minutes with init as its parent,
//! because nothing stopped the group. A run now starts in a process group of its own and
//! records it ([`super::owner::RunRecord`]); `majordomus offload stop <tag>` reads that record
//! on the host, decides here whether this caller may stop it, and signals the group — `TERM`,
//! then `KILL` to whatever is left after the grace period — never one process of it.
//!
//! What it refuses:
//!
//! - a run whose record names another session, or names none, unless `--force`: a session
//!   does not stop another session's work by accident;
//! - a record without a process group (a host with neither `setsid` nor Python could not
//!   start one): there is no group to stop, and one process of it is not the run;
//! - on a host with `/proc`, a group none of whose processes carries this run's tag in its
//!   environment ([`super::owner::RUN_ENV`]): the host has handed that group id to someone
//!   else since the record was written, and it is not ours to signal.
//!
//! A group that is already empty is reported as gone and its record removed: a record that
//! outlived its run is evidence until somebody looks, and this is somebody looking.

use std::io::Write;

use serde::Serialize;

use super::declaration::{OffloadDeclaration, OffloadHost};
use super::host::remote_of;
use super::owner::{runs_directory, RunOwner, RunRecord};
use super::remote::Remote;
use super::run::{Refusal, EXIT_PRECONDITION, EXIT_UNAVAILABLE};

/// The record names another session (or none) and `--force` was not given.
pub const EXIT_NOT_OWNER: u8 = 13;

/// How a stop is asked for.
#[derive(Debug, Clone, Default)]
pub struct StopRequest {
    /// The run's tag, as the derive printed it.
    pub tag: String,
    /// A declared host id or address; absent is the declaration's only host.
    pub host: Option<String>,
    /// Stop it although its record names another session, or none.
    pub force: bool,
    /// Seconds between `TERM` and `KILL`.
    pub grace_seconds: u64,
}

/// What a stop did.
#[derive(Debug, Clone, Serialize)]
pub struct StopReport {
    /// The host it ran on.
    pub host: String,
    /// The run's record, as the host kept it.
    pub record: RunRecord,
    /// How many processes the group had when the stop began.
    pub members: usize,
    /// How the group was known to be this run's: `environ` (a member carries the run's tag),
    /// `record` (a host without `/proc`: the record alone), or `gone` (nothing to verify).
    pub verified: String,
    /// The signals sent to the group, in order.
    pub signals: Vec<String>,
    /// Whether the record was removed here; otherwise the run's own shell removes it.
    pub record_removed: bool,
}

fn refuse(code: u8, reason: impl Into<String>) -> Refusal {
    Refusal {
        code,
        reason: reason.into(),
    }
}

/// Whether `tag` can name a run: what the derive makes one of, ten to forty letters and
/// digits, so a tag is never a path on the host.
pub fn valid_tag(tag: &str) -> bool {
    (10..=40).contains(&tag.len()) && tag.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Whether `caller` may stop the run `record` describes: the record names the caller's own
/// session, or `force` says to stop it anyway. The refusal names the owner, so the person
/// knows whom to ask.
pub fn may_stop(record: &RunRecord, caller: &RunOwner, force: bool) -> Result<(), Refusal> {
    let owner = record.owner.session.as_deref();
    if force || (owner.is_some() && owner == caller.session.as_deref()) {
        return Ok(());
    }
    let whose = match (owner, &record.owner.peer, &record.owner.issue) {
        (Some(s), peer, issue) => format!(
            "session {s}{}{}",
            peer.as_deref()
                .map(|p| format!(", peer {p}"))
                .unwrap_or_default(),
            issue
                .as_deref()
                .map(|i| format!(", issue {i}"))
                .unwrap_or_default()
        ),
        (None, _, _) => "no session (a run started outside a provider)".to_string(),
    };
    Err(refuse(
        EXIT_NOT_OWNER,
        format!(
            "run {} on {} belongs to {whose}, and this caller is {}; ask its owner, or pass \
             --force to stop it anyway",
            record.tag,
            record.host,
            caller.session.as_deref().unwrap_or("no session")
        ),
    ))
}

/// The host a stop goes to: the one named, or the declaration's only host.
fn pick(d: &OffloadDeclaration, named: Option<&str>) -> Result<OffloadHost, Refusal> {
    let ids = || {
        d.hosts
            .iter()
            .map(|h| h.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match named {
        Some(name) => d.host(name).cloned().ok_or_else(|| {
            refuse(
                EXIT_UNAVAILABLE,
                format!("'{name}' is not a declared host; declared: {}", ids()),
            )
        }),
        None if d.hosts.len() == 1 => Ok(d.hosts[0].clone()),
        None => Err(refuse(
            EXIT_PRECONDITION,
            format!(
                "{} hosts are declared; name the one the run is on with --host ({})",
                d.hosts.len(),
                ids()
            ),
        )),
    }
}

/// Read a run record on the host. Arguments: runs directory, tag. Exit 3 when there is none.
const READ: &str = r#"runs=$1; tag=$2
case $runs in /*) ;; *) runs=$HOME/$runs ;; esac
[ -f "$runs/$tag.run" ] || exit 3
cat "$runs/$tag.run""#;

/// Stop a run's process group on the host. Arguments: runs directory, tag, process group,
/// the run's shell pid, grace seconds, who stops it. Prints `members <n>`, then either
/// `record removed` (an empty group) or `verified environ|record`, `signalled TERM`,
/// possibly `signalled KILL`, `stopped`, and `record removed|kept`. Exit 4 when the group is
/// not this run's (no member carries its tag, on a host with `/proc`), 5 when members
/// survive `KILL`.
///
/// The marker `<runs>/<tag>.stopped` tells the run's own shell, which sees its derive end
/// with a signal, that the run was stopped rather than failed.
const STOP: &str = r#"runs=$1; tag=$2; pg=$3; pid=$4; grace=$5; by=$6
case $runs in /*) ;; *) runs=$HOME/$runs ;; esac
record="$runs/$tag.run"
members() { ps -e -o pid= -o pgid= 2>/dev/null | awk -v g="$pg" '$2 == g { print $1 }'; }
removed() {
  if kill -0 "$pid" 2>/dev/null; then echo "record kept"
  else rm -f "$record" "$runs/$tag.stopped"; echo "record removed"; fi
}
m=$(members)
if [ -z "$m" ]; then echo "members 0"; removed; exit 0; fi
echo "members $(printf '%s\n' $m | wc -l | tr -d ' ')"
if [ -d /proc/self ]; then
  ours=""
  for p in $m; do
    if tr '\0' '\n' < "/proc/$p/environ" 2>/dev/null | grep -qx "MAJORDOMUS_OWNER_RUN=$tag"; then
      ours=1; break
    fi
  done
  [ -n "$ours" ] || { echo "reused"; exit 4; }
  echo "verified environ"
else
  echo "verified record"
fi
printf '%s\n' "$by" > "$runs/$tag.stopped"
kill -TERM -"$pg" 2>/dev/null
echo "signalled TERM"
i=0
while [ -n "$(members)" ] && [ "$i" -lt "$grace" ]; do sleep 1; i=$((i+1)); done
if [ -n "$(members)" ]; then kill -KILL -"$pg" 2>/dev/null; echo "signalled KILL"; sleep 1; fi
left=$(members)
if [ -n "$left" ]; then echo "survivors" $left; exit 5; fi
echo "stopped"
removed"#;

/// Stop the run `request` names, on its host, as `caller`. Progress goes to `log`.
pub fn stop(
    d: &OffloadDeclaration,
    request: &StopRequest,
    caller: &RunOwner,
    log: &mut dyn Write,
) -> Result<StopReport, Refusal> {
    let host = pick(d, request.host.as_deref())?;
    stop_on(
        &remote_of(&host),
        &host.id,
        &runs_directory(&d.queue.directory),
        request,
        caller,
        log,
    )
}

/// [`stop`] on a host already reached: what a test drives with this machine as the host.
pub(crate) fn stop_on(
    remote: &Remote,
    host: &str,
    runs: &str,
    request: &StopRequest,
    caller: &RunOwner,
    log: &mut dyn Write,
) -> Result<StopReport, Refusal> {
    let tag = request.tag.as_str();
    if !valid_tag(tag) {
        return Err(refuse(
            EXIT_PRECONDITION,
            format!("'{tag}' is not a run tag (ten to forty letters and digits)"),
        ));
    }
    let unreachable = |e: std::io::Error| refuse(EXIT_UNAVAILABLE, format!("{host}: {e}"));
    let read = remote.run(READ, &[runs, tag], None).map_err(unreachable)?;
    match read.code {
        Some(0) => {}
        Some(3) => {
            return Err(refuse(
                EXIT_UNAVAILABLE,
                format!(
                "{host} holds no run record {runs}/{tag}.run: the run ended, or never ran there"
            ),
            ))
        }
        other => {
            return Err(refuse(
                EXIT_UNAVAILABLE,
                format!(
                    "{host}: the run record could not be read ({other:?}): {}",
                    read.stderr.trim()
                ),
            ))
        }
    }
    let record = RunRecord::parse(&read.stdout)
        .map_err(|e| refuse(EXIT_UNAVAILABLE, format!("{host}: {runs}/{tag}.run: {e}")))?;
    may_stop(&record, caller, request.force)?;
    let Some(pgid) = record.pgid.filter(|p| *p > 1) else {
        return Err(refuse(
            EXIT_UNAVAILABLE,
            format!(
                "run {tag} on {host} has no process group of its own (its host had neither \
                 setsid nor python3); its shell is pid {}",
                record.pid
            ),
        ));
    };
    let _ = writeln!(
        log,
        "offload stop: {tag} on {host}, process group {pgid}, owner {}",
        record.owner.session.as_deref().unwrap_or("(unnamed)")
    );
    let by = caller.session.clone().unwrap_or_else(|| "unnamed".into());
    let out = remote
        .run(
            STOP,
            &[
                runs,
                tag,
                &pgid.to_string(),
                &record.pid.to_string(),
                &request.grace_seconds.to_string(),
                &by,
            ],
            None,
        )
        .map_err(unreachable)?;
    let _ = log.write_all(out.stderr.as_bytes());
    let report = report_of(host, record, &out.stdout);
    match out.code {
        Some(0) => Ok(report),
        Some(4) => Err(refuse(
            EXIT_UNAVAILABLE,
            format!(
                "process group {pgid} on {host} is not run {tag}'s any more: none of its \
                 processes carries {}={tag}; nothing was signalled",
                super::owner::RUN_ENV
            ),
        )),
        Some(5) => Err(refuse(
            EXIT_UNAVAILABLE,
            format!(
                "process group {pgid} on {host} survived TERM and KILL: {}",
                out.stdout.trim()
            ),
        )),
        other => Err(refuse(
            EXIT_UNAVAILABLE,
            format!(
                "{host}: the stop ended with {other:?}: {}",
                out.stderr.trim()
            ),
        )),
    }
}

/// The report the stop script's lines make.
fn report_of(host: &str, record: RunRecord, stdout: &str) -> StopReport {
    let mut report = StopReport {
        host: host.to_string(),
        record,
        members: 0,
        verified: "gone".into(),
        signals: Vec::new(),
        record_removed: false,
    };
    for line in stdout.lines() {
        if let Some(n) = line.strip_prefix("members ") {
            report.members = n.trim().parse().unwrap_or(0);
        } else if let Some(v) = line.strip_prefix("verified ") {
            report.verified = v.trim().to_string();
        } else if let Some(s) = line.strip_prefix("signalled ") {
            report.signals.push(s.trim().to_string());
        } else if line == "record removed" {
            report.record_removed = true;
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn declaration(hosts: &[&str]) -> OffloadDeclaration {
        let hosts: Vec<serde_json::Value> = hosts
            .iter()
            .map(|h| serde_json::json!({ "id": h, "address": format!("u@{h}") }))
            .collect();
        serde_json::from_value(serde_json::json!({
            "schema": "derive-offload/v1", "kind": "derive-offload", "id": "derive",
            "hosts": hosts
        }))
        .unwrap()
    }

    fn record(session: Option<&str>, pgid: Option<u32>) -> RunRecord {
        RunRecord {
            tag: "0123456789".into(),
            pid: 1,
            pgid,
            started_at: 0,
            host: "lundra".into(),
            cwd: String::new(),
            owner: RunOwner {
                session: session.map(String::from),
                peer: Some("p2".into()),
                issue: Some("I3956".into()),
                host: "mac".into(),
                pid: Some(9),
            },
        }
    }

    fn me(session: &str) -> RunOwner {
        RunOwner {
            session: Some(session.into()),
            ..RunOwner::default()
        }
    }

    #[test]
    fn a_tag_is_letters_and_digits_of_a_commit_length() {
        assert!(valid_tag("0123456789abcdef0123456789abcdef01234567"));
        for bad in [
            "",
            "012345678",
            "0123456789/",
            "0123456789 x",
            &"a".repeat(41),
        ] {
            assert!(!valid_tag(bad), "{bad}");
        }
    }

    /// Only the record's own session stops a run without --force; the refusal names the
    /// owner, its peer and issue, or says the run has no session.
    #[test]
    fn only_the_owner_stops_a_run_without_force() {
        let r = record(Some("claude-code-a"), Some(5));
        assert!(may_stop(&r, &me("claude-code-a"), false).is_ok());
        let e = may_stop(&r, &me("claude-code-b"), false).unwrap_err();
        assert_eq!(e.code, EXIT_NOT_OWNER);
        assert!(
            e.reason
                .contains("belongs to session claude-code-a, peer p2, issue I3956")
                && e.reason.contains("this caller is claude-code-b"),
            "{}",
            e.reason
        );
        assert!(may_stop(&r, &me("claude-code-b"), true).is_ok());
        let unnamed = record(None, Some(5));
        let e = may_stop(&unnamed, &RunOwner::default(), false).unwrap_err();
        assert!(e
            .reason
            .contains("no session (a run started outside a provider)"));
        assert!(
            e.reason.contains("this caller is no session"),
            "{}",
            e.reason
        );
        assert!(may_stop(&unnamed, &me("claude-code-a"), false).is_err());
        let mut bare = record(Some("s"), None);
        bare.owner.peer = None;
        bare.owner.issue = None;
        let e = may_stop(&bare, &me("t"), false).unwrap_err();
        assert!(e.reason.contains("belongs to session s,"), "{}", e.reason);
    }

    #[test]
    fn a_stop_goes_to_the_named_host_or_the_only_one() {
        let one = declaration(&["lundra"]);
        assert_eq!(pick(&one, None).unwrap().id, "lundra");
        assert_eq!(pick(&one, Some("u@lundra")).unwrap().id, "lundra");
        let e = pick(&one, Some("nowhere")).unwrap_err();
        assert_eq!(e.code, EXIT_UNAVAILABLE);
        assert!(e.reason.contains("declared: lundra"), "{}", e.reason);
        let two = declaration(&["lundra", "mac"]);
        let e = pick(&two, None).unwrap_err();
        assert_eq!(e.code, EXIT_PRECONDITION);
        assert!(e.reason.contains("--host (lundra, mac)"), "{}", e.reason);
    }

    /// A stop whose host cannot be decided is refused before any host is contacted.
    #[test]
    fn a_stop_with_no_host_to_go_to_contacts_nothing() {
        let two = declaration(&["lundra", "mac"]);
        let req = StopRequest {
            tag: "0123456789".into(),
            ..StopRequest::default()
        };
        let e = stop(&two, &req, &me("claude-code-a"), &mut Vec::new()).unwrap_err();
        assert_eq!(e.code, EXIT_PRECONDITION, "{}", e.reason);
        let named = StopRequest {
            host: Some("nowhere".into()),
            ..req
        };
        let e = stop(&two, &named, &me("claude-code-a"), &mut Vec::new()).unwrap_err();
        assert_eq!(e.code, EXIT_UNAVAILABLE, "{}", e.reason);
    }

    #[test]
    fn the_report_reads_every_line_of_the_stop() {
        let r = report_of(
            "h",
            record(Some("s"), Some(5)),
            "members 3\nverified environ\nsignalled TERM\nsignalled KILL\nstopped\nrecord removed\n",
        );
        assert_eq!((r.members, r.verified.as_str()), (3, "environ"));
        assert_eq!(r.signals, ["TERM", "KILL"]);
        assert!(r.record_removed);
        let r = report_of("h", record(Some("s"), Some(5)), "members x\nrecord kept\n");
        assert_eq!(
            (r.members, r.verified.as_str(), r.record_removed),
            (0, "gone", false)
        );
    }

    fn write_record(runs: &std::path::Path, tag: &str, body: &str) {
        std::fs::create_dir_all(runs).unwrap();
        std::fs::write(runs.join(format!("{tag}.run")), body).unwrap();
    }

    /// What a stop refuses before it signals anything: a tag that is not one, a host
    /// holding no record, a record it cannot read, someone else's run, a run with no group.
    #[test]
    fn a_stop_refuses_before_it_signals() {
        let tmp = tempfile::tempdir().unwrap();
        let runs = tmp.path().join("runs");
        let runs_s = runs.to_str().unwrap();
        let ask = |tag: &str, force: bool| {
            let req = StopRequest {
                tag: tag.into(),
                force,
                grace_seconds: 1,
                ..StopRequest::default()
            };
            let mut log = Vec::new();
            stop_on(
                &Remote::local(),
                "local",
                runs_s,
                &req,
                &me("claude-code-a"),
                &mut log,
            )
        };
        assert_eq!(ask("../x", false).unwrap_err().code, EXIT_PRECONDITION);
        let e = ask("0123456789", false).unwrap_err();
        assert!(e.reason.contains("holds no run record"), "{}", e.reason);
        write_record(&runs, "0123456789", "not a record\n");
        let e = ask("0123456789", false).unwrap_err();
        assert!(e.reason.contains("not a run record"), "{}", e.reason);
        let theirs = "majordomus-offload-run/v1\ntag 0123456789\npid 1\npgid 99999\nhost h\nowner_session claude-code-b\n";
        write_record(&runs, "0123456789", theirs);
        assert_eq!(ask("0123456789", false).unwrap_err().code, EXIT_NOT_OWNER);
        let groupless = "majordomus-offload-run/v1\ntag 0123456789\npid 7\npgid\nhost h\nowner_session claude-code-a\n";
        write_record(&runs, "0123456789", groupless);
        let e = ask("0123456789", false).unwrap_err();
        assert!(e.reason.contains("no process group of its own") && e.reason.contains("pid 7"));
        let unreadable = tmp.path().join("no-such-ssh");
        let req = StopRequest {
            tag: "0123456789".into(),
            ..StopRequest::default()
        };
        let e = stop_on(
            &Remote::through(&unreadable),
            "gone",
            runs_s,
            &req,
            &me("x"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(e.code, EXIT_UNAVAILABLE);
        let failing = tmp.path().join("failing-ssh");
        std::fs::write(&failing, "#!/bin/sh\necho 'ssh: refused' >&2\nexit 255\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&failing, std::fs::Permissions::from_mode(0o755)).unwrap();
        let e = stop_on(
            &Remote::through(&failing),
            "down",
            runs_s,
            &req,
            &me("x"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(
            e.reason
                .contains("could not be read (Some(255)): ssh: refused"),
            "{}",
            e.reason
        );
    }

    /// A record whose group has no process left is a leftover: the stop says it is gone and
    /// removes the record, signalling nothing.
    #[test]
    fn a_record_whose_group_is_gone_is_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let runs = tmp.path().join("runs");
        // a group id and a shell pid nothing has: a child that has ended and been reaped
        let mut ended = std::process::Command::new("true").spawn().unwrap();
        let dead = ended.id();
        ended.wait().unwrap();
        let body = format!(
            "majordomus-offload-run/v1\ntag abcdefabcd\npid {dead}\npgid {dead}\nhost h\nowner_session claude-code-a\n"
        );
        write_record(&runs, "abcdefabcd", &body);
        let req = StopRequest {
            tag: "abcdefabcd".into(),
            grace_seconds: 1,
            ..StopRequest::default()
        };
        let mut log = Vec::new();
        let r = stop_on(
            &Remote::local(),
            "local",
            runs.to_str().unwrap(),
            &req,
            &me("claude-code-a"),
            &mut log,
        )
        .unwrap();
        assert_eq!((r.members, r.verified.as_str()), (0, "gone"));
        assert!(r.signals.is_empty() && r.record_removed);
        assert!(!runs.join("abcdefabcd.run").exists());
        assert!(String::from_utf8(log)
            .unwrap()
            .contains(&format!("process group {dead}")));
    }

    /// A live run is stopped as a group: the derive, and a grandchild of it that would have
    /// outlived its parent, are both gone; the run's shell reports it stopped (exit 14) and
    /// removes its own record. The run is the real derive script, started here as a host
    /// starts it.
    #[test]
    fn a_live_run_is_stopped_as_a_group() {
        let tmp = tempfile::tempdir().unwrap();
        let at = |p: &str| tmp.path().join(p);
        let tag = "fedcba9876";
        for d in ["lock", "wt/offload-fedcba9876", "target"] {
            std::fs::create_dir_all(at(d)).unwrap();
        }
        let derive = at("derive");
        std::fs::write(
            &derive,
            format!(
                "#!/bin/sh\nsleep 300 &\necho $! > {gc}\nwait\n",
                gc = at("grandchild").display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&derive, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runs = at("runs");
        let run = super::super::run::derive_command(&super::super::run::DeriveArgs {
            lock: at("lock").to_str().unwrap(),
            worktrees: at("wt").to_str().unwrap(),
            tag,
            shared: at("target").to_str().unwrap(),
            derive: derive.to_str().unwrap(),
            spool: at("spool").to_str().unwrap(),
            prefix: "",
            observe: "",
            held: false,
            runs: runs.to_str().unwrap(),
            owner: &me("claude-code-a").args(),
            touch: &[],
        });
        let shell = std::process::Command::new("sh")
            .arg("-c")
            .arg(run.0)
            .arg("sh")
            .args(&run.1)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let record = runs.join(format!("{tag}.run"));
        let deadline = Instant::now() + Duration::from_secs(30);
        while !(record.exists() && at("grandchild").exists()) {
            assert!(Instant::now() < deadline, "the run never recorded itself");
            std::thread::sleep(Duration::from_millis(50));
        }
        let grandchild: i32 = std::fs::read_to_string(at("grandchild"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let req = StopRequest {
            tag: tag.into(),
            grace_seconds: 5,
            ..StopRequest::default()
        };
        let stopped = stop_on(
            &Remote::local(),
            "local",
            runs.to_str().unwrap(),
            &req,
            &me("claude-code-a"),
            &mut Vec::new(),
        );
        let out = shell.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let r = match stopped {
            Ok(r) => r,
            Err(e) => {
                // SAFETY: the pid was written by this test's own derive a moment ago
                unsafe { libc::kill(grandchild, libc::SIGKILL) };
                panic!("{}: {text}", e.reason);
            }
        };
        assert!(r.members >= 2, "the derive and its grandchild: {r:?}");
        assert_eq!(r.signals.first().map(String::as_str), Some("TERM"));
        assert!(
            ["environ", "record"].contains(&r.verified.as_str()),
            "{r:?}"
        );
        assert_eq!(out.status.code(), Some(14), "{text}");
        assert!(String::from_utf8_lossy(&out.stderr).contains("by session claude-code-a"));
        // SAFETY: kill with signal 0 only asks whether the pid exists
        assert_ne!(
            unsafe { libc::kill(grandchild, 0) },
            0,
            "the grandchild outlived its group"
        );
        assert!(!record.exists(), "the record ends with the run");
        assert!(!runs.join(format!("{tag}.stopped")).exists());
    }
}
