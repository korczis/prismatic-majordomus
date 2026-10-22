//! The public surface: everything a caller can hold, read from the one declaration that
//! states it.
//!
//! # Why this exists as a type
//!
//! A version is a claim about compatibility, and a claim nobody can check is worth nothing.
//! Elm's package manager answers this by refusing to let the author choose: it reads the
//! public API of both versions and computes the smallest honest bump. This module is the
//! first half of that — the reading — and [`super::compat`] is the second.
//!
//! The surface is not discovered by crawling a running server, parsing rendered Markdown or
//! calling MCP. Every interface of this repository is a projection of the capability
//! registry (ADR 0002, ADR 0027), so the registry *is* the public surface and the
//! projections are what it looks like from outside. Comparing projections would compare
//! renderings; comparing the registry compares the contract.
//!
//! # Two origins, one shape
//!
//! ```text
//!   the current tree    generate::registry_manifest(&registry)   in process, never stale
//!   a released ref      git show <ref>:docs/generated/registry.json
//! ```
//!
//! The current tree is read from the live registry rather than from the committed file,
//! because a committed projection can be stale and a surface comparison against a stale
//! *current* side is a false verdict in the dangerous direction: it would report a removal
//! that did not happen, or miss an addition that did. A historical ref has no such option —
//! the executable of that commit is not available — so the committed projection is used,
//! and [`Surface::read_ref`] refuses loudly rather than guessing when a ref does not carry
//! one. That asymmetry is deliberate and is the reason the generated registry is committed
//! at every commit at all.
//!
//! ```
//! use majordomus_cli::release::surface::{public_capabilities, Origin, Surface, SURFACE_SCHEMA};
//! use serde_json::json;
//!
//! let surface = Surface {
//!     schema: SURFACE_SCHEMA.to_string(),
//!     origin: Origin::WorkingTree,
//!     capabilities: public_capabilities(&json!({"capabilities": [
//!         {"id": "a.one", "kind": "query", "visibility": "public",
//!          "exposure": {"http": {"method": "GET", "path": "/api/v1/a"}}},
//!         {"id": "a.hidden", "kind": "query", "visibility": "internal"},
//!     ]})),
//! };
//! // An internal capability is not a promise to anyone, so it is not surface.
//! assert_eq!(surface.capabilities.len(), 1);
//! assert_eq!(surface.atoms(), 2, "the identity and the route");
//! assert!(surface.fingerprint().starts_with("sha256:"));
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The committed projection of the registry, which is how a released ref's surface is read.
pub const REGISTRY: &str = "docs/generated/registry.json";

/// The schema this module's normalisation is defined against. It is carried into the plan so
/// that a verdict recorded today can be reproduced when the rules below change.
pub const SURFACE_SCHEMA: &str = "majordomus/public-surface/v1";

/// Where a surface was read from.
///
/// ```
/// use majordomus_cli::release::surface::Origin;
/// let origin = Origin::Ref { reference: "v0.5.0".into(), commit: "3a032c1e".into() };
/// // A ref origin names the commit, so a verdict is never pinned to a moving name.
/// assert_ne!(origin, Origin::WorkingTree);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "origin", rename_all = "snake_case")]
#[schemars(rename = "ReleaseSurfaceOrigin")]
pub enum Origin {
    /// The live registry of this executable, which is what the current tree declares.
    WorkingTree,
    /// The committed projection at a ref.
    Ref {
        /// The ref as it was named: `v0.5.0`.
        reference: String,
        /// The commit it resolved to, so the verdict names a commit and not a moving name.
        commit: String,
    },
}

/// One HTTP binding a caller can hold.
///
/// ```
/// use majordomus_cli::release::surface::HttpBinding;
/// let binding = HttpBinding { method: "GET".into(), path: "/api/v1/release/analysis".into() };
/// assert_eq!(binding.to_string(), "GET /api/v1/release/analysis");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseHttpBinding")]
pub struct HttpBinding {
    /// `GET`.
    pub method: String,
    /// `/api/v1/release/analysis`.
    pub path: String,
}

impl std::fmt::Display for HttpBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.method, self.path)
    }
}

