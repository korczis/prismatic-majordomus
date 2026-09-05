//! The registry: every capability, once, validated, in a deterministic order, with the
//! lookups projections need. Built at one place per process from the builtin executables
//! and the repository's declarative objects; nothing else adds to it.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::index::Index;

use super::handler::{CapabilityError, Context, Executable, Handler};
use super::model::{Capability, CapabilityId, CapabilityKind, HttpMethod, Provenance};

/// A capability with, for a query, its handler.
pub struct Entry {
    pub capability: Capability,
    handler: Option<Arc<dyn Handler>>,
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
    InvalidId {
        id: String,
        provenance: String,
        reason: String,
    },
    #[error("capability '{id}' is defined twice: {first} and {second}")]
    DuplicateId {
        id: String,
        first: String,
        second: String,
    },
    #[error("MCP tool '{name}' is claimed by '{first}' and '{second}'")]
    DuplicateMcpName {
        name: String,
        first: String,
        second: String,
    },
    #[error("MCP resource '{uri}' is claimed by '{first}' and '{second}'")]
    DuplicateMcpUri {
        uri: String,
        first: String,
        second: String,
    },
    #[error("HTTP route {method} {path} is claimed by '{first}' and '{second}'")]
    DuplicateHttpRoute {
        method: String,
        path: String,
        first: String,
        second: String,
    },
    #[error("CLI path '{path}' is claimed by '{first}' and '{second}'")]
    DuplicateCliPath {
        path: String,
        first: String,
        second: String,
    },
    #[error("capability '{id}' ({provenance}): invalid {projection} exposure: {reason}")]
    InvalidExposure {
        id: String,
        provenance: String,
        projection: String,
        reason: String,
    },
    #[error("capability '{id}' ({provenance}) is {stability} and cannot be exposed as executable through {projection}")]
    NotExecutable {
        id: String,
        provenance: String,
        stability: String,
        projection: String,
    },
    #[error("capability '{id}' ({provenance}): {reason}")]
    Shape {
        id: String,
        provenance: String,
        reason: String,
    },
}

#[derive(Debug, Default)]
pub struct CapabilityRegistry {
    entries: BTreeMap<CapabilityId, Entry>,
    by_mcp_tool: BTreeMap<String, CapabilityId>,
    by_mcp_uri: BTreeMap<String, CapabilityId>,
    by_http: BTreeMap<(HttpMethod, String), CapabilityId>,
    by_cli: BTreeMap<Vec<String>, CapabilityId>,
}

/// The one composition point.
#[derive(Default)]
pub struct Builder {
    pending: Vec<Entry>,
}

impl Builder {
    pub fn with_builtin(mut self, executables: Vec<Executable>) -> Self {
        for e in executables {
            self.pending.push(Entry {
                capability: e.capability,
                handler: Some(e.handler),
            });
        }
        self
    }

    /// Every object of the index becomes a resource capability.
    pub fn with_index(mut self, index: &Index) -> Self {
        for object in &index.objects {
            self.pending.push(Entry {
                capability: super::declarative::capability_of(object),
                handler: None,
            });
        }
        self
    }

    /// Validate every invariant and build. Errors are collected, not stopped at the first,
    /// and reported in a deterministic order.
    pub fn build(self) -> Result<CapabilityRegistry, Vec<RegistryError>> {
        let mut errors = Vec::new();
        let mut registry = CapabilityRegistry::default();
        let mut pending = self.pending;
        pending.sort_by(|a, b| {
            a.capability.id.cmp(&b.capability.id).then_with(|| {
                a.capability
                    .provenance
                    .to_string()
                    .cmp(&b.capability.provenance.to_string())
            })
        });
        for entry in pending {
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
            match (c.kind, entry.handler.is_some()) {
                (CapabilityKind::Query, false) => {
                    errors.push(RegistryError::Shape {
                        id: id.clone(),
                        provenance: prov.clone(),
                        reason: "a query needs a handler".into(),
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
            registry.entries.insert(c.id.clone(), entry);
        }
        if errors.is_empty() {
            Ok(registry)
        } else {
            Err(errors)
        }
    }
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
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Every capability, by id.
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.entries.values().map(|e| &e.capability)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.entries
            .get(&CapabilityId::unchecked(id))
            .map(|e| &e.capability)
    }

    pub fn by_mcp_tool(&self, name: &str) -> Option<&Capability> {
        self.by_mcp_tool
            .get(name)
            .and_then(|id| self.get(id.as_str()))
    }

    pub fn by_mcp_uri(&self, uri: &str) -> Option<&Capability> {
        self.by_mcp_uri
            .get(uri)
            .and_then(|id| self.get(id.as_str()))
    }

    pub fn by_http(&self, method: HttpMethod, path: &str) -> Option<&Capability> {
        self.by_http
            .get(&(method, path.to_string()))
            .and_then(|id| self.get(id.as_str()))
    }

    pub fn by_cli(&self, path: &[String]) -> Option<&Capability> {
        self.by_cli.get(path).and_then(|id| self.get(id.as_str()))
    }

    /// Execute a query by id. A resource, or an unknown id, is `NotFound`.
    pub fn call(&self, ctx: &Context, id: &str, input: Value) -> Result<Value, CapabilityError> {
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
        let mut s = Summary::default();
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
pub struct Summary {
    pub total: usize,
    pub builtin: usize,
    pub declarative: usize,
    pub by_kind: BTreeMap<String, usize>,
    pub by_stability: BTreeMap<String, usize>,
    pub mcp_tools: usize,
    pub mcp_resources: usize,
    pub http_routes: usize,
    pub cli_commands: usize,
}
