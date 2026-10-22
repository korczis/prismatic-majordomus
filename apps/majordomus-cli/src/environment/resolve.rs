//! The resolver: one function that builds a [`RepositoryEnvironment`], and the only place
//! any surface gets one.
//!
//! # The two resolutions
//!
//! A full resolution has the index; a fast one does not, and may not build it. That is not
//! a preference. Building the index reads and validates every declared file of the layer,
//! which is measured in seconds even in a release build, and a fast resolution is what
//! runs when a person types `cd`.
//!
//! Everything else is cheap enough for either: the project identity is compile-time
//! constants, the repository comes from one manifest, version control from one `git`
//! call, the toolchains from a handful of `exists()`, the provider projections from the
//! policy and its templates, the services from one small file and at most one bounded
//! connection attempt. The three answers that are not cheap — what the layer holds,
//! what the workflow runner says, and what versions are installed — come from
//! [`super::cache`], each with its own fingerprint, and are reported as unavailable when
//! no cache can supply them.
//!
//! Unavailable is a real answer here, and the discipline the whole module is built on: a
//! count that was not counted is absent, never zero, because a person reads a zero as a
//! broken repository and acts on it.

use std::path::Path;

use crate::capability::CapabilityRegistry;
use crate::index::Index;
use crate::model::{Diagnostic, Severity};
use crate::repository::Repository;
use crate::share::Share;

use super::cache::{fingerprint_of, Cache, TOOLCHAIN_LIFETIME};
use super::{
    services, toolchain, vcs, workflows, FieldSource, KindCount, LayerSummary, ProjectIdentity,
    ProjectionState, ProviderState, RepositoryEnvironment, RepositoryIdentity, Resolution,
    TierState, ToolchainAvailability, VcsState,
};

/// What a caller wants resolved, and what it will pay for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvironmentQuery {
    /// How much may be read.
    pub resolution: Resolution,
    /// Whether a running server's address may be contacted. A resolution that says no
    /// reports every service `unknown`, which is what it knows.
    pub probe_services: bool,
    /// Whether the cache may supply the tiers this resolution cannot compute.
    pub use_cache: bool,
    /// Whether what was computed is written back for the next fast resolution.
    pub write_cache: bool,
}

impl EnvironmentQuery {
    /// What a shell prompt asks for: everything cheap, the cache for the rest, a bounded
    /// probe of the local address, and no write of tiers it did not compute.
    pub fn fast() -> Self {
        EnvironmentQuery {
            resolution: Resolution::Fast,
            probe_services: true,
            use_cache: true,
            write_cache: true,
        }
    }

    /// What a person waiting on a command asks for: everything, written back.
    pub fn full() -> Self {
        EnvironmentQuery {
            resolution: Resolution::Full,
            probe_services: true,
            use_cache: false,
            write_cache: true,
        }
    }

    /// The same, without touching anything outside this process: no probe, no cache read,
    /// no cache write. What a test and a hosted request use.
    pub fn sealed(self) -> Self {
        EnvironmentQuery {
            probe_services: false,
            use_cache: false,
            write_cache: false,
            ..self
        }
    }
}

/// What the resolver may read. A fast resolution carries no index and no registry; a full
/// one carries both, and the presence of the index is what makes it full.
pub struct Inputs<'a> {
    /// The repository, already discovered.
    pub repository: &'a Repository,
    /// The distribution directory, when it was located.
    pub share: Option<&'a Share>,
    /// The index, in a full resolution.
    pub index: Option<&'a Index>,
    /// The registry, in a full resolution.
    pub registry: Option<&'a CapabilityRegistry>,
    /// The policy, when the caller has already read it. The resolver reads it itself when
    /// this is `None`, which is what every caller that needs nothing else from it does.
    ///
    /// It is here for the one caller that does: `majordomus env enter` asks the policy
    /// whether entry ensures a runtime, and then resolves a snapshot whose provider
    /// projections come out of the same file. Two reads of one canonical file inside one
    /// invocation is what `project.hot-path-reads-once` is about, and this is the hot path
    /// by definition — it runs on every `cd`.
    pub policy: Option<&'a crate::policy::LoadedPolicy>,
}