/// One public capability, reduced to what a caller outside this repository can depend on.
///
/// Everything here is a promise; nothing here is an implementation detail. The title and the
/// description are deliberately absent — they are how the contract is explained, not what it
/// is, and a reworded description that forced a release would make the gate a thing to route
/// around within a week.
///
/// ```
/// use majordomus_cli::release::surface::{public_capabilities, PublicCapability};
/// use serde_json::json;
/// let caps = public_capabilities(&json!({"capabilities": [{
///     "id": "a.one", "kind": "query", "visibility": "public",
///     "exposure": {"cli": {"path": ["a", "one"]}},
///     "output": {"schema": {"type": "object", "description": "prose"}}
/// }]}));
/// let capability: &PublicCapability = &caps["a.one"];
/// assert_eq!(capability.cli.as_deref(), Some("a one"));
/// // Prose is normalised away: a reworded description is not a contract change.
/// assert!(capability.output.as_ref().unwrap().get("description").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleasePublicCapability")]
pub struct PublicCapability {
    /// `release.analysis`.
    pub id: String,
    /// `query` or `command`: whether calling it changes anything.
    pub kind: String,
    /// The MCP tool name, when it answers to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_tool: Option<String>,
    /// The MCP resource URI, when it is readable as one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_resource: Option<String>,
    /// The HTTP method and path, when it is bound to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpBinding>,
    /// The command-line path that dispatches it, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    /// The normalised input contract: what a caller may send.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    /// The normalised output contract: what a caller is promised back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
}

/// The public surface of one tree or one ref.
///
/// ```
/// use majordomus_cli::release::surface::{public_capabilities, Origin, Surface, SURFACE_SCHEMA};
/// use serde_json::json;
/// let of = |caps| Surface {
///     schema: SURFACE_SCHEMA.to_string(),
///     origin: Origin::WorkingTree,
///     capabilities: public_capabilities(&json!({"capabilities": caps})),
/// };
/// // The same surface enumerated in a different order is the same surface.
/// let one = of(json!([{"id": "b", "kind": "query"}, {"id": "a", "kind": "query"}]));
/// let other = of(json!([{"id": "a", "kind": "query"}, {"id": "b", "kind": "query"}]));
/// assert_eq!(one.fingerprint(), other.fingerprint());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleasePublicSurface")]
pub struct Surface {
    /// The normalisation these capabilities were reduced by.
    pub schema: String,
    /// Where it was read from.
    pub origin: Origin,
    /// Every public capability, by id, in a deterministic order.
    pub capabilities: BTreeMap<String, PublicCapability>,
}

/// Why a surface could not be read.
///
/// Each is a different fact, and collapsing them would make a check that cannot reach its
/// subject indistinguishable from one that found nothing wrong:
///
/// ```
/// use majordomus_cli::release::surface::SurfaceError;
/// let e = SurfaceError::NothingPublished;
/// assert!(e.to_string().contains("published nothing"));
/// let e = SurfaceError::NoRegistry { reference: "v0.1.0".into() };
/// assert!(e.to_string().contains("carries no"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceError {
    /// The ref carries no committed registry projection, so its surface is unknowable.
    NoRegistry {
        /// The ref that was asked.
        reference: String,
    },
    /// The ref is not a commit in this repository, or git could not be run.
    NoSuchRef {
        /// The ref that was asked.
        reference: String,
        /// What git said.
        reason: String,
    },
    /// The repository has published nothing at all, so there is no baseline to measure
    /// against. Distinct from a ref that cannot be resolved: nothing is missing from this
    /// clone, there is simply no release yet, and telling a first-time user to fetch tags
    /// that do not exist sends them looking for a fault that is not there.
    NothingPublished,
    /// The registry was found but is not the document this reads.
    Malformed {
        /// Where it came from.
        source: String,
        /// What was wrong with it.
        reason: String,
    },
}

impl std::fmt::Display for SurfaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SurfaceError::NoRegistry { reference } => write!(
                f,
                "{reference} carries no {REGISTRY}, so the surface it published cannot be read; \
                 a release before the registry was committed cannot be compared with"
            ),
            SurfaceError::NoSuchRef { reference, reason } => write!(
                f,
                "{reference} is not a commit in this repository ({reason}); \
                 a shallow clone is the usual cause — fetch the tags and their commits"
            ),
            SurfaceError::NothingPublished => write!(
                f,
                "this repository has published nothing: there is no release record and no \
                 version tag, so there is no baseline to measure this tree against"
            ),
            SurfaceError::Malformed { source, reason } => {
                write!(f, "the registry at {source} could not be read: {reason}")
            }
        }
    }
}

