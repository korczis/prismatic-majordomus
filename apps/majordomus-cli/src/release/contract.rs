//! The public contract, as a normalised snapshot.
//!
//! A release's compatibility is not an opinion about the commits it contains; it is a
//! statement about what callers can still do. So the question is answered the way a
//! compiler answers it: take the contract as it was at the baseline release, take it as it
//! is now, and diff them.
//!
//! ```text
//!   capability registry ─┐
//!   command graph ───────┤
//!   document schemas ────┼──▶ ContractSnapshot ──▶ fingerprint
//!   distribution model ──┘        (normalised, sorted)
//! ```
//!
//! # What is in the contract
//!
//! Everything a caller outside this repository can depend on and cannot see changing until
//! it breaks:
//!
//! - **capabilities of this executable** — the kind, the input and output schemas property
//!   by property, and every projection each one declares (MCP tool, HTTP method and path,
//!   command path);
//! - **commands** — every command the command graph knows, with its arguments;
//! - **document kinds** — the schema of every `.ai/` object a repository authors, field by
//!   field, because a repository's own files are validated against them;
//! - **distribution targets** — the platforms a release publishes an artifact for.
//!
//! # What is deliberately not in it
//!
//! Titles, descriptions, tags, benchmark and cache policy, stability, provenance, module
//! membership, ordering, counts. None of them is something a caller's program reads, and
//! including them would make every prose edit a release-worthy contract change.
//!
//! Nor the **declarative** capabilities — the ones the registry derives from a
//! repository's own objects, one per rule, ADR, claim and prompt it holds. They are a fact
//! about *that repository*, not about this executable: writing an ADR would otherwise be an
//! additive change to the tool's public contract and would require a version bump, and
//! every repository would state a different contract from the same binary. What is
//! promised is that the *kind* exists and is read a particular way, which the document-kind
//! surface carries.
//!
//! A subsystem that cries wolf about compatibility is one nobody reads, so precision here
//! is worth more than coverage: [`super::diff`] is only allowed to be certain about what
//! this module is certain about.
//!
//! # Determinism
//!
//! Every collection is a [`BTreeMap`], every list is sorted before it is written, and no
//! value here is read from the clock, the environment or the filesystem's enumeration
//! order. The same repository state produces the same snapshot, byte for byte, including
//! the fingerprint — which is what makes a snapshot committed at a tag usable as a
//! baseline months later.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::capability::model::{Capability, CapabilityKind, Provenance};
use crate::capability::registry::CapabilityRegistry;
use crate::command_graph::CommandGraph;
use crate::distribution::Model as DistributionModel;

/// The schema identifier this document is written under.
pub const SCHEMA: &str = "contract/v1";

/// Which part of the public surface an entry belongs to.
///
/// The surface decides nothing on its own; it groups entries so that an explanation can
/// say *what* changed before it says *how*, and so that two entries with the same name on
/// different surfaces are two entries.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ContractSurface")]
pub enum Surface {
    /// A capability of the registry: the operation itself, whatever it is projected as.
    Capability,
    /// A command of the command graph, from whichever program offers it.
    Command,
    /// The schema of a declarative document kind a repository authors.
    DocumentKind,
    /// A platform a release publishes an artifact for.
    Target,
}

impl Surface {
    /// The word a report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Capability => "capability",
            Self::Command => "command",
            Self::DocumentKind => "document-kind",
            Self::Target => "target",
        }
    }

    /// How a person reads an entry of this surface in a sentence.
    pub fn noun(self) -> &'static str {
        match self {
            Self::Capability => "capability",
            Self::Command => "command",
            Self::DocumentKind => "document kind",
            Self::Target => "distribution target",
        }
    }
}

/// One thing the public contract states, and every fact about it that can break a caller.
///
/// The facts are a flat map on purpose. A fact is named by a path (`input.document`,
/// `exposure.http.path`) and valued by a normalised string; what the change of a
/// particular fact means is [`super::diff::FactPolicy`], which is data and not a match arm
/// buried in a classifier. Adding a fact to a snapshot therefore does not require touching
/// the diff, and a fact whose policy nobody declared is reported as unclassified rather
/// than silently ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ContractEntry")]
pub struct ContractEntry {
    /// Which part of the surface this is.
    pub surface: Surface,
    /// The entry's identity within its surface: a capability id, a command path, a schema
    /// id, a target id. Stable across releases; renaming one is a removal and an addition.
    pub id: String,
    /// Every fact, by path. Sorted, so the document is byte-stable.
    pub facts: BTreeMap<String, String>,
}

