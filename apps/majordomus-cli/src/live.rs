//! The repository as a long-lived process must see it: current, not as it was when the
//! process started.
//!
//! # The defect this exists to end
//!
//! [`crate::app::App::load`] builds one [`Index`](crate::index::Index) and one
//! [`Context`] and hands them out as `Arc`s. For a command that answers a question and
//! exits, that is exactly right: the picture cannot go stale inside a process that lives
//! for 40 ms. The shared server lives for hours, and it held the same two values for its
//! whole life. A session would commit, ask `GET /api/v1/repository` for the head it had
//! just written, and be told the head the server had read when it started — the same
//! answer from MCP, from the Cockpit and from every capability that reads the index, with
//! nothing in any of them saying the picture was old. Measured on `origin/master` at
//! `faba8fdef`: head `A` before a commit, head `A` after it, a rule committed while the
//! server ran absent from `objects`, and a working tree reported `clean` while it was
//! dirty. Only restarting the process fixed it.
//!
//! # What is watched, and why it is these files
//!
//! Git announces every move of the repository by writing a small, fixed set of files:
//! `HEAD`, the staging index, `packed-refs`, the reflog, and the in-progress markers a
//! merge or a rebase leaves. A commit, a checkout, a reset, a merge, a rebase and a branch
//! switch each write at least one of them. So the question "has the repository moved?"
//! is [`Stamp`]: the size and modification time of those files, hashed — one `stat` per
//! watched file, taken on the request's own thread before it is answered, which
//! `the_stamp_is_cheap_enough_to_take_on_every_request` holds to well under the
//! millisecond that would make it matter. No thread, no interval, no watcher to leak, and
//! nothing over the network.
//!
//! The limit, named rather than hidden: an edit to a tracked file that is never staged
//! changes no git control file, so this does not see it. Seeing it would mean a `stat` per
//! declared object on every request — a number that grows with the layer, against an HTTP
//! p50 the benchmark baselines record in the hundreds of microseconds — which is the
//! per-request regression the performance doctrine refuses. What a long-lived session
//! actually loses to the frozen picture is the commit it just made, and a commit always
//! moves git. `git add` moves it too: the staging index is watched.
//!
//! # The obligation that comes with watching: this process must not write them
//!
//! `git status` and `git diff` refresh the staging index as a side effect, writing back
//! stat data that has gone stale or is *racily* close to the index's own modification
//! time. So a served page that asked git about the working tree moved the stamp itself,
//! and the *next* request paid a whole [`crate::app::App::load`] for a repository that had
//! not moved — an observer disturbing what it observes, and charging the following caller
//! for it. Measured over the Cockpit page sweep against a committed fixture:
//! `repository_scans` 1 → 2 → 4 across `/cockpit/continuity` and `/cockpit/health`, with
//! `.git/index`'s modification time moving at exactly those two points and its size never
//! changing. Every read this crate makes therefore goes through
//! [`crate::git::read_only`], which is that one rule made into a constructor;
//! `a_page_costs_no_rebuild_of_anything_canonical` holds it, page by page.
//!
//! # What a new generation costs, and who pays it
//!
//! Rebuilding is a whole [`crate::app::App::load`]: discovery, every declared file read
//! and validated, the registry, the Why catalogue, the product model, the web topology.
//! It is paid once per repository move, by the one request that noticed, and never
//! concurrently — a second request arriving mid-rebuild is served the previous generation
//! rather than queued behind it. Everything a generation carries is a projection of the
//! index and is therefore rebuilt with it. What is *not* a projection of the index is
//! carried across: the peer board, the executions this process is running and the
//! capability executor, because a reload is not a restart and an attached peer must not
//! lose its place on the board because somebody committed.
//!
//! ```
//! use std::sync::Arc;
//! use majordomus_cli::live::Live;
//!
//! // A pinned view never asks the filesystem anything: what a one-shot command,
//! // a benchmark and a test want, and what they cost.
//! # fn example(ctx: Arc<majordomus_cli::capability::Context>) {
//! let live = Live::pinned(ctx);
//! assert_eq!(live.generation(), 0, "a pinned view has one generation, forever");
//! let a = live.current();
//! let b = live.current();
//! assert!(Arc::ptr_eq(&a, &b), "and hands out the same context every time");
//! # }
//! ```

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use crate::capability::Context;
use crate::cli::RepoArgs;

