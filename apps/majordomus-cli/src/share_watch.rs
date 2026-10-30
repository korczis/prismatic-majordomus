//! A running server sees its share change: a fingerprint of the share directory, taken on a
//! request's own thread, and the standing that follows from it (ADR 0163, point 4).
//!
//! A server reads its share directory for its whole life, and the directory moves under it:
//! a checkout is pulled or switched to an old branch, an installation is replaced, `ui/` is
//! removed by a clean. On 2026-10-14 the shared server of this repository served the stale
//! primary checkout's 0.21.0 share for hours and every `/ui/assets/` file answered 404; the
//! pages were unstyled and nothing on them said why. A share measured once, at the start,
//! says nothing about the hour after it.
//!
//! So the server keeps one reading of its share, and a request refreshes it before it is
//! answered — beside the tree stamp [`crate::live`] takes the same way, and for the same
//! reason: `project.the-server-sees-the-current-tree` forbids a long-lived process a thread
//! or an interval of its own. The work is bounded twice. The [`Fingerprint`] is three
//! `stat`s — `version.txt`'s size and modification time, and whether each surface's
//! directory (`ui/`, `cockpit/`) is there — and it is taken at most once per [`INTERVAL`],
//! however many requests arrive. Only a fingerprint that moved costs more: the share is
//! measured again ([`ServedShare::inspect`], which reads `version.txt`), a change of standing
//! is logged once, and the [`generation`] moves, so that each surface's asset directory is
//! decided again on its next read ([`crate::cockpit::assets::Assets`]). A share repaired
//! under a running server is served without a restart; a share that goes bad under it is
//! reported on the next request — in the log, and on every page of the UI and the Cockpit
//! ([`skew`]).
//!
//! The reading never replaces anything. A server that ended itself because its share moved
//! would take its attached sessions with it; the decision to replace it belongs to whoever
//! asks for a server — `serve ensure`, which the session-start hook runs — and that reads
//! the same standing from the share the server's lease names.
//!
//! ```
//! use std::time::{Duration, Instant};
//! use majordomus_cli::share::ShareStanding;
//! use majordomus_cli::share_watch::ShareWatch;
//! let dir = tempfile::tempdir().unwrap();
//! std::fs::write(dir.path().join("version.txt"), "version=1.0.0\n").unwrap();
//! std::fs::create_dir(dir.path().join("ui")).unwrap();
//! std::fs::create_dir(dir.path().join("cockpit")).unwrap();
//! let start = Instant::now();
//! let watch = ShareWatch::new(dir.path(), Some("installation"), "1.0.0", start);
//! assert_eq!(watch.observe_at(start).standing, ShareStanding::Current);
//!
//! // the directory moves under the running server: its ui/ is removed
//! std::fs::remove_dir(dir.path().join("ui")).unwrap();
//! assert_eq!(watch.observe_at(start).standing, ShareStanding::Current, "inside the interval");
//! let later = start + Duration::from_secs(5);
//! let reading = watch.observe_at(later);
//! assert_eq!(reading.standing, ShareStanding::Skewed);
//! assert!(reading.reason.unwrap().contains("holds no ui/"));
//! ```

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use crate::share::{ServedShare, ShareStanding, VERSION_FILE};

/// The longest a server goes without fingerprinting its share again while it is asked
/// things: a request arriving sooner than this after the last fingerprint is answered from
/// the reading there is. One second keeps the cost at three `stat`s a second under any load,
/// and a share that moved is seen by the request a person makes after looking at it.
pub const INTERVAL: Duration = Duration::from_secs(1);

/// The surface directories a server serves pages from, whose presence is part of the
/// fingerprint: `ui/` for `/ui` and `cockpit/` for `/cockpit`.
const SURFACES: [&str; 2] = [crate::ui::ASSETS_DIR, crate::cockpit::assets::DIR];

/// What a share directory looks like from outside, in three `stat`s: the size and
/// modification time of its `version.txt`, and whether each surface's directory is there.
/// Two equal fingerprints are a share that has not moved; a different one is measured again.
///
/// ```
/// use majordomus_cli::share_watch::Fingerprint;
/// let dir = tempfile::tempdir().unwrap();
/// let bare = Fingerprint::of(dir.path());
/// std::fs::create_dir(dir.path().join("cockpit")).unwrap();
/// assert_ne!(Fingerprint::of(dir.path()), bare, "a surface's directory appeared");
/// assert_eq!(Fingerprint::of(dir.path()), Fingerprint::of(dir.path()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    version_file: Option<(u64, Option<SystemTime>)>,
    surfaces: [bool; SURFACES.len()],
}