/// Build a snapshot.
///
/// Never fails. Anything that cannot be read becomes a diagnostic and an absent value; a
/// snapshot is the report of what could be learnt, and a resolver that refused to answer
/// because one thing was missing would take the shell down with it.
pub fn resolve(inputs: &Inputs<'_>, query: &EnvironmentQuery) -> RepositoryEnvironment {
    let root = inputs.repository.root();
    let local_half = inputs.repository.local_path();
    let mut diagnostics = Vec::new();
    let mut provenance = Vec::new();

    let project = ProjectIdentity::of_this_build();
    provenance.push(FieldSource::exact(
        "project.version",
        Some(project.version.clone()),
        "the crate manifest, at compile time (CARGO_PKG_VERSION)",
        "environment::resolve",
    ));
    provenance.push(FieldSource::exact(
        "project.summary",
        Some(project.summary.clone()),
        "majordomus_cli::about::SUMMARY",
        "environment::resolve",
    ));
    provenance.push(FieldSource::exact(
        "project.commit",
        Some(project.commit.clone()),
        "build.rs, at compile time (MAJORDOMUS_COMMIT)",
        "environment::resolve",
    ));

    let repository = repository_identity(inputs.repository);
    provenance.push(FieldSource::exact(
        "repository.name",
        Some(repository.name.clone()),
        "the base name of the repository root",
        "environment::resolve",
    ));
    provenance.push(FieldSource::exact(
        "repository.sections",
        Some(format!("{} section(s)", repository.sections.len())),
        crate::repository::MANIFEST,
        "environment::resolve",
    ));

    // ---------------------------------------------------------------- version control
    let vcs = vcs::inspect(root);
    match &vcs {
        VcsState::Git(tree) => {
            provenance.push(FieldSource::exact(
                "vcs.branch",
                tree.branch.clone(),
                vcs::SOURCE,
                "environment::vcs",
            ));
            provenance.push(FieldSource::exact(
                "vcs.clean",
                Some(tree.clean.to_string()),
                vcs::SOURCE,
                "environment::vcs",
            ));
            provenance.push(match (tree.ahead, tree.behind) {
                (Some(a), Some(b)) => FieldSource::exact(
                    "vcs.ahead_behind",
                    Some(format!("{a}/{b}")),
                    vcs::SOURCE,
                    "environment::vcs",
                ),
                _ => FieldSource::unknown(
                    "vcs.ahead_behind",
                    "the branch tracks no upstream",
                    "environment::vcs",
                ),
            });
        }
        VcsState::Unavailable { reason } => {
            diagnostics.push(Diagnostic::warning(
                "vcs_unavailable",
                None,
                format!("version control could not be read: {reason}"),
            ));
            provenance.push(FieldSource::unknown(
                "vcs.branch",
                format!("{}: {reason}", vcs::SOURCE),
                "environment::vcs",
            ));
        }
    }

    let mut cache = if query.use_cache || query.write_cache {
        Cache::load(root, &local_half)
    } else {
        Cache::default()
    };

    // ---------------------------------------------------------------- what the layer holds
    let layer_fingerprint = layer_fingerprint(inputs.repository, &vcs);
    let layer = match (inputs.index, inputs.registry) {
        (Some(index), Some(registry)) => {
            let summary = summarise(index, registry);
            provenance.push(FieldSource::exact(
                "layer.objects",
                summary.objects.map(|n| n.to_string()),
                "the index this process built",
                "environment::resolve",
            ));
            summary
        }
        _ => {
            let cached = query
                .use_cache
                .then_some(cache.tiers.layer.as_ref())
                .flatten()
                .and_then(|e| e.fresh(&layer_fingerprint, None));
            match cached {
                Some(summary) => {
                    let mut summary = summary.clone();
                    summary.state = TierState::Cached;
                    provenance.push(FieldSource::cached(
                        "layer.objects",
                        summary.objects.map(|n| n.to_string()),
                        format!(
                            "{}, written by the last full resolution",
                            super::cache::CACHE_PATH
                        ),
                        "environment::cache",
                    ));
                    summary
                }
                None => {
                    diagnostics.push(Diagnostic::info(
                        "environment_layer_not_counted",
                        None,
                        "what the layer holds is not in the cache for this state of the repository; `majordomus env status` counts it and writes the cache",
                    ));
                    provenance.push(FieldSource::unknown(
                        "layer.objects",
                        "a fast resolution does not build the index, and no cache entry matched",
                        "environment::cache",
                    ));
                    LayerSummary::unavailable()
                }
            }
        }
    };
    if layer.degraded == Some(true) {
        diagnostics.push(Diagnostic::warning(
            "layer_degraded",
            None,
            format!(
                "{} declared file(s) did not become objects; `majordomus capabilities validate` names them",
                layer.invalid.unwrap_or_default()
            ),
        ));
    }

    // ---------------------------------------------------------------- workflows
    let workflow_fingerprint = workflow_fingerprint(root);
    let cached_workflows = query
        .use_cache
        .then_some(cache.tiers.workflows.as_ref())
        .flatten()
        .and_then(|e| e.fresh(&workflow_fingerprint, None));
    let workflows = match cached_workflows {
        Some(catalogue) => {
            let mut catalogue = catalogue.clone();
            catalogue.state = TierState::Cached;
            provenance.push(FieldSource::cached(
                "workflows",
                Some(format!("{} workflow(s)", catalogue.workflows.len())),
                format!("{}, keyed by the workflow files", super::cache::CACHE_PATH),
                "environment::cache",
            ));
            catalogue
        }
        None => {
            // Asking the runner costs two short subprocesses, which a fast resolution can
            // afford when the answer is not already known; it is the index it cannot.
            let catalogue = workflows::resolve(root);
            provenance.push(match catalogue.state {
                TierState::Unavailable => FieldSource::unknown(
                    "workflows",
                    "no workflow runner answered for this repository",
                    "environment::workflows",
                ),
                _ => FieldSource::exact(
                    "workflows",
                    Some(format!("{} workflow(s)", catalogue.workflows.len())),
                    workflows::SOURCE,
                    "environment::workflows",
                ),
            });
            catalogue
        }
    };

    // ---------------------------------------------------------------- toolchains
    let declared = toolchain::declared(root);
    let toolchain_fingerprint = fingerprint_of(
        root,
        &declared
            .iter()
            .map(|t| t.declared_by.clone())
            .collect::<Vec<_>>(),
        &["toolchains"],
    );
    let toolchains = if query.resolution == Resolution::Full {
        let resolved = toolchain::with_installed(declared);
        provenance.push(FieldSource::exact(
            "toolchains",
            Some(format!("{} declared", resolved.len())),
            "each toolchain's own --version, bounded",
            "environment::toolchain",
        ));
        resolved
    } else {
        let cached = query
            .use_cache
            .then_some(cache.tiers.toolchains.as_ref())
            .flatten()
            .and_then(|e| e.fresh(&toolchain_fingerprint, Some(TOOLCHAIN_LIFETIME)))
            .cloned();
        match cached {
            Some(resolved) => {
                provenance.push(FieldSource::cached(
                    "toolchains",
                    Some(format!("{} declared", resolved.len())),
                    format!("{}, within its lifetime", super::cache::CACHE_PATH),
                    "environment::cache",
                ));
                resolved
            }
            None => {
                provenance.push(FieldSource::exact(
                    "toolchains",
                    Some(format!("{} declared", declared.len())),
                    "the file that declares each one; no version was asked",
                    "environment::toolchain",
                ));
                declared
            }
        }
    };
    for missing in toolchains
        .iter()
        .filter(|t| t.availability == ToolchainAvailability::Missing)
    {
        diagnostics.push(Diagnostic::warning(
            "toolchain_missing",
            Some(missing.declared_by.clone()),
            format!("{} is declared here and is not installed", missing.title),
        ));
    }

    // ---------------------------------------------------------------- providers
    let (providers, provider_source) =
        provider_states(inputs.repository, inputs.share, inputs.policy);
    provenance.push(FieldSource::exact(
        "providers",
        Some(format!("{} projection(s)", providers.len())),
        provider_source,
        "environment::resolve",
    ));
    for stale in providers
        .iter()
        .filter(|p| matches!(p.state, ProjectionState::Stale | ProjectionState::Absent))
    {
        diagnostics.push(Diagnostic::warning(
            "projection_stale",
            Some(stale.target.clone()),
            format!(
                "{} is {} against the policy that renders it; `majordomus generate` rewrites it",
                stale.target,
                match stale.state {
                    ProjectionState::Absent => "missing",
                    _ => "stale",
                }
            ),
        ));
    }

    // ---------------------------------------------------------------- services
    let services = services::resolve(root, &local_half, query.probe_services);
    provenance.push(match services.first().and_then(|s| s.url.clone()) {
        Some(_) => FieldSource::exact(
            "services.url",
            services.first().and_then(|s| s.url.clone()),
            format!("{}/{}", local_half, crate::lease::LEASE_PATH),
            "environment::services",
        ),
        None => FieldSource::unknown(
            "services.url",
            format!(
                "no server holds the lease at {}/{}",
                local_half,
                crate::lease::LEASE_PATH
            ),
            "environment::services",
        ),
    });

    let environment = RepositoryEnvironment {
        schema: RepositoryEnvironment::schema_id(),
        generated_at: crate::peers::rfc3339(std::time::SystemTime::now()),
        resolution: query.resolution,
        project,
        repository,
        vcs,
        toolchains,
        layer,
        workflows,
        providers,
        services,
        diagnostics,
        provenance,
    };

    if query.write_cache {
        // Only what this resolution actually computed is written back; a tier taken from
        // the cache is rewritten with the same fingerprint, which is a no-op, and a tier
        // nothing resolved is left as it was rather than being replaced with an absence.
        if environment.layer.state == TierState::Resolved {
            cache.tiers.layer = Some(Cache::entry(layer_fingerprint, environment.layer.clone()));
        }
        if environment.workflows.state == TierState::Resolved {
            cache.tiers.workflows = Some(Cache::entry(
                workflow_fingerprint,
                environment.workflows.clone(),
            ));
        }
        if environment
            .toolchains
            .iter()
            .any(|t| t.availability != ToolchainAvailability::Unknown)
        {
            cache.tiers.toolchains = Some(Cache::entry(
                toolchain_fingerprint,
                environment.toolchains.clone(),
            ));
        }
        if let Err(e) = cache.store(root, &local_half) {
            tracing::debug!(error = %e, "the environment cache could not be written");
        }
    }

    environment
}