/// The git control files whose size and modification time say the repository has moved.
///
/// Relative to the *per-worktree* git directory (`git rev-parse --git-dir`): a linked
/// worktree has its own `HEAD`, its own staging index and its own reflog, and watching the
/// primary checkout's instead would make every worktree's server blind to its own commits.
pub const WORKTREE_FILES: [&str; 6] = [
    "HEAD",
    "index",
    "ORIG_HEAD",
    "MERGE_HEAD",
    "logs/HEAD",
    "rebase-merge",
];

/// The git control files shared by every worktree of one repository
/// (`git rev-parse --git-common-dir`): where a ref that is not this worktree's moves.
///
/// `refs` is the directory itself, which changes when a ref file is created, renamed or
/// removed — the shape a ref update takes, since git writes a lock file and renames it
/// over the target.
pub const COMMON_FILES: [&str; 2] = ["packed-refs", "refs"];

/// The salt the stamp is taken under, so that it can never collide with another
/// fingerprint computed over the same files for another purpose.
pub const SALT: &str = "live/repository-generation";

/// A picture of the git control files: what the repository looked like when a generation
/// was built.
///
/// Two stamps that are equal mean git has not been asked to move anything. They are
/// compared, never interpreted: nothing reads a head or a branch out of one.
///
/// ```
/// use majordomus_cli::live::Stamp;
/// use std::process::Command;
///
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// let git = |args: &[&str]| {
///     Command::new("git").arg("-C").arg(dir.path()).args(args).output().expect("git")
/// };
/// git(&["init", "-q"]);
/// git(&["config", "user.email", "t@example.com"]);
/// git(&["config", "user.name", "t"]);
/// std::fs::write(dir.path().join("a"), "a").expect("a file");
/// git(&["add", "-A"]);
/// git(&["commit", "-qm", "one"]);
///
/// let watch = Stamp::watching(dir.path()).expect("a work tree is watchable");
/// let before = watch.take();
/// assert_eq!(before, watch.take(), "nothing moved, nothing changed");
///
/// std::fs::write(dir.path().join("b"), "b").expect("a second file");
/// git(&["add", "-A"]);
/// git(&["commit", "-qm", "two"]);
/// assert_ne!(before, watch.take(), "a commit moved the repository");
/// ```
#[derive(Debug, Clone)]
pub struct Stamp {
    root: PathBuf,
    paths: Vec<String>,
}

impl Stamp {
    /// The control files of the checkout at `root`, resolved once.
    ///
    /// `git rev-parse` is run exactly here — one subprocess for the life of the process —
    /// because the two directories it names do not move while a checkout exists, and a
    /// subprocess on the request path is the cost this whole module exists to avoid.
    /// `None` when the directory is not a work tree, which is a repository of the layer
    /// that is simply not version controlled: it has no moves to follow.
    ///
    /// ```
    /// use majordomus_cli::live::Stamp;
    ///
    /// // a directory git knows nothing about has nothing to watch, and that is not a failure
    /// let plain = tempfile::tempdir().expect("a temporary directory");
    /// assert!(Stamp::watching(plain.path()).is_none());
    /// ```
    pub fn watching(root: &Path) -> Option<Stamp> {
        let git_dir = resolve(root, "--git-dir")?;
        let common = resolve(root, "--git-common-dir").unwrap_or_else(|| git_dir.clone());
        let mut paths: Vec<String> = Vec::with_capacity(WORKTREE_FILES.len() + COMMON_FILES.len());
        for name in WORKTREE_FILES {
            paths.push(git_dir.join(name).to_string_lossy().into_owned());
        }
        for name in COMMON_FILES {
            paths.push(common.join(name).to_string_lossy().into_owned());
        }
        Some(Stamp {
            root: root.to_path_buf(),
            paths,
        })
    }

