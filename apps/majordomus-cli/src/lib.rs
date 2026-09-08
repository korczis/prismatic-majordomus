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
pub mod commands;
pub mod deploy;
pub mod discovery;
pub mod distribution;
pub mod error;
pub mod generate;
pub mod git;
pub mod graph;
pub mod http;
pub mod index;
pub mod lease;
pub mod logging;
pub mod mcp;
pub mod metadata;
pub mod model;
pub mod peers;
pub mod perf;
pub mod policy;
pub mod proto;
pub mod providers;
pub mod quality;
pub mod repository;
pub mod scope;
pub mod share;
pub mod shared;
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
