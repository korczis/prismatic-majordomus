//! The registry: every capability, once, validated, in a deterministic order, with the
//! lookups projections need. Built at one place per process from the builtin executables
//! and the repository's declarative objects; nothing else adds to it.
//!
//! The lifecycle is three steps and there is no fourth: a [`Builder`] is handed the
//! sources, [`Builder::build`] checks every invariant and collects every reason it could
//! not, and what comes out is immutable for the life of the process. Nothing registers
//! itself, so "every capability, once" is a property of one function rather than of a
//! startup order, and a projection that wants a lookup asks for one here instead of
//! keeping a table beside its renderer.
//!
//! ```
//! use majordomus_cli::capability::{builtin, CapabilityRegistry, HttpMethod};
//!
//! let registry = CapabilityRegistry::builder()
//!     .with_modules(builtin::modules())
//!     .build()
//!     .expect("the composed application is valid");
//!
//! // every lookup answers with the one descriptor
//! let by_id = registry.get("repository.info").unwrap();
//! let by_route = registry.by_http(HttpMethod::Get, "/api/v1/repository").unwrap();
//! assert_eq!(by_id.id, by_route.id);
//!
//! // and the collection is in id order, whoever composed it and in whatever order
//! let ids: Vec<&str> = registry.iter().map(|c| c.id.as_str()).collect();
//! let mut sorted = ids.clone();
//! sorted.sort();
//! assert_eq!(ids, sorted, "iteration is id order");
//! assert_eq!(registry.len(), ids.len());
//! ```

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::index::Index;

use super::benchmark::CaseProvider;
use super::handler::{CapabilityError, Context, Executable, Handler};
use super::model::{
    Availability, BenchmarkPolicy, Capability, CapabilityId, CapabilityKind, ExecutionPolicy,
    HttpMethod, McpResource, ModuleId, Provenance, Stability, Visibility, WaiverReason,
};
use super::module::ModuleDescriptor;

/// A capability with, for an executable, its handler and its benchmark cases.
///
/// The registry's own bookkeeping. The handler and the case provider are private and
/// nothing outside this module can build one or be handed one: a caller asks for the
/// descriptor with [`CapabilityRegistry::get`], for the cases with
/// [`CapabilityRegistry::cases`], and for the behaviour by calling
/// [`CapabilityRegistry::dispatch`], because a handler that could be taken out of the
/// registry could be called without the counters, the cache and the validation that make
/// the registry the one execution path.
pub struct Entry {
    /// The descriptor.
    pub capability: Capability,
    handler: Option<Arc<dyn Handler>>,
    cases: Option<CaseProvider>,
}

/// Where a module's entry in the registry came from.
///
/// The distinction is worth recording because only the first kind describes itself: a
/// module composed with `module!` brought its own title, paragraph and stability, and the
/// other two are named after a namespace or a kind and given a sentence the registry
/// writes for them. A reader of the generated reference can then tell a module somebody
/// designed from a bucket that happened to exist.
///
/// ```
/// use majordomus_cli::capability::{builtin, registry::ModuleSource, CapabilityRegistry};
/// let composed = CapabilityRegistry::builder().with_modules(builtin::modules()).build().unwrap();
/// assert_eq!(composed.module("repository").unwrap().source, ModuleSource::Builtin);
///
/// // the same executables without their descriptors: the module is derived from the
/// // namespace of the ids, and says so rather than claiming to have been declared
/// let derived = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
/// assert_eq!(derived.module("repository").unwrap().source, ModuleSource::Derived);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ModuleSource {
    /// Declared with `module!` and composed with `compose_modules!`.
    Builtin,
    /// Derived from the namespace of executables composed without a module descriptor.
    Derived,
    /// One kind of declarative objects of the repository's layer.
    Declarative,
}

/// One module as the registry knows it.
///
/// The capability count is the module's own, not a filter over the registry run at render
/// time, so a listing of modules costs nothing and cannot disagree with the collection it
/// is a summary of. Stability is optional because only a composed module declares one.
///
/// ```
/// use majordomus_cli::capability::{builtin, CapabilityRegistry};
/// use majordomus_cli::capability::registry::ModuleInfo;
/// let registry = CapabilityRegistry::builder().with_modules(builtin::modules()).build().unwrap();
/// let info: &ModuleInfo = registry.module("objects").unwrap();
/// assert_eq!(info.id.as_str(), "objects");
/// assert!(info.stability.is_some(), "a composed module says where it stands");
/// assert_eq!(
///     info.capabilities,
///     registry.iter().filter(|c| c.module.as_str() == "objects").count()
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ModuleInfo {
    /// The identity.
    pub id: ModuleId,
    /// The short name.
    pub title: String,
    /// One paragraph.
    pub description: String,
    /// Where the module stands, when it declared it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stability: Option<Stability>,
    /// Where the entry came from.
    pub source: ModuleSource,
    /// How many capabilities it composes.
    pub capabilities: usize,
}

impl std::fmt::Debug for Entry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Entry")
            .field("capability", &self.capability)
            .field("handler", &self.handler.is_some())
            .finish()
    }
}

