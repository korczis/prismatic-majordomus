//! Majordomus control-plane CLI.
//!
//! The library half of the executable. Dependencies flow inward toward the domain model:
//!
//! ```text
//! cli / commands  ->  mcp (surface, protocol, stdio)  ->  index / model
//!                     discovery, metadata, repository, git  ->  model, error
//! ```
//!
//! Nothing below `commands` knows about clap; nothing below `mcp` knows about JSON-RPC; and
//! the model knows nothing about either.
#![warn(missing_docs)]

pub mod about;
pub mod app;
pub mod bench;
pub mod capability;
pub mod cli;
pub mod cockpit;
pub mod command_graph;
pub mod commands;
pub mod commit;
pub mod deploy;
pub(crate) mod design;
pub mod devcontext;
pub mod devtask;
pub mod diagram;
pub mod discovery;
pub(crate) mod distribution;
pub mod environment;
pub(crate) mod error;
pub mod evidence;
pub mod execution;
pub mod gates;
pub mod generate;
pub mod generation;
pub mod git;
pub mod graph;
pub mod http;
pub mod index;
pub mod lease;
pub mod ledger;
pub mod live;
pub mod logging;
pub mod mcp;
pub mod mesh;
pub mod metadata;
pub mod model;
pub mod models;
pub mod order;
pub mod peers;
pub mod perf;
pub mod plan;
pub mod policy;
pub mod product;
pub mod proto;
pub mod providers;
pub mod quality;
pub mod release;
pub mod repository;
pub mod rules;
pub mod scope;
pub mod session;
pub mod share;
pub(crate) mod shared;
pub mod site;
pub mod synthetic;
pub mod web;
pub mod why;
pub mod worktree;

pub use error::Error;
pub use index::Index;
pub use model::{Diagnostic, Object, Provenance, Severity};
pub use repository::Repository;

/// The executable's version, from the crate manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Rust target triple this executable was built for, from `build.rs`. The same string
/// the distribution model names a target by, so that a build can say which artifact it is.
pub const TARGET: &str = env!("MAJORDOMUS_TARGET");

/// The cargo profile this executable was built with.
pub const PROFILE: &str = env!("MAJORDOMUS_PROFILE");

/// The commit this executable was built from, or `unknown` outside a work tree. Read at
/// build time: nothing here shells out to git, and an installed binary needs no repository
/// in order to say what it is.
pub const COMMIT: &str = env!("MAJORDOMUS_COMMIT");

/// The generation this executable was built from: the digest `generation::crate_generation`
/// takes over the crate sources, compiled in by `build.rs`, or `unknown` when the build
/// could not read them.
///
/// [`VERSION`] says what this executable calls itself; this says what it *is*. Two builds
/// of `majordomus-cli 0.4.0` from different revisions of the crate carry different models
/// and derive a repository differently, and only this tells them apart — which is why
/// `majordomus generate` compares it against the tree it is asked to derive rather than
/// comparing versions.
pub const GENERATION: &str = env!("MAJORDOMUS_GENERATION");