/// The public contract at one point in the repository's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ContractSnapshot")]
pub struct ContractSnapshot {
    /// The document's schema.
    pub schema: String,
    /// Every entry, sorted by surface and then by id.
    pub entries: Vec<ContractEntry>,
    /// The digest of the entries, as [`ContractSnapshot::fingerprint`] computes it.
    pub fingerprint: String,
}

impl ContractSnapshot {
    /// Build a snapshot from whatever a process can see.
    ///
    /// Each input is optional because a snapshot of what is available is more useful than
    /// no snapshot: a repository with no distribution model still has a capability
    /// contract, and comparing two snapshots that both lack a surface is sound. What is
    /// *not* sound is comparing a snapshot that has a surface against one that does not,
    /// and [`super::diff::diff`] refuses that rather than reporting every target as
    /// removed.
    pub fn build(
        registry: Option<&CapabilityRegistry>,
        graph: Option<&CommandGraph>,
        kinds: &BTreeMap<String, Value>,
        distribution: Option<&DistributionModel>,
    ) -> Self {
        let mut entries = Vec::new();
        if let Some(registry) = registry {
            for capability in registry.iter().filter(is_public) {
                entries.push(capability_entry(capability));
            }
        }
        if let Some(graph) = graph {
            entries.extend(command_entries(graph));
        }
        for (id, schema) in kinds {
            entries.push(document_kind_entry(id, schema));
        }
        if let Some(model) = distribution {
            for target in &model.targets {
                entries.push(target_entry(target));
            }
        }
        Self::from_entries(entries)
    }

    /// A snapshot over entries that already exist: sorted, deduplicated, fingerprinted.
    ///
    /// The one constructor. [`ContractSnapshot::build`] gathers entries and calls this, so
    /// a snapshot assembled any other way — a test's, a baseline read back from git — is
    /// normalised by exactly the same code that normalises the real one, and the
    /// fingerprint of two equal contracts is equal however they were built.
    pub fn from_entries(mut entries: Vec<ContractEntry>) -> Self {
        entries.sort_by(|a, b| (a.surface, &a.id).cmp(&(b.surface, &b.id)));
        entries.dedup_by(|a, b| a.surface == b.surface && a.id == b.id);
        let fingerprint = fingerprint(&entries);
        Self {
            schema: SCHEMA.into(),
            entries,
            fingerprint,
        }
    }

    /// Which surfaces this snapshot covers.
    ///
    /// A surface with no entry is a surface the process that built the snapshot could not
    /// see, not a surface that is empty: the difference decides whether a diff may read
    /// "every target was removed" or must refuse to compare that surface at all.
    pub fn surfaces(&self) -> BTreeSet<Surface> {
        self.entries.iter().map(|e| e.surface).collect()
    }

    /// The entry with this surface and id.
    pub fn entry(&self, surface: Surface, id: &str) -> Option<&ContractEntry> {
        self.entries
            .iter()
            .find(|e| e.surface == surface && e.id == id)
    }

    /// Parse a snapshot written by an older or newer build.
    ///
    /// A baseline snapshot is read from a git object months after it was written, so this
    /// is the compatibility boundary of the release subsystem with itself: an unknown
    /// schema is refused by name rather than mis-parsed, and an unknown surface makes the
    /// whole document unreadable rather than silently dropping entries — a dropped entry
    /// reads as a removal, which reads as a breaking change, which is the one mistake this
    /// subsystem must never make on its own.
    pub fn parse(text: &str) -> Result<Self, String> {
        let snapshot: Self =
            serde_json::from_str(text).map_err(|e| format!("the snapshot is not readable: {e}"))?;
        if snapshot.schema != SCHEMA {
            return Err(format!(
                "the snapshot states schema `{}` and this build reads `{SCHEMA}`",
                snapshot.schema
            ));
        }
        let computed = fingerprint(&snapshot.entries);
        if computed != snapshot.fingerprint {
            return Err(format!(
                "the snapshot's fingerprint is {} and its entries hash to {computed}: it was \
                 edited after it was written",
                snapshot.fingerprint
            ));
        }
        Ok(snapshot)
    }

