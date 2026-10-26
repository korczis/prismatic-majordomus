//! The machine slot: a counting, machine-wide admission for heavy work (I4006).
//!
//! On 2026-10-22 eleven cargo processes from eleven worktrees of one Mac queued for 47
//! minutes and more at 0% CPU on one shared build lock, each of them then rebuilding the
//! crate; pre-push hooks timed out at 900 s and 2400 s inside that queue, the load stood
//! between 50 and 120, and five cold private builds started because the lock was held, which
//! pushed free disk under 35 GiB. Rule `project.the-cooperation-model-is-the-owners-word`
//! clause 7 already said what should have happened: at most five builders run at once on one
//! machine, and none starts while less than 35 GiB is free. Nothing held a build to it.
//!
//! This is what holds it. A slot is a file in a directory every checkout of the machine
//! agrees on — beside the derive lock, in the directory holding the primary checkout
//! (`lib/machine_lock.sh`), or where `MAJORDOMUS_MACHINE_SLOTS` says. Under one `flock(2)` on
//! that directory's `.lock`, an admission reads the holders, reclaims the ones whose process
//! is proven gone, measures the free disk and either writes a holder record or waits in a
//! queue whose records anyone can read. The numbers are the rule's, declared once in the
//! policy at `machine.heavy_work` and read from there; nothing here carries a default for
//! them, so a repository that declares none admits everything and says so.
//!
//! What it decides:
//! - **A holder is a process.** Its record names the pid and the process's start time. It is
//!   reclaimed only when that pid no longer runs, or runs with another start time (the pid was
//!   reused); a process whose start cannot be read is alive, because reclaiming what was not
//!   proven gone hands its slot to a second builder. No name is ever matched.
//! - **The wait is bounded and visible.** A waiter writes a record naming itself, what it
//!   waits for and since when; [`status`] lists them, so a session can report "waiting on
//!   machine.slot" as progress with a reason. Admission is in arrival order.
//! - **A refusal is an answer.** Past the bound, or below the disk floor, the caller gets a
//!   typed refusal naming every holder, never an error and never a silent wait.
//!
//! The module is the crate's own (`pub(crate)`): it is reached through the `machine` capabilities,
//! and its unit tests below drive it against real processes and a directory of their own.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The rule whose numbers the slot holds.
pub const RULE: &str = "project.the-cooperation-model-is-the-owners-word";
/// Where in the policy those numbers are declared.
pub const POLICY_KEY: &str = "machine.heavy_work";
/// The variable that names the slot directory instead of the one beside the primary checkout.
pub const DIRECTORY_ENV: &str = "MAJORDOMUS_MACHINE_SLOTS";
/// The slot directory's name beside the primary checkout, next to `.derive.lock`.
pub const DIRECTORY_NAME: &str = ".machine.slots";
/// What a waiter is said to be waiting on, the word a progress report carries.
pub const WAITING_ON: &str = "machine.slot";

/// The limits an admission is judged by, and where they were read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotLimits {
    /// How many holders the machine admits at once; absent admits every caller.
    pub slots: Option<u32>,
    /// GiB that must be free; absent refuses nobody for disk.
    pub min_free_gib: Option<u64>,
    /// How long a caller that names no bound waits, in seconds; absent waits not at all.
    pub wait_seconds: Option<u64>,
    /// The rule whose numbers these are.
    pub rule: String,
    /// Where they are declared: the policy file and the key.
    pub declared_in: String,
}

impl SlotLimits {
    /// SlotLimits as a policy declares them, at the policy's canonical place.
    pub fn declared(slots: Option<u32>, min_free_gib: Option<u64>, wait: Option<u64>) -> Self {
        SlotLimits {
            slots,
            min_free_gib,
            wait_seconds: wait,
            rule: RULE.into(),
            declared_in: format!(".ai/repo/policy.yaml {POLICY_KEY}"),
        }
    }

    /// The limits of a policy file's `machine.heavy_work` block.
    pub fn from_policy(policy: &crate::policy::HeavyWorkPolicy, policy_path: &str) -> Self {
        SlotLimits {
            declared_in: format!("{policy_path} {POLICY_KEY}"),
            ..SlotLimits::declared(policy.slots, policy.min_free_gib, policy.wait_seconds)
        }
    }
}

