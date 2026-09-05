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

pub mod app;
pub mod capability;
pub mod cli;
pub mod commands;
pub mod discovery;
pub mod error;
pub mod git;
pub mod index;
pub mod logging;
pub mod mcp;
pub mod metadata;
pub mod model;
pub mod repository;

pub use error::Error;
pub use index::Index;
pub use model::{Diagnostic, Object, Provenance, Severity};
pub use repository::Repository;

/// The executable's version, from the crate manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
