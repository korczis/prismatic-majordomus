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
//! One kind of unstaged edit is seen anyway: the ones this process makes. A capability whose
//! effect is a repository mutation — `plan.transition` stamping an issue — writes a tracked
//! file and moves no git control file, and a Cockpit page read right after the move showed
//! the status from before it. The executor counts every such call that succeeds, whichever
//! transport or execution made it, and outlives a reload; a generation records the count it
//! was built after, and a count that has moved is a repository that has moved.
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
//! concurrently — a second request arriving mid-rebuild waits for it and is answered from
//! the generation it produced, because it has seen the move too and the previous
//! generation is the stale answer it was asking about. Everything a generation carries is a projection of the
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
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

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

/// How long a stamp whose load failed is answered from the previous generation before the
/// load is tried again. A failure is not retried on every request — a layer that will not
/// load costs a whole load each time — and it is not given up on either: the change it
/// failed on is still the repository's, so it is read once the interval has passed, or at
/// once when git moves again.
pub const RETRY_AFTER: Duration = Duration::from_secs(5);

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
    /// The executor's write count this generation was built after. A write to a tracked
    /// file moves no git control file; a write this process made moves this.
    writes: u64,
}

impl Generation {
    /// Is this generation still the repository as it is: git has not moved it, and this
    /// process has not written it since?
    fn current(&self, taken: &str) -> bool {
        self.stamp == taken && self.writes == self.ctx.executor.writes()
    }
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
    /// Held by the one thread rebuilding. A request that finds it taken waits: it has
    /// seen the repository move, so the generation that exists is not an answer to it.
    rebuilding: Mutex<()>,
}

