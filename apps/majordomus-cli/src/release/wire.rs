//! The wire surface: what another executable of this repository depends on, priced by the
//! version gate beside the capabilities (I2164).
//!
//! The release surface ([`super::surface`]) is the capability registry: what a caller can
//! name. Two executables of different releases also meet below it — on the mesh, where a
//! discovery datagram, a link handshake and journal events cross between machines, and in
//! the lease file, which one release writes and another reads. None of that was on the
//! surface, so a change to it could ship as a patch release and reach a peer that could not
//! read it. This module reduces those promises to comparable facts:
//!
//! ```text
//!   discovery   the protocol versions a datagram may carry      mesh::protocol
//!   link        the protocol versions a handshake accepts        mesh::link
//!   lease       the schema of the lease file                     lease::SCHEMA
//!   events      every journal event kind, its fields, their types and which are required
//! ```
//!
//! and [`diff`] prices a movement the way [`super::compat::diff`] prices a capability's: a
//! peer that keeps working is minor, a peer that stops working is major.
//!
//! ```
//! use majordomus_cli::release::wire::{diff, WireSurface};
//! let now = WireSurface::current();
//! assert!(diff(&now, &now).is_empty(), "the wire diffed with itself is empty");
//! assert!(now.events.iter().any(|a| a.starts_with("event claim_acquired.scope")));
//! ```

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::compat::{Direction, Impact, SurfaceChange, SurfaceKind};
use super::surface::REGISTRY;

/// The versions of a protocol this executable reads (`min`) and writes (`max`).
///
/// ```
/// use majordomus_cli::release::wire::ProtocolRange;
/// let r = ProtocolRange { min: 1, max: 2 };
/// assert_eq!(r.to_string(), "1..=2");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolRange {
    /// The oldest version read.
    pub min: u32,
    /// The newest version, the one written.
    pub max: u32,
}

impl std::fmt::Display for ProtocolRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..={}", self.min, self.max)
    }
}

/// What another release of this executable depends on below the capabilities.
///
/// ```
/// use majordomus_cli::release::wire::WireSurface;
/// let wire = WireSurface::current();
/// assert_eq!(wire.lease_schema, majordomus_cli::lease::SCHEMA);
/// // it travels in the registry manifest, and reads back as itself
/// let back: WireSurface = serde_json::from_value(serde_json::to_value(&wire).unwrap()).unwrap();
/// assert_eq!(back, wire);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSurface {
    /// Discovery datagrams (`mesh::protocol`).
    pub discovery: ProtocolRange,
    /// The link handshake (`mesh::link`).
    pub link: ProtocolRange,
    /// The lease file's schema (`lease::SCHEMA`).
    pub lease_schema: String,
    /// One atom per journal event kind, per field with its type, and per required field:
    /// `event claim_acquired`, `event claim_acquired.scope: array`,
    /// `event claim_acquired.scope required`.
    pub events: BTreeSet<String>,
}

impl WireSurface {
    /// What this executable speaks and stores: the protocol constants it was built with, the
    /// lease schema it writes, and the journal event schema reduced to atoms. Read from the
    /// code itself, never from a committed projection, for the same reason the capability
    /// surface of the current tree is: a stale current side would be a false verdict.
    ///
    /// ```
    /// use majordomus_cli::release::wire::WireSurface;
    /// let wire = WireSurface::current();
    /// assert_eq!(wire.link.max, majordomus_cli::mesh::link::LINK_PROTOCOL_MAX);
    /// assert!(wire.discovery.min <= wire.discovery.max);
    /// ```
    pub fn current() -> WireSurface {
        let schema = schemars::schema_for!(crate::mesh::journal::EventBody);
        WireSurface {
            discovery: ProtocolRange {
                min: crate::mesh::protocol::MIN_PROTOCOL_VERSION,
                max: crate::mesh::protocol::PROTOCOL_VERSION,
            },
            link: ProtocolRange {
                min: crate::mesh::link::LINK_PROTOCOL_MIN,
                max: crate::mesh::link::LINK_PROTOCOL_MAX,
            },
            lease_schema: crate::lease::SCHEMA.to_string(),
            events: event_atoms(&serde_json::to_value(&schema).unwrap_or(Value::Null)),
        }
    }