/// Why the registry could not be built. Every variant names the provenance of every party.
///
/// Naming both parties is the whole design of this type. "Duplicate id" tells a reader
/// nothing they can act on; "this id, declared here and there" is a diff away from fixed,
/// and it is the reason [`Builder::build`] collects every error instead of returning the
/// first one it met.
///
/// ```
/// use majordomus_cli::capability::{builtin, CapabilityRegistry, RegistryError};
/// // the same executables composed twice: every id is claimed a second time
/// let errors = CapabilityRegistry::builder()
///     .with_builtin(builtin::all())
///     .with_builtin(builtin::all())
///     .build()
///     .unwrap_err();
/// assert_eq!(errors.len(), builtin::all().len(), "one error per id, not one for the batch");
/// let RegistryError::DuplicateId { id, first, second } = &errors[0] else {
///     panic!("a second claim on an id is a DuplicateId")
/// };
/// assert_eq!(first, second, "here the two claimants are the same declaration");
/// assert!(errors[0].to_string().contains(id.as_str()), "the message names the id");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum RegistryError {
    #[error("capability '{id}' ({provenance}): invalid id: {reason}")]
    /// The id does not satisfy the grammar.
    InvalidId {
        /// The id as written.
        id: String,
        /// Where the descriptor came from.
        provenance: String,
        /// What the grammar objects to.
        reason: String,
    },
    #[error("capability '{id}' is defined twice: {first} and {second}")]
    /// Two descriptors claim one id.
    DuplicateId {
        /// The id.
        id: String,
        /// The provenance of the descriptor seen first, in id order.
        first: String,
        /// The provenance of the other.
        second: String,
    },
    #[error("MCP tool '{name}' is claimed by '{first}' and '{second}'")]
    /// Two capabilities claim one MCP tool name.
    DuplicateMcpName {
        /// The tool name.
        name: String,
        /// The id of the capability seen first.
        first: String,
        /// The id of the other.
        second: String,
    },
    #[error("MCP resource '{uri}' is claimed by '{first}' and '{second}'")]
    /// Two capabilities claim one MCP resource URI.
    DuplicateMcpUri {
        /// The URI.
        uri: String,
        /// The id of the capability seen first.
        first: String,
        /// The id of the other.
        second: String,
    },
    #[error("HTTP route {method} {path} is claimed by '{first}' and '{second}'")]
    /// Two capabilities claim one HTTP method and path.
    DuplicateHttpRoute {
        /// The method.
        method: String,
        /// The path.
        path: String,
        /// The id of the capability seen first.
        first: String,
        /// The id of the other.
        second: String,
    },
    #[error("CLI path '{path}' is claimed by '{first}' and '{second}'")]
    /// Two capabilities claim one CLI path.
    DuplicateCliPath {
        /// The words, joined with spaces.
        path: String,
        /// The id of the capability seen first.
        first: String,
        /// The id of the other.
        second: String,
    },
    #[error("capability '{id}' ({provenance}): invalid {projection} exposure: {reason}")]
    /// A declared exposure cannot be served by its projection.
    InvalidExposure {
        /// The id.
        id: String,
        /// Where the descriptor came from.
        provenance: String,
        /// `MCP`, `HTTP` or `CLI`.
        projection: String,
        /// What is wrong with the exposure.
        reason: String,
    },
    #[error("capability '{id}' ({provenance}) is {stability} and cannot be exposed as executable through {projection}")]
    /// A planned or unsupported capability declares an executable exposure.
    NotExecutable {
        /// The id.
        id: String,
        /// Where the descriptor came from.
        provenance: String,
        /// The stability that forbids execution.
        stability: String,
        /// The projection the exposure is for.
        projection: String,
    },
    #[error("module '{id}': invalid id: {reason}")]
    /// A module id does not satisfy the grammar.
    InvalidModuleId {
        /// The id as written.
        id: String,
        /// What the grammar objects to.
        reason: String,
    },
    #[error("module '{id}' is composed twice ({first} and {second})")]
    /// Two modules claim one id, or a builtin module's id is a declarative kind.
    DuplicateModule {
        /// The id.
        id: String,
        /// The source seen first.
        first: String,
        /// The other.
        second: String,
    },
    #[error("capability '{id}' ({provenance}) is composed in module '{module}' but its namespace is '{namespace}'")]
    /// A capability's id does not belong to the module that composes it.
    ModuleMismatch {
        /// The capability id.
        id: String,
        /// Where the descriptor came from.
        provenance: String,
        /// The module that composed it.
        module: String,
        /// The id's namespace.
        namespace: String,
    },
    #[error("capability '{id}' ({provenance}): invalid cache policy: {reason}")]
    /// The cache policy keeps nothing or contradicts itself.
    InvalidCachePolicy {
        /// The id.
        id: String,
        /// Where the descriptor came from.
        provenance: String,
        /// What is wrong.
        reason: String,
    },
    #[error("capability '{id}' ({provenance}): {reason}")]
    /// The descriptor's shape contradicts its kind: a query without a handler, a resource with one or with a callable exposure.
    Shape {
        /// The id.
        id: String,
        /// Where the descriptor came from.
        provenance: String,
        /// What is contradictory.
        reason: String,
    },
}