/// Whether a holder's process is still the one that took the slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotLiveness {
    /// The pid runs, with the start time the record names.
    Alive,
    /// The pid runs, and its start time could not be read: kept, never reclaimed.
    Unverified,
    /// The pid no longer runs, or runs with another start time: the slot is reclaimable.
    Dead,
}

/// One holder of a slot, as a reader sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotHolder {
    /// The process that holds the slot.
    pub pid: u32,
    /// What it said it is doing.
    pub purpose: String,
    /// When it was admitted (RFC 3339, UTC).
    pub since: String,
    /// Seconds since then.
    pub age_seconds: u64,
    /// Whether that process still runs.
    pub liveness: SlotLiveness,
}

/// One waiter for a slot, as a reader sees it: the progress a waiting session reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotWaiter {
    /// The process that waits.
    pub pid: u32,
    /// What it will do once admitted.
    pub purpose: String,
    /// When it started waiting (RFC 3339, UTC).
    pub since: String,
    /// Seconds it has waited.
    pub waiting_seconds: u64,
    /// What it waits on, in the words a progress report carries: `machine.slot: 5 of 5 held`.
    pub waiting_on: String,
}

/// The slot directory as it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotStatus {
    /// The directory the slots are kept in.
    pub directory: String,
    /// The limits, and where they were read.
    pub limits: SlotLimits,
    /// MiB free on the volume measured; absent when it could not be measured.
    pub free_mib: Option<u64>,
    /// The volume measured: the work's own path, or the slot directory.
    pub volume: String,
    /// How many holders whose process is not proven gone.
    pub held: usize,
    /// Every holder, oldest first.
    pub holders: Vec<SlotHolder>,
    /// Every waiter, in the order they will be admitted.
    pub waiters: Vec<SlotWaiter>,
    /// One line: held of slots, free against the floor, the waiters.
    pub summary: String,
}

/// What an acquisition came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotVerdict {
    /// A slot was taken for the caller's process.
    Admitted,
    /// The caller's process already held one; nothing was taken.
    AlreadyHeld,
    /// Refused; the refusal says why and who holds the slots.
    Refused,
}

/// Why an acquisition was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotRefusalReason {
    /// Every slot stayed held for the whole wait.
    SlotsFull,
    /// Less than the floor is free on the volume the work writes to.
    DiskFloor,
}

/// A refusal, with what it measured and what to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotRefusal {
    /// Which limit refused.
    pub reason: SlotRefusalReason,
    /// The measured values and every holder, in a sentence.
    pub detail: String,
    /// What the caller can do instead.
    pub remedy: String,
}

/// The answer to an acquisition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotAnswer {
    /// Admitted, already held, or refused.
    pub verdict: SlotVerdict,
    /// Present when refused.
    pub refusal: Option<SlotRefusal>,
    /// Seconds the caller waited in the queue.
    pub waited_seconds: u64,
    /// Holders reclaimed on the way, because their process was proven gone.
    pub reclaimed: Vec<SlotHolder>,
    /// The slot directory as it stood when the answer was given.
    pub status: SlotStatus,
}

/// The answer to a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotRelease {
    /// Whether the process held a slot that is now free.
    pub released: bool,
    /// The record that was removed.
    pub holder: Option<SlotHolder>,
    /// Whether a waiter record of the process was withdrawn as well.
    pub withdrew_waiter: bool,
}

/// What a caller asks for.
#[derive(Debug, Clone)]
pub struct Request {
    /// The process that will hold the slot; it must be running.
    pub pid: u32,
    /// What it is about to do, in a person's words.
    pub purpose: String,
    /// How long to wait for a slot; `None` waits as long as the limits say.
    pub wait: Option<Duration>,
    /// How often to look again while waiting.
    pub poll: Duration,
    /// The path the work writes to, whose volume the floor is judged on.
    pub volume: Option<PathBuf>,
}

impl Request {
    /// A request with the limits' wait, a look every second and the slot directory's volume.
    pub fn new(pid: u32, purpose: &str) -> Self {
        Request {
            pid,
            purpose: purpose.into(),
            wait: None,
            poll: Duration::from_secs(1),
            volume: None,
        }
    }
}

/// A record on disk: one file per process, under `holders/` or `waiters/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    pid: u32,
    /// The process's start time when the record was written, when the platform says it.
    started: Option<String>,
    purpose: String,
    /// Seconds since the epoch.
    since: u64,
    /// The pid as text, the order's tie-breaker; filled in when the record is read.
    #[serde(skip)]
    identity: String,
}