/// The identity of the checkout.
fn repository_identity(repository: &Repository) -> RepositoryIdentity {
    let root = repository.root();
    RepositoryIdentity {
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.display().to_string()),
        root: root.display().to_string(),
        layer_schema: repository.manifest().schema.clone(),
        sections: repository
            .manifest()
            .sections
            .iter()
            .map(|(name, path)| (name.clone(), format!(".ai/{path}")))
            .collect(),
        local_path: repository.local_path(),
        // A linked work tree has a `.git` file rather than a `.git` directory. One stat.
        linked_worktree: root.join(".git").is_file(),
    }
}

/// What the layer holds, from the index and the registry that already hold it.
fn summarise(index: &Index, registry: &CapabilityRegistry) -> LayerSummary {
    LayerSummary {
        state: TierState::Resolved,
        kinds: index
            .kinds()
            .into_iter()
            .map(|(kind, count)| KindCount {
                kind: kind.to_string(),
                count,
            })
            .collect(),
        objects: Some(index.objects.len()),
        capabilities: Some(registry.summary().total),
        invalid: Some(
            index
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count(),
        ),
        degraded: Some(index.state == crate::index::State::Degraded),
    }
}

/// The provider projections the policy declares, each against the file it renders to.
fn provider_states(
    repository: &Repository,
    share: Option<&Share>,
    already: Option<&crate::policy::LoadedPolicy>,
) -> (Vec<ProviderState>, String) {
    let source = format!(
        "{}, projections[]",
        repository
            .section_path("policy")
            .unwrap_or_else(|| ".ai/repo/policy.yaml".into())
    );
    let read;
    let policy = match already {
        Some(policy) => policy,
        None => {
            let Ok(loaded) = crate::policy::LoadedPolicy::load(repository) else {
                return (Vec::new(), format!("{source} (unreadable)"));
            };
            read = loaded;
            &read
        }
    };
    // Rendering needs the templates, which live in the distribution. Without it the
    // projections are still known — the policy declares them — but whether each file is
    // current is not, and `Unknown` says exactly that.
    let rendered = share.and_then(|share| {
        crate::providers::artifacts(repository, share, policy)
            .ok()
            .map(|artifacts| {
                artifacts
                    .into_iter()
                    .map(|a| {
                        let state = crate::providers::state_of(repository.root(), &a);
                        (a.path, state)
                    })
                    .collect::<Vec<_>>()
            })
    });
    let states = policy
        .policy
        .projections
        .iter()
        .map(|p| ProviderState {
            id: p.provider.clone(),
            target: p.target.clone(),
            always_loaded: p.always_loaded,
            state: match rendered
                .as_ref()
                .and_then(|r| r.iter().find(|(path, _)| *path == p.target))
            {
                Some((_, crate::providers::TargetState::Current)) => ProjectionState::Current,
                Some((_, crate::providers::TargetState::Stale)) => ProjectionState::Stale,
                Some((_, crate::providers::TargetState::Absent)) => ProjectionState::Absent,
                None => ProjectionState::Unknown,
            },
        })
        .collect();
    (states, source)
}