    /// The wire surface a ref committed, from its registry manifest; `None` when the ref
    /// predates the `wire` key, or carries no manifest — then there is nothing to compare
    /// against, which the caller reports rather than reading as "unchanged".
    ///
    /// ```
    /// use majordomus_cli::release::wire::WireSurface;
    /// // a directory that is no repository has no ref to read
    /// let nowhere = tempfile::tempdir().unwrap();
    /// assert_eq!(WireSurface::read_ref(nowhere.path(), "v0.1.0"), None);
    /// ```
    pub fn read_ref(root: &Path, reference: &str) -> Option<WireSurface> {
        let out = Command::new("git")
            .current_dir(root)
            .args(["show", &format!("{reference}:{REGISTRY}")])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let manifest: Value = serde_json::from_slice(&out.stdout).ok()?;
        serde_json::from_value(manifest.get("wire")?.clone()).ok()
    }
}

/// The atoms of a JSON schema of the event enum: every variant (its `kind` tag), every field
/// with its type, and every required field — of the variants and of the types they refer to.
///
/// ```
/// use majordomus_cli::release::wire::event_atoms;
/// let schema = serde_json::json!({"oneOf": [{"type": "object",
///     "properties": {"kind": {"const": "a"}, "x": {"type": "string"}}, "required": ["kind", "x"]}]});
/// let atoms = event_atoms(&schema);
/// assert!(atoms.contains("event a"));
/// assert!(atoms.contains("event a.x: string"));
/// assert!(atoms.contains("event a.x required"));
/// ```
pub fn event_atoms(schema: &Value) -> BTreeSet<String> {
    let mut atoms = BTreeSet::new();
    let variants = schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for variant in &variants {
        let kind = variant
            .pointer("/properties/kind/const")
            .or_else(|| variant.pointer("/properties/kind/enum/0"))
            .and_then(Value::as_str)
            .unwrap_or("?");
        atoms.insert(format!("event {kind}"));
        shape(&format!("event {kind}"), variant, &mut atoms);
    }
    for key in ["$defs", "definitions"] {
        if let Some(defs) = schema.get(key).and_then(Value::as_object) {
            for (name, def) in defs {
                shape(&format!("type {name}"), def, &mut atoms);
            }
        }
    }
    atoms
}

fn shape(prefix: &str, object: &Value, atoms: &mut BTreeSet<String>) {
    if let Some(properties) = object.get("properties").and_then(Value::as_object) {
        for (field, schema) in properties {
            if field == "kind" {
                continue;
            }
            atoms.insert(format!("{prefix}.{field}: {}", type_of(schema)));
        }
    }
    if let Some(required) = object.get("required").and_then(Value::as_array) {
        for field in required.iter().filter_map(Value::as_str) {
            if field != "kind" {
                atoms.insert(format!("{prefix}.{field} required"));
            }
        }
    }
}

/// A field's type as one word or a reference: enough to tell a change of type apart.
fn type_of(schema: &Value) -> String {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        return r.rsplit('/').next().unwrap_or(r).to_string();
    }
    match schema.get("type") {
        Some(Value::String(t)) => t.clone(),
        Some(Value::Array(ts)) => ts
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("|"),
        _ => schema
            .get("anyOf")
            .or_else(|| schema.get("oneOf"))
            .and_then(Value::as_array)
            .map(|alts| alts.iter().map(type_of).collect::<Vec<_>>().join("|"))
            .unwrap_or_else(|| "any".into()),
    }
}

fn change(impact: Impact, direction: Direction, id: String, detail: &str) -> SurfaceChange {
    SurfaceChange {
        impact,
        direction,
        surface: SurfaceKind::Wire,
        capability: "wire".into(),
        id,
        detail: detail.into(),
    }
}

fn range(name: &str, base: ProtocolRange, head: ProtocolRange, out: &mut Vec<SurfaceChange>) {
    if head.max > base.max {
        out.push(change(
            Impact::Minor,
            Direction::Added,
            format!("{name} protocol {base} -> {head}"),
            "a newer version is spoken; the older is still read",
        ));
    }
    if head.max < base.max {
        out.push(change(
            Impact::Major,
            Direction::Removed,
            format!("{name} protocol {base} -> {head}"),
            "a version peers of the base release write is no longer written",
        ));
    }
    if head.min > base.min {
        out.push(change(
            Impact::Major,
            Direction::Removed,
            format!("{name} protocol {base} -> {head}"),
            "peers that speak only an older version are refused",
        ));
    }
    if head.min < base.min {
        out.push(change(
            Impact::Minor,
            Direction::Added,
            format!("{name} protocol {base} -> {head}"),
            "an older version is read again",
        ));
    }
}

