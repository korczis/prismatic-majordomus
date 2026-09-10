//! The canonical command graph: one typed model of everything this repository can be
//! asked to run, and the projections every developer-facing surface is rendered from.
//!
//! The capability registry ([`crate::capability`]) is the same idea for the machine
//! surfaces: a capability is declared once and MCP, HTTP, OpenAPI and the Cockpit are
//! projections of it. This module is its counterpart for the surfaces a person types at.
//!
//! ```text
//!   clap declaration      share/commands.yaml      .ai/repo/workflows/commands/*.yaml
//!   (native.rs)           (shell.rs)               (workflow.rs)
//!         \                     |                        /
//!          \                    |                       /
//!           ------------- graph.rs: compose, validate, name ------------
//!                                 |
//!         ┌───────────┬───────────┼────────────┬─────────────┐
//!         ▼           ▼           ▼            ▼             ▼
//!      just.rs   completion.rs  activate.rs  the `commands`  the generated
//!      (bridge)  (one engine)   (entry)      capabilities    reference
//!                                            (MCP/HTTP/      (docs, site)
//!                                             OpenAPI/
//!                                             Cockpit)
//! ```
//!
//! The direction never reverses. No projection is read back as a source, no surface
//! invokes another surface, and no surface holds a list of commands: a command added to
//! any one of the three declarations appears in all of them at once, and a command
//! removed disappears from all of them at once.
//!
//! Building the graph costs a walk of the clap declaration, one small YAML file and a
//! directory read — no index, no registry, no subprocess, no network — which is what lets
//! repository entry and tab completion both go through it and stay inside their budgets.

pub mod activate;
pub mod completion;
pub mod facts;
pub mod graph;
pub mod just;
pub mod model;
pub mod native;
pub mod projection;
pub mod semantics;
pub mod shell;
pub mod values;
pub mod workflow;

pub use activate::{BannerMode, Entry};
pub use completion::{CompletionCandidate, CompletionRequest, CompletionResponse, Engine};
pub use facts::Facts;
pub use graph::{CommandGraph, Surface};
pub use model::{
    Alias, ArgumentSpec, Availability, CommandId, CommandNode, Deprecation, Diagnostic,
    EffectClass, EnumValue, Example, Execution, Interactivity, Program, Projections, Provenance,
    Requirement, Sensitivity, ValueRegistry, ValueSource, Visibility, SCHEMA,
};
pub use semantics::CommandSemantics;
pub use values::ValueIndex;

use std::path::Path;

/// Write a file so that a reader sees either the whole of the old content or the whole of
/// the new one: a temporary file beside the target, then a rename, which is atomic within
/// a filesystem. Two shells entering the same repository at once therefore cannot produce
/// a half-written bridge or a truncated cache, and neither has to hold a lock to be safe.
///
/// The temporary name carries the process id, so concurrent writers do not collide on it.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&temporary, bytes)?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&temporary);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_is_all_or_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        atomic_write(&path, b"first").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
        atomic_write(&path, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no temporary is left behind"
        );
    }

    #[test]
    fn concurrent_writers_leave_one_whole_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("file");
        let bodies = ["a".repeat(4096), "b".repeat(4096)];
        std::thread::scope(|s| {
            for body in &bodies {
                let path = path.clone();
                s.spawn(move || {
                    for _ in 0..20 {
                        atomic_write(&path, body.as_bytes()).unwrap();
                    }
                });
            }
        });
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(bodies.contains(&text), "one of the two, whole");
    }
}