/// The fingerprint of everything that can change what the layer holds: the commit, the
/// files that configure discovery, and whatever is dirty under a section of the layer.
///
/// The dirty set is what makes this exact rather than approximate. A rule edited and not
/// committed moves no commit and touches no configuration file, so a fingerprint over
/// those alone would keep serving the old count; the file appears in the status report
/// this snapshot has already paid for, and its size and modification time go into the
/// fingerprint at the cost of one `stat`.
fn layer_fingerprint(repository: &Repository, vcs: &VcsState) -> String {
    let mut paths: Vec<String> = vec![
        crate::repository::MANIFEST.to_string(),
        ".ai/repo/knowledge/sources.yaml".to_string(),
        ".ai/repo/knowledge/kinds.yaml".to_string(),
        "share/kinds.yaml".to_string(),
    ];
    if let Some(policy) = repository.section_path("policy") {
        paths.push(policy);
    }
    if let Some(scope) = repository.section_path("scope") {
        paths.push(scope);
    }
    let head = vcs
        .tree()
        .and_then(|t| t.head.clone())
        .unwrap_or_else(|| "unborn".into());
    if let Some(tree) = vcs.tree() {
        // only the dirty paths that could be objects of the layer, so that a change to an
        // unrelated file costs nothing
        paths.extend(
            tree.changed_paths
                .iter()
                .filter(|p| is_layer_path(repository, p))
                .cloned(),
        );
    }
    fingerprint_of(repository.root(), &paths, &["layer", &head])
}