impl std::error::Error for SurfaceError {}

impl Surface {
    /// The surface of the current tree, from the live registry.
    ///
    /// Never reads the committed projection: a stale `docs/generated/registry.json` would
    /// make this side of the comparison a description of an older tree, and every verdict
    /// drawn from it wrong in a way nothing else would catch.
    ///
    /// ```
    /// use majordomus_cli::capability::registry::CapabilityRegistry;
    /// use majordomus_cli::release::surface::{Origin, Surface};
    /// let registry = CapabilityRegistry::builder().build().expect("an empty registry composes");
    /// let surface = Surface::of_registry(&registry);
    /// assert_eq!(surface.origin, Origin::WorkingTree);
    /// ```
    pub fn of_registry(registry: &crate::capability::registry::CapabilityRegistry) -> Surface {
        let manifest = crate::generate::registry_manifest(registry);
        // The manifest is this executable's own projection, so a shape it cannot read is a
        // bug here rather than bad input; an empty surface is the honest answer and the
        // diff against it will be loud.
        Surface {
            schema: SURFACE_SCHEMA.to_string(),
            origin: Origin::WorkingTree,
            capabilities: public_capabilities(&manifest),
        }
    }

    /// The surface a ref published, from the registry projection committed at it.
    ///
    /// ```
    /// use majordomus_cli::release::surface::{Surface, SurfaceError};
    /// let dir = tempfile::tempdir().unwrap();
    /// // Not a work tree, so there is no such ref — and it says so rather than
    /// // comparing against an empty surface.
    /// assert!(matches!(
    ///     Surface::read_ref(dir.path(), "v0.5.0"),
    ///     Err(SurfaceError::NoSuchRef { .. })
    /// ));
    /// ```
    pub fn read_ref(root: &Path, reference: &str) -> Result<Surface, SurfaceError> {
        let commit = rev_parse(root, reference).ok_or_else(|| SurfaceError::NoSuchRef {
            reference: reference.to_string(),
            reason: "git could not resolve it to a commit".to_string(),
        })?;
        let out = Command::new("git")
            .current_dir(root)
            .args(["show", &format!("{reference}:{REGISTRY}")])
            .output()
            .map_err(|e| SurfaceError::NoSuchRef {
                reference: reference.to_string(),
                reason: e.to_string(),
            })?;
        if !out.status.success() {
            return Err(SurfaceError::NoRegistry {
                reference: reference.to_string(),
            });
        }
        let manifest: Value =
            serde_json::from_slice(&out.stdout).map_err(|e| SurfaceError::Malformed {
                source: format!("{reference}:{REGISTRY}"),
                reason: e.to_string(),
            })?;
        if manifest
            .get("capabilities")
            .and_then(|c| c.as_array())
            .is_none()
        {
            return Err(SurfaceError::Malformed {
                source: format!("{reference}:{REGISTRY}"),
                reason: "it carries no `capabilities` array".to_string(),
            });
        }
        Ok(Surface {
            schema: SURFACE_SCHEMA.to_string(),
            origin: Origin::Ref {
                reference: reference.to_string(),
                commit,
            },
            capabilities: public_capabilities(&manifest),
        })
    }

    /// How many public atoms this surface carries: the things a caller can hold, counted.
    ///
    /// One per capability identity, plus one per binding it answers to. This is what the
    /// reports mean by "87 public atoms"; it is a size, never an input to a verdict.
    ///
    /// ```
    /// use majordomus_cli::release::surface::{public_capabilities, Origin, Surface, SURFACE_SCHEMA};
    /// use serde_json::json;
    /// let surface = Surface {
    ///     schema: SURFACE_SCHEMA.to_string(),
    ///     origin: Origin::WorkingTree,
    ///     capabilities: public_capabilities(&json!({"capabilities": [{
    ///         "id": "a.one", "kind": "query", "visibility": "public",
    ///         "exposure": {"mcp": {"tool": "t"}, "http": {"method": "GET", "path": "/x"}}
    ///     }]})),
    /// };
    /// assert_eq!(surface.atoms(), 3, "the identity, the tool, the route");
    /// ```
    pub fn atoms(&self) -> usize {
        self.capabilities
            .values()
            .map(|c| {
                1 + usize::from(c.mcp_tool.is_some())
                    + usize::from(c.mcp_resource.is_some())
                    + usize::from(c.http.is_some())
                    + usize::from(c.cli.is_some())
            })
            .sum()
    }

