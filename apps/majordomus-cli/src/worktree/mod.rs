//! Git worktree management: where a linked worktree belongs, and everything that follows.
//!
//! # The invariant
//!
//! For a repository at `<parent>/<repo>`, every linked worktree lives at
//! `<parent>/<repo>-wt/<name>`. The primary checkout stays where it is. The container is a
//! plain directory beside the checkout, not inside it, so nothing about it is committed,
//! `.gitignore` never has to mention it, and deleting it is a bounded operation.
//!
//! # The architecture
//!
//! ```text
//!   WorktreePolicy          .ai/repo/policy.yaml — strategy and suffix, the one truth
//!         +
//!   RepositoryIdentity      git common dir, primary checkout, current checkout
//!         ↓
//!   canonical_root()        parent(primary)/basename(primary)+suffix
//!         ↓
//!   WorktreeService         list, status, create, remove, migrate, prune — decided once
//!      ↙    ↓    ↘
//!    CLI  doctor  capability registry → MCP, HTTP, OpenAPI, Swagger, cockpit, docs
//! ```
//!
//! Every surface is an adapter. The command line renders; the registry projects; doctor
//! reads the report and turns it into findings. None of them derives a path, applies the
//! policy, or decides whether an operation is safe — [`WorktreeService`] does, once.
//!
//! # Why the identity is not the current directory
//!
//! Run from `~/dev/foo-wt/issue-123/apps/x/src`, `git rev-parse --show-toplevel` answers
//! `~/dev/foo-wt/issue-123`. Deriving a container from *that* gives
//! `~/dev/foo-wt/issue-123-wt`: a new container per worktree, nesting for ever. The primary
//! checkout is what the container is named after, and it is read from the repository's
//! shared metadata — see [`identity`] — so the answer is the same from every directory of
//! every worktree.
//!
//! # Safety
//!
//! Nothing here deletes a directory because its name looks right. Before any destructive
//! step: the path is a work tree git registered *for this repository*, it is not the primary
//! checkout, it is not locked, and it holds no uncommitted or untracked work. Paths are
//! compared canonicalised, so a symlink cannot smuggle a directory into the container. Git
//! is asked to do the moving and removing, and every mutating operation holds one
//! repository-scoped lock — see [`lock`] — so two agents cannot half-register the same
//! worktree.

pub mod error;
pub mod git;
pub mod identity;
pub mod lock;
pub mod naming;
pub mod policy;
pub mod service;
pub mod topology;

pub use error::{Result, WorktreeError, EXIT_INTERNAL, EXIT_MISSING, EXIT_REFUSED};
pub use identity::{RepositoryIdentity, ResolvedPath};
pub use lock::WorktreeLock;
pub use naming::WorktreeName;
pub use policy::{
    CleanupPolicy, Disposition, EnforcementPolicy, Level, NamingPolicy, NamingStrategy, RootPolicy,
    RootStrategy, WorktreePolicy,
};
pub use service::{
    issue_of, CreateReport, CreateRequest, MigrationPlan, MigrationStep, PolicyStatus, PruneReport,
    RemoveReport, RootReport, StatusReport, WorktreeKind, WorktreeReport, WorktreeService,
    WorktreeTallies, WorktreeView, WorktreeViolation, ROOT_MARKER,
};
pub use topology::{parse_porcelain, parse_porcelain_nul, WorktreeRecord};