    /// The document as it is committed: pretty JSON with one trailing newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }
}

/// The digest of a set of entries.
///
/// Over the rendered facts and nothing else: two snapshots with the same fingerprint state
/// the same contract, whichever build wrote them.
fn fingerprint(entries: &[ContractEntry]) -> String {
    let mut hasher = Sha256::new();
    for entry in entries {
        hasher.update(entry.surface.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(entry.id.as_bytes());
        hasher.update([0]);
        for (key, value) in &entry.facts {
            hasher.update(key.as_bytes());
            hasher.update([1]);
            hasher.update(value.as_bytes());
            hasher.update([1]);
        }
        hasher.update([2]);
    }
    format!("{:x}", hasher.finalize())
}

/// Whether a capability is part of *this executable's* contract.
///
/// The builtin ones are: they exist wherever the binary does. A declarative one exists
/// because a particular repository holds a particular file, and the binary makes no promise
/// about which files anybody holds.
fn is_public(capability: &&Capability) -> bool {
    matches!(capability.provenance, Provenance::Builtin { .. })
}

/// One capability, as the contract states it.
fn capability_entry(c: &Capability) -> ContractEntry {
    let mut facts = BTreeMap::new();
    facts.insert(
        "kind".into(),
        match c.kind {
            CapabilityKind::Query => "query",
            CapabilityKind::Command => "command",
            CapabilityKind::Resource => "resource",
        }
        .into(),
    );
    schema_facts(&mut facts, "input", &c.input.schema);
    schema_facts(&mut facts, "output", &c.output.schema);
    if let Some(mcp) = &c.exposure.mcp {
        if let Some(tool) = &mcp.tool {
            facts.insert("exposure.mcp.tool".into(), tool.clone());
        }
        if let Some(resource) = &mcp.resource {
            facts.insert("exposure.mcp.resource".into(), resource.uri.clone());
        }
    }
    if let Some(http) = &c.exposure.http {
        facts.insert(
            "exposure.http".into(),
            format!("{} {}", http.method.as_str(), http.path),
        );
    }
    if let Some(cli) = &c.exposure.cli {
        facts.insert("exposure.cli".into(), cli.path.join(" "));
    }
    ContractEntry {
        surface: Surface::Capability,
        id: c.id.as_str().to_string(),
        facts,
    }
}

/// The facts a JSON Schema states about a caller's data.
///
/// One fact per property, valued by its normalised type and whether it is required. That
/// is the whole of what a caller can break against: a property that disappears, a property
/// that changes type, a property that becomes required. Descriptions, titles, examples and
/// ordering are not facts.
///
/// Nested objects are walked, so `input.filter.since` is a fact of its own and a change
/// deep in a structure is as visible as one at the top. `$defs` are resolved once, by name,
/// and a reference that cannot be resolved becomes the fact `$ref:<name>` rather than
/// nothing — an unresolvable reference must still change when the thing it names changes.
fn schema_facts(facts: &mut BTreeMap<String, String>, prefix: &str, schema: &Value) {
    let defs = schema.get("$defs").and_then(Value::as_object);
    walk_schema(facts, prefix, schema, defs, 0);
}

/// The recursion behind [`schema_facts`]. `depth` stops a schema that refers to itself.
fn walk_schema(
    facts: &mut BTreeMap<String, String>,
    prefix: &str,
    schema: &Value,
    defs: Option<&serde_json::Map<String, Value>>,
    depth: usize,
) {
    const MAX_DEPTH: usize = 8;
    let resolved = resolve(schema, defs);
    let schema = resolved.as_ref();
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for (name, property) in properties {
        let path = format!("{prefix}.{name}");
        let property = resolve(property, defs);
        let kind = type_of(property.as_ref(), defs);
        let requirement = if required.contains(name.as_str()) {
            "required"
        } else {
            "optional"
        };
        facts.insert(path.clone(), format!("{kind} {requirement}"));
        if depth < MAX_DEPTH {
            walk_schema(facts, &path, property.as_ref(), defs, depth + 1);
        }
    }
}

/// A schema with one level of `$ref` followed, when it can be.
///
/// Returns a borrow when nothing was followed, so the common case allocates nothing.
fn resolve<'a>(
    schema: &'a Value,
    defs: Option<&'a serde_json::Map<String, Value>>,
) -> std::borrow::Cow<'a, Value> {
    let target = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix("#/$defs/"))
        .and_then(|name| defs.and_then(|d| d.get(name)));
    match target {
        Some(t) => std::borrow::Cow::Borrowed(t),
        None => std::borrow::Cow::Borrowed(schema),
    }
}

