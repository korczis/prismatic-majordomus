//! The branch-to-worktree topology: where every linked worktree of a repository belongs,
//! derived from git and from nothing else, and everything that follows from it.
//!
//! # The invariant
//!
//! ```text
//! <parent>/<repo>                       the primary checkout, which hosts the trunk
//! <parent>/<repo>-wt/                   the container, derived: parent + name + "-wt"
//! <parent>/<repo>-wt/<branch name>      the one worktree of every non-trunk branch
//! ```
//!
//! The branch name *is* the relative path: `feature/providers/openai-streaming` lives at
//! `<repo>-wt/feature/providers/openai-streaming`, hierarchy preserved, nothing flattened,
//! nothing hashed, nothing registered. Git is the registry: the identity comes from the
//! common git directory, the worktrees from `git worktree list --porcelain`, the branches
//! from `for-each-ref`, and this module interprets them into one typed
//! [`RepositoryTopology`] that every surface renders.
//!
//! # The architecture
//!
//! ```text
//!   git (common dir, worktree list, for-each-ref, status)
//!         ↓
//!   RepositoryIdentity            primary, current, container, trunk — from anywhere inside
//!         ↓
//!   path::expected_path()         container / branch name, proved to stay inside the container
//!         ↓
//!   WorktreeService               topology, status, guard, inspect, create, migrate, repair
//!      ↙      ↓       ↘
//!    CLI   capability   git hooks → MCP, HTTP, OpenAPI, Swagger, the Cockpit, docs
//! ```
//!
//! Every surface is an adapter. Nothing outside this module derives a path, decides a
//! standing, or judges whether a move is safe; a second implementation of any of those is
//! the defect this module exists to prevent.
//!
//! # Safety
//!
//! Nothing here deletes a directory because its name looks right, and nothing here resets,
//! stashes, cleans or checks out anything. A migration moves a worktree with `git worktree
//! move`, dirty state included, and proves with a [`fingerprint::WorktreeFingerprint`]
//! taken before and after that the branch, the commit, the index, the staged and unstaged
//! changes and the untracked files are the same on the other side. Every mutation holds one
//! repository-scoped [`lock::WorktreeLock`], so two agents cannot half-register one path.

pub(crate) mod error;
pub mod fingerprint;
pub(crate) mod git;
pub(crate) mod identity;
pub(crate) mod lock;
pub(crate) mod migrate;
pub(crate) mod model;
pub(crate) mod path;
pub(crate) mod service;
pub mod state;
pub(crate) mod topology;
pub(crate) mod trace;

pub use error::{Result, WorktreeError, EXIT_INTERNAL, EXIT_MISSING, EXIT_REFUSED};
pub use fingerprint::WorktreeFingerprint;
pub use identity::{RepositoryIdentity, ResolvedPath, Trunk, TrunkSource};
pub use lock::WorktreeLock;
pub use migrate::{MigrationAction, MigrationOptions, MigrationPlan, MigrationStep, StepOutcome};
pub use model::{
    BranchState, ContainerView, DiagnosticCode, DirtyState, GuardVerdict, InspectReport,
    RepairReport, RepositoryTopology, RepositoryView, Severity, Standing, StatusReport,
    TopologyDiagnostic, TopologyTallies, TrunkView, UpstreamState, WorktreeKind, WorktreeState,
    SCHEMA,
};
pub use path::{container_root, detached_label, expected_path, BranchName, CONTAINER_SUFFIX};
pub use service::{CreateReport, CreateRequest, Detail, RemoveReport, WorktreeService};
pub use topology::{parse_porcelain, parse_porcelain_nul, WorktreeRecord};
pub use trace::{
    Attribution, BranchTrace, CommitAttribution, CommitRef, Integration, IssueTrace, TraceReport,
    TraceTallies, Tracer,
};