/// Arrival order: the oldest record first, the pid breaking a tie within one second.
impl crate::order::Ordered for Record {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let since = i64::try_from(self.since).unwrap_or(i64::MAX);
        crate::order::OrderKey::plain("", &self.identity).ranked(since)
    }
}

/// The slot directory for a checkout: `MAJORDOMUS_MACHINE_SLOTS`, else `.machine.slots` in
/// the directory holding the primary checkout, beside the derive lock. `None` outside git.
pub fn directory(root: &Path) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(DIRECTORY_ENV).filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    crate::capability::builtin::brief::primary_home(root).map(|h| h.join(DIRECTORY_NAME))
}

/// The start time of a process, as a string that changes when the pid is reused; `None`
/// when the platform cannot say it or the process is gone.
pub fn process_start(pid: u32) -> Option<String> {
    platform_start(pid)
}

#[cfg(target_os = "macos")]
fn platform_start(pid: u32) -> Option<String> {
    let pid = libc::c_int::try_from(pid).ok()?;
    // SAFETY: proc_bsdinfo is plain old data, so all zeroes is a valid value of it.
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: the buffer is a proc_bsdinfo of exactly `size` bytes, owned by this frame.
    let n = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        )
    };
    (n == size).then(|| format!("{}.{:06}", info.pbi_start_tvsec, info.pbi_start_tvusec))
}