/// The normalised type of a schema: the one string that changes when a caller's value has
/// to change.
///
/// `["string","null"]` and `{"anyOf":[{"type":"string"},{"type":"null"}]}` are the same
/// type stated two ways — schemars writes both, depending on how the Rust type was
/// spelled — and normalising them together is what stops a refactor that changes nothing
/// for a caller from reading as a breaking change. An enumeration's members are part of
/// its type: removing a variant breaks whoever sends it.
fn type_of(schema: &Value, defs: Option<&serde_json::Map<String, Value>>) -> String {
    let mut members: BTreeSet<String> = BTreeSet::new();
    collect_types(schema, defs, &mut members, 0);
    if members.is_empty() {
        return "any".into();
    }
    members.into_iter().collect::<Vec<_>>().join("|")
}

/// Every alternative a schema admits, flattened and sorted.
fn collect_types(
    schema: &Value,
    defs: Option<&serde_json::Map<String, Value>>,
    out: &mut BTreeSet<String>,
    depth: usize,
) {
    const MAX_DEPTH: usize = 8;
    if depth > MAX_DEPTH {
        return;
    }
    let resolved = resolve(schema, defs);
    let schema = resolved.as_ref();
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        // an unresolvable reference is still an identity: it changes when its target's
        // name changes, which is exactly when a caller has to look again.
        out.insert(r.to_string());
        return;
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        if let Some(branches) = schema.get(key).and_then(Value::as_array) {
            for branch in branches {
                collect_types(branch, defs, out, depth + 1);
            }
            return;
        }
    }
    if let Some(constant) = schema.get("const").and_then(Value::as_str) {
        out.insert(format!("const:{constant}"));
        return;
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let mut members: Vec<String> = values
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string())
            })
            .collect();
        members.sort();
        out.insert(format!("enum:{}", members.join(",")));
        return;
    }
    match schema.get("type") {
        Some(Value::String(t)) => {
            out.insert(item_type(t, schema, defs, depth));
        }
        Some(Value::Array(ts)) => {
            for t in ts.iter().filter_map(Value::as_str) {
                out.insert(item_type(t, schema, defs, depth));
            }
        }
        _ => {}
    }
}

/// An array's type carries its item type: `array<string>` and `array<integer>` are not the
/// same promise.
fn item_type(
    t: &str,
    schema: &Value,
    defs: Option<&serde_json::Map<String, Value>>,
    depth: usize,
) -> String {
    if t != "array" {
        return t.to_string();
    }
    match schema.get("items") {
        Some(items) => {
            let mut inner = BTreeSet::new();
            collect_types(items, defs, &mut inner, depth + 1);
            format!(
                "array<{}>",
                if inner.is_empty() {
                    "any".to_string()
                } else {
                    inner.into_iter().collect::<Vec<_>>().join("|")
                }
            )
        }
        None => "array<any>".into(),
    }
}

