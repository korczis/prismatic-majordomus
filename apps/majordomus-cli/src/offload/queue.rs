//! The FIFO queue on a host: who derives next, decided in Rust over what the host reports.
//!
//! The v1 script waited with `until mkdir lock; do sleep 15; done`. That is a race, not a
//! queue: on 2026-10-11 five sessions waited on lundra, the oldest for 2 h 12 min, while
//! later arrivals won the lock. Here every waiter writes a ticket named
//! `<arrival>-<pid>-<tag>`, where the pid is its [`Holder`](super::remote::Holder) on the
//! host, and takes the lock only when its ticket is the oldest one whose holder is alive.
//!
//! - A ticket whose holder is dead is removed by whoever sees it, so a waiter that was
//!   killed or lost its connection blocks nobody.
//! - The lock is a directory made by `mkdir` holding one empty file per process of the
//!   derive that holds it, named by its pid. When every one of those processes is dead, the
//!   lock is reclaimed, by any waiter, and the reclaim names the dead pids.
//! - A lock directory with no pid file in it was taken by a v1 or v2 waiter (or is being
//!   taken this instant) and cannot be checked: it is waited for, never reclaimed.
//!
//! The paths and the ticket names are the v2 script's, so waiters of both kinds see each
//! other's tickets while sessions move over.

use std::io::Write;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::remote::Remote;

/// One waiter's ticket as the host reported it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ticket {
    /// The file name: `<arrival>-<pid>-<tag>`.
    pub name: String,
    /// Whether the pid in the name is a running process on the host.
    pub alive: bool,
}

/// The lock as the host reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LockState {
    /// Whether the lock directory exists.
    pub present: bool,
    /// The pid files in it, each with whether that process runs.
    pub holders: Vec<(u32, bool)>,
}

impl LockState {
    /// Held by somebody who may still be working: present, and either a holder is alive or
    /// there is no holder file to check.
    pub fn held(&self) -> bool {
        self.present && (self.holders.is_empty() || self.holders.iter().any(|(_, a)| *a))
    }

    /// Present, with holder files, every one of them dead: safe to reclaim.
    pub fn stale(&self) -> bool {
        self.present && !self.holders.is_empty() && self.holders.iter().all(|(_, a)| !*a)
    }
}

/// One look at a host: its free space, its lock and its queue.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Observation {
    /// KiB free under the host's home directory, when `df` answered.
    pub free_kib: Option<u64>,
    /// The lock.
    pub lock: LockState,
    /// The tickets, oldest first (sorted by name, which is how v2 sorts them).
    pub tickets: Vec<Ticket>,
}

impl Observation {
    /// Parse what [`OBSERVE`] prints. A line this module does not write is an error, so a
    /// login banner or a changed script is reported, never misread as an empty queue.
    pub fn parse(text: &str) -> Result<Observation, String> {
        let mut o = Observation::default();
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let w: Vec<&str> = line.split_whitespace().collect();
            match w.as_slice() {
                ["free", n] => o.free_kib = n.parse().ok(),
                ["lock"] => o.lock.present = true,
                ["holder", pid, state] => {
                    let pid = pid
                        .parse()
                        .map_err(|_| format!("holder pid {pid:?} is not a number"))?;
                    o.lock.holders.push((pid, *state == "alive"));
                }
                ["ticket", name, state] => o.tickets.push(Ticket {
                    name: name.to_string(),
                    alive: *state == "alive",
                }),
                _ => {
                    return Err(format!(
                        "the host printed a line the queue does not write: {line:?}"
                    ))
                }
            }
        }
        o.tickets.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(o)
    }

    /// How many derives are running or waiting: every live ticket, and the lock when held.
    pub fn load(&self) -> usize {
        self.tickets.iter().filter(|t| t.alive).count() + usize::from(self.lock.held())
    }

    /// GiB free, rounded down, as v2 computed it.
    pub fn free_gib(&self) -> Option<u64> {
        self.free_kib.map(|k| k / 1_048_576)
    }
}