impl Fingerprint {
    /// Take the fingerprint of the share directory at `path`.
    pub fn of(path: &Path) -> Self {
        let version_file = std::fs::metadata(path.join(VERSION_FILE))
            .ok()
            .map(|m| (m.len(), m.modified().ok()));
        Fingerprint {
            version_file,
            surfaces: SURFACES.map(|dir| path.join(dir).is_dir()),
        }
    }
}

/// What a watch remembers between two requests.
#[derive(Debug)]
struct Reading {
    /// When the fingerprint was last taken.
    taken: Instant,
    fingerprint: Fingerprint,
    share: ServedShare,
}

/// The share a server serves, fingerprinted again when a request asks and the interval has
/// passed, with its last measurement kept.
#[derive(Debug)]
pub struct ShareWatch {
    path: PathBuf,
    origin: Option<String>,
    version: String,
    interval: Duration,
    reading: Mutex<Reading>,
    generation: AtomicU64,
}

impl ShareWatch {
    /// A watch over the share at `path`, found by `origin`, served by an executable of
    /// `version`: measured once, at `now`, and fingerprinted again at most once per
    /// [`INTERVAL`] after that.
    pub fn new(path: &Path, origin: Option<&str>, version: &str, now: Instant) -> Self {
        ShareWatch {
            path: path.to_path_buf(),
            origin: origin.map(str::to_string),
            version: version.to_string(),
            interval: INTERVAL,
            reading: Mutex::new(Reading {
                taken: now,
                fingerprint: Fingerprint::of(path),
                share: ServedShare::inspect(path, origin, version),
            }),
            generation: AtomicU64::new(0),
        }
    }

    /// The share as the last measurement found it; nothing is taken from the disk.
    pub fn last(&self) -> ServedShare {
        self.lock().share.clone()
    }

