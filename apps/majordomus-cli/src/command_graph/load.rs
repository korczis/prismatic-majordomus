//! Loading the graph, at two speeds.
//!
//! A completion runs on every TAB and must feel like nothing at all; `commands graph` runs
//! when a person asks and may take a moment. The difference is not a different graph — it
//! is which inputs are read:
//!
//! | | clap tree | shell tool registry | capability registry | workflows | cache |
//! |---|---|---|---|---|---|
//! | [`fast`] | walked | read | composed | from the cache, when there is one | read |
//! | [`full`] | walked | read | composed | asked of the runner | written |
//!
//! Neither loads the repository index. The index takes seconds to build and answers a
//! question the command graph does not ask: nothing about *what commands exist* depends on
//! what is in `.ai/`. That is the whole reason the completion can be synchronous.
//!
//! The workflows are the one input that costs a subprocess, so they are cached: [`full`]
//! writes the graph beside the bridge it materialises, and [`fast`] reads it back. A cache
//! that is missing or from another version of the executable is not an error — the fast
//! path answers from the two declarations it can read directly, which is every command of
//! both programs and none of the repository's own workflows.

use std::path::{Path, PathBuf};

use crate::capability::builtin;
use crate::capability::registry::CapabilityRegistry;
use crate::cli::RepoArgs;
use crate::environment::workflows;
use crate::error::{Error, Result};
use crate::repository::Repository;
use crate::share::Share;

use super::{bridge, build, CommandGraph, Inputs};

/// The file the full load writes and the fast load reads.
pub const CACHE: &str = "graph.json";

/// A loaded graph and where it came from.
pub struct Loaded {
    /// The graph.
    pub graph: CommandGraph,
    /// The repository root it describes.
    pub root: PathBuf,
    /// Whether the workflows in it came from the cache rather than from the runner.
    pub cached: bool,
}

/// The capability registry, composed from the modules and nothing else.
///
/// No index, so no declarative capabilities and no filesystem walk. Every command-line
/// exposure is declared by a builtin module, so the join is complete without one. Public
/// because the completion answers `ValueSource::Capability` from it: composing the modules
/// is arithmetic over compiled-in descriptors, which is why it is affordable on the hot
/// path when building the index is not.
pub fn registry() -> Option<CapabilityRegistry> {
    CapabilityRegistry::builder()
        .with_modules(builtin::modules())
        .build()
        .ok()
}

/// The whole graph, with the workflows asked of the runner, and the cache refreshed.
pub fn full(repo: &RepoArgs) -> Result<Loaded> {
    let root = root_of(repo)?;
    let share = Share::locate(repo.share.as_deref(), &root).ok();
    let registry = registry();
    let catalogue = workflows::resolve(&root);
    let graph = build(&Inputs {
        registry: registry.as_ref(),
        share: share.as_ref().map(|s| s.dir()),
        workflows: Some(&catalogue),
        bridged: bridge::bridged(&root),
    });
    let _ = write_cache(&root, &graph);
    Ok(Loaded {
        graph,
        root,
        cached: false,
    })
}

/// The graph as fast as it can be had: no subprocess, no index, no network.
pub fn fast(repo: &RepoArgs) -> Result<Loaded> {
    let root = root_of(repo)?;
    if let Some(graph) = read_cache(&root) {
        return Ok(Loaded {
            graph,
            root,
            cached: true,
        });
    }
    let share = Share::locate(repo.share.as_deref(), &root).ok();
    let registry = registry();
    let graph = build(&Inputs {
        registry: registry.as_ref(),
        share: share.as_ref().map(|s| s.dir()),
        workflows: None,
        bridged: Default::default(),
    });
    Ok(Loaded {
        graph,
        root,
        cached: false,
    })
}

/// The graph of a repository whose root is already known, without asking the runner.
///
/// What a server answers from: it holds an index and therefore a root, it must not spawn a
/// subprocess per request, and the cache the last materialisation wrote is exactly the
/// answer it wants.
pub fn fast_at(root: &Path, share: Option<&Path>) -> CommandGraph {
    if let Some(graph) = read_cache(root) {
        return graph;
    }
    let registry = registry();
    build(&Inputs {
        registry: registry.as_ref(),
        share,
        workflows: None,
        bridged: Default::default(),
    })
}

/// The whole graph of a repository whose root is already known.
pub fn full_at(root: &Path, share: Option<&Path>) -> CommandGraph {
    let registry = registry();
    let catalogue = workflows::resolve(root);
    build(&Inputs {
        registry: registry.as_ref(),
        share,
        workflows: Some(&catalogue),
        bridged: bridge::bridged(root),
    })
}

/// Where the repository is.
pub fn root(repo: &RepoArgs) -> Result<PathBuf> {
    root_of(repo)
}

/// Where the repository is.
fn root_of(repo: &RepoArgs) -> Result<PathBuf> {
    let start = repo
        .repo
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    Ok(Repository::discover(&start)?.root().to_path_buf())
}

/// The cached graph, when there is one this executable wrote.
///
/// A cache from another schema is ignored rather than migrated: it costs one subprocess to
/// produce a correct one, and reading a document whose shape is not known is how a cache
/// becomes a source of wrong answers.
pub fn read_cache(root: &Path) -> Option<CommandGraph> {
    let text = std::fs::read_to_string(root.join(bridge::DIRECTORY).join(CACHE)).ok()?;
    let graph: CommandGraph = serde_json::from_str(&text).ok()?;
    (graph.schema == super::SCHEMA).then_some(graph)
}

/// Write the cache, beside the bridge, atomically.
pub fn write_cache(root: &Path, graph: &CommandGraph) -> std::io::Result<()> {
    let dir = root.join(bridge::DIRECTORY);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(CACHE);
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(
        &tmp,
        serde_json::to_vec(graph).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(&tmp, &path)
}

/// The repository root, for a caller that has already discovered it.
pub fn cache_path(root: &Path) -> PathBuf {
    root.join(bridge::DIRECTORY).join(CACHE)
}

/// Map a repository error into the crate's own.
pub fn missing_repository(e: Error) -> Error {
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cache_round_trips_and_a_foreign_one_is_ignored() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let graph = build(&Inputs::default());
        write_cache(dir.path(), &graph).expect("the cache is written");
        let read = read_cache(dir.path()).expect("the cache is read back");
        assert_eq!(read.fingerprint, graph.fingerprint);
        assert_eq!(read.commands.len(), graph.commands.len());

        std::fs::write(cache_path(dir.path()), br#"{"schema":"other/v9"}"#).unwrap();
        assert!(read_cache(dir.path()).is_none());
    }
}