#[derive(Debug, Default)]
/// Every capability, once, validated, in id order, with the lookups projections need.
///
/// A value, not a service: composed once per process and immutable afterwards, which is
/// what lets every transport share one by `Arc` and answer a request without rebuilding
/// canonical state. The lookup tables are built during validation, which is also where a
/// second claim on a tool name, a route or a command path is refused — so a projection
/// never has to decide what to do about two capabilities that both say they are `GET
/// /api/v1/health`.
///
/// ```
/// use majordomus_cli::capability::{builtin, CapabilityRegistry, HttpMethod};
/// let registry = CapabilityRegistry::builder().with_modules(builtin::modules()).build().unwrap();
///
/// // every exposure any descriptor declares resolves back to the descriptor that
/// // declared it: the tables are a reading of the collection, not a second opinion
/// for c in registry.iter() {
///     if let Some(http) = &c.exposure.http {
///         assert_eq!(registry.by_http(http.method, &http.path).unwrap().id, c.id);
///     }
///     if let Some(cli) = &c.exposure.cli {
///         assert_eq!(registry.by_cli(&cli.path).unwrap().id, c.id);
///     }
///     if let Some(tool) = c.exposure.mcp.as_ref().and_then(|m| m.tool.as_ref()) {
///         assert_eq!(registry.by_mcp_tool(tool).unwrap().id, c.id);
///     }
/// }
/// assert!(registry.by_http(HttpMethod::Get, "/api/v1/nothing-declares-this").is_none());
/// ```
pub struct CapabilityRegistry {
    entries: BTreeMap<CapabilityId, Entry>,
    modules: BTreeMap<ModuleId, ModuleInfo>,
    fingerprint: String,
    by_mcp_tool: BTreeMap<String, CapabilityId>,
    by_mcp_uri: BTreeMap<String, CapabilityId>,
    by_http: BTreeMap<(HttpMethod, String), CapabilityId>,
    by_cli: BTreeMap<Vec<String>, CapabilityId>,
}

/// The one composition point: the sources of capabilities go in, every invariant is
/// checked, and either a registry or the full list of reasons comes out.
///
/// Nothing registers itself into it. A builder is handed the modules the application
/// composes and, where there is a repository to read, its index; whatever is not handed to
/// a builder is not in the registry. That is what makes the composition auditable by
/// reading one function rather than by reasoning about which module was linked first.
///
/// ```
/// use majordomus_cli::capability::{builtin, registry::Builder};
/// let registry = Builder::default().with_modules(builtin::modules()).build().unwrap();
/// assert!(registry.get("repository.info").is_some());
/// // and a builder handed nothing builds an empty registry rather than failing
/// assert!(Builder::default().build().unwrap().is_empty());
/// ```
#[derive(Default)]
pub struct Builder {
    pending: Vec<Entry>,
    modules: Vec<ModuleInfo>,
    index_fingerprint: String,
}

impl Builder {
    /// Add the application's modules, as `compose_modules!` produced them: their metadata
    /// and every executable they compose.
    ///
    /// The metadata is the difference between this and [`Builder::with_builtin`]. A module
    /// added here describes itself, and the registry can therefore refuse a capability
    /// whose id namespace is not the module that composed it — a check that has nothing to
    /// compare against when the module was only inferred.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, registry::Builder};
    /// let registry = Builder::default().with_modules(builtin::modules()).build().unwrap();
    /// let module = registry.module("health").unwrap();
    /// assert!(!module.description.is_empty(), "a composed module describes itself");
    /// // every capability was stamped with the module that composed it
    /// assert!(registry.iter().all(|c| !c.module.as_str().is_empty()));
    /// ```
    pub fn with_modules(mut self, modules: Vec<ModuleDescriptor>) -> Self {
        for m in modules {
            self.modules.push(ModuleInfo {
                id: m.id.clone(),
                title: m.title,
                description: m.description,
                stability: Some(m.stability),
                source: ModuleSource::Builtin,
                capabilities: m.capabilities.len(),
            });
            self = self.with_builtin(m.capabilities);
        }
        self
    }

    /// Add executables without a module descriptor; each one's module is the namespace of
    /// its id.
    ///
    /// What a test or a benchmark wants: the behaviour of the application without the
    /// module metadata that only a listing needs. The modules still exist in the registry,
    /// derived from the namespaces and marked as derived, because a capability with no
    /// module at all would be a capability no listing could file.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, registry::ModuleSource, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// assert_eq!(registry.module("objects").unwrap().source, ModuleSource::Derived);
    /// assert_eq!(registry.get("objects.get").unwrap().module.as_str(), "objects");
    /// ```
    pub fn with_builtin(mut self, executables: Vec<Executable>) -> Self {
        for e in executables {
            self.pending.push(Entry {
                capability: e.capability,
                handler: Some(e.handler),
                cases: Some(e.cases),
            });
        }
        self
    }

    /// Every object of the index becomes a resource capability; the index's fingerprint
    /// joins the registry's.
    ///
    /// This is where the repository's own content enters the model, and it enters as
    /// capabilities rather than as a second collection beside them: a rule and
    /// `repository.info` are looked up, listed, counted and served through one registry.
    /// Folding the index's fingerprint into the registry's is what makes a cached answer
    /// from a process that read a different repository state impossible to hand back.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityKind, CapabilityRegistry};
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let index = repo.index().unwrap();
    /// let registry = CapabilityRegistry::builder()
    ///     .with_modules(builtin::modules())
    ///     .with_index(&index)
    ///     .build()
    ///     .unwrap();
    ///
    /// // one resource per object of the layer, and nothing else became one
    /// let resources = registry.iter().filter(|c| c.kind == CapabilityKind::Resource).count();
    /// assert_eq!(resources, index.objects.len());
    /// assert!(!registry.fingerprint().is_empty(), "the state it was read from is recorded");
    /// ```
    pub fn with_index(mut self, index: &Index) -> Self {
        self.index_fingerprint = index.fingerprint.clone();
        for object in &index.objects {
            self.pending.push(Entry {
                capability: super::declarative::capability_of(object),
                handler: None,
                cases: None,
            });
        }
        self
    }