    /// How many times the share has been found changed since the watch began: what a
    /// surface compares to decide its asset directory again.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// The share as it stands at `now`, read on the caller's thread: the last reading inside
    /// the interval, else a fresh fingerprint, and a fresh measurement only when that moved.
    /// A change of standing is logged once — a warning naming the reason when the share has
    /// become skewed, information when it is the server's own again.
    pub fn observe_at(&self, now: Instant) -> ServedShare {
        let mut reading = self.lock();
        if now.saturating_duration_since(reading.taken) < self.interval {
            return reading.share.clone();
        }
        reading.taken = now;
        let fingerprint = Fingerprint::of(&self.path);
        if fingerprint == reading.fingerprint {
            return reading.share.clone();
        }
        let share = ServedShare::inspect(&self.path, self.origin.as_deref(), &self.version);
        if share.standing != reading.share.standing {
            match share.standing {
                ShareStanding::Skewed => tracing::warn!(
                    share = %self.path.display(),
                    "the share this server serves has become skewed: {}",
                    share.reason.as_deref().unwrap_or("no reason given")
                ),
                ShareStanding::Current => tracing::info!(
                    share = %self.path.display(),
                    "the share this server serves is its own again"
                ),
            }
        }
        reading.fingerprint = fingerprint;
        reading.share = share.clone();
        self.generation.fetch_add(1, Ordering::SeqCst);
        share
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Reading> {
        self.reading.lock().unwrap_or_else(|p| p.into_inner())
    }
}

static WATCH: OnceLock<ShareWatch> = OnceLock::new();

/// Watch the share this process serves, from now on. The first call decides, as a process
/// serves one share for its whole life; a later call changes nothing. The share is measured
/// here once, and a skewed one is logged, where a person reading why a page is blank looks
/// first.
pub fn install(path: &Path, origin: Option<&str>, version: &str) {
    let watch = WATCH.get_or_init(|| ShareWatch::new(path, origin, version, Instant::now()));
    if let Some(reason) = watch.last().reason {
        tracing::warn!(share = %path.display(), "{reason}");
    }
}

/// The share this process serves, as it stands now: what [`crate::http::Router::handle`]
/// asks before it answers a request. `None` in a process that watches none — one that is
/// not a server, or a server started without a share directory.
pub fn observe() -> Option<ServedShare> {
    WATCH.get().map(|w| w.observe_at(Instant::now()))
}

/// How many times this process's share has been found changed; zero when it watches none.
pub fn generation() -> u64 {
    WATCH.get().map_or(0, ShareWatch::generation)
}

/// Why this process's share is skewed, when its last reading found it so: what the UI and
/// the Cockpit draw over every page.
pub fn skew() -> Option<String> {
    WATCH
        .get()
        .map(ShareWatch::last)
        .filter(|s| s.standing == ShareStanding::Skewed)
        .map(|s| s.reason.unwrap_or_else(|| "the share is skewed".into()))
}

/// The line a page draws when the share its server serves is skewed: the reason, and what
/// ends it.
///
/// ```
/// let line = majordomus_cli::share_watch::banner("the share at /s states version 0.21.0");
/// assert!(line.contains("0.21.0") && line.contains("majordomus serve ensure"));
/// ```
pub fn banner(reason: &str) -> String {
    format!(
        "This server serves a share that is not its own, so its pages may be unstyled or blank: {reason}. `majordomus serve ensure` replaces it with a server over its own share."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own_share(dir: &Path, version: &str) {
        std::fs::write(dir.join(VERSION_FILE), format!("version={version}\n")).unwrap();
        for surface in SURFACES {
            std::fs::create_dir_all(dir.join(surface)).unwrap();
        }
    }

    /// A request after the interval flips the standing the moment the directory stops being
    /// the server's own, moves the generation once per change, and flips it back when the
    /// share is repaired; a request inside the interval reads nothing from the disk.
    #[test]
    fn the_request_time_check_flips_the_standing() {
        let dir = tempfile::tempdir().unwrap();
        own_share(dir.path(), "2.0.0");
        let t0 = Instant::now();
        let watch = ShareWatch::new(dir.path(), Some("MAJORDOMUS_SHARE"), "2.0.0", t0);
        assert_eq!(watch.observe_at(t0).standing, ShareStanding::Current);
        assert_eq!(watch.generation(), 0);

        std::fs::remove_dir(dir.path().join("cockpit")).unwrap();
        assert_eq!(
            watch.observe_at(t0 + INTERVAL / 2).standing,
            ShareStanding::Current,
            "inside the interval the reading there is answers"
        );
        let t1 = t0 + INTERVAL;
        let skewed = watch.observe_at(t1);
        assert_eq!(skewed.standing, ShareStanding::Skewed);
        assert!(skewed
            .reason
            .as_deref()
            .unwrap()
            .contains("holds no cockpit/"));
        assert_eq!(watch.last(), skewed, "the reading is kept");
        assert_eq!(watch.generation(), 1);

        let t2 = t1 + INTERVAL;
        assert_eq!(
            watch.observe_at(t2),
            skewed,
            "an unmoved share is not measured again"
        );
        assert_eq!(watch.generation(), 1);

        own_share(dir.path(), "2.0.0");
        let repaired = watch.observe_at(t2 + INTERVAL);
        assert_eq!(repaired.standing, ShareStanding::Current);
        assert_eq!(watch.generation(), 2);
    }

    /// A share that states another version once its `version.txt` is rewritten is skewed by
    /// the next request after the interval, with the version it states in the reason.
    #[test]
    fn a_rewritten_version_file_is_seen() {
        let dir = tempfile::tempdir().unwrap();
        own_share(dir.path(), "3.0.0");
        let t0 = Instant::now();
        let watch = ShareWatch::new(dir.path(), None, "3.0.0", t0);
        // a longer line, so that the size moves even where the clock's grain hides the mtime
        std::fs::write(dir.path().join(VERSION_FILE), "# old\nversion=2.9.0\n").unwrap();
        let reading = watch.observe_at(t0 + INTERVAL);
        assert_eq!(reading.standing, ShareStanding::Skewed);
        let why = reading.reason.unwrap();
        assert!(why.contains("states version 2.9.0"), "{why}");
    }

    /// The fingerprint is cheap enough to take on a request's thread: well under the
    /// millisecond that would show in the HTTP p50.
    #[test]
    fn the_fingerprint_is_cheap_enough_to_take_on_a_request() {
        let dir = tempfile::tempdir().unwrap();
        own_share(dir.path(), "1.0.0");
        let rounds = 200;
        let started = Instant::now();
        for _ in 0..rounds {
            let _ = Fingerprint::of(dir.path());
        }
        let each = started.elapsed() / rounds;
        assert!(
            each < Duration::from_millis(2),
            "a fingerprint took {each:?}"
        );
    }

    #[test]
    fn the_banner_names_the_reason_and_the_remedy() {
        let line = banner("the share at /s holds no ui/");
        assert!(line.contains("holds no ui/"), "{line}");
        assert!(line.contains("`majordomus serve ensure`"), "{line}");
    }
}