/// Every runnable command of the graph, as the contract states it.
///
/// The graph already holds every command from every program; the contract keeps the shape
/// a caller types — the invocation, the arguments, whether each is required — and drops
/// everything about how it is presented and where it came from.
///
/// A node that only groups the commands under it is not in the contract: nobody can run
/// `majordomus distribution` on its own, so nothing can break when the group gains or
/// loses a subcommand — only the subcommands themselves are promises.
///
/// Nor is a workflow. A `just` recipe is a convenience for somebody standing in this
/// checkout; it is not shipped, nobody outside can run it, and the runner that reports it
/// is not installed everywhere — a contract that included them would differ between two
/// machines looking at the same commit, which is the one thing a baseline may never do.
fn command_entries(graph: &CommandGraph) -> Vec<ContractEntry> {
    use crate::command_graph::Origin;
    graph
        .commands
        .iter()
        .filter(|node| node.runnable)
        .filter(|node| matches!(node.origin, Origin::Executable | Origin::Tool))
        .map(|node| {
            let mut facts = BTreeMap::new();
            facts.insert("invocation".into(), node.invocation.clone());
            for argument in &node.arguments {
                facts.insert(
                    format!("argument.{}", argument.name),
                    format!(
                        "{} {} {}",
                        if argument.positional {
                            "positional"
                        } else if argument.takes_value {
                            "option"
                        } else {
                            "flag"
                        },
                        if argument.required {
                            "required"
                        } else {
                            "optional"
                        },
                        argument_values(argument),
                    ),
                );
            }
            let mut aliases = node.aliases.clone();
            aliases.sort();
            if !aliases.is_empty() {
                facts.insert("aliases".into(), aliases.join(","));
            }
            ContractEntry {
                surface: Surface::Command,
                id: node.id.as_str().to_string(),
                facts,
            }
        })
        .collect()
}

/// The values an argument accepts, as one normalised word.
///
/// A closed set of values is a promise: removing one breaks whoever passes it. An argument
/// that takes anything says `open`, and gaining a closed set from `open` is a narrowing,
/// which the fact makes visible.
fn argument_values(argument: &crate::command_graph::ArgumentSpec) -> String {
    if argument.values.is_empty() {
        return "open".into();
    }
    let mut values: Vec<&str> = argument.values.iter().map(|v| v.value.as_str()).collect();
    values.sort_unstable();
    format!("[{}]", values.join(","))
}

/// One document kind's schema, as the contract states it.
///
/// A repository's own files are validated against these, so a field that becomes required
/// invalidates documents that were valid yesterday. That is a breaking change to everyone
/// who has a repository, which is why the kinds are in the contract at all.
fn document_kind_entry(id: &str, schema: &Value) -> ContractEntry {
    let mut facts = BTreeMap::new();
    schema_facts(&mut facts, "field", schema);
    if schema
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .is_some_and(|open| !open)
    {
        facts.insert("closed".into(), "true".into());
    }
    ContractEntry {
        surface: Surface::DocumentKind,
        id: id.to_string(),
        facts,
    }
}