    /// A stable identity for this surface.
    ///
    /// Deterministic, ordering-independent and schema-versioned: the same logical surface
    /// hashes the same however the registry enumerated it, because the capabilities are a
    /// sorted map and every schema was normalised before it got here. Two trees with the
    /// same fingerprint have the same public contract, which is what makes it usable as
    /// release evidence rather than decoration.
    ///
    /// ```
    /// use majordomus_cli::capability::registry::CapabilityRegistry;
    /// use majordomus_cli::release::surface::Surface;
    /// let registry = CapabilityRegistry::builder().build().expect("an empty registry composes");
    /// let surface = Surface::of_registry(&registry);
    /// assert_eq!(surface.fingerprint(), surface.fingerprint(), "deterministic");
    /// assert!(surface.fingerprint().starts_with("sha256:"));
    /// ```
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(SURFACE_SCHEMA.as_bytes());
        hasher.update(b"\n");
        for (id, c) in &self.capabilities {
            hasher.update(id.as_bytes());
            hasher.update(b"\0");
            // Serialised through serde_json, whose object keys here are already sorted:
            // `PublicCapability` is a struct with a fixed field order and every schema
            // inside it was rebuilt into a BTreeMap by `normalise`.
            let bytes = serde_json::to_vec(c).unwrap_or_default();
            hasher.update(&bytes);
            hasher.update(b"\n");
        }
        format!("sha256:{}", hasher.hex())
    }
}

/// Resolve a ref to a full commit id.
fn rev_parse(root: &Path, reference: &str) -> Option<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Reduce a registry manifest to the public capabilities it declares.
///
/// A capability that is not public is not part of the contract and is dropped here rather
/// than carried and filtered later, so that nothing downstream can accidentally hold one.
///
/// ```
/// use majordomus_cli::release::surface::public_capabilities;
/// use serde_json::json;
/// let caps = public_capabilities(&json!({"capabilities": [
///     {"id": "a.one", "kind": "query", "visibility": "public"},
///     {"id": "a.two", "kind": "query", "visibility": "internal"},
///     {"id": "a.three", "kind": "query"},
/// ]}));
/// assert!(caps.contains_key("a.one"));
/// assert!(!caps.contains_key("a.two"), "internal is not a promise to anyone");
/// assert!(caps.contains_key("a.three"), "the registry's own default is public");
/// ```
pub fn public_capabilities(manifest: &Value) -> BTreeMap<String, PublicCapability> {
    let mut out = BTreeMap::new();
    let Some(list) = manifest.get("capabilities").and_then(|c| c.as_array()) else {
        return out;
    };
    for c in list {
        // A capability with no stated visibility is public: that is the registry's own
        // default, and reading it differently here would make this a second policy.
        let visibility = c
            .get("visibility")
            .and_then(|v| v.as_str())
            .unwrap_or("public");
        if visibility != "public" {
            continue;
        }
        let Some(id) = c.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let exposure = c.get("exposure");
        let mcp = exposure.and_then(|e| e.get("mcp"));
        let http = exposure.and_then(|e| e.get("http")).and_then(|h| {
            Some(HttpBinding {
                method: h.get("method")?.as_str()?.to_string(),
                path: h.get("path")?.as_str()?.to_string(),
            })
        });
        let cli = exposure
            .and_then(|e| e.get("cli"))
            .and_then(|c| c.get("path"))
            .and_then(|p| p.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            });
        out.insert(
            id.to_string(),
            PublicCapability {
                id: id.to_string(),
                kind: c
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("query")
                    .to_string(),
                mcp_tool: mcp
                    .and_then(|m| m.get("tool"))
                    .and_then(|t| t.as_str())
                    .map(str::to_string),
                mcp_resource: mcp
                    .and_then(|m| m.get("resource"))
                    .and_then(|r| r.get("uri"))
                    .and_then(|u| u.as_str())
                    .map(str::to_string),
                http,
                cli,
                input: c.get("input").and_then(|i| i.get("schema")).map(normalise),
                output: c.get("output").and_then(|o| o.get("schema")).map(normalise),
            },
        );
    }
    out
}