/// What a waiter does next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    /// Its ticket is the oldest live one and the lock is free: take it.
    Take,
    /// Every holder of the lock is dead: reclaim it, then look again.
    Reclaim {
        /// The dead holders' pids.
        dead: Vec<u32>,
    },
    /// Wait: this many live tickets are ahead, and the lock is or is not held.
    Wait {
        /// Live tickets before this one.
        ahead: usize,
        /// Whether the lock is held.
        locked: bool,
    },
    /// The waiter's own ticket is not on the host (removed as dead, or never written).
    Lost,
}

/// The decision, and the dead tickets to remove first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decision {
    /// Tickets whose holder is dead, oldest first.
    pub remove: Vec<String>,
    /// What to do.
    pub step: Step,
}

/// Decide one waiter's next step. Pure: the same observation always gives the same answer.
pub fn decide(obs: &Observation, mine: &str) -> Decision {
    let remove: Vec<String> = obs
        .tickets
        .iter()
        .filter(|t| !t.alive && t.name != mine)
        .map(|t| t.name.clone())
        .collect();
    let step = match obs.tickets.iter().find(|t| t.name == mine) {
        None | Some(Ticket { alive: false, .. }) => Step::Lost,
        Some(_) if obs.lock.stale() => Step::Reclaim {
            dead: obs.lock.holders.iter().map(|(p, _)| *p).collect(),
        },
        Some(_) => {
            let ahead = obs
                .tickets
                .iter()
                .take_while(|t| t.name != mine)
                .filter(|t| t.alive)
                .count();
            if ahead == 0 && !obs.lock.present {
                Step::Take
            } else {
                Step::Wait {
                    ahead,
                    locked: obs.lock.present,
                }
            }
        }
    };
    Decision { remove, step }
}

/// The name of a ticket: the arrival time in nanoseconds since the epoch, nineteen digits as
/// v2's `date +%s%N` writes them, then the holder's pid and the derive's tag. The time is
/// the waiter's own clock read once: waiters on one machine are ordered exactly, and the
/// machines of the cluster keep their clocks within milliseconds of each other.
pub fn ticket_name(arrival_nanos: u128, pid: u32, tag: &str) -> String {
    format!("{arrival_nanos:019}-{pid}-{tag}")
}

/// Report the host: free KiB under `$HOME`, the lock and each holder file, each ticket.
/// Arguments: the queue directory, the lock directory.
pub const OBSERVE: &str = r#"q=$1; l=$2
f=$(df -Pk "$HOME" 2>/dev/null | awk 'NR==2{print $4}')
echo "free ${f:-unknown}"
if [ -d "$l" ]; then
  echo lock
  for p in "$l"/*; do
    [ -e "$p" ] || continue
    n=${p##*/}
    case $n in ''|*[!0-9]*) continue ;; esac
    if kill -0 "$n" 2>/dev/null; then echo "holder $n alive"; else echo "holder $n dead"; fi
  done
fi
if [ -d "$q" ]; then
  for t in "$q"/*; do
    [ -e "$t" ] || continue
    n=${t##*/}
    pid=$(printf '%s' "$n" | cut -d- -f2)
    case $pid in ''|*[!0-9]*) echo "ticket $n dead"; continue ;; esac
    if kill -0 "$pid" 2>/dev/null; then echo "ticket $n alive"; else echo "ticket $n dead"; fi
  done
fi"#;

/// Write a ticket. Arguments: the queue directory, the ticket's name.
const ENQUEUE: &str = r#"mkdir -p "$1" && : > "$1/$2""#;

/// Remove tickets. Arguments: the queue directory, then the names.
const REMOVE: &str = r#"d=$1; shift; for n in "$@"; do rm -f "$d/$n"; done"#;

/// Take the lock, atomically, naming the holder. Exit 3 when somebody else has it.
/// Arguments: the lock directory, the holder's pid.
const TAKE: &str = r#"mkdir "$1" 2>/dev/null || exit 3; : > "$1/$2""#;