struct Watch {
    args: RepoArgs,
    stamp: Stamp,
    /// The clock a failed load's retry interval is measured on: the process's own, or a
    /// test's, so that the interval is asserted rather than slept through.
    clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    /// The stamp and the write count the last load failed at, and when. The generation keeps
    /// the stamp and the count it was built at, so the change stays unconsumed and is read
    /// again.
    failed: Mutex<Option<(String, u64, Instant)>>,
    /// How many loads were attempted: what a test counts to hold a failing load to one
    /// attempt per interval.
    loads: AtomicU64,
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
        let writes = ctx.executor.writes();
        Live {
            watch: None,
            state: RwLock::new(Generation {
                ctx,
                stamp: String::new(),
                number: 0,
                writes,
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
        Live::watching_on(args, ctx, Arc::new(Instant::now))
    }

    /// [`Live::watching`], measuring the retry interval of a failed load on `clock`.
    fn watching_on(
        args: RepoArgs,
        ctx: Arc<Context>,
        clock: Arc<dyn Fn() -> Instant + Send + Sync>,
    ) -> Live {
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
        let writes = ctx.executor.writes();
        Live {
            watch: Some(Watch {
                args,
                stamp,
                clock,
                failed: Mutex::new(None),
                loads: AtomicU64::new(0),
            }),
            state: RwLock::new(Generation {
                ctx,
                stamp: taken,
                number: 0,
                writes,
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
            if state.current(&taken) || watch.waiting(&taken, state.ctx.executor.writes()) {
                return View {
                    generation: state.number,
                    ctx: Arc::clone(&state.ctx),
                };
            }
        }
        self.reload(watch, taken)
    }

    fn reload(&self, watch: &Watch, taken: String) -> View {
        // A caller that arrives mid-rebuild has already seen the stamp move: the generation
        // that exists is known to be stale, and answering from it is the frozen picture
        // again for as long as a load takes. The server's own tick takes a view twice a
        // second, so it is usually the one rebuilding, and a session that commits and asks
        // at once was handed the commit before its own. It waits instead; the check below
        // then finds the new generation, or rebuilds when the rebuilder's stamp was older
        // than this caller's.
        let _one_at_a_time = self
            .rebuilding
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = {
            let state = read(&self.state);
            // Asked again, now that the rebuild this caller waited for is over. It finds
            // the new generation when that rebuild read the stamp this caller saw — or it
            // finds that the rebuild failed at that very stamp, and then it is answered
            // from the previous generation like every caller inside the retry interval. A
            // waiter that loaded here instead would try the failing load once per waiter:
            // every request that queued behind one failure would repeat it, which is the
            // cost the interval exists to bound.
            if state.current(&taken) || watch.waiting(&taken, state.ctx.executor.writes()) {
                return View {
                    generation: state.number,
                    ctx: Arc::clone(&state.ctx),
                };
            }
            Arc::clone(&state.ctx)
        };
        // read before the load: a write that lands while the layer is being read is one the
        // new generation may not carry, and it must leave the next reading stale
        let writes = previous.executor.writes();
        watch.loads.fetch_add(1, Ordering::SeqCst);
        let built = crate::app::App::load(&watch.args);
        let mut state = write(&self.state);
        match built {
            Ok(app) => {
                *lock(&watch.failed) = None;
                state.writes = writes;
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
                // error. The generation keeps the stamp it was built at, so the change is
                // not consumed by a load that failed to read it; the failure is recorded
                // instead, and holds the retry off for RETRY_AFTER — never once per request,
                // never given up on (Watch::waiting).
                *lock(&watch.failed) = Some((taken, writes, (watch.clock)()));
                tracing::warn!(
                    error = %e,
                    "the repository moved but the layer would not load; serving generation {} and reading it again in {}s, or at once when it moves again",
                    state.number,
                    RETRY_AFTER.as_secs()
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

impl Watch {
    /// Whether `taken` and `writes` are the stamp and the write count the last load failed
    /// at, inside the retry interval: the previous generation answers, and nothing is loaded.
    /// A stamp or a count that moved since, or an interval that has passed, is read again.
    fn waiting(&self, taken: &str, writes: u64) -> bool {
        lock(&self.failed)
            .as_ref()
            .is_some_and(|(stamp, failed_at, at)| {
                stamp == taken && *failed_at == writes && (self.clock)() < *at + RETRY_AFTER
            })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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

    /// A synthetic layer in a git work tree, loaded once, with the arguments that reload it
    /// and a clock the test moves by hand.
    struct Layer {
        repo: crate::synthetic::SyntheticRepository,
        live: Live,
        now: Arc<Mutex<Instant>>,
    }

    impl Layer {
        fn new() -> Layer {
            let repo = crate::synthetic::SyntheticRepository::small().expect("a synthetic layer");
            let layer = |args: &[&str]| {
                Command::new("git")
                    .arg("-C")
                    .arg(repo.root())
                    .args(args)
                    .output()
                    .expect("git")
            };
            layer(&["init", "-q"]);
            layer(&["config", "user.email", "t@example.com"]);
            layer(&["config", "user.name", "t"]);
            layer(&["add", "-A"]);
            layer(&["commit", "-qm", "layer"]);
            let args = RepoArgs {
                repo: Some(repo.root().to_path_buf()),
                share: Some(crate::synthetic::crate_share()),
                discovery: crate::cli::DiscoveryMode::Filesystem,
                ..Default::default()
            };
            let ctx = crate::app::App::load(&args)
                .expect("the layer loads")
                .context;
            let now = Arc::new(Mutex::new(Instant::now()));
            let clock = {
                let now = Arc::clone(&now);
                Arc::new(move || *lock(&now))
            };
            Layer {
                live: Live::watching_on(args, ctx, clock),
                repo,
                now,
            }
        }

        fn git(&self, args: &[&str]) {
            let out = Command::new("git")
                .arg("-C")
                .arg(self.repo.root())
                .args(args)
                .output()
                .expect("git");
            assert!(out.status.success(), "git {args:?}: {out:?}");
        }

        fn manifest(&self) -> PathBuf {
            self.repo.root().join(".ai/manifest.yaml")
        }

        /// The manifest gains a key nothing declares, so the layer will not load, and the
        /// change is committed: git moves.
        fn break_and_commit(&self) -> String {
            let manifest = std::fs::read_to_string(self.manifest()).expect("a manifest");
            std::fs::write(
                self.manifest(),
                format!("{manifest}a_key_nothing_declares: 1\n"),
            )
            .expect("the manifest is written");
            self.git(&["commit", "-qam", "a manifest that will not load"]);
            manifest
        }

        fn loads(&self) -> u64 {
            self.live
                .watch
                .as_ref()
                .expect("a watching view")
                .loads
                .load(Ordering::SeqCst)
        }

        fn advance(&self, by: Duration) {
            let mut now = lock(&self.now);
            *now += by;
        }
    }

    #[test]
    fn a_failed_load_consumes_no_change_and_is_read_again_after_the_interval() {
        let l = Layer::new();
        let good = l.break_and_commit();
        assert_eq!(l.live.view().generation, 0, "the layer will not load");
        assert_eq!(l.loads(), 1);
        // the layer is mended in the work tree alone: no git control file moves, so only the
        // retry can find it
        std::fs::write(l.manifest(), good).expect("the manifest is written");
        assert_eq!(
            l.live.view().generation,
            0,
            "inside the interval, nothing is loaded"
        );
        assert_eq!(
            l.loads(),
            1,
            "a failed load is not retried on every request"
        );
        l.advance(RETRY_AFTER);
        assert_eq!(
            l.live.view().generation,
            1,
            "the change the failed load did not read is read once the interval has passed"
        );
        assert_eq!(l.loads(), 2);
        assert_eq!(l.live.view().generation, 1, "and is then current");
        assert_eq!(l.loads(), 2);
    }

    #[test]
    fn a_failure_is_read_again_at_once_when_the_repository_moves_again() {
        let l = Layer::new();
        let good = l.break_and_commit();
        assert_eq!(l.live.view().generation, 0);
        std::fs::write(l.manifest(), good).expect("the manifest is written");
        l.git(&["commit", "-qam", "the manifest mended"]);
        assert_eq!(
            l.live.view().generation,
            1,
            "a new move is read at once, whatever the interval"
        );
        assert_eq!(l.loads(), 2);
    }

    #[test]
    fn a_write_whose_reload_fails_is_read_again_after_the_interval() {
        let l = Layer::new();
        // the layer is broken in the work tree and this process writes: no git control file
        // moves, so only the write count says the repository moved
        let good = std::fs::read_to_string(l.manifest()).expect("a manifest");
        std::fs::write(l.manifest(), format!("{good}a_key_nothing_declares: 1\n"))
            .expect("the manifest is written");
        l.live.current().executor.count_write();
        assert_eq!(l.live.view().generation, 0, "the layer will not load");
        assert_eq!(l.loads(), 1);
        std::fs::write(l.manifest(), &good).expect("the manifest is written");
        assert_eq!(
            l.live.view().generation,
            0,
            "inside the interval, nothing is loaded"
        );
        assert_eq!(l.loads(), 1);
        l.advance(RETRY_AFTER);
        assert_eq!(
            l.live.view().generation,
            1,
            "the write the failed load did not read is read once the interval has passed"
        );
        assert_eq!(l.loads(), 2);
    }

    #[test]
    fn a_new_write_after_a_failed_load_is_read_at_once() {
        let l = Layer::new();
        let good = l.break_and_commit();
        assert_eq!(l.live.view().generation, 0);
        std::fs::write(l.manifest(), good).expect("the manifest is written");
        l.live.current().executor.count_write();
        assert_eq!(
            l.live.view().generation,
            1,
            "a write is a move, read at once whatever the interval"
        );
        assert_eq!(l.loads(), 2);
    }

    #[test]
    fn a_load_that_keeps_failing_is_tried_once_per_interval() {
        let l = Layer::new();
        l.break_and_commit();
        for _ in 0..5 {
            assert_eq!(l.live.view().generation, 0);
        }
        assert_eq!(l.loads(), 1);
        l.advance(RETRY_AFTER - Duration::from_millis(1));
        assert_eq!(l.live.view().generation, 0);
        assert_eq!(l.loads(), 1, "not a moment before the interval");
        l.advance(Duration::from_millis(1));
        for _ in 0..3 {
            assert_eq!(l.live.view().generation, 0);
        }
        assert_eq!(l.loads(), 2, "once when it has passed, and not again");
    }

    /// A caller that queued behind a rebuild which then failed is answered from the
    /// previous generation and loads nothing: the failure it waited for is the failure at
    /// the stamp it saw, and that is retried once per interval, not once per waiter.
    #[test]
    fn a_caller_that_waited_for_a_rebuild_that_failed_does_not_repeat_it() {
        let l = Layer::new();
        l.break_and_commit();
        let watch = l.live.watch.as_ref().expect("a watching view");
        // the rebuild somebody else is running
        let rebuilding = l.live.rebuilding.lock().expect("the rebuild");
        let (said, heard) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let _ = said.send(l.live.view().generation);
            });
            assert!(
                heard
                    .recv_timeout(std::time::Duration::from_millis(300))
                    .is_err(),
                "a caller that saw the move was answered while the rebuild was still running"
            );
            // that rebuild ends as a failed load does: the stamp and the write count it
            // failed at are recorded, and the generation is left where it was
            *lock(&watch.failed) = Some((
                watch.stamp.take(),
                read(&l.live.state).ctx.executor.writes(),
                (watch.clock)(),
            ));
            drop(rebuilding);
            let generation = heard
                .recv_timeout(std::time::Duration::from_secs(60))
                .expect("the caller is answered once the rebuild is over");
            assert_eq!(generation, 0, "from the generation that loaded");
        });
        assert_eq!(
            l.loads(),
            0,
            "the waiter repeated the load it had just waited to see fail"
        );
        // and the change is still read: once the interval has passed, by whoever asks
        l.advance(RETRY_AFTER);
        assert_eq!(l.live.view().generation, 0, "the layer still will not load");
        assert_eq!(l.loads(), 1, "tried once when the interval has passed");
    }

    /// A synthetic layer in a git work tree with one commit, and a `Live` watching it.
    fn followed() -> (crate::synthetic::SyntheticRepository, Live) {
        let fixture = crate::synthetic::SyntheticRepository::small().expect("a layer");
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(fixture.root())
                .args(args)
                .output()
                .expect("git")
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        git(&["add", "-A"]);
        git(&["commit", "-qm", "one"]);
        let args = RepoArgs {
            repo: Some(fixture.root().to_path_buf()),
            discovery: crate::cli::DiscoveryMode::Filesystem,
            share: Some(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")),
            ..Default::default()
        };
        let app = crate::app::App::load(&args).expect("the layer loads");
        let live = Live::watching(args, Arc::clone(&app.context));
        (fixture, live)
    }

    fn head_in(root: &Path) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn a_request_that_arrives_during_a_rebuild_waits_for_it() {
        let (fixture, live) = followed();
        let before = head_in(fixture.root());
        assert_eq!(head_of(&live.view().ctx), before, "it starts on HEAD");

        // the rebuild somebody else is running: the server's tick, in the process this
        // was found in
        let rebuilding = live.rebuilding.lock().expect("the rebuild");
        Command::new("git")
            .arg("-C")
            .arg(fixture.root())
            .args(["commit", "-q", "--allow-empty", "-m", "two"])
            .output()
            .expect("git");
        let after = head_in(fixture.root());
        assert_ne!(before, after, "the commit moved HEAD");

        let (said, heard) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                let _ = said.send(head_of(&live.view().ctx));
            });
            assert!(
                heard
                    .recv_timeout(std::time::Duration::from_millis(300))
                    .is_err(),
                "a caller that saw the move was answered while the rebuild was still running"
            );
            drop(rebuilding);
            let answered = heard
                .recv_timeout(std::time::Duration::from_secs(60))
                .expect("the caller is answered once the rebuild is over");
            assert_eq!(answered, after, "and with the commit it asked about");
        });
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