#[cfg(target_os = "linux")]
fn platform_start(pid: u32) -> Option<String> {
    // field 22 of /proc/<pid>/stat, counted after the command name, which may hold spaces
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace()
        .nth(19)
        .map(|ticks| format!("boot+{ticks}"))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_start(_: u32) -> Option<String> {
    None
}

fn liveness(record: &Record) -> SlotLiveness {
    if !crate::lease::alive(record.pid) {
        return SlotLiveness::Dead;
    }
    match (&record.started, process_start(record.pid)) {
        (Some(then), Some(now)) if *then != now => SlotLiveness::Dead,
        (Some(_), Some(_)) => SlotLiveness::Alive,
        _ => SlotLiveness::Unverified,
    }
}

/// MiB free on the volume holding `path`, measured at its nearest existing ancestor.
pub fn free_mib(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let existing = path.ancestors().find(|a| a.exists())?;
    let c = std::ffi::CString::new(existing.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs is plain old data; the path is a NUL-terminated string this frame owns.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    #[allow(clippy::unnecessary_cast)] // the field widths differ between platforms
    let bytes = (s.f_bavail as u64).saturating_mul(s.f_frsize as u64);
    Some(bytes / (1024 * 1024))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stamp(secs: u64) -> String {
    crate::peers::rfc3339(UNIX_EPOCH + Duration::from_secs(secs))
}

/// The directory, its two record directories and the lock every change is made under.
struct Slots {
    dir: PathBuf,
}

impl Slots {
    fn open(dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir.join("holders"))?;
        fs::create_dir_all(dir.join("waiters"))?;
        Ok(Slots {
            dir: dir.to_path_buf(),
        })
    }

    /// The exclusive lock every read-modify-write of the records is made under; released
    /// when the returned file is dropped.
    fn lock(&self) -> io::Result<File> {
        let f = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.dir.join(".lock"))?;
        // flock(2) by hand: File::lock is newer than the crate's minimum Rust version
        use std::os::unix::io::AsRawFd;
        loop {
            // SAFETY: flock(2) on a descriptor this frame owns; no memory is touched.
            if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } == 0 {
                return Ok(f);
            }
            let e = io::Error::last_os_error();
            if e.kind() != io::ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }

    fn path(&self, kind: &str, pid: u32) -> PathBuf {
        self.dir.join(kind).join(format!("{pid}.json"))
    }

    /// Every record of one kind; a file that is not a record is skipped, never removed.
    fn read(&self, kind: &str) -> Vec<Record> {
        let Ok(entries) = fs::read_dir(self.dir.join(kind)) else {
            return Vec::new();
        };
        let mut out: Vec<Record> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
            .filter_map(|e| fs::read(e.path()).ok())
            .filter_map(|b| serde_json::from_slice::<Record>(&b).ok())
            .map(|r| Record {
                identity: r.pid.to_string(),
                ..r
            })
            .collect();
        crate::order::canonical(&mut out);
        out
    }

    fn write(&self, kind: &str, record: &Record) -> io::Result<()> {
        let path = self.path(kind, record.pid);
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        fs::write(&tmp, serde_json::to_vec(record).map_err(io::Error::other)?)?;
        fs::rename(&tmp, &path)
    }

    fn remove(&self, kind: &str, pid: u32) -> bool {
        fs::remove_file(self.path(kind, pid)).is_ok()
    }

    /// Remove every record of one kind whose process is proven gone; the removed holders.
    fn reclaim(&self, kind: &str, at: u64) -> Vec<SlotHolder> {
        let mut gone = Vec::new();
        for r in self.read(kind) {
            if liveness(&r) == SlotLiveness::Dead && self.remove(kind, r.pid) {
                gone.push(holder_view(&r, SlotLiveness::Dead, at));
            }
        }
        gone
    }
}

fn holder_view(r: &Record, liveness: SlotLiveness, at: u64) -> SlotHolder {
    SlotHolder {
        pid: r.pid,
        purpose: r.purpose.clone(),
        since: stamp(r.since),
        age_seconds: at.saturating_sub(r.since),
        liveness,
    }
}

fn snapshot(slots: &Slots, limits: &SlotLimits, volume: &Path) -> SlotStatus {
    let at = now();
    let holders: Vec<SlotHolder> = slots
        .read("holders")
        .iter()
        .map(|r| holder_view(r, liveness(r), at))
        .collect();
    let held = holders
        .iter()
        .filter(|h| h.liveness != SlotLiveness::Dead)
        .count();
    let free = free_mib(volume);
    let waiting_on = waiting_on(held, limits);
    let waiters: Vec<SlotWaiter> = slots
        .read("waiters")
        .iter()
        .filter(|r| liveness(r) != SlotLiveness::Dead)
        .map(|r| SlotWaiter {
            pid: r.pid,
            purpose: r.purpose.clone(),
            since: stamp(r.since),
            waiting_seconds: at.saturating_sub(r.since),
            waiting_on: waiting_on.clone(),
        })
        .collect();
    let summary = format!(
        "{}; {}; {} waiting",
        waiting_on.trim_start_matches(&format!("{WAITING_ON}: ")),
        match (free, limits.min_free_gib) {
            (Some(f), Some(g)) => format!("{} GiB free, floor {g} GiB", f / 1024),
            (Some(f), None) => format!("{} GiB free, no floor declared", f / 1024),
            (None, _) => "free space not measured".into(),
        },
        waiters.len()
    );
    SlotStatus {
        directory: slots.dir.display().to_string(),
        limits: limits.clone(),
        free_mib: free,
        volume: volume.display().to_string(),
        held,
        holders,
        waiters,
        summary,
    }
}

fn waiting_on(held: usize, limits: &SlotLimits) -> String {
    match limits.slots {
        Some(n) => format!("{WAITING_ON}: {held} of {n} held"),
        None => format!("{WAITING_ON}: {held} held, no limit declared"),
    }
}

/// The holders in one clause: `pid 123 (cargo build, 4m 2s), pid 456 (...)`.
fn named(holders: &[SlotHolder]) -> String {
    if holders.is_empty() {
        return "no holder".into();
    }
    holders
        .iter()
        .map(|h| {
            format!(
                "pid {} ({}, {}m {}s)",
                h.pid,
                h.purpose,
                h.age_seconds / 60,
                h.age_seconds % 60
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The slot directory as it stands, for a work whose files go to `volume` (the slot
/// directory's own when `None`). Reads only: a dead holder is reported, not removed.
pub fn status(dir: &Path, limits: &SlotLimits, volume: Option<&Path>) -> io::Result<SlotStatus> {
    // a read creates nothing: a directory nobody has taken a slot in yet holds no record
    let slots = Slots {
        dir: dir.to_path_buf(),
    };
    Ok(snapshot(&slots, limits, volume.unwrap_or(dir)))
}

/// Take a slot for `request.pid`, waiting in arrival order up to the request's bound.
///
/// Errors are only the directory's: it could not be created, locked or written. Every
/// judgement — admitted, already held, refused for disk or for a full machine — is an
/// [`SlotAnswer`].
pub fn acquire(dir: &Path, limits: &SlotLimits, request: &Request) -> io::Result<SlotAnswer> {
    if !crate::lease::alive(request.pid) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "pid {} is not a running process, so it cannot hold a slot",
                request.pid
            ),
        ));
    }
    let slots = Slots::open(dir)?;
    let volume = request.volume.clone().unwrap_or_else(|| dir.to_path_buf());
    let wait = request
        .wait
        .unwrap_or_else(|| Duration::from_secs(limits.wait_seconds.unwrap_or(0)));
    let started = Instant::now();
    let mine = Record {
        pid: request.pid,
        started: process_start(request.pid),
        purpose: request.purpose.clone(),
        since: now(),
        identity: request.pid.to_string(),
    };
    let mut reclaimed = Vec::new();
    loop {
        let decided = {
            let _guard = slots.lock()?;
            let at = now();
            reclaimed.extend(slots.reclaim("holders", at));
            slots.reclaim("waiters", at);
            admit(&slots, limits, &mine, &volume, at)?
        };
        let waited = started.elapsed().as_secs();
        match decided {
            Admission::Decided(verdict, refusal) => {
                let _guard = slots.lock()?;
                slots.remove("waiters", mine.pid);
                return Ok(SlotAnswer {
                    verdict,
                    refusal,
                    waited_seconds: waited,
                    reclaimed,
                    status: snapshot(&slots, limits, &volume),
                });
            }
            Admission::Wait if started.elapsed() >= wait => {
                let _guard = slots.lock()?;
                slots.remove("waiters", mine.pid);
                let status = snapshot(&slots, limits, &volume);
                let refusal = SlotRefusal {
                    reason: SlotRefusalReason::SlotsFull,
                    detail: format!(
                        "{} of {} machine slots stayed held for {waited}s: {}; the limit is rule {} clause 7, declared at {}",
                        status.held,
                        limits.slots.unwrap_or_default(),
                        named(&status.holders),
                        limits.rule,
                        limits.declared_in
                    ),
                    remedy: "wait for a holder to finish and ask again (a longer wait), or run the work on another machine (majordomus offload)".into(),
                };
                return Ok(SlotAnswer {
                    verdict: SlotVerdict::Refused,
                    refusal: Some(refusal),
                    waited_seconds: waited,
                    reclaimed,
                    status,
                });
            }
            Admission::Wait => std::thread::sleep(
                request
                    .poll
                    .min(wait.saturating_sub(started.elapsed()))
                    .max(Duration::from_millis(50)),
            ),
        }
    }
}

enum Admission {
    Decided(SlotVerdict, Option<SlotRefusal>),
    Wait,
}

/// One look, under the lock: admit, refuse, or enter (or stay in) the queue.
fn admit(
    slots: &Slots,
    limits: &SlotLimits,
    mine: &Record,
    volume: &Path,
    at: u64,
) -> io::Result<Admission> {
    let holders = slots.read("holders");
    if let Some(own) = holders.iter().find(|h| h.pid == mine.pid) {
        if own.started == mine.started {
            return Ok(Admission::Decided(SlotVerdict::AlreadyHeld, None));
        }
    }
    if let Some(floor) = limits.min_free_gib {
        if let Some(free) = free_mib(volume) {
            if free < floor.saturating_mul(1024) {
                let views: Vec<SlotHolder> = holders
                    .iter()
                    .map(|r| holder_view(r, liveness(r), at))
                    .collect();
                return Ok(Admission::Decided(
                    SlotVerdict::Refused,
                    Some(SlotRefusal {
                        reason: SlotRefusalReason::DiskFloor,
                        detail: format!(
                            "{} GiB ({free} MiB) free on the volume of {}, under the floor of {floor} GiB (rule {} clause 7, declared at {}); holders: {}",
                            free / 1024,
                            volume.display(),
                            limits.rule,
                            limits.declared_in,
                            named(&views)
                        ),
                        remedy: "free space first (scripts/reap-orphans --targets, then --reclaim), or run the work on another machine (majordomus offload)".into(),
                    }),
                ));
            }
        }
    }
    let Some(n) = limits.slots else {
        slots.write("holders", mine)?;
        return Ok(Admission::Decided(SlotVerdict::Admitted, None));
    };
    // arrival order: the waiters that came before this one are admitted before it
    let waiters = slots.read("waiters");
    let queued = waiters.iter().find(|w| w.pid == mine.pid).cloned();
    let since = queued.as_ref().map_or(mine.since, |q| q.since);
    let ahead = waiters
        .iter()
        .filter(|w| w.pid != mine.pid && (w.since, w.pid) < (since, mine.pid))
        .count();
    let held = holders.iter().filter(|h| h.pid != mine.pid).count();
    if held + ahead < n as usize {
        slots.write("holders", mine)?;
        return Ok(Admission::Decided(SlotVerdict::Admitted, None));
    }
    if queued.is_none() {
        slots.write("waiters", mine)?;
    }
    Ok(Admission::Wait)
}

/// Give back the slot `pid` holds, and withdraw it from the queue if it waits.
pub fn release(dir: &Path, pid: u32) -> io::Result<SlotRelease> {
    let slots = Slots::open(dir)?;
    let _guard = slots.lock()?;
    let at = now();
    let record = slots.read("holders").into_iter().find(|r| r.pid == pid);
    let released = record.is_some() && slots.remove("holders", pid);
    let withdrew_waiter = slots.remove("waiters", pid);
    Ok(SlotRelease {
        released,
        holder: record.map(|r| {
            let l = liveness(&r);
            holder_view(&r, l, at)
        }),
        withdrew_waiter,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command};

    fn dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("mj-slot-{name}-{}-{}", std::process::id(), now()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// A process of this machine that runs until it is killed.
    fn sleeper() -> Child {
        Command::new("sleep")
            .arg("300")
            .spawn()
            .expect("sleep runs")
    }

    fn quick(pid: u32, purpose: &str, wait_ms: u64) -> Request {
        Request {
            pid,
            purpose: purpose.into(),
            wait: Some(Duration::from_millis(wait_ms)),
            poll: Duration::from_millis(50),
            volume: None,
        }
    }

    #[test]
    fn n_holders_are_admitted_and_the_next_waits_its_bound_and_is_refused_naming_them() {
        let d = dir("full");
        let limits = SlotLimits::declared(Some(2), None, None);
        let (mut a, mut b, mut c) = (sleeper(), sleeper(), sleeper());
        for (p, what) in [(&a, "build a"), (&b, "build b")] {
            let got = acquire(&d, &limits, &quick(p.id(), what, 0)).unwrap();
            assert_eq!(got.verdict, SlotVerdict::Admitted, "{what}");
        }
        let t = Instant::now();
        let third = acquire(&d, &limits, &quick(c.id(), "build c", 300)).unwrap();
        assert!(
            t.elapsed() >= Duration::from_millis(300),
            "it waited its bound"
        );
        assert!(t.elapsed() < Duration::from_secs(5), "and no longer");
        assert_eq!(third.verdict, SlotVerdict::Refused);
        let r = third.refusal.expect("a refusal");
        assert_eq!(r.reason, SlotRefusalReason::SlotsFull);
        assert!(
            r.detail.contains(&format!("pid {} (build a", a.id())),
            "{}",
            r.detail
        );
        assert!(
            r.detail.contains(&format!("pid {} (build b", b.id())),
            "{}",
            r.detail
        );
        assert!(r.detail.contains(RULE), "{}", r.detail);
        assert!(
            third.status.waiters.is_empty(),
            "a refused waiter leaves the queue"
        );
        for p in [&mut a, &mut b, &mut c] {
            let _ = p.kill();
            let _ = p.wait();
        }
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_dead_holder_is_reclaimed_and_named() {
        let d = dir("dead");
        let limits = SlotLimits::declared(Some(1), None, None);
        let mut a = sleeper();
        let pid = a.id();
        assert_eq!(
            acquire(&d, &limits, &quick(pid, "gone soon", 0))
                .unwrap()
                .verdict,
            SlotVerdict::Admitted
        );
        a.kill().unwrap();
        a.wait().unwrap();
        assert_eq!(
            status(&d, &limits, None).unwrap().holders[0].liveness,
            SlotLiveness::Dead,
            "reported, not removed, by a read"
        );
        let mut b = sleeper();
        let got = acquire(&d, &limits, &quick(b.id(), "next", 0)).unwrap();
        assert_eq!(got.verdict, SlotVerdict::Admitted);
        assert_eq!(got.reclaimed.len(), 1);
        assert_eq!(got.reclaimed[0].pid, pid);
        assert_eq!(got.status.held, 1);
        let _ = b.kill();
        let _ = b.wait();
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_live_pid_with_another_start_time_is_a_reused_pid_and_is_reclaimed() {
        let d = dir("reused");
        let limits = SlotLimits::declared(Some(1), None, None);
        let slots = Slots::open(&d).unwrap();
        // this test's own pid runs, but not since the time this record says
        let me = std::process::id();
        let forged = Record {
            pid: me,
            started: Some("1.000000".into()),
            purpose: "an earlier process".into(),
            since: 1,
            identity: String::new(),
        };
        slots.write("holders", &forged).unwrap();
        if process_start(me).is_some() {
            assert_eq!(liveness(&forged), SlotLiveness::Dead);
            let mut b = sleeper();
            let got = acquire(&d, &limits, &quick(b.id(), "after reuse", 0)).unwrap();
            assert_eq!(got.verdict, SlotVerdict::Admitted);
            assert_eq!(got.reclaimed[0].purpose, "an earlier process");
            let _ = b.kill();
            let _ = b.wait();
        }
        // a record whose start cannot be compared is kept: unverified is alive
        let unknown = Record {
            started: None,
            ..forged
        };
        assert_eq!(liveness(&unknown), SlotLiveness::Unverified);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_disk_floor_refuses_at_once_naming_the_measurement() {
        let d = dir("floor");
        // a floor no disk meets
        let limits = SlotLimits::declared(Some(5), Some(u64::MAX / 4096), None);
        let t = Instant::now();
        let got = acquire(&d, &limits, &quick(std::process::id(), "too big", 10_000)).unwrap();
        assert!(
            t.elapsed() < Duration::from_secs(5),
            "a disk refusal does not wait"
        );
        assert_eq!(got.verdict, SlotVerdict::Refused);
        let r = got.refusal.unwrap();
        assert_eq!(r.reason, SlotRefusalReason::DiskFloor);
        assert!(r.detail.contains("MiB) free"), "{}", r.detail);
        assert_eq!(got.status.held, 0, "nothing was taken");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_waiter_is_listed_with_what_it_waits_on_and_is_admitted_in_turn() {
        let d = dir("queue");
        let limits = SlotLimits::declared(Some(1), None, None);
        let (mut a, mut b) = (sleeper(), sleeper());
        acquire(&d, &limits, &quick(a.id(), "first", 0)).unwrap();
        let (dd, l2, bp) = (d.clone(), limits.clone(), b.id());
        let waiting =
            std::thread::spawn(move || acquire(&dd, &l2, &quick(bp, "second", 10_000)).unwrap());
        // the waiter appears in the status a progress report reads
        let mut seen = None;
        for _ in 0..100 {
            let s = status(&d, &limits, None).unwrap();
            if let Some(w) = s.waiters.first() {
                seen = Some(w.clone());
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let w = seen.expect("the waiter is listed");
        assert_eq!((w.pid, w.purpose.as_str()), (b.id(), "second"));
        assert_eq!(w.waiting_on, "machine.slot: 1 of 1 held");
        assert!(release(&d, a.id()).unwrap().released);
        let got = waiting.join().unwrap();
        assert_eq!(got.verdict, SlotVerdict::Admitted);
        assert!(got.status.waiters.is_empty());
        for p in [&mut a, &mut b] {
            let _ = p.kill();
            let _ = p.wait();
        }
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn undeclared_limits_admit_and_say_so() {
        let d = dir("undeclared");
        let limits = SlotLimits::declared(None, None, None);
        let got = acquire(&d, &limits, &quick(std::process::id(), "anything", 0)).unwrap();
        assert_eq!(got.verdict, SlotVerdict::Admitted);
        assert!(
            got.status.summary.contains("no limit declared"),
            "{}",
            got.status.summary
        );
        assert!(
            got.status.summary.contains("no floor declared"),
            "{}",
            got.status.summary
        );
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_same_process_has_the_same_start_and_a_gone_one_has_none() {
        let me = process_start(std::process::id());
        if cfg!(any(target_os = "macos", target_os = "linux")) {
            assert!(me.is_some());
            assert_eq!(me, process_start(std::process::id()));
        }
        let mut a = sleeper();
        let pid = a.id();
        a.kill().unwrap();
        a.wait().unwrap();
        assert_eq!(process_start(pid), None);
    }

    #[test]
    fn a_pid_that_does_not_run_cannot_hold_a_slot() {
        let d = dir("nopid");
        let mut a = sleeper();
        let pid = a.id();
        a.kill().unwrap();
        a.wait().unwrap();
        let err = acquire(
            &d,
            &SlotLimits::declared(Some(1), None, None),
            &quick(pid, "x", 0),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        let _ = fs::remove_dir_all(&d);
    }
}