/// The keys of a schema that explain it rather than constrain it.
///
/// Removing these is what stops a reworded doc comment from demanding a release. Everything
/// else a schema carries is a constraint a caller can violate, and is kept.
const PROSE_KEYS: &[&str] = &["description", "title", "$comment", "examples", "example"];

/// Reduce a JSON Schema to the part of it a caller can break against.
///
/// Two normalisations, and no more, because every one of them is a place a real contract
/// change could hide:
///
/// - the prose keys above are dropped, at every depth;
/// - object keys are rebuilt in sorted order, so that a generator that emitted them
///   differently produces the same value.
///
/// Array order is *kept*. A schema's `required` list, its `enum` and its `oneOf` are sets in
/// meaning but the comparison in [`super::compat`] treats them as sets explicitly, where it
/// knows what they mean. Sorting them here would also silently reorder `prefixItems`, where
/// order is the contract.
///
/// ```
/// use majordomus_cli::release::surface::normalise;
/// use serde_json::json;
/// let one = json!({"type": "object", "description": "one wording"});
/// let other = json!({"type": "object", "description": "another, at length"});
/// assert_eq!(normalise(&one), normalise(&other), "prose is not contract");
/// // and it is a fixed point, so normalising twice changes nothing further
/// let once = normalise(&one);
/// assert_eq!(normalise(&once), once);
/// ```
pub fn normalise(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            // BTreeSet gives the sorted key order without sorting a Vec of clones.
            let keys: BTreeSet<&String> = map.keys().collect();
            for k in keys {
                if PROSE_KEYS.contains(&k.as_str()) {
                    continue;
                }
                out.insert(k.clone(), normalise(&map[k]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalise).collect()),
        other => other.clone(),
    }
}

/// A SHA-256, so that the fingerprint needs no dependency this crate does not already have.
///
/// The implementation is the FIPS 180-4 one, written out rather than pulled in: this crate
/// has no hash dependency, and adding one for a single fingerprint would be a larger change
/// to the dependency policy than the feature warrants.
struct Sha256 {
    state: [u32; 8],
    buffer: Vec<u8>,
    length: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    fn new() -> Sha256 {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: Vec::with_capacity(64),
            length: 0,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        self.buffer.extend_from_slice(bytes);
        while self.buffer.len() >= 64 {
            let block: [u8; 64] = self.buffer[..64].try_into().expect("64 bytes");
            self.compress(&block);
            self.buffer.drain(..64);
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (s, v) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *s = s.wrapping_add(v);
        }
    }

