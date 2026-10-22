//! The release: what this project has shipped, what it would ship next, and the changelog
//! that is a projection of both rather than a file somebody remembered to update.
//!
//! # The gap this closes
//!
//! Every other public fact in this repository has one canonical declaration and a set of
//! projections derived from it — a command, a capability, a schema, a page. The release did
//! not. Its version was two hand-written strings that a check compared with each other, and
//! nothing raised them; its changelog did not exist at all, and what changed in a version
//! lived only in GitHub's release notes, outside the repository that produced it.
//!
//! Both facts were already *in* the tree, unread:
//!
//! ```text
//!   .ai/repo/releases/*.yaml   what shipped, when, from which commit   (kind release-record)
//!   .ai/repo/adrs/*.md         the decisions, dated                    (kind adr)
//!   .ai/repo/features/*.md     what the product gained                 (kind feature)
//!   git log <tag>..<tag>       everything that has no object of its own
//! ```
//!
//! This module joins them. Nothing here is authored: a section per release record, its
//! decisions the ADRs dated inside that release's window, its changes the conventional
//! commits in that range, its artifacts the record's own evidence. What follows the last
//! release is the unreleased section, and the version it implies is arithmetic over the
//! commit types rather than a number somebody chose.
//!
//! # Why the version stays declared twice
//!
//! [`version`] does not change that, and should not. `scripts/release-version` states the
//! reason and it still holds: an installed tree has no `Cargo.toml`, and the crate is
//! compiled before the shell tool exists, so neither program can read the other's copy at
//! run time. What was missing was not a single source — it was a single *writer*. `bump`
//! is that writer, and the existing check remains the proof it worked.

pub mod changelog;
pub mod commits;
pub mod compat;
pub mod model;
pub mod surface;
pub mod version;

pub use changelog::{compose, compose_published};
pub use compat::{analyze, Impact, VersionPlan};
pub use model::{Change, ChangeKind, Changelog, ReleaseSection, VersionReport};
pub use surface::Surface;
pub use version::{bump_of, Bump};