/// Could a repository-relative path be an object of the layer? Derived from the manifest's
/// own sections plus the two trees outside `.ai/` that the source classes reach into.
fn is_layer_path(repository: &Repository, path: &str) -> bool {
    if path.starts_with(&repository.local_path()) {
        return false;
    }
    path.starts_with(".ai/")
        || path.starts_with("docs/")
        || path.starts_with("share/")
        || path.starts_with("lib/")
        || path.starts_with("test/cases/")
        || path.starts_with("bin/")
}

/// The fingerprint of the workflow files: the justfile and whatever it imports.
fn workflow_fingerprint(root: &Path) -> String {
    let mut paths = vec!["justfile".to_string(), ".justfile".to_string()];
    // One directory read of the module directory, when there is one. A justfile split into
    // modules must expire when any of them changes, and reading one directory is the whole
    // cost of knowing that.
    if let Ok(entries) = std::fs::read_dir(root.join(".just")) {
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .map(|name| format!(".just/{name}"))
            .collect();
        crate::order::canonical(&mut names);
        paths.extend(names);
    } else {
        // The absence of the directory is itself part of the fingerprint: creating it must
        // expire the catalogue.
        paths.push(".just".to_string());
    }
    fingerprint_of(root, &paths, &["workflows"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    fn repository() -> SyntheticRepository {
        SyntheticRepository::new(Shape {
            rules: 3,
            ..Shape::default()
        })
        .expect("a synthetic repository")
    }

    fn full(repo: &SyntheticRepository) -> RepositoryEnvironment {
        let index = repo.index().expect("an index");
        let registry = CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .with_index(&index)
            .build()
            .expect("a registry");
        let repository = Repository::open(repo.root()).expect("a repository");
        resolve(
            &Inputs {
                repository: &repository,
                share: None,
                index: Some(&index),
                registry: Some(&registry),
                policy: None,
            },
            &EnvironmentQuery::full().sealed(),
        )
    }

    fn fast(repo: &SyntheticRepository, query: EnvironmentQuery) -> RepositoryEnvironment {
        let repository = Repository::open(repo.root()).expect("a repository");
        resolve(
            &Inputs {
                repository: &repository,
                share: None,
                index: None,
                registry: None,
                policy: None,
            },
            &query,
        )
    }

    #[test]
    fn a_full_resolution_counts_what_the_layer_holds() {
        let repo = repository();
        let env = full(&repo);
        assert_eq!(env.resolution, Resolution::Full);
        assert_eq!(env.layer.state, TierState::Resolved);
        assert!(env.layer.objects.unwrap_or(0) > 0);
        assert_eq!(env.layer.kind("rule"), Some(3));
    }

    /// The discipline the module is built on: a fast resolution with nothing cached says
    /// it does not know, and never says zero.
    #[test]
    fn a_fast_resolution_with_a_cold_cache_reports_unknown_and_never_zero() {
        let repo = repository();
        let env = fast(&repo, EnvironmentQuery::fast().sealed());
        assert_eq!(env.layer.state, TierState::Unavailable);
        assert_eq!(env.layer.objects, None);
        assert_eq!(env.layer.kind("rule"), None);
        assert!(env
            .diagnostics
            .iter()
            .any(|d| d.code == "environment_layer_not_counted"));
        assert_eq!(
            env.explain("layer.objects").map(|p| p.confidence),
            Some(super::super::Confidence::Unknown)
        );
    }

    #[test]
    fn a_full_resolution_warms_the_cache_a_fast_one_then_reads() {
        let repo = repository();
        let index = repo.index().expect("an index");
        let registry = CapabilityRegistry::builder()
            .with_modules(crate::capability::builtin::modules())
            .with_index(&index)
            .build()
            .expect("a registry");
        let repository = Repository::open(repo.root()).expect("a repository");
        let warm = resolve(
            &Inputs {
                repository: &repository,
                share: None,
                index: Some(&index),
                registry: Some(&registry),
                policy: None,
            },
            &EnvironmentQuery {
                probe_services: false,
                ..EnvironmentQuery::full()
            },
        );
        assert_eq!(warm.layer.state, TierState::Resolved);

        let then = fast(
            &repo,
            EnvironmentQuery {
                probe_services: false,
                write_cache: false,
                ..EnvironmentQuery::fast()
            },
        );
        assert_eq!(then.layer.state, TierState::Cached);
        assert_eq!(then.layer.objects, warm.layer.objects);
        assert_eq!(then.layer.kinds, warm.layer.kinds);
        assert_eq!(
            then.explain("layer.objects").map(|p| p.confidence),
            Some(super::super::Confidence::Cached)
        );
    }

    /// The two resolutions differ in what they may read, never in what they mean. Every
    /// field a fast resolution does fill must equal the full one's.
    #[test]
    fn fast_and_full_agree_on_everything_fast_can_see() {
        let repo = repository();
        let full = full(&repo);
        let fast = fast(&repo, EnvironmentQuery::fast().sealed());
        assert_eq!(fast.project, full.project);
        assert_eq!(fast.repository, full.repository);
        assert_eq!(fast.vcs, full.vcs);
        assert_eq!(fast.providers, full.providers);
        let ids = |e: &RepositoryEnvironment| -> Vec<String> {
            e.services.iter().map(|s| s.id.clone()).collect()
        };
        assert_eq!(ids(&fast), ids(&full));
        assert_eq!(
            fast.toolchains
                .iter()
                .map(|t| t.id.clone())
                .collect::<Vec<_>>(),
            full.toolchains
                .iter()
                .map(|t| t.id.clone())
                .collect::<Vec<_>>()
        );
    }

    /// Two snapshots of one unchanged repository must be the same document apart from the
    /// moment they were taken, or every consumer that compares them sees news that is not
    /// there.
    #[test]
    fn a_snapshot_of_an_unchanged_repository_is_deterministic() {
        let repo = repository();
        let a = full(&repo);
        let b = full(&repo);
        assert_eq!(a.digest(), b.digest());
        let mut a_json = serde_json::to_value(&a).expect("json");
        let mut b_json = serde_json::to_value(&b).expect("json");
        for v in [&mut a_json, &mut b_json] {
            v["generated_at"] = serde_json::Value::Null;
        }
        assert_eq!(a_json, b_json);
    }

    /// The digest answers "is there anything new to show a person", so it moves when a
    /// count moves. It deliberately does not move when a file's bytes change without
    /// changing what the snapshot says: a banner that turned into news on every keystroke
    /// would be one a person learns to ignore.
    #[test]
    fn the_digest_follows_what_the_snapshot_says_and_not_every_byte() {
        let repo = repository();
        let before = full(&repo).digest();

        repo.touch_rule(0, "-changed").expect("a changed rule");
        assert_eq!(
            before,
            full(&repo).digest(),
            "editing a rule's body changes no count and so is not news"
        );

        std::fs::write(
            repo.root().join(".ai/repo/rules/project/rule-99.v1.md"),
            "---\nid: project.rule-99\nversion: 1\nkind: rule\ntitle: T\ndescription: D\nstatement: S\nstatus: active\nclass: advisory\n---\n\n# Rationale\n\nx\n",
        )
        .expect("a new rule");
        assert_ne!(
            before,
            full(&repo).digest(),
            "a rule that did not exist before is news"
        );
    }

    /// The digest answers "would a person see anything new", so the resolution that
    /// produced it must not be part of it: a warm fast banner and a full one of the same
    /// repository would otherwise always look like different news.
    #[test]
    fn the_digest_ignores_the_moment_and_the_resolution() {
        let repo = repository();
        let mut a = full(&repo);
        let mut b = a.clone();
        b.generated_at = "1999-01-01T00:00:00Z".into();
        b.resolution = Resolution::Fast;
        b.provenance.clear();
        assert_eq!(a.digest(), b.digest());
        a.repository.name = "renamed".into();
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn a_repository_that_is_not_a_work_tree_still_produces_a_snapshot() {
        let repo = repository();
        let env = fast(&repo, EnvironmentQuery::fast().sealed());
        // The synthetic repository is not a git work tree, which is exactly the
        // degradation this asserts: a diagnostic, not a failure.
        if matches!(env.vcs, VcsState::Unavailable { .. }) {
            assert!(env.diagnostics.iter().any(|d| d.code == "vcs_unavailable"));
        }
        assert!(!env.schema.is_empty());
        assert_eq!(env.project.version, crate::VERSION);
    }

    #[test]
    fn every_named_field_of_the_snapshot_can_say_where_it_came_from() {
        let repo = repository();
        let env = full(&repo);
        for field in [
            "project.version",
            "project.summary",
            "project.commit",
            "repository.name",
            "repository.sections",
            "layer.objects",
            "workflows",
            "toolchains",
            "providers",
            "services.url",
        ] {
            let source = env
                .explain(field)
                .unwrap_or_else(|| panic!("{field} has no provenance"));
            assert!(!source.source.is_empty(), "{field} names no source");
            assert!(!source.resolver.is_empty(), "{field} names no resolver");
        }
    }
}
