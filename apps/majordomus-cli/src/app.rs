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
use crate::share::Share;

/// The loaded application: the repository, the distribution, the kind schema and the context every command works from.
pub struct App {
    /// The repository the process was pointed at.
    pub repository: Repository,
    /// The distribution directory the kinds and schemas were read from.
    pub share: Share,
    /// The kind schema: the distribution's kinds and schemas plus the repository's additions.
    pub schema: KindSchema,
    /// The index and the registry, shared by every projection.
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
        let repository = {
            let _phase = crate::perf::phase(crate::perf::Phase::RepositoryDiscovery);
            Repository::discover(&start)?
        };
        tracing::info!(repository_root = %repository.root().display(), "repository found");
        let share = Share::locate(args.share.as_deref(), repository.root())?;
        tracing::info!(share = %share.dir().display(), origin = share.origin, "share directory");
        let schema = KindSchema::load(&share, &repository)?;
        if let Some(manifest_schema) = schema.schema("manifest") {
            let violations = manifest_schema.validate(&repository.manifest_value()?);
            if !violations.is_empty() {
                let unknown: Vec<String> = violations
                    .iter()
                    .flat_map(|v| v.unknown_keys.clone())
                    .collect();
                let path = repository.root().join(crate::repository::MANIFEST);
                return Err(if unknown.is_empty() {
                    Error::InvalidManifest {
                        path,
                        reason: violations
                            .iter()
                            .map(|v| format!("{}: {}", v.path, v.message))
                            .collect::<Vec<_>>()
                            .join("; "),
                    }
                } else {
                    Error::UnknownKeys {
                        path,
                        keys: unknown,
                    }
                });
            }
        }
        let sources = Sources::load(&repository)?;
        let git_state = git::inspect(repository.root());
        let source: Box<dyn DiscoverySource> = match args.discovery {
            DiscoveryMode::Vcs => {
                if let git::GitState::Unavailable { reason } = &git_state {
                    return Err(Error::Git {
                        reason: format!("{reason}; discovery through the version-control index needs git (or pass --discovery filesystem)"),
                    });
                }
                Box::new(VcsIndex::default())
            }
            DiscoveryMode::Filesystem => Box::new(FileSystem {
                excluded: vec![".git".into(), repository.local_path()],
            }),
        };
        let scope = crate::scope::Scope::load(&share, &repository)?;
        tracing::info!(origin = ?scope.origin(), path = scope.path(), "scope");
        let index = Index::build(
            &repository,
            &sources,
            &schema,
            source.as_ref(),
            git_state,
            scope,
        )?;
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
        Self::compose(repository, share, schema, index)
    }

    /// Compose the registry over an index already built (tests build their own).
    pub fn compose(
        repository: Repository,
        share: Share,
        schema: KindSchema,
        index: Index,
    ) -> Result<Self> {
        let registry = CapabilityRegistry::builder()
            .with_modules(builtin::modules())
            .with_index(&index)
            .build()
            .map_err(|errors| Error::Registry { errors })?;
        tracing::info!(capabilities = registry.len(), "registry built");
        let context = Arc::new(Context::new(Arc::new(index), Arc::new(registry)));
        Ok(App {
            repository,
            share,
            schema,
            context,
        })
    }

    /// The index of the repository's objects.
    pub fn index(&self) -> &Index {
        &self.context.index
    }

    /// The capability registry.
    pub fn registry(&self) -> &CapabilityRegistry {
        &self.context.registry
    }
}