    /// The files this stamp is taken over, absolute. The hash sorts them, so this order
    /// is the order they were collected in and carries no meaning of its own.
    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    /// Take the stamp: one `stat` per watched file, hashed.
    ///
    /// A file that is not there contributes its absence, which is itself a move worth
    /// noticing — `MERGE_HEAD` appearing and disappearing is how a merge begins and ends.
    /// The work is [`crate::environment::cache::fingerprint_of`], which is the
    /// fingerprint-over-files this crate already had; a second implementation of it would
    /// be a second thing to keep correct.
    ///
    /// ```
    /// use majordomus_cli::live::Stamp;
    /// use std::process::Command;
    ///
    /// let dir = tempfile::tempdir().expect("a temporary directory");
    /// Command::new("git").arg("-C").arg(dir.path()).args(["init", "-q"]).output().expect("git");
    /// let watch = Stamp::watching(dir.path()).expect("a work tree");
    /// // taking it twice over a repository nobody touched gives one value
    /// assert_eq!(watch.take(), watch.take());
    /// ```
    pub fn take(&self) -> String {
        crate::environment::cache::fingerprint_of(&self.root, &self.paths, &[SALT])
    }
}

fn resolve(root: &Path, flag: &str) -> Option<PathBuf> {
    let out = crate::git::read_only(root)
        .args(["rev-parse", flag])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return None;
    }
    let path = PathBuf::from(text);
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

/// One generation of the repository: the context built from it and the stamp it was built
/// at.
struct Generation {
    ctx: Arc<Context>,
    stamp: String,
    number: u64,
}

/// The repository a process reads, kept current.
///
/// Two forms, and the difference is whether anything is watched at all:
///
/// - [`Live::pinned`] — one context, forever, at no cost. A command that answers and
///   exits, a benchmark, a test: the picture cannot go stale inside them, and making them
///   pay a `stat` per call to prove it would be a regression bought with nothing.
/// - [`Live::watching`] — the shared server's form. Every [`Live::current`] takes a
///   [`Stamp`] first and rebuilds when it differs from the one the generation it holds was
///   built at.
///
/// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::live::Live;
/// # use majordomus_cli::capability::Context;
/// # fn example(ctx: Arc<Context>, args: majordomus_cli::cli::RepoArgs) {
/// // the shared server follows the repository; a one-shot command does not
/// let followed = Live::watching(args, Arc::clone(&ctx));
/// let pinned = Live::pinned(ctx);
/// assert_eq!(pinned.generation(), followed.generation(), "both start at the first");
/// # }
/// ```
pub struct Live {
    watch: Option<Watch>,
    state: RwLock<Generation>,
    /// Held by the one thread rebuilding. Never waited on: a request that finds it taken
    /// is served the generation that exists rather than queued behind a rebuild.
    rebuilding: Mutex<()>,
}

struct Watch {
    args: RepoArgs,
    stamp: Stamp,
}

impl std::fmt::Debug for Live {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = read(&self.state);
        f.debug_struct("Live")
            .field("watching", &self.watch.is_some())
            .field("generation", &state.number)
            .finish()
    }
}

impl Live {
    /// One context that never changes: no stamp is taken and nothing is rebuilt.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::live::Live;
    /// # fn example(ctx: Arc<majordomus_cli::capability::Context>) {
    /// let live = Live::pinned(Arc::clone(&ctx));
    /// assert!(Arc::ptr_eq(&live.current(), &ctx), "the very context it was given");
    /// # }
    /// ```
    pub fn pinned(ctx: Arc<Context>) -> Live {
        Live {
            watch: None,
            state: RwLock::new(Generation {
                ctx,
                stamp: String::new(),
                number: 0,
            }),
            rebuilding: Mutex::new(()),
        }
    }

    /// A context that follows the repository, reloaded with `args` when git moves it.
    ///
    /// `args` is how the process was pointed at the repository in the first place, kept so
    /// that a reload discovers the same root, the same distribution and the same discovery
    /// mode the first load did. Falls back to [`Live::pinned`] when the root is not a work
    /// tree: there is nothing to follow, and pretending otherwise would take a stamp over
    /// a handful of absent files on every request forever.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::live::Live;
    /// # fn example(ctx: Arc<majordomus_cli::capability::Context>, args: majordomus_cli::cli::RepoArgs) {
    /// let live = Live::watching(args, ctx);
    /// // the first reading is the one it was handed; a commit makes the next one the second
    /// assert_eq!(live.view().generation, 0);
    /// # }
    /// ```
    pub fn watching(args: RepoArgs, ctx: Arc<Context>) -> Live {
        let root = PathBuf::from(&ctx.index.repository.root);
        // The root is pinned to the one already discovered rather than left to be
        // rediscovered from `--repo`, which is `None` for a process started in the
        // repository: a server is detached from the terminal that started it, and a
        // reload that began from the current directory would rediscover whatever
        // directory the process happens to be in — a different repository, or none.
        let args = RepoArgs {
            repo: Some(root.clone()),
            ..args
        };
        let Some(stamp) = Stamp::watching(&root) else {
            tracing::info!(
                root = %root.display(),
                "the repository is not a git work tree: this process serves the layer as it reads it now and has no moves to follow"
            );
            return Live::pinned(ctx);
        };
        let taken = stamp.take();
        tracing::info!(
            watched = stamp.paths().len(),
            "following the repository: every request compares {} git control file(s) and reloads the layer when they move",
            stamp.paths().len()
        );
        Live {
            watch: Some(Watch { args, stamp }),
            state: RwLock::new(Generation {
                ctx,
                stamp: taken,
                number: 0,
            }),
            rebuilding: Mutex::new(()),
        }
    }