/// Reclaim a lock whose holders were all dead: check each again, remove its file, then the
/// directory, which fails — exit 3 — if anybody else's file appeared. Arguments: the lock
/// directory, then the dead pids.
const RECLAIM: &str = r#"l=$1; shift
for p in "$@"; do kill -0 "$p" 2>/dev/null && exit 3; rm -f "$l/$p"; done
rmdir "$l" 2>/dev/null || exit 3"#;

/// Release the lock: remove this derive's pid files, then the directory when it is empty.
/// Arguments: the lock directory, then the pids.
const RELEASE: &str =
    r#"l=$1; shift; for p in "$@"; do rm -f "$l/$p"; done; rmdir "$l" 2>/dev/null; exit 0"#;

/// Why waiting ended without the lock.
#[derive(Debug)]
pub enum QueueError {
    /// The host could not be asked, or answered in a way the queue does not read.
    Host(String),
    /// The waiter's ticket disappeared: its holder was taken for dead.
    Lost,
    /// The declared wait ran out; the ticket is withdrawn.
    TimedOut(Duration),
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueueError::Host(e) => write!(f, "{e}"),
            QueueError::Lost => write!(
                f,
                "this waiter's ticket is gone from the host's queue: its holder was taken for dead"
            ),
            QueueError::TimedOut(d) => write!(f, "gave up after {} s in the queue", d.as_secs()),
        }
    }
}

/// The queue of one host, as the declaration places it.
#[derive(Debug, Clone)]
pub struct Queue<'a> {
    /// The host.
    pub remote: &'a Remote,
    /// The ticket directory.
    pub directory: &'a str,
    /// The lock directory.
    pub lock: &'a str,
    /// Between looks.
    pub poll: Duration,
    /// In all.
    pub wait_max: Duration,
}

fn host_err(what: &str, e: impl std::fmt::Display) -> QueueError {
    QueueError::Host(format!("{what}: {e}"))
}