    fn hex(mut self) -> String {
        let bits = self.length.wrapping_mul(8);
        self.buffer.push(0x80);
        while self.buffer.len() % 64 != 56 {
            self.buffer.push(0);
        }
        let tail = bits.to_be_bytes();
        self.buffer.extend_from_slice(&tail);
        let blocks: Vec<[u8; 64]> = self
            .buffer
            .chunks_exact(64)
            .map(|c| c.try_into().expect("64 bytes"))
            .collect();
        self.buffer.clear();
        for b in blocks {
            self.compress(&b);
        }
        self.state.iter().map(|w| format!("{w:08x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The published vectors. A fingerprint nothing else can reproduce is not evidence, so
    /// the hash behind it is checked against the standard rather than against itself.
    #[test]
    fn the_hash_is_sha256() {
        let mut h = Sha256::new();
        h.update(b"abc");
        assert_eq!(
            h.hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut h = Sha256::new();
        h.update(b"");
        assert_eq!(
            h.hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // Longer than one block, and split across updates, because the surface is fed in
        // capability by capability rather than all at once.
        let mut h = Sha256::new();
        h.update(b"abcdbcdecdefdefgefghfghighijhi");
        h.update(b"jkijkljklmklmnlmnomnopnopq");
        assert_eq!(
            h.hex(),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn prose_is_not_contract() {
        let a = json!({"type": "object", "description": "one wording", "properties": {"x": {"type": "string", "title": "X"}}});
        let b = json!({"type": "object", "description": "another wording entirely", "properties": {"x": {"type": "string"}}});
        assert_eq!(
            normalise(&a),
            normalise(&b),
            "a reworded description is not a contract change"
        );
    }

    #[test]
    fn normalising_is_idempotent_and_orders_keys() {
        let messy = json!({"properties": {"b": {"type": "string"}, "a": {"type": "number"}}, "type": "object"});
        let once = normalise(&messy);
        assert_eq!(once, normalise(&once), "normalise is a fixed point");
        let keys: Vec<&String> = once.as_object().expect("object").keys().collect();
        assert_eq!(keys, vec!["properties", "type"], "keys come out sorted");
    }

    #[test]
    fn array_order_survives_normalisation() {
        // `prefixItems` is positional: sorting it would change the contract while claiming
        // to normalise it.
        let v = json!({"prefixItems": [{"type": "string"}, {"type": "number"}]});
        let n = normalise(&v);
        assert_eq!(n["prefixItems"][0]["type"], "string");
        assert_eq!(n["prefixItems"][1]["type"], "number");
    }

    fn manifest(caps: Value) -> Value {
        json!({"schema": "majordomus/capability-registry/v1", "capabilities": caps})
    }

    #[test]
    fn only_public_capabilities_are_surface() {
        let m = manifest(json!([
            {"id": "a.one", "kind": "query", "visibility": "public"},
            {"id": "a.two", "kind": "query", "visibility": "internal"},
            {"id": "a.three", "kind": "query"},
        ]));
        let caps = public_capabilities(&m);
        assert!(caps.contains_key("a.one"));
        assert!(
            !caps.contains_key("a.two"),
            "an internal capability is not a promise to anyone"
        );
        assert!(
            caps.contains_key("a.three"),
            "the registry's own default is public, and this must not be a second policy"
        );
    }

    #[test]
    fn every_binding_a_caller_can_hold_is_read() {
        let m = manifest(json!([{
            "id": "a.one", "kind": "query", "visibility": "public",
            "exposure": {
                "mcp": {"tool": "t", "resource": {"uri": "majordomus://r"}},
                "http": {"method": "GET", "path": "/api/v1/x"},
                "cli": {"path": ["a", "one"]}
            },
            "input": {"schema": {"type": "object"}},
            "output": {"schema": {"type": "object"}}
        }]));
        let c = &public_capabilities(&m)["a.one"];
        assert_eq!(c.mcp_tool.as_deref(), Some("t"));
        assert_eq!(c.mcp_resource.as_deref(), Some("majordomus://r"));
        assert_eq!(
            c.http.as_ref().map(|h| h.to_string()).as_deref(),
            Some("GET /api/v1/x")
        );
        assert_eq!(c.cli.as_deref(), Some("a one"));
        assert!(c.input.is_some() && c.output.is_some());
    }

    fn surface_of(caps: Value) -> Surface {
        Surface {
            schema: SURFACE_SCHEMA.to_string(),
            origin: Origin::WorkingTree,
            capabilities: public_capabilities(&manifest(caps)),
        }
    }

    #[test]
    fn the_fingerprint_is_deterministic_and_ordering_independent() {
        let one = surface_of(json!([
            {"id": "b.two", "kind": "query", "visibility": "public"},
            {"id": "a.one", "kind": "query", "visibility": "public"},
        ]));
        let other = surface_of(json!([
            {"id": "a.one", "kind": "query", "visibility": "public"},
            {"id": "b.two", "kind": "query", "visibility": "public"},
        ]));
        assert_eq!(
            one.fingerprint(),
            other.fingerprint(),
            "the same surface enumerated in a different order is the same surface"
        );
        assert_eq!(one.fingerprint(), one.fingerprint(), "and it is stable");
        assert!(one.fingerprint().starts_with("sha256:"));
    }

    #[test]
    fn a_changed_surface_changes_the_fingerprint() {
        let before = surface_of(json!([{"id": "a.one", "kind": "query", "visibility": "public"}]));
        let after = surface_of(json!([
            {"id": "a.one", "kind": "query", "visibility": "public"},
            {"id": "a.two", "kind": "query", "visibility": "public"},
        ]));
        assert_ne!(before.fingerprint(), after.fingerprint());
    }

    #[test]
    fn prose_does_not_move_the_fingerprint() {
        let before = surface_of(json!([{
            "id": "a.one", "kind": "query", "visibility": "public",
            "output": {"schema": {"type": "object", "description": "before"}}
        }]));
        let after = surface_of(json!([{
            "id": "a.one", "kind": "query", "visibility": "public",
            "output": {"schema": {"type": "object", "description": "after, at length"}}
        }]));
        assert_eq!(before.fingerprint(), after.fingerprint());
    }

    #[test]
    fn atoms_count_the_things_a_caller_can_hold() {
        let s = surface_of(json!([{
            "id": "a.one", "kind": "query", "visibility": "public",
            "exposure": {"mcp": {"tool": "t"}, "http": {"method": "GET", "path": "/x"}}
        }]));
        assert_eq!(s.atoms(), 3, "the identity, the tool, the route");
    }

    #[test]
    fn a_ref_without_a_registry_says_so_rather_than_comparing_with_nothing() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let run = |args: &[&str]| {
            Command::new("git")
                .current_dir(dir.path())
                .args(args)
                .output()
                .expect("git runs");
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "T"]);
        std::fs::write(dir.path().join("a.txt"), "x").expect("write");
        run(&["add", "-A"]);
        run(&["commit", "-qm", "one"]);

        match Surface::read_ref(dir.path(), "HEAD") {
            Err(SurfaceError::NoRegistry { reference }) => assert_eq!(reference, "HEAD"),
            other => panic!("a ref with no committed registry must refuse: {other:?}"),
        }
        match Surface::read_ref(dir.path(), "v9.9.9") {
            Err(SurfaceError::NoSuchRef { .. }) => {}
            other => panic!("a ref that does not exist must say so: {other:?}"),
        }
    }

    #[test]
    fn a_ref_with_a_registry_is_read_and_names_its_commit() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let run = |args: &[&str]| {
            Command::new("git")
                .current_dir(dir.path())
                .args(args)
                .output()
                .expect("git runs");
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "T"]);
        std::fs::create_dir_all(dir.path().join("docs/generated")).expect("mkdir");
        std::fs::write(
            dir.path().join(REGISTRY),
            serde_json::to_string(&manifest(json!([
                {"id": "a.one", "kind": "query", "visibility": "public"}
            ])))
            .expect("json"),
        )
        .expect("write");
        run(&["add", "-A"]);
        run(&["commit", "-qm", "one"]);

        let s = Surface::read_ref(dir.path(), "HEAD").expect("the registry is committed");
        assert!(s.capabilities.contains_key("a.one"));
        match s.origin {
            Origin::Ref { reference, commit } => {
                assert_eq!(reference, "HEAD");
                assert_eq!(
                    commit.len(),
                    40,
                    "the verdict names a commit, not a moving name"
                );
            }
            other => panic!("a ref surface carries a ref origin: {other:?}"),
        }
    }

    #[test]
    fn a_registry_that_is_not_one_is_refused_rather_than_read_as_empty() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let run = |args: &[&str]| {
            Command::new("git")
                .current_dir(dir.path())
                .args(args)
                .output()
                .expect("git runs");
        };
        run(&["init", "-q"]);
        run(&["config", "user.email", "t@example.com"]);
        run(&["config", "user.name", "T"]);
        std::fs::create_dir_all(dir.path().join("docs/generated")).expect("mkdir");
        // An empty surface read as valid would report every capability as removed.
        std::fs::write(dir.path().join(REGISTRY), "{\"schema\":\"x\"}").expect("write");
        run(&["add", "-A"]);
        run(&["commit", "-qm", "one"]);
        match Surface::read_ref(dir.path(), "HEAD") {
            Err(SurfaceError::Malformed { reason, .. }) => {
                assert!(
                    reason.contains("capabilities"),
                    "it names what was missing: {reason}"
                );
            }
            other => panic!("a registry with no capabilities array must refuse: {other:?}"),
        }
        std::fs::write(dir.path().join(REGISTRY), "not json at all").expect("write");
        run(&["add", "-A"]);
        run(&["commit", "-qm", "two"]);
        assert!(matches!(
            Surface::read_ref(dir.path(), "HEAD"),
            Err(SurfaceError::Malformed { .. })
        ));
    }
}