    /// Validate every invariant and build. Errors are collected, not stopped at the first,
    /// and reported in a deterministic order.
    ///
    /// Collecting is deliberate: the first error in a composition of several hundred
    /// capabilities is rarely the interesting one, and a build that reported it alone would
    /// be run once per mistake. The order is a function of the ids and the provenances
    /// rather than of the order the sources were added, so two runs over one tree produce
    /// the same list and a diff of it means something.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, registry::Builder, RegistryError};
    /// // the application composes cleanly, which is the assertion that matters most here
    /// let registry = Builder::default().with_modules(builtin::modules()).build().unwrap();
    /// assert!(registry.len() >= registry.modules().count());
    ///
    /// // and a module composed twice is a named error rather than a panic or a silent win
    /// let refused = Builder::default()
    ///     .with_modules(builtin::modules())
    ///     .with_modules(builtin::modules())
    ///     .build()
    ///     .unwrap_err();
    /// assert!(refused.iter().any(|e| matches!(e, RegistryError::DuplicateModule { .. })));
    /// ```
    pub fn build(self) -> Result<CapabilityRegistry, Vec<RegistryError>> {
        let _phase = crate::perf::phase(crate::perf::Phase::RegistryBuild);
        crate::perf::Counters::bump(&crate::perf::COUNTERS.registry_builds);
        let mut errors = Vec::new();
        let mut registry = CapabilityRegistry::default();
        for m in self.modules {
            let id = m.id.as_str().to_string();
            if let Err(reason) = ModuleId::parse(&id) {
                errors.push(RegistryError::InvalidModuleId { id, reason });
                continue;
            }
            if let Some(first) = registry.modules.get(&m.id) {
                errors.push(RegistryError::DuplicateModule {
                    id,
                    first: format!("{:?}", first.source).to_lowercase(),
                    second: "builtin".into(),
                });
                continue;
            }
            registry.modules.insert(m.id.clone(), m);
        }
        let mut pending = self.pending;
        pending.sort_by(|a, b| {
            a.capability.id.cmp(&b.capability.id).then_with(|| {
                a.capability
                    .provenance
                    .to_string()
                    .cmp(&b.capability.provenance.to_string())
            })
        });
        for mut entry in pending {
            let c = &entry.capability;
            let id = c.id.as_str().to_string();
            let prov = c.provenance.to_string();
            if let Err(reason) = CapabilityId::parse(&id) {
                errors.push(RegistryError::InvalidId {
                    id: id.clone(),
                    provenance: prov.clone(),
                    reason,
                });
                continue;
            }
            if let Some(first) = registry.entries.get(&c.id) {
                errors.push(RegistryError::DuplicateId {
                    id: id.clone(),
                    first: first.capability.provenance.to_string(),
                    second: prov.clone(),
                });
                continue;
            }
            // the module: stamped by `module!`, derived from the namespace otherwise
            let namespace = c.id.namespace().to_string();
            if entry.capability.module.as_str().is_empty() {
                entry.capability.module = ModuleId::unchecked(&namespace);
            }
            let c = &entry.capability;
            let is_builtin = matches!(c.provenance, Provenance::Builtin { .. });
            if is_builtin && c.module.as_str() != namespace {
                errors.push(RegistryError::ModuleMismatch {
                    id: id.clone(),
                    provenance: prov.clone(),
                    module: c.module.as_str().to_string(),
                    namespace,
                });
                continue;
            }
            match registry.modules.get_mut(&c.module) {
                Some(m) if m.source == ModuleSource::Builtin && is_builtin => {}
                Some(m) if m.source == ModuleSource::Builtin => {
                    errors.push(RegistryError::DuplicateModule {
                        id: c.module.as_str().to_string(),
                        first: "builtin".into(),
                        second: format!("declarative kind of {prov}"),
                    });
                    continue;
                }
                Some(m) => m.capabilities += 1,
                None => {
                    if let Err(reason) = ModuleId::parse(c.module.as_str()) {
                        errors.push(RegistryError::InvalidModuleId {
                            id: c.module.as_str().to_string(),
                            reason,
                        });
                        continue;
                    }
                    let (source, title, description) = if is_builtin {
                        (
                            ModuleSource::Derived,
                            c.module.as_str().to_string(),
                            format!("Executables in the '{}' namespace, composed without a module descriptor.", c.module),
                        )
                    } else {
                        (
                            ModuleSource::Declarative,
                            c.module.as_str().to_string(),
                            format!(
                                "Declarative objects of kind '{}' from the repository's AI layer.",
                                c.module
                            ),
                        )
                    };
                    registry.modules.insert(
                        c.module.clone(),
                        ModuleInfo {
                            id: c.module.clone(),
                            title,
                            description,
                            stability: None,
                            source,
                            capabilities: 1,
                        },
                    );
                }
            }
            if let Err(reason) = c.cache.validate() {
                errors.push(RegistryError::InvalidCachePolicy {
                    id: id.clone(),
                    provenance: prov.clone(),
                    reason,
                });
                continue;
            }
            // classification is not a value a descriptor may hold an opinion about: it
            // follows from the kind and the transports it declares, and a capability
            // arriving from outside this crate with a different one is refused rather
            // than believed. A projection reads these fields; if they could drift, every
            // surface downstream would be reading a claim instead of a fact.
            let availability = Availability::classify(c.kind, &c.exposure);
            let visibility = Visibility::classify(&c.exposure);
            if c.availability != availability || c.visibility != visibility {
                errors.push(RegistryError::Shape {
                    id: id.clone(),
                    provenance: prov.clone(),
                    reason: format!(
                        "declares availability {:?} and visibility {:?}; what it exposes makes it {availability:?} and {visibility:?}. Classification follows the kind and the transports, so change those or let the declaration classify itself",
                        c.availability, c.visibility
                    ),
                });
                continue;
            }
            match (c.kind, c.benchmark) {
                (
                    CapabilityKind::Resource,
                    BenchmarkPolicy::Waived {
                        reason: WaiverReason::NotExecutable,
                    },
                ) => {}
                (CapabilityKind::Resource, _) => {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason: "a resource is not a benchmark target: its policy is waived as not executable".into(),
                    });
                    continue;
                }
                (
                    _,
                    BenchmarkPolicy::Waived {
                        reason: WaiverReason::NotExecutable,
                    },
                ) => {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason:
                            "an executable cannot be waived as not executable; name a real reason"
                                .into(),
                    });
                    continue;
                }
                _ => {}
            }
            if c.cache.is_enabled() && c.kind == CapabilityKind::Command {
                errors.push(RegistryError::InvalidCachePolicy {
                    id: id.clone(),
                    provenance: prov.clone(),
                    reason: "a command changes state and is never cached".into(),
                });
                continue;
            }
            // classification is the registry's, not the declaration's, for the same reason
            // availability and visibility are: a descriptor arriving from outside this
            // crate with a policy of its own would be a claim a projection then reads as a
            // fact
            let policy = ExecutionPolicy::classify(c.kind);
            if c.execution != policy && c.execution != policy.stoppable() {
                errors.push(RegistryError::Shape {
                    id: id.clone(),
                    provenance: prov.clone(),
                    reason: format!(
                        "declares the execution policy {:?}; its kind makes it {policy:?}, and the only thing a declaration may add is cancellation (`.cancellable()`). The effect and the concurrency follow the kind",
                        c.execution
                    ),
                });
                continue;
            }
            match (c.kind, entry.handler.is_some()) {
                (CapabilityKind::Query, false) => {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason: "a query needs a handler".into(),
                    });
                    continue;
                }
                (CapabilityKind::Command, false) => {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason: "a command needs a handler".into(),
                    });
                    continue;
                }
                (CapabilityKind::Resource, true) => {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason: "a resource has no handler".into(),
                    });
                    continue;
                }
                _ => {}
            }
            if let Some(mcp) = &c.exposure.mcp {
                if let Some(tool) = &mcp.tool {
                    if !c.stability.executable() {
                        errors.push(not_executable(c, "MCP tool"));
                    }
                    if tool.is_empty()
                        || !tool
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    {
                        errors.push(RegistryError::InvalidExposure {
                            id: id.clone(),
                            provenance: prov.clone(),
                            projection: "MCP".into(),
                            reason: format!("tool name '{tool}' is not [a-z0-9_]+"),
                        });
                    } else if let Some(first) = registry.by_mcp_tool.get(tool) {
                        errors.push(RegistryError::DuplicateMcpName {
                            name: tool.clone(),
                            first: first.to_string(),
                            second: id.clone(),
                        });
                    } else {
                        registry.by_mcp_tool.insert(tool.clone(), c.id.clone());
                    }
                }
                if let Some(res) = &mcp.resource {
                    if !res.uri.starts_with("majordomus://")
                        || res.uri.contains(char::is_whitespace)
                    {
                        errors.push(RegistryError::InvalidExposure {
                            id: id.clone(),
                            provenance: prov.clone(),
                            projection: "MCP".into(),
                            reason: format!(
                                "resource uri '{}' is not a majordomus:// uri",
                                res.uri
                            ),
                        });
                    } else if let Some(first) = registry.by_mcp_uri.get(&res.uri) {
                        errors.push(RegistryError::DuplicateMcpUri {
                            uri: res.uri.clone(),
                            first: first.to_string(),
                            second: id.clone(),
                        });
                    } else {
                        registry.by_mcp_uri.insert(res.uri.clone(), c.id.clone());
                    }
                    // a read supplies no input: a query read as a resource must accept `{}`
                    let (_, required) = c.input.properties();
                    if c.kind.is_executable() && !required.is_empty() {
                        errors.push(RegistryError::Shape {
                            id: id.clone(),
                            provenance: prov.clone(),
                            reason: format!(
                                "a query read as the resource '{}' is called with no input, and its input requires {}",
                                res.uri,
                                required.join(", ")
                            ),
                        });
                    }
                }
            }
            if let Some(http) = &c.exposure.http {
                if !c.stability.executable() {
                    errors.push(not_executable(c, "HTTP"));
                }
                if let Err(reason) = http.validate() {
                    errors.push(RegistryError::InvalidExposure {
                        id: id.clone(),
                        provenance: prov.clone(),
                        projection: "HTTP".into(),
                        reason,
                    });
                } else if let Some(first) = registry.by_http.get(&(http.method, http.path.clone()))
                {
                    errors.push(RegistryError::DuplicateHttpRoute {
                        method: http.method.as_str().into(),
                        path: http.path.clone(),
                        first: first.to_string(),
                        second: id.clone(),
                    });
                } else {
                    registry
                        .by_http
                        .insert((http.method, http.path.clone()), c.id.clone());
                }
            }
            if let Some(cli) = &c.exposure.cli {
                if !c.stability.executable() {
                    errors.push(not_executable(c, "CLI"));
                }
                if cli.path.is_empty()
                    || cli
                        .path
                        .iter()
                        .any(|w| w.is_empty() || w.contains(char::is_whitespace))
                {
                    errors.push(RegistryError::InvalidExposure {
                        id: id.clone(),
                        provenance: prov.clone(),
                        projection: "CLI".into(),
                        reason: format!("path {:?} has an empty or blank word", cli.path),
                    });
                } else if let Some(first) = registry.by_cli.get(&cli.path) {
                    errors.push(RegistryError::DuplicateCliPath {
                        path: cli.path.join(" "),
                        first: first.to_string(),
                        second: id.clone(),
                    });
                } else {
                    registry.by_cli.insert(cli.path.clone(), c.id.clone());
                }
            }
            if c.kind == CapabilityKind::Resource
                && (c.exposure.http.is_some()
                    || c.exposure.cli.is_some()
                    || c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()))
            {
                errors.push(RegistryError::Shape {
                    id: id.clone(),
                    provenance: prov.clone(),
                    reason: "a resource is read, not called: only an MCP resource exposure applies"
                        .into(),
                });
            }
            if c.kind == CapabilityKind::Command {
                let word = c.kind.as_str();
                if c.exposure
                    .mcp
                    .as_ref()
                    .is_some_and(|m| m.resource.is_some())
                {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason: format!(
                            "a {word} is called, not read: it has no MCP resource exposure"
                        ),
                    });
                }
                if let Some(http) = &c.exposure.http {
                    if http.method != HttpMethod::Post {
                        errors.push(RegistryError::InvalidExposure {
                            id: id.clone(),
                            provenance: prov.clone(),
                            projection: "HTTP".into(),
                            reason: format!(
                                "a {word} changes state and is bound to POST, not {}",
                                http.method.as_str()
                            ),
                        });
                    }
                }
            }
            registry.entries.insert(c.id.clone(), entry);
        }
        if errors.is_empty() {
            registry.fingerprint = fingerprint(&self.index_fingerprint, &registry);
            Ok(registry)
        } else {
            Err(errors)
        }
    }
}