    /// Which generation is current, without taking a stamp.
    ///
    /// Only for a caller that already holds a [`View`] and wants to say which one it is; a
    /// caller that is about to *use* the context must take [`Live::view`], because reading
    /// the number and then the context is two reads with a reload possible between them —
    /// the bug this whole module exists to end, one layer in. A projection memoised under
    /// the number read before the reload is the previous generation's, handed out as the
    /// current one.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::live::Live;
    /// # fn example(ctx: Arc<majordomus_cli::capability::Context>) {
    /// let live = Live::pinned(ctx);
    /// assert_eq!(live.generation(), live.view().generation);
    /// # }
    /// ```
    pub fn generation(&self) -> u64 {
        read(&self.state).number
    }

    /// The context as it is now: the one that was loaded, or a newly built one when the
    /// repository has moved under this process.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::live::Live;
    /// # fn example(ctx: Arc<majordomus_cli::capability::Context>) {
    /// let live = Live::pinned(ctx);
    /// assert!(Arc::ptr_eq(&live.current(), &live.view().ctx));
    /// # }
    /// ```
    pub fn current(&self) -> Arc<Context> {
        self.view().ctx
    }

    /// The current generation and the context of it, as one reading.
    ///
    /// One call, so that a caller keying a memo on the number and filling it from the
    /// context cannot key the new context under the old number.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::live::{Live, Memo};
    /// # fn example(ctx: Arc<majordomus_cli::capability::Context>) {
    /// let live = Live::pinned(ctx);
    /// let memo: Memo<usize> = Memo::default();
    /// // the shape every projection uses: one reading, keyed and filled from itself
    /// let view = live.view();
    /// let objects = memo.get_or_init(view.generation, || view.ctx.index.objects.len());
    /// assert_eq!(*objects, view.ctx.index.objects.len());
    /// # }
    /// ```
    pub fn view(&self) -> View {
        let Some(watch) = self.watch.as_ref() else {
            let state = read(&self.state);
            return View {
                generation: state.number,
                ctx: Arc::clone(&state.ctx),
            };
        };
        let taken = watch.stamp.take();
        {
            let state = read(&self.state);
            if state.stamp == taken {
                return View {
                    generation: state.number,
                    ctx: Arc::clone(&state.ctx),
                };
            }
        }
        self.reload(watch, taken)
    }

    fn reload(&self, watch: &Watch, taken: String) -> View {
        // A second request arriving mid-rebuild is answered from the generation that
        // exists. Queueing it behind a rebuild would turn one commit into a stall for
        // every client of the server, and the answer it would wait for is one it can have
        // on its next call anyway.
        let Ok(_one_at_a_time) = self.rebuilding.try_lock() else {
            let state = read(&self.state);
            return View {
                generation: state.number,
                ctx: Arc::clone(&state.ctx),
            };
        };
        let previous = {
            let state = read(&self.state);
            if state.stamp == taken {
                return View {
                    generation: state.number,
                    ctx: Arc::clone(&state.ctx),
                };
            }
            Arc::clone(&state.ctx)
        };
        let built = crate::app::App::load(&watch.args);
        let mut state = write(&self.state);
        match built {
            Ok(app) => {
                state.ctx = Arc::new(app.context.continuing(&previous));
                state.number += 1;
                state.stamp = taken;
                tracing::info!(
                    generation = state.number,
                    objects = state.ctx.index.objects.len(),
                    head = %head_of(&state.ctx),
                    "the repository moved; this process reads generation {} of it",
                    state.number
                );
            }
            Err(e) => {
                // A repository caught mid-rebase, a manifest being edited, a checkout half
                // written: the last generation that loaded is a better answer than an
                // error, and the stamp is recorded so the failure is not retried on every
                // request until something changes again.
                state.stamp = taken;
                tracing::warn!(
                    error = %e,
                    "the repository moved but the layer would not load; serving generation {} until it changes again",
                    state.number
                );
            }
        }
        View {
            generation: state.number,
            ctx: Arc::clone(&state.ctx),
        }
    }
}

