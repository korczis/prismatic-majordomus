//! The application context every command starts from: the repository, its index, and
//! the capability registry composed once from the builtin executables and the index.
//! Commands differ in what they do with it; none of them builds it differently.

use std::sync::Arc;

use crate::capability::{builtin, CapabilityRegistry, Context};
use crate::cli::{DiscoveryMode, RepoArgs};
use crate::discovery::{DiscoverySource, FileSystem, Sources, VcsIndex};
use crate::error::{Error, Result};
use crate::git;
use crate::index::Index;
use crate::metadata::KindSchema;
use crate::model::Severity;
use crate::repository::Repository;

pub struct App {
    pub repository: Repository,
    pub context: Arc<Context>,
}

impl App {
    /// Discover the repository, build the index, log every diagnostic, compose the
    /// registry. `--strict` turns an error diagnostic into a startup error.
    pub fn load(args: &RepoArgs) -> Result<Self> {
        let start = match &args.repo {
            Some(p) => p.clone(),
            None => std::env::current_dir().map_err(|e| Error::io(".", e))?,
        };
        let repository = Repository::discover(&start)?;
        tracing::info!(repository_root = %repository.root().display(), "repository found");
        let sources = Sources::load(&repository)?;
        let schema = KindSchema::embedded()?;
        let git_state = git::inspect(repository.root());
        let source: Box<dyn DiscoverySource> = match args.discovery {
            DiscoveryMode::Vcs => {
                if let git::GitState::Unavailable { reason } = &git_state {
                    return Err(Error::Git {
                        reason: format!("{reason}; discovery through the version-control index needs git (or pass --discovery filesystem)"),
                    });
                }
                Box::new(VcsIndex)
            }
            DiscoveryMode::Filesystem => Box::new(FileSystem {
                excluded: vec![".git".into(), repository.local_path()],
            }),
        };
        let index = Index::build(&repository, &sources, &schema, source.as_ref(), git_state)?;
        for d in &index.diagnostics {
            let path = d.path.as_deref().unwrap_or("-");
            match d.severity {
                Severity::Error => {
                    tracing::error!(code = %d.code, source_path = path, "{}", d.message)
                }
                Severity::Warning => {
                    tracing::warn!(code = %d.code, source_path = path, "{}", d.message)
                }
                Severity::Info => {
                    tracing::info!(code = %d.code, source_path = path, "{}", d.message)
                }
            }
        }
        let errors = index.errors();
        if args.strict && errors > 0 {
            return Err(Error::StrictDiagnostics { count: errors });
        }
        Self::compose(repository, index)
    }

    /// Compose the registry over an index already built (tests build their own).
    pub fn compose(repository: Repository, index: Index) -> Result<Self> {
        let registry = CapabilityRegistry::builder()
            .with_builtin(builtin::all())
            .with_index(&index)
            .build()
            .map_err(|errors| Error::Registry { errors })?;
        tracing::info!(capabilities = registry.len(), "registry built");
        let context = Arc::new(Context {
            index: Arc::new(index),
            registry: Arc::new(registry),
        });
        Ok(App {
            repository,
            context,
        })
    }

    pub fn index(&self) -> &Index {
        &self.context.index
    }

    pub fn registry(&self) -> &CapabilityRegistry {
        &self.context.registry
    }
}