/// The registry's fingerprint: a hash of the index's fingerprint (every declarative
/// object's path and content), of every id, and of every builtin descriptor in full, in
/// id order. Stable across processes for the same code and the same repository state.
fn fingerprint(index: &str, registry: &CapabilityRegistry) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(index.as_bytes());
    for c in registry.iter() {
        h.update(c.id.as_str().as_bytes());
        h.update(b"\0");
        // a declarative descriptor is a function of the object the index fingerprint
        // already covers; hashing its (large, shared) schema per object would only cost
        if matches!(c.provenance, Provenance::Builtin { .. }) {
            h.update(serde_json::to_string(c).unwrap_or_default().as_bytes());
        }
        h.update(b"\n");
    }
    format!("{:x}", h.finalize())
}

fn not_executable(c: &Capability, projection: &str) -> RegistryError {
    RegistryError::NotExecutable {
        id: c.id.to_string(),
        provenance: c.provenance.to_string(),
        stability: serde_json::to_value(c.stability)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
        projection: projection.into(),
    }
}

impl CapabilityRegistry {
    /// Start composing a registry: the one entry point, so that a reader looking for
    /// where the capabilities of a process come from has one place to look.
    ///
    /// Equivalent to [`Builder::default`], and named on the registry because that is where
    /// somebody asking the question is already standing.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry, RegistryError};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// assert!(registry.get("repository.info").is_some());
    /// assert_eq!(registry.by_mcp_tool("majordomus_get").unwrap().id.as_str(), "objects.get");
    /// // the same executables twice: every id is claimed twice, and the registry says so
    /// let twice = CapabilityRegistry::builder().with_builtin(builtin::all()).with_builtin(builtin::all()).build();
    /// let errors = twice.unwrap_err();
    /// assert!(errors.iter().all(|e| matches!(e, RegistryError::DuplicateId { .. })));
    /// ```
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Every capability the registry holds, in canonical id order.
    ///
    /// The order is the collection's, not the caller's: it comes from the map the entries
    /// live in, so a generated document, a listing and a fingerprint taken over this
    /// iterator are the same sequence in every process. A projection that sorts the result
    /// of this is a second opinion about an order that already exists.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// let ids: Vec<&str> = registry.iter().map(|c| c.id.as_str()).collect();
    /// let mut sorted = ids.clone();
    /// sorted.sort();
    /// assert_eq!(ids, sorted);
    /// assert_eq!(ids.len(), registry.len());
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.entries.values().map(|e| &e.capability)
    }

    /// How many capabilities the registry holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Does the registry hold no capability at all? True only of one composed from no
    /// modules, no executables and no index, which is what a bare builder produces.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every module, by id: the composed ones, the derived ones, and one per declarative kind.
    pub fn modules(&self) -> impl Iterator<Item = &ModuleInfo> {
        self.modules.values()
    }

    /// One module by identity, or `None` when no capability of the registry belongs to it.
    ///
    /// A module exists here because something is filed under it, whether it was composed
    /// with `module!`, derived from a namespace, or is a kind of declarative object. There
    /// is no way to declare an empty module, and so no way for a listing to show one.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_modules(builtin::modules()).build().unwrap();
    /// let module = registry.module("quality").unwrap();
    /// assert_eq!(module.id.as_str(), "quality");
    /// assert!(registry.module("no-such-module").is_none());
    /// ```
    pub fn module(&self, id: &str) -> Option<&ModuleInfo> {
        self.modules.get(&ModuleId::unchecked(id))
    }

    /// The benchmark case provider of an executable; `None` for a resource or an unknown id.
    ///
    /// The provider, not the cases: the inputs depend on the repository being measured — a
    /// case for `objects.get` has to name an object that exists — so the registry hands
    /// back the function and the benchmark runner supplies the context it is called with.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityKind, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// assert!(registry.cases("repository.info").is_some());
    /// assert!(registry.cases("no.such-capability").is_none());
    /// // every executable has one, because the `capability!` macro cannot expand without it
    /// for c in registry.iter().filter(|c| c.kind.is_executable()) {
    ///     assert!(registry.cases(c.id.as_str()).is_some(), "{} has no cases", c.id);
    /// }
    /// ```
    pub fn cases(&self, id: &str) -> Option<CaseProvider> {
        self.entries
            .get(&CapabilityId::unchecked(id))
            .and_then(|e| e.cases)
    }

    /// A capability by canonical id: the lookup every other one resolves through.
    ///
    /// The id is taken as text and compared, never parsed, so a caller holding an id from
    /// a URL, a tool argument or a generated document asks with what it has. An id that
    /// does not exist is `None` rather than an error, because "is there such a capability"
    /// is a question several projections ask before they answer their own caller.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// assert_eq!(registry.get("repository.info").unwrap().id.as_str(), "repository.info");
    /// assert!(registry.get("repository").is_none(), "not an id, and not an error either");
    /// assert!(registry.get("").is_none());
    /// ```
    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.entries
            .get(&CapabilityId::unchecked(id))
            .map(|e| &e.capability)
    }

    /// The capability an MCP tool name projects.
    ///
    /// The table is built while the registry is validated, which is where a second
    /// capability claiming one tool name is refused; by the time a session can ask this
    /// question, the answer is unambiguous or there is no registry.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// let c = registry.by_mcp_tool("majordomus_repository").unwrap();
    /// assert_eq!(c.exposure.mcp.as_ref().unwrap().tool.as_deref(), Some("majordomus_repository"));
    /// assert!(registry.by_mcp_tool("repository.info").is_none(), "a tool name is not an id");
    /// ```
    pub fn by_mcp_tool(&self, name: &str) -> Option<&Capability> {
        self.by_mcp_tool
            .get(name)
            .and_then(|id| self.get(id.as_str()))
    }

    /// The capability an MCP resource URI projects, with the resource exposure it matched:
    /// a declarative object, or a query read as a document (`majordomus://repository`).
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityKind, CapabilityRegistry};
    /// let r = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// let (c, res) = r.by_mcp_uri(builtin::REPOSITORY_URI).unwrap();
    /// assert_eq!((c.id.as_str(), c.kind, res.name.as_str()), ("repository.info", CapabilityKind::Query, "repository"));
    /// assert!(r.by_mcp_uri("majordomus://rule/none@1").is_none());
    /// ```
    pub fn by_mcp_uri(&self, uri: &str) -> Option<(&Capability, &McpResource)> {
        let c = self
            .by_mcp_uri
            .get(uri)
            .and_then(|id| self.get(id.as_str()))?;
        let res = c.exposure.mcp.as_ref()?.resource.as_ref()?;
        Some((c, res))
    }

    /// The capability an HTTP method and path project.
    ///
    /// The method is part of the key, so one path may be a read and a call and they are
    /// two capabilities rather than one with a branch inside it. The path is matched whole:
    /// this projection has no path parameters, because a parameter in a route is an input
    /// that the canonical input schema would then not describe.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry, HttpMethod};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// let c = registry.by_http(HttpMethod::Get, "/api/v1/repository").unwrap();
    /// assert_eq!(c.id.as_str(), "repository.info");
    /// assert!(registry.by_http(HttpMethod::Post, "/api/v1/repository").is_none(), "the method is part of the key");
    /// assert!(registry.by_http(HttpMethod::Get, "/api/v1/repository/").is_none(), "matched whole");
    /// ```
    pub fn by_http(&self, method: HttpMethod, path: &str) -> Option<&Capability> {
        self.by_http
            .get(&(method, path.to_string()))
            .and_then(|id| self.get(id.as_str()))
    }

    /// The capability a CLI path projects.
    ///
    /// The words, as the command tree walks them, and not a joined line: the key is the
    /// sequence, so `capabilities list` and a single argument spelled `"capabilities list"`
    /// are different questions and only the first is a command.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_builtin(builtin::all()).build().unwrap();
    /// let path = vec!["capabilities".to_string(), "list".to_string()];
    /// let c = registry.by_cli(&path).unwrap();
    /// assert_eq!(c.exposure.cli.as_ref().unwrap().path, path);
    /// assert!(registry.by_cli(&["capabilities list".to_string()]).is_none());
    /// assert!(registry.by_cli(&["capabilities".to_string()]).is_none(), "a prefix is not a command");
    /// ```
    pub fn by_cli(&self, path: &[String]) -> Option<&Capability> {
        self.by_cli.get(path).and_then(|id| self.get(id.as_str()))
    }

    /// The fingerprint of this registry and the repository state it was built from.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Run the handler of an executable by id, with no counters and no cache: the raw
    /// dispatch the executor wraps. Everything else calls [`Context::execute`]. A
    /// resource, or an unknown id, is `NotFound`.
    ///
    /// ```
    /// use majordomus_cli::capability::CapabilityError;
    /// use majordomus_cli::synthetic::SyntheticRepository;
    ///
    /// let repo = SyntheticRepository::small().unwrap();
    /// let ctx = repo.context().unwrap();
    /// let out = ctx.registry.dispatch(&ctx, "repository.info", serde_json::json!({})).unwrap();
    /// // the handler read the context it was given, and nothing else could have answered
    /// assert_eq!(out["objects"], ctx.index.objects.len());
    ///
    /// // an id nothing declares, and a resource, are both refusals rather than panics
    /// let missing = ctx.registry.dispatch(&ctx, "no.such-capability", serde_json::json!({}));
    /// assert!(matches!(missing, Err(CapabilityError::NotFound(_))));
    /// ```
    pub fn dispatch(
        &self,
        ctx: &Context,
        id: &str,
        input: Value,
    ) -> Result<Value, CapabilityError> {
        let entry = self
            .entries
            .get(&CapabilityId::unchecked(id))
            .ok_or_else(|| CapabilityError::NotFound(format!("capability {id}")))?;
        let handler = entry.handler.as_ref().ok_or_else(|| {
            CapabilityError::NotFound(format!("capability {id} is a resource, not a query"))
        })?;
        tracing::debug!(capability_id = id, "call");
        handler.call(ctx, input)
    }

    /// Counts by kind, by stability and by projection, for introspection.
    ///
    /// Computed by one walk over the collection every time it is asked for, rather than
    /// maintained as the registry is built: a counter kept beside a collection is a second
    /// copy of the collection's size, and it is the copy that goes wrong.
    ///
    /// ```
    /// use majordomus_cli::capability::{builtin, CapabilityRegistry};
    /// let registry = CapabilityRegistry::builder().with_modules(builtin::modules()).build().unwrap();
    /// let summary = registry.summary();
    /// assert_eq!(summary.total, registry.len());
    /// assert_eq!(summary.builtin + summary.declarative, summary.total);
    /// assert_eq!(summary.by_kind.values().sum::<usize>(), summary.total);
    /// assert_eq!(summary.modules, registry.modules().count());
    /// ```
    pub fn summary(&self) -> Summary {
        let mut s = Summary {
            modules: self.modules.len(),
            ..Default::default()
        };
        for c in self.iter() {
            *s.by_kind
                .entry(format!("{:?}", c.kind).to_lowercase())
                .or_default() += 1;
            let st = serde_json::to_value(c.stability)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            *s.by_stability.entry(st).or_default() += 1;
            if c.exposure.mcp.as_ref().is_some_and(|m| m.tool.is_some()) {
                s.mcp_tools += 1;
            }
            if c.exposure
                .mcp
                .as_ref()
                .is_some_and(|m| m.resource.is_some())
            {
                s.mcp_resources += 1;
            }
            if c.exposure.http.is_some() {
                s.http_routes += 1;
            }
            if c.exposure.cli.is_some() {
                s.cli_commands += 1;
            }
            match c.benchmark {
                BenchmarkPolicy::Required => s.benchmark_required += 1,
                BenchmarkPolicy::Waived {
                    reason: WaiverReason::NotExecutable,
                } => {}
                BenchmarkPolicy::Waived { .. } => s.benchmark_waived += 1,
            }
            if c.cache.is_enabled() {
                s.cached += 1;
            }
            match c.provenance {
                Provenance::Builtin { .. } => s.builtin += 1,
                Provenance::Declarative { .. } => s.declarative += 1,
            }
        }
        s.total = self.len();
        s
    }
}