/// One reading of the repository: which generation, and the context of it.
///
/// The pair is taken together because a memo keyed on the number and filled from the
/// context must not mix two readings: taking the number first, then the context, lets a
/// reload happen between them and files the new context under the old number, where the
/// next request finds the *previous* generation and calls it current. That is the original
/// defect in miniature, and it is why there is no way to ask for the two separately on the
/// path that uses them.
///
/// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::live::Live;
/// # use majordomus_cli::live::View;
/// # fn example(ctx: Arc<majordomus_cli::capability::Context>) {
/// let view: View = Live::pinned(ctx).view();
/// assert_eq!(view.generation, 0);
/// assert!(view.ctx.registry.len() > 0, "the context of that generation, not another");
/// # }
/// ```
#[derive(Clone)]
pub struct View {
    /// Which generation of the repository this is.
    pub generation: u64,
    /// The context built from it.
    pub ctx: Arc<Context>,
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View")
            .field("generation", &self.generation)
            .field("objects", &self.ctx.index.objects.len())
            .finish()
    }
}

/// What a projection is built over: something that can hand out the current context.
///
/// One trait so that the shared server passes the [`Live`] it watches with and every other
/// caller — one-shot commands, benchmarks, the crate's own tests — passes the
/// [`Arc<Context>`] it already has and gets a pinned view without saying so.
///
/// ```
/// # use std::sync::Arc;
/// # use majordomus_cli::live::{IntoLive, Live};
/// # fn example(ctx: Arc<majordomus_cli::capability::Context>, live: Arc<Live>) {
/// // a projection takes either, and neither caller has to say which it holds
/// fn projection(over: impl IntoLive) -> u64 { over.into_live().generation() }
/// assert_eq!(projection(ctx), 0);
/// assert_eq!(projection(live), 0);
/// # }
/// ```
pub trait IntoLive {
    /// The live view this value stands for.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::live::IntoLive;
    /// # fn example(ctx: Arc<majordomus_cli::capability::Context>) {
    /// // a plain context becomes a view that follows nothing and costs nothing
    /// assert_eq!(ctx.into_live().generation(), 0);
    /// # }
    /// ```
    fn into_live(self) -> Arc<Live>;
}

impl IntoLive for Arc<Live> {
    fn into_live(self) -> Arc<Live> {
        self
    }
}

impl IntoLive for Arc<Context> {
    fn into_live(self) -> Arc<Live> {
        Arc::new(Live::pinned(self))
    }
}

/// A value derived from one generation of the repository, kept until that generation ends.
///
/// The OpenAPI document, the MCP tool and resource listings and the resolved web surfaces
/// are all expensive functions of the index and the registry, and all of them used to be
/// `OnceLock`s — right while the index was immutable for the life of a process, and a
/// second frozen picture the moment it was not. A memo answers from the generation it was
/// built for and rebuilds for the next one.
///
/// ```
/// use majordomus_cli::live::Memo;
///
/// let memo: Memo<String> = Memo::default();
/// let a = memo.get_or_init(0, || "built once".to_string());
/// let b = memo.get_or_init(0, || panic!("the same generation must not rebuild"));
/// assert_eq!(*a, *b);
/// let c = memo.get_or_init(1, || "built again".to_string());
/// assert_eq!(*c, "built again", "a new generation is a new value");
/// ```
#[derive(Debug)]
pub struct Memo<T> {
    cell: Mutex<Option<(u64, Arc<T>)>>,
}

impl<T> Default for Memo<T> {
    fn default() -> Self {
        Memo {
            cell: Mutex::new(None),
        }
    }
}

