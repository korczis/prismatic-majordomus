//! The release: what this project has shipped, what it would ship next, and the changelog
//! that is a projection of both rather than a file somebody remembered to update.
//!
//! # The gap this closes
//!
//! Every other public fact in this repository has one canonical declaration and a set of
//! projections derived from it — a command, a capability, a schema, a page. The release did
//! not. Its version was two hand-written strings that a check compared with each other, and
//! nothing raised them (it is authored once now, below); its changelog did not exist at all,
//! and what changed in a version lived only in GitHub's release notes, outside the repository
//! that produced it.
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
//! # The version is authored once
//!
//! The version is authored in one place, the crate manifest's `[package] version`, and
//! [`version`] owns every other statement of it: the compiled constant, the lock's record,
//! and `share/version.txt` — the projection the shell tool reads at start-up, because an
//! installed tree has no `Cargo.toml`. That projection is a `majordomus generate` artifact
//! and ships beside the tool, so the reason the version was once written twice by hand no
//! longer holds (ADR 0085). `release bump` writes the manifest, `scripts/derive` derives the
//! rest, and [`version::diagnose`] refuses a version anybody writes down by hand.

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