/// One distribution target, as the contract states it.
fn target_entry(t: &crate::distribution::Target) -> ContractEntry {
    use crate::distribution::Status;
    let mut facts = BTreeMap::new();
    facts.insert("rust-target".into(), t.rust_target.clone());
    facts.insert(
        "status".into(),
        match t.status {
            Status::Supported => "supported",
            Status::Experimental => "experimental",
            Status::Unavailable => "unavailable",
        }
        .into(),
    );
    ContractEntry {
        surface: Surface::Target,
        id: t.id.clone(),
        facts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema_of(value: Value) -> BTreeMap<String, String> {
        let mut facts = BTreeMap::new();
        schema_facts(&mut facts, "input", &value);
        facts
    }

    #[test]
    fn a_property_is_one_fact_naming_its_type_and_whether_it_is_required() {
        let facts = schema_of(json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "limit": { "type": "integer" },
            },
            "required": ["name"],
        }));
        assert_eq!(facts["input.name"], "string required");
        assert_eq!(facts["input.limit"], "integer optional");
    }

    /// The refactor that must not read as a breaking change: `Option<String>` written as a
    /// nullable type and written as an `anyOf` are the same promise to a caller, and
    /// schemars writes both depending on how the field was spelled.
    #[test]
    fn the_two_spellings_of_an_optional_string_normalise_together() {
        let a = schema_of(json!({
            "type": "object",
            "properties": { "since": { "type": ["string", "null"] } },
        }));
        let b = schema_of(json!({
            "type": "object",
            "properties": {
                "since": { "anyOf": [{ "type": "string" }, { "type": "null" }] }
            },
        }));
        assert_eq!(a, b);
        assert_eq!(a["input.since"], "null|string optional");
    }

    #[test]
    fn prose_is_not_a_fact() {
        let a = schema_of(json!({
            "type": "object",
            "title": "Input",
            "description": "the old sentence",
            "properties": { "name": { "type": "string", "description": "a name" } },
        }));
        let b = schema_of(json!({
            "type": "object",
            "title": "Input",
            "description": "a completely rewritten sentence",
            "properties": { "name": { "type": "string", "description": "the name" } },
        }));
        assert_eq!(a, b, "editing a description changes no fact");
    }

    #[test]
    fn an_enumeration_carries_its_members() {
        let facts = schema_of(json!({
            "type": "object",
            "properties": {
                "channel": { "enum": ["stable", "prerelease"] },
            },
        }));
        assert_eq!(facts["input.channel"], "enum:prerelease,stable optional");
        let narrowed = schema_of(json!({
            "type": "object",
            "properties": { "channel": { "enum": ["stable"] } },
        }));
        assert_ne!(
            facts["input.channel"], narrowed["input.channel"],
            "removing a member changes the fact"
        );
    }

    #[test]
    fn a_definition_is_resolved_and_a_nested_object_is_walked() {
        let facts = schema_of(json!({
            "type": "object",
            "properties": { "query": { "$ref": "#/$defs/Query" } },
            "$defs": {
                "Query": {
                    "type": "object",
                    "properties": { "limit": { "type": "integer" } },
                    "required": ["limit"],
                }
            },
        }));
        assert_eq!(facts["input.query"], "object optional");
        assert_eq!(facts["input.query.limit"], "integer required");
    }

    #[test]
    fn an_array_carries_its_item_type() {
        let facts = schema_of(json!({
            "type": "object",
            "properties": {
                "scope": { "type": "array", "items": { "type": "string" } },
            },
        }));
        assert_eq!(facts["input.scope"], "array<string> optional");
    }

    #[test]
    fn a_snapshot_refuses_a_fingerprint_that_does_not_match_its_entries() {
        let snapshot = ContractSnapshot {
            schema: SCHEMA.into(),
            entries: vec![ContractEntry {
                surface: Surface::Capability,
                id: "a.b".into(),
                facts: BTreeMap::from([("kind".into(), "query".into())]),
            }],
            fingerprint: "0".repeat(64),
        };
        let text = snapshot.to_json();
        let err = ContractSnapshot::parse(&text).unwrap_err();
        assert!(err.contains("edited after it was written"), "{err}");
    }

    #[test]
    fn a_snapshot_round_trips_through_its_committed_form() {
        let entries = vec![ContractEntry {
            surface: Surface::Capability,
            id: "a.b".into(),
            facts: BTreeMap::from([("kind".into(), "query".into())]),
        }];
        let snapshot = ContractSnapshot {
            schema: SCHEMA.into(),
            fingerprint: fingerprint(&entries),
            entries,
        };
        let back = ContractSnapshot::parse(&snapshot.to_json()).expect("round trip");
        assert_eq!(snapshot, back);
    }

    #[test]
    fn a_snapshot_from_a_later_schema_is_refused_by_name() {
        let text = json!({ "schema": "contract/v2", "entries": [], "fingerprint": "x" });
        let err = ContractSnapshot::parse(&text.to_string()).unwrap_err();
        assert!(err.contains("contract/v2"), "{err}");
    }

    /// The fingerprint is over the facts, not over the file: two builds that write the
    /// same contract with different pretty-printing agree.
    #[test]
    fn the_fingerprint_is_over_the_facts() {
        let a = vec![ContractEntry {
            surface: Surface::Capability,
            id: "a.b".into(),
            facts: BTreeMap::from([("kind".into(), "query".into())]),
        }];
        let b = a.clone();
        assert_eq!(fingerprint(&a), fingerprint(&b));
        let mut c = a.clone();
        c[0].facts.insert("kind".into(), "command".into());
        assert_ne!(fingerprint(&a), fingerprint(&c));
    }

    /// A fact's key and value are separated by a byte no key or value contains, so two
    /// different splits cannot hash the same.
    #[test]
    fn the_fingerprint_cannot_be_confused_by_concatenation() {
        let one = vec![ContractEntry {
            surface: Surface::Capability,
            id: "a".into(),
            facts: BTreeMap::from([("bc".into(), "d".into())]),
        }];
        let other = vec![ContractEntry {
            surface: Surface::Capability,
            id: "a".into(),
            facts: BTreeMap::from([("b".into(), "cd".into())]),
        }];
        assert_ne!(fingerprint(&one), fingerprint(&other));
    }
}