impl<T> Memo<T> {
    /// The value for `generation`, building it once when this memo does not hold it.
    ///
    /// The lock is held across `build`, so two threads that arrive together on a new
    /// generation do the work once rather than twice.
    ///
    /// ```
    /// use majordomus_cli::live::Memo;
    ///
    /// let memo: Memo<Vec<u8>> = Memo::default();
    /// let first = memo.get_or_init(3, || vec![1, 2, 3]);
    /// let again = memo.get_or_init(3, || unreachable!("the same generation is not rebuilt"));
    /// assert!(std::sync::Arc::ptr_eq(&first, &again));
    /// assert_eq!(*memo.get_or_init(4, Vec::new), Vec::<u8>::new(), "a new generation rebuilds");
    /// ```
    pub fn get_or_init(&self, generation: u64, build: impl FnOnce() -> T) -> Arc<T> {
        let mut cell = self.cell.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((held, value)) = cell.as_ref() {
            if *held == generation {
                return Arc::clone(value);
            }
        }
        let value = Arc::new(build());
        *cell = Some((generation, Arc::clone(&value)));
        value
    }
}

/// The head the index was built at, for the log line that announces a generation.
fn head_of(ctx: &Context) -> String {
    match &ctx.index.repository.git {
        crate::git::GitState::Available(info) => info.head.clone().unwrap_or_else(|| "-".into()),
        crate::git::GitState::Unavailable { .. } => "-".into(),
    }
}

fn read<T>(lock: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write<T>(lock: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Instant;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .expect("git")
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(dir.path().join("a"), "a").expect("a file");
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        dir
    }

    #[test]
    fn a_directory_that_is_not_a_work_tree_has_nothing_to_watch() {
        let plain = tempfile::tempdir().expect("a temporary directory");
        assert!(
            Stamp::watching(plain.path()).is_none(),
            "there are no moves to follow where there is no git"
        );
    }

    #[test]
    fn a_commit_moves_the_stamp_and_a_read_does_not() {
        let dir = repo();
        let watch = Stamp::watching(dir.path()).expect("a work tree");
        let before = watch.take();
        // reading the repository is not moving it
        let _ = std::fs::read_to_string(dir.path().join("a"));
        assert_eq!(before, watch.take());
        Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["commit", "-q", "--allow-empty", "-m", "two"])
            .output()
            .expect("git");
        assert_ne!(before, watch.take(), "a commit is a move");
    }

    #[test]
    fn staging_a_file_moves_the_stamp() {
        let dir = repo();
        let watch = Stamp::watching(dir.path()).expect("a work tree");
        let before = watch.take();
        std::fs::write(dir.path().join("b"), "b").expect("a file");
        Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["add", "-A"])
            .output()
            .expect("git");
        assert_ne!(before, watch.take(), "the staging index is watched too");
    }

    #[test]
    fn the_stamp_is_cheap_enough_to_take_on_every_request() {
        // The whole design rests on this: the check runs on the request's own thread,
        // before the request is answered, and the HTTP p50 this repository records is a
        // few hundred microseconds. A stamp that cost a millisecond would be the
        // regression the performance doctrine refuses, and the file set would have to
        // shrink. The bound is generous — this runs on loaded CI machines — and it still
        // fails long before the cost could matter.
        let dir = repo();
        let watch = Stamp::watching(dir.path()).expect("a work tree");
        let _warm = watch.take();
        let rounds = 200;
        let started = Instant::now();
        for _ in 0..rounds {
            let _ = watch.take();
        }
        let each = started.elapsed() / rounds;
        assert!(
            each < std::time::Duration::from_millis(2),
            "a stamp took {each:?}, which is too much to take on every request"
        );
    }

    #[test]
    fn the_paths_are_the_worktree_files_then_the_common_ones() {
        let dir = repo();
        let watch = Stamp::watching(dir.path()).expect("a work tree");
        assert_eq!(
            watch.paths().len(),
            WORKTREE_FILES.len() + COMMON_FILES.len()
        );
        assert!(
            watch.paths().iter().all(|p| p.starts_with('/')),
            "absolute, so that a linked worktree's git directory outside the root is read"
        );
    }

    #[test]
    fn a_memo_of_one_generation_builds_once() {
        let memo: Memo<u64> = Memo::default();
        let mut built = 0u64;
        for _ in 0..5 {
            memo.get_or_init(7, || {
                built += 1;
                7
            });
        }
        assert_eq!(built, 1);
    }
}
