//! The lifecycle of work: what state every worktree, branch and pull request is in, why,
//! and whether removing any of it is provably safe.
//!
//! # Why this exists
//!
//! A repository accumulates worktrees, branches and pull requests, and nothing in git says
//! what any of them is *for* any more. The topology module answers "where does this
//! worktree belong"; nothing answered "is anyone still doing this, did it land, and would
//! deleting it lose anything". Without that answer, cleanup is a guess, and a guess that
//! goes wrong is unrecoverable.
//!
//! It has gone wrong here. A sweep committed the dirty state of forty-two worktrees onto
//! their own branches and pushed none of them; eleven of those branches already had a
//! merged pull request. Every naive reading — "the pull request is merged, the branch is
//! disposable" — deletes eleven pieces of work that exist on one disk. That case is the
//! test this module is written against.
//!
//! # The architecture
//!
//! ```text
//!   git  (rev-list --not --remotes=origin, merge-tree, first-parent merges, archive tags)
//!   gh   (optional, bounded, and never believed about mergeability)
//!         ↓
//!   measure::*                one bounded subprocess per fact, nothing written
//!         ↓
//!   LifecycleService          precedence, evidence, blockers, findings
//!      ↙      ↓       ↘
//!    CLI   capability   the prune that refuses by default
//! ```
//!
//! # What is load-bearing
//!
//! **Unique means unique.** [`measure::unique_commits`] counts commits reachable from *no*
//! ref under `refs/remotes/origin/`, not commits on a branch whose name is missing from
//! origin. The two differ in both directions and only the first is safe to act on.
//!
//! **Mergeability is decided here, not asked.** This repository configures a `merge=derived`
//! driver per clone; the forge cannot run it and calls pull requests conflicted that merge
//! cleanly in this checkout. [`measure::conflicts_with`] decides with
//! `git merge-tree --write-tree`; the forge's own field is recorded as an untrusted hint.
//!
//! **Ageing is declared once.** Every threshold is a key of `.ai/repo/policy.yaml` under
//! `lifecycle:`, resolved by [`AgingPolicy::resolve`] and carried in the report, and every
//! age is measured against [`Clock`] — which every test fixes, so no verdict here depends
//! on when it ran.
//!
//! **Nothing is removed on a guess.** [`LifecycleService::cleanup_plan`] refuses by default
//! and puts everything it will not touch in `refused`, each entry carrying the measurement
//! that refused it and the command that clears it.

pub(crate) mod clock;
pub(crate) mod forge;
pub(crate) mod measure;
pub(crate) mod model;
pub(crate) mod policy;
pub(crate) mod service;

pub use clock::Clock;
pub use model::{CleanupPlan, LifecycleReport};
pub use policy::{AgingPolicy, LifecyclePolicy};
pub use service::{LifecycleService, Request};