/// Every movement of the wire between two releases, priced: what keeps a peer of the base
/// release working is minor, what stops it is major.
///
/// ```
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::wire::{diff, WireSurface};
/// let base = WireSurface::current();
/// // a newer link protocol, the old still read: minor
/// let mut head = base.clone();
/// head.link.max += 1;
/// assert_eq!(diff(&base, &head)[0].impact, Impact::Minor);
/// // an optional event field added: minor; made required: major
/// let mut head = base.clone();
/// head.events.insert("event claim_acquired.note: string".into());
/// assert_eq!(diff(&base, &head)[0].impact, Impact::Minor);
/// head.events.insert("event claim_acquired.note required".into());
/// assert!(diff(&base, &head).iter().any(|c| c.impact == Impact::Major));
/// // another lease schema: major
/// let mut head = base.clone();
/// head.lease_schema = "majordomus-mcp-lease/v2".into();
/// assert_eq!(diff(&base, &head)[0].impact, Impact::Major);
/// ```
pub fn diff(base: &WireSurface, head: &WireSurface) -> Vec<SurfaceChange> {
    let mut out = Vec::new();
    range("discovery", base.discovery, head.discovery, &mut out);
    range("link", base.link, head.link, &mut out);
    if base.lease_schema != head.lease_schema {
        out.push(change(
            Impact::Major,
            Direction::Changed,
            format!("lease {} -> {}", base.lease_schema, head.lease_schema),
            "a lease one release writes is a lease the other must read",
        ));
    }
    for gone in base.events.difference(&head.events) {
        let (impact, detail) = if gone.ends_with(" required") {
            (Impact::Minor, "a field peers had to send is optional now")
        } else {
            (
                Impact::Major,
                "an event or field peers of the base release write is no longer read as it was",
            )
        };
        out.push(change(impact, Direction::Removed, gone.clone(), detail));
    }
    // a kind or a type that is itself new: peers of the base release store an unknown event
    // kind without reading it, so what it requires is nobody's burden yet
    let owner = |atom: &str| -> String {
        let (head_part, _) = atom.split_once('.').unwrap_or((atom, ""));
        head_part.to_string()
    };
    let known: BTreeSet<String> = base.events.iter().map(|a| owner(a)).collect();
    for new in head.events.difference(&base.events) {
        let (impact, detail) = if new.ends_with(" required") && known.contains(&owner(new)) {
            (
                Impact::Major,
                "peers of the base release do not send a field this one requires",
            )
        } else {
            (
                Impact::Minor,
                "peers of the base release ignore what they do not know",
            )
        };
        out.push(change(impact, Direction::Added, new.clone(), detail));
    }
    out.sort_by(|a, b| b.impact.cmp(&a.impact).then_with(|| a.id.cmp(&b.id)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The journal event schema this executable carries reduces to atoms for every kind the
    /// fold reads, so a change to any of them is seen.
    #[test]
    fn every_event_kind_is_on_the_wire_surface() {
        let wire = WireSurface::current();
        for kind in crate::mesh::journal::KNOWN_KINDS {
            assert!(
                wire.events.contains(&format!("event {kind}")),
                "{kind} is missing from {:?}",
                wire.events
            );
        }
    }

    /// A field retyped is a removal and an addition, and the removal breaks a peer.
    #[test]
    fn a_retyped_field_is_major() {
        let base = WireSurface::current();
        let mut head = base.clone();
        head.events.remove("event claim_acquired.session: string");
        head.events
            .insert("event claim_acquired.session: integer".into());
        let changes = diff(&base, &head);
        assert!(changes.iter().any(|c| c.impact == Impact::Major));
    }

    /// A new event kind with required fields burdens nobody: peers of the base release store
    /// a kind they do not know without reading it.
    #[test]
    fn a_new_event_kind_is_minor_even_with_required_fields() {
        let base = WireSurface::current();
        let mut head = base.clone();
        head.events.insert("event lease_moved".into());
        head.events.insert("event lease_moved.to: string".into());
        head.events.insert("event lease_moved.to required".into());
        let changes = diff(&base, &head);
        assert!(!changes.is_empty());
        assert!(
            changes.iter().all(|c| c.impact == Impact::Minor),
            "{changes:?}"
        );
    }
}