#[derive(
    Debug, Default, Clone, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema,
)]
/// The registry counted: by source, kind, stability and projection.
///
/// Every field is a reading of one collection, which is why they add up: the sources sum
/// to the total, and so does each breakdown. A surface that shows two of these numbers is
/// showing two views of the same walk and cannot contradict itself.
///
/// ```
/// use majordomus_cli::capability::{builtin, CapabilityRegistry};
/// let registry = CapabilityRegistry::builder().with_modules(builtin::modules()).build().unwrap();
/// let summary: majordomus_cli::capability::registry::Summary = registry.summary();
/// assert_eq!(summary.by_stability.values().sum::<usize>(), summary.total);
/// assert!(summary.mcp_tools <= summary.total && summary.http_routes <= summary.total);
/// assert_eq!(summary.builtin, builtin::all().len(), "no index was read");
/// ```
pub struct Summary {
    /// Every capability.
    pub total: usize,
    /// Composed in Rust.
    pub builtin: usize,
    /// Read from the layer.
    pub declarative: usize,
    /// By kind (`query`, `resource`).
    pub by_kind: BTreeMap<String, usize>,
    /// By stability.
    pub by_stability: BTreeMap<String, usize>,
    /// With an MCP tool exposure.
    pub mcp_tools: usize,
    /// With an MCP resource exposure.
    pub mcp_resources: usize,
    /// With an HTTP exposure.
    pub http_routes: usize,
    /// With a CLI exposure.
    pub cli_commands: usize,
    /// Modules: composed, derived, and one per declarative kind.
    pub modules: usize,
    /// Executables whose benchmark policy is required.
    pub benchmark_required: usize,
    /// Executables waived from benchmarking for a typed reason.
    pub benchmark_waived: usize,
    /// Executables the executor caches.
    pub cached: usize,
}
