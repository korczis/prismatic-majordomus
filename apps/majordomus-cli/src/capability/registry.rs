//! The registry: every capability, once, validated, in a deterministic order, with the
//! lookups projections need. Built at one place per process from the builtin executables
//! and the repository's declarative objects; nothing else adds to it.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::index::Index;

use super::benchmark::CaseProvider;
use super::handler::{CapabilityError, Context, Executable, Handler};
use super::model::{
    BenchmarkPolicy, Capability, CapabilityId, CapabilityKind, HttpMethod, ModuleId, Provenance,
    Stability, WaiverReason,
};
use super::module::ModuleDescriptor;

/// A capability with, for an executable, its handler and its benchmark cases.
pub struct Entry {
    /// The descriptor.
    pub capability: Capability,
    handler: Option<Arc<dyn Handler>>,
    cases: Option<CaseProvider>,
}

/// Where a module's entry in the registry came from.
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
pub struct CapabilityRegistry {
    entries: BTreeMap<CapabilityId, Entry>,
    modules: BTreeMap<ModuleId, ModuleInfo>,
    fingerprint: String,
    by_mcp_tool: BTreeMap<String, CapabilityId>,
    by_mcp_uri: BTreeMap<String, CapabilityId>,
    by_http: BTreeMap<(HttpMethod, String), CapabilityId>,
    by_cli: BTreeMap<Vec<String>, CapabilityId>,
}

/// The one composition point.
#[derive(Default)]
pub struct Builder {
    pending: Vec<Entry>,
    modules: Vec<ModuleInfo>,
    index_fingerprint: String,
}

impl Builder {
    /// Add the application's modules, as `compose_modules!` produced them: their metadata
    /// and every executable they compose.
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
                if c.exposure
                    .mcp
                    .as_ref()
                    .is_some_and(|m| m.resource.is_some())
                {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason: "a command is called, not read: it has no MCP resource exposure"
                            .into(),
                    });
                }
                if let Some(http) = &c.exposure.http {
                    if http.method != HttpMethod::Post {
                        errors.push(RegistryError::InvalidExposure {
                            id: id.clone(),
                            provenance: prov.clone(),
                            projection: "HTTP".into(),
                            reason: format!(
                                "a command changes state and is bound to POST, not {}",
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
/// object's path and content) and of every descriptor, in id order. Stable across
/// processes for the same code and the same repository state.
fn fingerprint(index: &str, registry: &CapabilityRegistry) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(index.as_bytes());
    for c in registry.iter() {
        h.update(c.id.as_str().as_bytes());
        h.update(b"\0");
        h.update(serde_json::to_string(c).unwrap_or_default().as_bytes());
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
    /// Start composing a registry.
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

    /// Every capability, by id.
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.entries.values().map(|e| &e.capability)
    }

    /// How many capabilities the registry holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Does the registry hold nothing?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every module, by id: the composed ones, the derived ones, and one per declarative kind.
    pub fn modules(&self) -> impl Iterator<Item = &ModuleInfo> {
        self.modules.values()
    }

    /// One module by id.
    pub fn module(&self, id: &str) -> Option<&ModuleInfo> {
        self.modules.get(&ModuleId::unchecked(id))
    }

    /// The benchmark case provider of an executable; `None` for a resource or an unknown id.
    pub fn cases(&self, id: &str) -> Option<CaseProvider> {
        self.entries
            .get(&CapabilityId::unchecked(id))
            .and_then(|e| e.cases)
    }

    /// A capability by canonical id.
    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.entries
            .get(&CapabilityId::unchecked(id))
            .map(|e| &e.capability)
    }

    /// The capability an MCP tool name projects.
    pub fn by_mcp_tool(&self, name: &str) -> Option<&Capability> {
        self.by_mcp_tool
            .get(name)
            .and_then(|id| self.get(id.as_str()))
    }

    /// The capability an MCP resource URI projects.
    pub fn by_mcp_uri(&self, uri: &str) -> Option<&Capability> {
        self.by_mcp_uri
            .get(uri)
            .and_then(|id| self.get(id.as_str()))
    }

    /// The capability an HTTP method and path project.
    pub fn by_http(&self, method: HttpMethod, path: &str) -> Option<&Capability> {
        self.by_http
            .get(&(method, path.to_string()))
            .and_then(|id| self.get(id.as_str()))
    }

    /// The capability a CLI path projects.
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