impl Queue<'_> {
    /// Look at the host once.
    pub fn observe(&self) -> Result<Observation, QueueError> {
        let out = self
            .remote
            .run(OBSERVE, &[self.directory, self.lock], None)
            .map_err(|e| host_err(&format!("{} could not be asked", self.remote.label), e))?;
        if !out.ok() {
            return Err(QueueError::Host(format!(
                "{} did not answer the observation (exit {:?}): {}",
                self.remote.label,
                out.code,
                out.stderr.trim()
            )));
        }
        Observation::parse(&out.stdout).map_err(QueueError::Host)
    }

    /// Join the queue under the holder's pid; returns the ticket's name.
    pub fn enqueue(&self, holder_pid: u32, tag: &str) -> Result<String, QueueError> {
        let arrival = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let name = ticket_name(arrival, holder_pid, tag);
        let out = self
            .remote
            .run(ENQUEUE, &[self.directory, &name], None)
            .map_err(|e| host_err("the ticket could not be written", e))?;
        if !out.ok() {
            return Err(QueueError::Host(format!(
                "the ticket could not be written: {}",
                out.stderr.trim()
            )));
        }
        Ok(name)
    }

    /// Withdraw a ticket; a ticket already gone is not an error.
    pub fn withdraw(&self, ticket: &str) {
        let _ = self.remote.run(REMOVE, &[self.directory, ticket], None);
    }

    /// Release the lock held under these pids.
    pub fn release(&self, pids: &[u32]) {
        let pids: Vec<String> = pids.iter().map(u32::to_string).collect();
        let mut args = vec![self.lock];
        args.extend(pids.iter().map(String::as_str));
        let _ = self.remote.run(RELEASE, &args, None);
    }

    /// Wait in the queue until this ticket takes the lock under `holder_pid`. Every change
    /// of position, every removed ticket and every reclaim is written to `log`. On success
    /// the ticket is withdrawn and the lock is held; on failure the ticket is withdrawn.
    pub fn wait(
        &self,
        ticket: &str,
        holder_pid: u32,
        log: &mut dyn Write,
    ) -> Result<(), QueueError> {
        let started = Instant::now();
        let mut last: Option<Step> = None;
        let result = loop {
            let obs = match self.observe() {
                Ok(o) => o,
                Err(e) => break Err(e),
            };
            let d = decide(&obs, ticket);
            if !d.remove.is_empty() {
                let _ = writeln!(
                    log,
                    "derive-offload: removed {} dead waiter ticket(s) on {}: {}",
                    d.remove.len(),
                    self.remote.label,
                    d.remove.join(", ")
                );
                let mut args = vec![self.directory];
                args.extend(d.remove.iter().map(String::as_str));
                let _ = self.remote.run(REMOVE, &args, None);
            }
            match &d.step {
                Step::Lost => break Err(QueueError::Lost),
                Step::Take => {
                    let pid = holder_pid.to_string();
                    match self.remote.run(TAKE, &[self.lock, &pid], None) {
                        Ok(o) if o.ok() => break Ok(()),
                        Ok(_) => {} // somebody outside the queue took it first: look again
                        Err(e) => break Err(host_err("the lock could not be taken", e)),
                    }
                }
                Step::Reclaim { dead } => {
                    let pids: Vec<String> = dead.iter().map(u32::to_string).collect();
                    let mut args = vec![self.lock];
                    args.extend(pids.iter().map(String::as_str));
                    if let Ok(o) = self.remote.run(RECLAIM, &args, None) {
                        if o.ok() {
                            let _ = writeln!(
                                log,
                                "derive-offload: reclaimed the lock on {} from dead holder(s) {}",
                                self.remote.label,
                                pids.join(", ")
                            );
                            continue;
                        }
                    }
                }
                Step::Wait { ahead, locked } => {
                    if last.as_ref() != Some(&d.step) {
                        let _ = writeln!(
                            log,
                            "derive-offload: waiting on {}: {ahead} ahead, lock {}",
                            self.remote.label,
                            if *locked { "held" } else { "free" }
                        );
                    }
                }
            }
            last = Some(d.step);
            if started.elapsed() >= self.wait_max {
                break Err(QueueError::TimedOut(started.elapsed()));
            }
            std::thread::sleep(self.poll);
        };
        self.withdraw(ticket);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::remote::Holder;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    #[test]
    fn a_ticket_name_sorts_by_arrival_and_names_its_holder() {
        assert_eq!(
            ticket_name(1_760_140_000_000_000_005, 4242, "0123456789"),
            "1760140000000000005-4242-0123456789"
        );
        assert!(
            ticket_name(9, 2, "t") < ticket_name(10, 1, "t"),
            "arrival is zero-padded"
        );
    }

    fn on_disk<'a>(r: &'a Remote, dir: &'a Path, lock: &'a Path) -> Queue<'a> {
        Queue {
            remote: r,
            directory: dir.to_str().unwrap(),
            lock: lock.to_str().unwrap(),
            poll: Duration::from_millis(40),
            wait_max: Duration::from_secs(30),
        }
    }

    /// Two waiters on one host take the lock in the order they arrived, although the later one
    /// started waiting first: with the old `until mkdir` loop it would simply have won.
    #[test]
    fn two_waiters_are_served_in_arrival_order() {
        let tmp = tempfile::tempdir().unwrap();
        let (dir, lock) = (tmp.path().join("queue"), tmp.path().join("lock"));
        let r = Remote::local();
        let q = on_disk(&r, &dir, &lock);
        let first = Holder::start(&r).unwrap();
        let first_ticket = q.enqueue(first.pid, "aaaaaaaaaa").unwrap();
        let second = Holder::start(&r).unwrap();
        let second_ticket = q.enqueue(second.pid, "bbbbbbbbbb").unwrap();
        assert!(
            first_ticket < second_ticket,
            "{first_ticket} does not sort before {second_ticket}"
        );

        let order = Arc::new(Mutex::new(Vec::new()));
        std::thread::scope(|s| {
            let late_but_eager = s.spawn(|| {
                let q = on_disk(&r, &dir, &lock);
                q.wait(&second_ticket, second.pid, &mut std::io::sink())
                    .unwrap();
                order.lock().unwrap().push("second");
                q.release(&[second.pid]);
            });
            std::thread::sleep(Duration::from_millis(300));
            assert!(
                order.lock().unwrap().is_empty(),
                "the second waiter took the lock ahead of the first"
            );
            let q = on_disk(&r, &dir, &lock);
            q.wait(&first_ticket, first.pid, &mut std::io::sink())
                .unwrap();
            order.lock().unwrap().push("first");
            std::thread::sleep(Duration::from_millis(200));
            q.release(&[first.pid]);
            late_but_eager.join().unwrap();
        });
        assert_eq!(*order.lock().unwrap(), ["first", "second"]);
        assert!(!lock.exists(), "the lock outlived its last holder");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "a ticket outlived its waiter"
        );
    }

    /// A waiter whose holder died — killed, or its connection lost — leaves a ticket ahead of
    /// a live one. The live one removes it, says so, and takes the lock.
    #[test]
    fn a_dead_waiters_ticket_does_not_block_the_queue() {
        let tmp = tempfile::tempdir().unwrap();
        let (dir, lock) = (tmp.path().join("queue"), tmp.path().join("lock"));
        let r = Remote::local();
        let q = on_disk(&r, &dir, &lock);
        let doomed = Holder::start(&r).unwrap();
        let dead_ticket = q.enqueue(doomed.pid, "dddddddddd").unwrap();
        drop(doomed);
        let live = Holder::start(&r).unwrap();
        let live_ticket = q.enqueue(live.pid, "llllllllll").unwrap();
        assert!(dead_ticket < live_ticket);

        let mut log = Vec::new();
        let started = Instant::now();
        q.wait(&live_ticket, live.pid, &mut log).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the dead ticket held the queue"
        );
        let log = String::from_utf8(log).unwrap();
        assert!(
            log.contains("removed 1 dead waiter ticket") && log.contains(&dead_ticket),
            "{log}"
        );
        assert!(!dir.join(&dead_ticket).exists());
        assert!(
            lock.join(live.pid.to_string()).exists(),
            "the lock does not name its holder"
        );
        q.release(&[live.pid]);
        assert!(!lock.exists());
    }

    /// A lock whose every holder is dead is reclaimed aloud; a lock with no holder file — a v1 or
    /// v2 waiter's, which nobody can check — is waited for until the declared wait runs out.
    #[test]
    fn a_dead_lock_is_reclaimed_and_an_uncheckable_one_is_waited_for() {
        let tmp = tempfile::tempdir().unwrap();
        let (dir, lock) = (tmp.path().join("queue"), tmp.path().join("lock"));
        let r = Remote::local();
        let gone = Holder::start(&r).unwrap();
        let gone_pid = gone.pid;
        drop(gone);
        std::fs::create_dir(&lock).unwrap();
        std::fs::write(lock.join(gone_pid.to_string()), "").unwrap();

        let q = on_disk(&r, &dir, &lock);
        let me = Holder::start(&r).unwrap();
        let ticket = q.enqueue(me.pid, "rrrrrrrrrr").unwrap();
        let mut log = Vec::new();
        q.wait(&ticket, me.pid, &mut log).unwrap();
        let log = String::from_utf8(log).unwrap();
        assert!(
            log.contains(&format!(
                "reclaimed the lock on local from dead holder(s) {gone_pid}"
            )),
            "{log}"
        );
        q.release(&[me.pid]);

        std::fs::create_dir(&lock).unwrap(); // the way v1 and v2 take it: an empty directory
        let mut q = on_disk(&r, &dir, &lock);
        q.wait_max = Duration::from_millis(400);
        let ticket = q.enqueue(me.pid, "wwwwwwwwww").unwrap();
        let err = q.wait(&ticket, me.pid, &mut std::io::sink()).unwrap_err();
        assert!(matches!(err, QueueError::TimedOut(_)), "{err}");
        assert!(
            lock.exists(),
            "a lock nobody can check was taken from its holder"
        );
        assert!(
            !dir.join(&ticket).exists(),
            "a waiter that gave up left its ticket"
        );
        let obs: Observation = q.observe().unwrap();
        assert!(obs.lock.held() && obs.tickets.is_empty());
    }

    fn obs(lines: &str) -> Observation {
        Observation::parse(lines).unwrap()
    }

    #[test]
    fn the_oldest_live_ticket_takes_a_free_lock_and_only_it() {
        let o = obs("ticket 300-3-c alive\nticket 100-1-a alive\nticket 200-2-b alive\n");
        assert_eq!(
            o.tickets[0].name, "100-1-a",
            "tickets are sorted oldest first"
        );
        assert_eq!(decide(&o, "100-1-a").step, Step::Take);
        assert_eq!(
            decide(&o, "200-2-b").step,
            Step::Wait {
                ahead: 1,
                locked: false
            }
        );
        assert_eq!(
            decide(&o, "300-3-c").step,
            Step::Wait {
                ahead: 2,
                locked: false
            }
        );
        assert_eq!(o.load(), 3);
    }

    #[test]
    fn the_head_waits_for_a_held_lock() {
        let o = obs("lock\nholder 77 alive\nticket 100-1-a alive\n");
        assert_eq!(
            decide(&o, "100-1-a").step,
            Step::Wait {
                ahead: 0,
                locked: true
            }
        );
        assert_eq!(o.load(), 2);
    }

    #[test]
    fn a_lock_without_a_holder_file_is_waited_for_never_reclaimed() {
        let o = obs("lock\nticket 100-1-a alive\n");
        assert!(o.lock.held() && !o.lock.stale());
        assert_eq!(
            decide(&o, "100-1-a").step,
            Step::Wait {
                ahead: 0,
                locked: true
            }
        );
    }

    #[test]
    fn a_lock_whose_holders_are_all_dead_is_reclaimed_by_any_waiter() {
        let o = obs(
            "lock\nholder 77 dead\nholder 78 dead\nticket 100-1-a alive\nticket 200-2-b alive\n",
        );
        for mine in ["100-1-a", "200-2-b"] {
            assert_eq!(decide(&o, mine).step, Step::Reclaim { dead: vec![77, 78] });
        }
        let one_alive = obs("lock\nholder 77 dead\nholder 78 alive\nticket 100-1-a alive\n");
        assert_eq!(
            decide(&one_alive, "100-1-a").step,
            Step::Wait {
                ahead: 0,
                locked: true
            }
        );
    }

    #[test]
    fn dead_tickets_are_removed_and_skipped_wherever_they_stand() {
        let o = obs("ticket 100-1-a dead\nticket 200-2-b alive\nticket 300-3-c dead\nticket 400-4-d alive\n");
        let d = decide(&o, "400-4-d");
        assert_eq!(d.remove, vec!["100-1-a", "300-3-c"]);
        assert_eq!(
            d.step,
            Step::Wait {
                ahead: 1,
                locked: false
            }
        );
        assert_eq!(o.load(), 2);
    }

    #[test]
    fn a_waiter_whose_ticket_is_gone_or_dead_is_lost() {
        let o = obs("ticket 100-1-a alive\n");
        assert_eq!(decide(&o, "999-9-z").step, Step::Lost);
        let o = obs("ticket 100-1-a dead\n");
        let d = decide(&o, "100-1-a");
        assert_eq!(d.step, Step::Lost);
        assert!(d.remove.is_empty(), "a waiter never removes its own ticket");
    }

    #[test]
    fn a_line_the_queue_does_not_write_is_an_error_not_an_empty_queue() {
        assert!(Observation::parse("Welcome to lundra\n").is_err());
        assert!(Observation::parse("holder x alive\n").is_err());
        assert_eq!(obs("free unknown\n").free_kib, None);
        assert_eq!(obs("free 20971520\n").free_gib(), Some(20));
    }
}
