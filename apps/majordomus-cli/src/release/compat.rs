//! The compatibility engine: how much the public contract moved, and therefore what the
//! smallest honest version is.
//!
//! # The authority this takes away
//!
//! Before this module there were two answers to one question. `scripts/ci/version-matches-surface`
//! compared the registry of two refs and said what the *surface* required; [`super::version`]
//! counted `feat:` and `fix:` in the commit subjects and said what the *commits* implied, and
//! that second answer was the one `release bump` actually wrote. So a capability deleted
//! under a `refactor:` heading was a patch to the writer and a breaking change to the gate,
//! and the tree could carry an under-versioned public contract until CI happened to look.
//!
//! There is one authority here now, and it is the contract:
//!
//! ```text
//!   surface(base) ─┐
//!                  ├─ diff ─ implied impact ─ policy ─ required impact ─ required version
//!   surface(head) ─┘                                          │
//!                                                             ▼
//!                                              one writer, which may not go under it
//! ```
//!
//! Conventional commits did not go away and are not worthless — they are how a change
//! explains itself, and they are the whole of the changelog's prose. They are carried here
//! as [`CommitEvidence`], beside the verdict rather than deciding it, and when they
//! *understate* what the contract did the plan says so explicitly. That line is the point of
//! the subsystem: a human label and a measured fact, disagreeing, in the open.
//!
//! # Why unknown means major
//!
//! [`compare_schema`] models the schema mutations it can name and refuses to guess at the
//! rest: a key it does not understand, changed, is [`Impact::Major`] with a diagnostic
//! saying which pointer and that it is unmodelled. A false major is an argument; a false
//! patch is a broken caller who finds out by breaking. Prose keys are normalised away first
//! (see [`super::surface::normalise`]) precisely so that this conservatism costs nothing on
//! the changes that are not contract changes at all.
//!
//! ```
//! use majordomus_cli::release::compat::{Impact, Mode, Policy};
//! use majordomus_cli::release::version::Version;
//!
//! // Below 1.0.0 a removal costs a minor rather than 1.0.0 — and is still named breaking.
//! let policy = Policy::for_version(Version::parse("0.5.0").unwrap());
//! assert_eq!(policy.mode, Mode::Pre1_0Strict);
//! assert_eq!(policy.required_of(Impact::Major), Impact::Minor);
//!
//! // At 1.0.0 the shift ends and the implied impact is the requirement.
//! let policy = Policy::for_version(Version::parse("1.2.0").unwrap());
//! assert_eq!(policy.required_of(Impact::Major), Impact::Major);
//! assert_eq!(policy.required_of(Impact::None), Impact::None, "an unchanged surface owes nothing");
//! ```

use std::collections::BTreeSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::surface::{PublicCapability, Surface, SurfaceError};
use super::version::Version;
use crate::model::Object;

/// The policy these verdicts were reached under, carried into every plan so that a decision
/// recorded today can still be explained when the rules below change.
pub const POLICY_SCHEMA: &str = "majordomus/version-policy/v1";

/// How much a change moves the contract.
///
/// Ordered, so that "the declared bump covers what is required" is a comparison rather than
/// a table:
///
/// ```
/// use majordomus_cli::release::compat::Impact;
/// assert!(Impact::Major > Impact::Minor);
/// assert!(Impact::Minor > Impact::Patch);
/// assert!(Impact::Patch > Impact::None);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseImpact")]
pub enum Impact {
    /// The contract did not move.
    None,
    /// It moved behind the boundary: a caller cannot tell.
    Patch,
    /// Something arrived; every existing caller still works.
    Minor,
    /// Something a caller could hold is gone or changed under it.
    Major,
}

impl Impact {
    /// The word every surface renders: the command line's report, the JSON every machine
    /// surface answers with, the Cockpit's badge and the gate's refusal all print this and
    /// never a wording of their own.
    ///
    /// ```
    /// use majordomus_cli::release::compat::Impact;
    /// assert_eq!(Impact::Major.as_str(), "major");
    /// assert_eq!(Impact::None.as_str(), "none");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Impact::None => "none",
            Impact::Patch => "patch",
            Impact::Minor => "minor",
            Impact::Major => "major",
        }
    }

    /// The impact a word names.
    ///
    /// Every impact round-trips through its word, and nothing else is one:
    ///
    /// ```
    /// use majordomus_cli::release::compat::Impact;
    /// assert_eq!(Impact::parse("minor"), Some(Impact::Minor));
    /// assert_eq!(Impact::parse("enormous"), None);
    /// ```
    pub fn parse(word: &str) -> Option<Impact> {
        match word {
            "none" => Some(Impact::None),
            "patch" => Some(Impact::Patch),
            "minor" => Some(Impact::Minor),
            "major" => Some(Impact::Major),
            _ => None,
        }
    }
}

/// Whether a change added, removed or altered part of the contract.
///
/// ```
/// use majordomus_cli::release::compat::Direction;
/// assert_eq!(Direction::Removed.sign(), '-');
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseChangeDirection")]
pub enum Direction {
    /// It was not there before.
    Added,
    /// It was there before and is not now.
    Removed,
    /// It is there in both, and differs.
    Changed,
}

impl Direction {
    /// The sign the reports print beside a movement, so that a list of changes reads as a
    /// diff does: what arrived, what left, and what stayed and differs.
    ///
    /// ```
    /// use majordomus_cli::release::compat::Direction;
    /// assert_eq!(Direction::Added.sign(), '+');
    /// assert_eq!(Direction::Changed.sign(), '~');
    /// ```
    pub fn sign(self) -> char {
        match self {
            Direction::Added => '+',
            Direction::Removed => '-',
            Direction::Changed => '~',
        }
    }
}

/// Which projection of the contract a change is in.
///
/// Not where it was *found* — everything here is found in the registry — but which promise
/// it is, so that a reader sees "a route is gone" rather than a JSON pointer.
///
/// ```
/// use majordomus_cli::release::compat::SurfaceKind;
/// assert_eq!(SurfaceKind::Http.as_str(), "route");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseSurfaceKind")]
pub enum SurfaceKind {
    /// The capability itself.
    Capability,
    /// Whether calling it changes anything.
    Kind,
    /// An MCP tool name.
    McpTool,
    /// An MCP resource URI.
    McpResource,
    /// An HTTP method and path.
    Http,
    /// A command-line path.
    Cli,
    /// What a caller may send.
    Input,
    /// What a caller is promised back.
    Output,
}

impl SurfaceKind {
    /// The word the reports print.
    ///
    /// The word names the promise a reader recognises, not the field it came from:
    ///
    /// ```
    /// use majordomus_cli::release::compat::SurfaceKind;
    /// assert_eq!(SurfaceKind::Http.as_str(), "route");
    /// assert_eq!(SurfaceKind::Cli.as_str(), "command");
    /// assert_eq!(SurfaceKind::McpTool.as_str(), "mcp tool");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            SurfaceKind::Capability => "capability",
            SurfaceKind::Kind => "kind",
            SurfaceKind::McpTool => "mcp tool",
            SurfaceKind::McpResource => "mcp resource",
            SurfaceKind::Http => "route",
            SurfaceKind::Cli => "command",
            SurfaceKind::Input => "input",
            SurfaceKind::Output => "output",
        }
    }
}

/// One movement of the public contract, with the reason it counts for what it does.
///
/// ```
/// use majordomus_cli::release::compat::{Direction, Impact, SurfaceChange, SurfaceKind};
/// let change = SurfaceChange {
///     impact: Impact::Major,
///     direction: Direction::Removed,
///     surface: SurfaceKind::Http,
///     capability: "alpha.list".into(),
///     id: "GET /api/v1/alpha".into(),
///     detail: "the capability survives; this way of reaching it does not".into(),
/// };
/// assert_eq!(change.to_string(), "- route GET /api/v1/alpha");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseSurfaceChange")]
pub struct SurfaceChange {
    /// How much it moves the contract on its own.
    pub impact: Impact,
    /// Added, removed, or altered.
    pub direction: Direction,
    /// Which promise it is.
    pub surface: SurfaceKind,
    /// The capability it belongs to.
    pub capability: String,
    /// The thing itself: `GET /api/v1/foo`, `majordomus_release_analysis`, `release.analysis`.
    pub id: String,
    /// Why this impact and not a smaller one, in one clause.
    pub detail: String,
}

impl std::fmt::Display for SurfaceChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.direction.sign(),
            self.surface.as_str(),
            self.id
        )
    }
}

// ---------------------------------------------------------------------------- the diff

/// Every way the contract moved between two surfaces.
///
/// A capability that was renamed appears as a removal and an addition, and deliberately so:
/// that *is* what happened to a caller holding the old name, and collapsing the two into one
/// "rename" would lose the breaking half while looking tidier.
///
/// ```
/// use majordomus_cli::release::compat::{diff, Impact};
/// use majordomus_cli::release::surface::{public_capabilities, Origin, Surface, SURFACE_SCHEMA};
/// use serde_json::json;
///
/// let surface = |caps| Surface {
///     schema: SURFACE_SCHEMA.to_string(),
///     origin: Origin::WorkingTree,
///     capabilities: public_capabilities(&json!({"capabilities": caps})),
/// };
/// let before = surface(json!([{"id": "a.one", "kind": "query", "visibility": "public"}]));
/// let after = surface(json!([
///     {"id": "a.one", "kind": "query", "visibility": "public"},
///     {"id": "a.two", "kind": "query", "visibility": "public"},
/// ]));
///
/// assert!(diff(&before, &before).is_empty(), "a surface diffed with itself is empty");
/// let found = diff(&before, &after);
/// assert_eq!(found.len(), 1);
/// assert_eq!(found[0].impact, Impact::Minor, "an addition breaks nobody");
/// // and the other way round, the same movement is breaking
/// assert_eq!(diff(&after, &before)[0].impact, Impact::Major);
/// ```
pub fn diff(base: &Surface, head: &Surface) -> Vec<SurfaceChange> {
    let mut changes = Vec::new();
    let ids: BTreeSet<&String> = base
        .capabilities
        .keys()
        .chain(head.capabilities.keys())
        .collect();

    for id in ids {
        match (base.capabilities.get(id), head.capabilities.get(id)) {
            (None, Some(_)) => changes.push(SurfaceChange {
                impact: Impact::Minor,
                direction: Direction::Added,
                surface: SurfaceKind::Capability,
                capability: id.clone(),
                id: id.clone(),
                detail: "a capability nothing could call before".into(),
                // Its bindings are not listed separately: they arrived with it, and a
                // reader counting "one new capability" should not also count three atoms.
            }),
            (Some(_), None) => changes.push(SurfaceChange {
                impact: Impact::Major,
                direction: Direction::Removed,
                surface: SurfaceKind::Capability,
                capability: id.clone(),
                id: id.clone(),
                detail: "a caller that held it has nothing to call".into(),
            }),
            (Some(b), Some(h)) => compare_capability(b, h, &mut changes),
            (None, None) => unreachable!("the id came from one of the two maps"),
        }
    }
    changes.sort_by(|a, b| {
        // Worst first, then stable by what it is, so two runs print the same report.
        b.impact
            .cmp(&a.impact)
            .then_with(|| a.capability.cmp(&b.capability))
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.detail.cmp(&b.detail))
    });
    changes
}

/// One capability present in both: every binding, then both schemas.
fn compare_capability(
    base: &PublicCapability,
    head: &PublicCapability,
    out: &mut Vec<SurfaceChange>,
) {
    let id = &base.id;

    if base.kind != head.kind {
        out.push(SurfaceChange {
            impact: Impact::Major,
            direction: Direction::Changed,
            surface: SurfaceKind::Kind,
            capability: id.clone(),
            id: format!("{} -> {}", base.kind, head.kind),
            detail: "whether calling it changes anything is part of the contract".into(),
        });
    }

    binding(
        out,
        id,
        SurfaceKind::McpTool,
        base.mcp_tool.as_deref(),
        head.mcp_tool.as_deref(),
    );
    binding(
        out,
        id,
        SurfaceKind::McpResource,
        base.mcp_resource.as_deref(),
        head.mcp_resource.as_deref(),
    );
    binding(
        out,
        id,
        SurfaceKind::Cli,
        base.cli.as_deref(),
        head.cli.as_deref(),
    );
    let b_http = base.http.as_ref().map(ToString::to_string);
    let h_http = head.http.as_ref().map(ToString::to_string);
    binding(
        out,
        id,
        SurfaceKind::Http,
        b_http.as_deref(),
        h_http.as_deref(),
    );

    schema(
        out,
        id,
        SurfaceKind::Input,
        base.input.as_ref(),
        head.input.as_ref(),
    );
    schema(
        out,
        id,
        SurfaceKind::Output,
        base.output.as_ref(),
        head.output.as_ref(),
    );
}

/// One optional name a caller can hold: gone is major, arrived is minor, moved is both.
fn binding(
    out: &mut Vec<SurfaceChange>,
    capability: &str,
    kind: SurfaceKind,
    base: Option<&str>,
    head: Option<&str>,
) {
    match (base, head) {
        (Some(b), Some(h)) if b != h => {
            // Two entries, not one "changed": a caller holding `b` is broken, and `h` is a
            // thing that did not exist. Reporting a single move would understate the first.
            out.push(SurfaceChange {
                impact: Impact::Major,
                direction: Direction::Removed,
                surface: kind,
                capability: capability.to_string(),
                id: b.to_string(),
                detail: format!("moved to `{h}`; a caller holding this one breaks"),
            });
            out.push(SurfaceChange {
                impact: Impact::Minor,
                direction: Direction::Added,
                surface: kind,
                capability: capability.to_string(),
                id: h.to_string(),
                detail: format!("where `{b}` used to answer"),
            });
        }
        (Some(b), None) => out.push(SurfaceChange {
            impact: Impact::Major,
            direction: Direction::Removed,
            surface: kind,
            capability: capability.to_string(),
            id: b.to_string(),
            detail: "the capability survives; this way of reaching it does not".into(),
        }),
        (None, Some(h)) => out.push(SurfaceChange {
            impact: Impact::Minor,
            direction: Direction::Added,
            surface: kind,
            capability: capability.to_string(),
            id: h.to_string(),
            detail: "a new way to reach a capability that already existed".into(),
        }),
        _ => {}
    }
}

/// One of the two contracts, compared structurally.
fn schema(
    out: &mut Vec<SurfaceChange>,
    capability: &str,
    role: SurfaceKind,
    base: Option<&Value>,
    head: Option<&Value>,
) {
    match (base, head) {
        (Some(b), Some(h)) => {
            let mut found = Vec::new();
            compare_schema(role, b, h, "", &mut found);
            for (impact, pointer, detail) in found {
                out.push(SurfaceChange {
                    impact,
                    direction: Direction::Changed,
                    surface: role,
                    capability: capability.to_string(),
                    id: if pointer.is_empty() {
                        "the schema".to_string()
                    } else {
                        pointer
                    },
                    detail,
                });
            }
        }
        // A contract that appears constrains a caller that had none; one that disappears
        // stops promising. Neither is a shape this repository produces — every capability
        // declares both — so both are reported rather than ignored.
        (None, Some(_)) => out.push(SurfaceChange {
            impact: if role == SurfaceKind::Input {
                Impact::Major
            } else {
                Impact::Minor
            },
            direction: Direction::Added,
            surface: role,
            capability: capability.to_string(),
            id: "the schema".into(),
            detail: "a contract where there was none".into(),
        }),
        (Some(_), None) => out.push(SurfaceChange {
            impact: Impact::Major,
            direction: Direction::Removed,
            surface: role,
            capability: capability.to_string(),
            id: "the schema".into(),
            detail: "the contract it stated is gone".into(),
        }),
        (None, None) => {}
    }
}

/// The schema comparison, by JSON pointer.
///
/// `role` decides the direction compatibility runs in, and it is the whole reason the two
/// are not one function: an input is what a caller *sends*, so accepting more is compatible
/// and demanding more is breaking; an output is what a caller *receives*, so promising more
/// is compatible and promising less is breaking. The same edit is minor in one and major in
/// the other, which is exactly the mistake a textual diff of two schemas makes.
fn compare_schema(
    role: SurfaceKind,
    base: &Value,
    head: &Value,
    at: &str,
    out: &mut Vec<(Impact, String, String)>,
) {
    if base == head {
        return;
    }
    let input = role == SurfaceKind::Input;
    let here = |suffix: &str| {
        if suffix.is_empty() {
            at.to_string()
        } else if at.is_empty() {
            suffix.to_string()
        } else {
            format!("{at}.{suffix}")
        }
    };

    let (Some(b), Some(h)) = (base.as_object(), head.as_object()) else {
        // `true` / `false` schemas, or one side replaced wholesale.
        out.push((
            Impact::Major,
            here(""),
            "the schema was replaced by one of a different shape".into(),
        ));
        return;
    };

    // ------------------------------------------------------------------------ type
    if b.get("type") != h.get("type") {
        out.push((
            Impact::Major,
            here("type"),
            format!(
                "the type changed from {} to {}",
                render(b.get("type")),
                render(h.get("type"))
            ),
        ));
    }

    // ------------------------------------------------------------------------ enum
    // A value that was accepted and is not any more breaks the caller that sends it; one
    // that is accepted now and was not breaks nobody. The same holds for an output read by
    // a caller that only knows the old values, which is why widening is minor and not none.
    values(b.get("enum"), h.get("enum"), &mut |gone, new| {
        for v in gone {
            out.push((
                Impact::Major,
                here("enum"),
                format!("the value {v} is no longer accepted"),
            ));
        }
        for v in new {
            out.push((
                Impact::Minor,
                here("enum"),
                format!("the value {v} is accepted as well"),
            ));
        }
    });

    // --------------------------------------------------------- oneOf / anyOf / allOf
    for key in ["oneOf", "anyOf"] {
        values(b.get(key), h.get(key), &mut |gone, new| {
            for v in gone {
                out.push((Impact::Major, here(key), format!("the variant {v} is gone")));
            }
            for v in new {
                out.push((Impact::Minor, here(key), format!("the variant {v} is new")));
            }
        });
    }
    if b.get("allOf") != h.get("allOf") {
        // Every branch of an allOf must hold at once, so adding one narrows and removing
        // one widens; rather than model that, it is named and called breaking.
        out.push((
            Impact::Major,
            here("allOf"),
            "the conjunction of constraints changed".into(),
        ));
    }

    // -------------------------------------------------------------------- required
    let b_req = string_set(b.get("required"));
    let h_req = string_set(h.get("required"));
    for name in h_req.difference(&b_req) {
        out.push(if input {
            (
                Impact::Major,
                here(&format!("required.{name}")),
                format!("`{name}` is now required; a caller that did not send it is refused"),
            )
        } else {
            (
                Impact::Minor,
                here(&format!("required.{name}")),
                format!("`{name}` is now always present; a stronger promise than before"),
            )
        });
    }
    for name in b_req.difference(&h_req) {
        out.push(if input {
            (
                Impact::Minor,
                here(&format!("required.{name}")),
                format!("`{name}` is no longer required; every existing caller still works"),
            )
        } else {
            (
                Impact::Major,
                here(&format!("required.{name}")),
                format!("`{name}` is no longer promised; a caller that reads it may find nothing"),
            )
        });
    }

    // ------------------------------------------------------------------ properties
    let empty = serde_json::Map::new();
    let b_props = b
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap_or(&empty);
    let h_props = h
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap_or(&empty);
    let names: BTreeSet<&String> = b_props.keys().chain(h_props.keys()).collect();
    for name in names {
        match (b_props.get(name), h_props.get(name)) {
            (None, Some(_)) => out.push((
                Impact::Minor,
                here(&format!("properties.{name}")),
                if input {
                    format!("`{name}` may now be sent; nothing that worked stopped working")
                } else {
                    format!("`{name}` is returned as well; a caller ignoring it is unaffected")
                },
            )),
            (Some(_), None) => out.push((
                Impact::Major,
                here(&format!("properties.{name}")),
                if input {
                    // Every input schema in this repository derives `deny_unknown_fields`,
                    // so a property that is gone is not ignored — it is rejected.
                    format!("`{name}` is refused now; a caller still sending it gets an error")
                } else {
                    format!("`{name}` is gone; a caller reading it finds nothing")
                },
            )),
            (Some(bp), Some(hp)) => {
                compare_schema(role, bp, hp, &here(&format!("properties.{name}")), out)
            }
            (None, None) => unreachable!("the name came from one of the two maps"),
        }
    }

    // ----------------------------------------------------------------------- $defs
    for key in ["$defs", "definitions"] {
        let b_defs = b.get(key).and_then(|d| d.as_object()).unwrap_or(&empty);
        let h_defs = h.get(key).and_then(|d| d.as_object()).unwrap_or(&empty);
        let names: BTreeSet<&String> = b_defs.keys().chain(h_defs.keys()).collect();
        for name in names {
            match (b_defs.get(name), h_defs.get(name)) {
                (Some(bd), Some(hd)) => {
                    compare_schema(role, bd, hd, &here(&format!("{key}.{name}")), out)
                }
                // A definition is only emitted when something refers to it, so one arriving
                // or leaving is a referenced shape arriving or leaving.
                (None, Some(_)) => out.push((
                    Impact::Minor,
                    here(&format!("{key}.{name}")),
                    format!("the shape `{name}` is part of the contract now"),
                )),
                (Some(_), None) => out.push((
                    Impact::Major,
                    here(&format!("{key}.{name}")),
                    format!("the shape `{name}` is gone"),
                )),
                (None, None) => unreachable!("the name came from one of the two maps"),
            }
        }
    }

    // ----------------------------------------------------------------------- items
    for key in ["items", "additionalItems", "contains"] {
        match (b.get(key), h.get(key)) {
            (Some(bi), Some(hi)) => compare_schema(role, bi, hi, &here(key), out),
            (None, None) => {}
            _ => out.push((
                Impact::Major,
                here(key),
                format!("`{key}` is stated on one side only"),
            )),
        }
    }
    if let (Some(Value::Array(ba)), Some(Value::Array(ha))) =
        (b.get("prefixItems"), h.get("prefixItems"))
    {
        // Positional: index i of one is index i of the other, and a length change is a
        // change to what a caller must supply or may read.
        if ba.len() != ha.len() {
            out.push((
                Impact::Major,
                here("prefixItems"),
                format!("the tuple went from {} to {} positions", ba.len(), ha.len()),
            ));
        }
        for (i, (bi, hi)) in ba.iter().zip(ha).enumerate() {
            compare_schema(role, bi, hi, &here(&format!("prefixItems.{i}")), out);
        }
    }

    // ------------------------------------------------------ additionalProperties, $ref
    if b.get("additionalProperties") != h.get("additionalProperties") {
        match (b.get("additionalProperties"), h.get("additionalProperties")) {
            // Closing an open object refuses input that used to be accepted; opening a
            // closed one accepts more. For an output the reasoning runs the other way, and
            // neither direction is common enough here to be worth more than naming it.
            (Some(Value::Bool(true)) | None, Some(Value::Bool(false))) => out.push((
                Impact::Major,
                here("additionalProperties"),
                "unknown properties are refused now".into(),
            )),
            (Some(Value::Bool(false)), Some(Value::Bool(true)) | None) => out.push((
                if input { Impact::Minor } else { Impact::Major },
                here("additionalProperties"),
                if input {
                    "unknown properties are accepted now".into()
                } else {
                    "the shape of what is returned is no longer closed".to_string()
                },
            )),
            _ => out.push((
                Impact::Major,
                here("additionalProperties"),
                "what may appear beyond the declared properties changed".into(),
            )),
        }
    }
    if b.get("$ref") != h.get("$ref") {
        out.push((
            Impact::Major,
            here("$ref"),
            format!(
                "it points at {} instead of {}",
                render(h.get("$ref")),
                render(b.get("$ref"))
            ),
        ));
    }

    // ------------------------------------------------------- everything not modelled
    //
    // A constraint this comparator has no rule for, that differs, is breaking — because the
    // alternative is to call an unknown contract change a patch, and a caller finds that out
    // by breaking. The pointer and the key are named so the answer is arguable rather than
    // mysterious, and every key above is excluded because it already has a rule.
    const MODELLED: &[&str] = &[
        "type",
        "enum",
        "oneOf",
        "anyOf",
        "allOf",
        "required",
        "properties",
        "$defs",
        "definitions",
        "items",
        "additionalItems",
        "contains",
        "prefixItems",
        "additionalProperties",
        "$ref",
    ];
    let keys: BTreeSet<&String> = b.keys().chain(h.keys()).collect();
    for key in keys {
        if MODELLED.contains(&key.as_str()) {
            continue;
        }
        if b.get(key) != h.get(key) {
            out.push((
                Impact::Major,
                here(key),
                format!(
                    "`{key}` changed from {} to {}, and this comparator does not model what \
                     that does to a caller; it is counted as breaking rather than guessed at",
                    render(b.get(key)),
                    render(h.get(key))
                ),
            ));
        }
    }
}

/// Compare two arrays as sets of values, calling back with what went and what arrived.
fn values(
    base: Option<&Value>,
    head: Option<&Value>,
    f: &mut impl FnMut(Vec<String>, Vec<String>),
) {
    let set = |v: Option<&Value>| -> BTreeSet<String> {
        v.and_then(|v| v.as_array())
            .map(|a| a.iter().map(compact).collect())
            .unwrap_or_default()
    };
    let (b, h) = (set(base), set(head));
    if b == h {
        return;
    }
    f(
        b.difference(&h).cloned().collect(),
        h.difference(&b).cloned().collect(),
    )
}

/// The `required` list as a set.
fn string_set(v: Option<&Value>) -> BTreeSet<String> {
    v.and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// One value, compactly, for a message a person reads.
fn compact(v: &Value) -> String {
    match v {
        Value::String(s) => format!("`{s}`"),
        other => serde_json::to_string(other).unwrap_or_else(|_| "?".into()),
    }
}

/// A value that may be absent, for a message a person reads.
fn render(v: Option<&Value>) -> String {
    v.map_or_else(|| "absent".to_string(), compact)
}

// -------------------------------------------------------------------------- the policy

/// How this project turns a measured impact into the version it owes.
///
/// Semantic versioning hands `0.y.z` a blanket exemption — "anything MAY change at any
/// time" — and Elm refuses the exemption by starting every package at `1.0.0`, because a
/// rule with an escape hatch is a rule about when to use the hatch. This repository is `0.x`
/// and has not earned `1.0.0`, so neither answer is available: demanding `major` would
/// demand `1.0.0` for a single removal, and granting the exemption would let a breaking
/// change ship as a patch.
///
/// So below `1.0.0` the floor is a minor. Any movement of the surface, gone or new, is at
/// least a minor release; only an unchanged surface owes nothing. That is the strongest
/// signal `0.x` has, and a removal is still *named* as breaking whatever the verdict, so the
/// evidence does not quietly become a minor along with the number.
///
/// At `1.0.0` the shift ends: [`Mode::Semver`] is the ordinary rule, and the transition is
/// one line of [`Policy::required_of`].
///
/// ```
/// use majordomus_cli::release::compat::{Mode, Policy};
/// use majordomus_cli::release::version::Version;
/// assert_eq!(Policy::for_version(Version::parse("0.9.0").unwrap()).mode, Mode::Pre1_0Strict);
/// assert_eq!(Policy::for_version(Version::parse("1.0.0").unwrap()).mode, Mode::Semver);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ReleasePolicyMode")]
pub enum Mode {
    /// Below `1.0.0`: breaking changes are allowed, and cost a minor rather than nothing.
    Pre1_0Strict,
    /// At or above `1.0.0`: the impact is the requirement.
    Semver,
}

/// The compatibility policy, named and versioned.
///
/// ```
/// use majordomus_cli::release::compat::{Impact, Policy, POLICY_SCHEMA};
/// use majordomus_cli::release::version::Version;
/// let policy = Policy::for_version(Version::parse("0.5.0").unwrap());
/// assert_eq!(policy.schema, POLICY_SCHEMA, "a verdict carries the rules it was reached under");
/// assert_eq!(policy.required_of(Impact::Major), Impact::Minor);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleasePolicy")]
pub struct Policy {
    /// `majordomus/version-policy/v1`.
    pub schema: String,
    /// Which rule is in force, decided by the baseline version alone.
    pub mode: Mode,
}

impl Policy {
    /// The policy that governs a project at this version.
    ///
    /// ```
    /// use majordomus_cli::release::compat::{Mode, Policy};
    /// use majordomus_cli::release::version::Version;
    /// // the baseline decides the mode, not the version being written
    /// assert_eq!(Policy::for_version(Version::parse("0.1.0").unwrap()).mode, Mode::Pre1_0Strict);
    /// assert_eq!(Policy::for_version(Version::parse("2.3.4").unwrap()).mode, Mode::Semver);
    /// ```
    pub fn for_version(base: Version) -> Policy {
        Policy {
            schema: POLICY_SCHEMA.to_string(),
            mode: if base.major == 0 {
                Mode::Pre1_0Strict
            } else {
                Mode::Semver
            },
        }
    }

    /// The smallest release this impact is allowed to ship as.
    ///
    /// An unchanged surface owes nothing under either mode. This runs over every tree, not
    /// only over a release, and most commits are behind the boundary: demanding a bump for
    /// each of them would make the gate a thing to be worked around within a day.
    ///
    /// ```
    /// use majordomus_cli::release::compat::{Impact, Policy};
    /// use majordomus_cli::release::version::Version;
    ///
    /// let pre = Policy::for_version(Version::parse("0.5.0").unwrap());
    /// assert_eq!(pre.required_of(Impact::None), Impact::None);
    /// assert_eq!(pre.required_of(Impact::Minor), Impact::Minor);
    /// assert_eq!(pre.required_of(Impact::Major), Impact::Minor, "0.x floors a removal at a minor");
    ///
    /// let one = Policy::for_version(Version::parse("1.0.0").unwrap());
    /// assert_eq!(one.required_of(Impact::Major), Impact::Major);
    /// ```
    pub fn required_of(&self, implied: Impact) -> Impact {
        match implied {
            Impact::None | Impact::Patch => Impact::None,
            Impact::Minor => Impact::Minor,
            Impact::Major => match self.mode {
                Mode::Pre1_0Strict => Impact::Minor,
                Mode::Semver => Impact::Major,
            },
        }
    }

    /// How this policy explains itself, in the plan and in `--explain`.
    ///
    /// ```
    /// use majordomus_cli::release::compat::Policy;
    /// use majordomus_cli::release::version::Version;
    /// let policy = Policy::for_version(Version::parse("0.5.0").unwrap());
    /// assert!(policy.statement().contains("below 1.0.0"));
    /// ```
    pub fn statement(&self) -> &'static str {
        match self.mode {
            Mode::Pre1_0Strict => {
                "below 1.0.0: an unchanged surface owes nothing, and any movement of it — \
                 arrived or gone — is at least a minor; a removal is named as breaking but \
                 does not demand 1.0.0"
            }
            Mode::Semver => {
                "at or above 1.0.0: an unchanged surface owes nothing, an addition is a \
                 minor, and anything a caller could hold that is gone or changed under it \
                 is a major"
            }
        }
    }
}

// ------------------------------------------------------------------------- the baseline

/// The release this tree is measured against, and how it was found.
///
/// ```
/// use majordomus_cli::release::compat::Baseline;
/// let baseline = Baseline {
///     version: "0.5.0".into(),
///     reference: "v0.5.0".into(),
///     read_at: "3a032c1e033b".into(),
///     commit: "3a032c1e033b".into(),
///     recorded: true,
///     atoms: 300,
///     fingerprint: "sha256:…".into(),
/// };
/// // `recorded` is the half that matters: a baseline taken from a tag alone is reported.
/// assert!(baseline.recorded);
/// assert_eq!(baseline.reference, "v0.5.0");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseBaseline")]
pub struct Baseline {
    /// `0.5.0`.
    pub version: String,
    /// `v0.5.0`: the release, as a reader recognises it.
    pub reference: String,
    /// The ref the surface was actually read from — the commit the release record names,
    /// which a clone always has, rather than a tag it may not.
    pub read_at: String,
    /// The commit that ref resolved to.
    pub commit: String,
    /// Whether the layer holds a release record for it, which is what makes it canonical
    /// rather than the newest thing that looked like a tag.
    pub recorded: bool,
    /// How many public atoms it published.
    pub atoms: usize,
    /// The fingerprint of the surface it published.
    pub fingerprint: String,
}

/// Something about the release state that a person should be told, whatever the verdict.
///
/// ```
/// use majordomus_cli::release::compat::{Diagnostic, Severity};
/// let d = Diagnostic {
///     id: "tag-without-record".into(),
///     severity: Severity::Warning,
///     message: "v0.1.0 is tagged and the layer holds no release record for it".into(),
/// };
/// assert_eq!(d.severity, Severity::Warning, "a warning states the plan still stands");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseDiagnostic")]
pub struct Diagnostic {
    /// `tag-without-record`, `record-without-tag`, `tag-commit-mismatch`.
    pub id: String,
    /// Whether it stops a release or only warns.
    pub severity: Severity,
    /// What is wrong, in one sentence.
    pub message: String,
}

/// How much a diagnostic matters.
///
/// An error says the plan itself cannot be trusted and blocks the verdict; a warning says
/// something about the release state is wrong and the verdict stands anyway.
///
/// ```
/// use majordomus_cli::release::compat::Severity;
/// assert_ne!(Severity::Error, Severity::Warning);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseSeverity")]
pub enum Severity {
    /// The plan cannot be trusted; the verdict is blocked.
    Error,
    /// The plan stands, and something is still wrong.
    Warning,
}

/// Every version tag this repository carries, newest last.
fn tags(root: &Path) -> Vec<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["tag", "--list", "v[0-9]*"])
        .output();
    let Ok(out) = out else { return Vec::new() };
    let mut list: Vec<(Version, String)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| Version::parse(l.trim_start_matches('v')).map(|v| (v, l.to_string())))
        .collect();
    list.sort_by_key(|(v, _)| (v.major, v.minor, v.patch));
    list.into_iter().map(|(_, t)| t).collect()
}

/// The release records the layer holds, newest last by version.
fn records(objects: &[Object]) -> Vec<(Version, String, Option<String>, Option<String>)> {
    let mut list: Vec<_> = objects
        .iter()
        .filter(|o| o.kind == super::changelog::RELEASE_KIND)
        .filter_map(|o| {
            let raw = o.metadata.get("version")?.as_str()?;
            let v = Version::parse(raw)?;
            let tag = o
                .metadata
                .get("tag")
                .and_then(|t| t.as_str())
                .map(str::to_string);
            let commit = o
                .metadata
                .get("commit")
                .and_then(|c| c.as_str())
                .map(str::to_string);
            Some((v, raw.to_string(), tag, commit))
        })
        .collect();
    list.sort_by_key(|(v, _, _, _)| (v.major, v.minor, v.patch));
    list
}

/// Find the release to measure against, and say what is incoherent about the release state
/// on the way.
///
/// The canonical baseline is the newest release the *layer records*, because a record is
/// what this repository writes when something is actually published; a tag is git's copy of
/// that fact and can be created by anyone. When the two disagree the record still wins and
/// the disagreement is reported, so the answer is never silently drawn from whichever source
/// happened to be newer.
fn resolve_baseline(
    root: &Path,
    objects: &[Object],
    since: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Baseline, SurfaceError> {
    let tags = tags(root);
    let records = records(objects);

    // Every tag should have a record and every record a tag; neither direction is checked
    // anywhere else, and each is how a release goes half-published.
    let tagged: BTreeSet<String> = tags.iter().cloned().collect();
    let recorded_tags: BTreeSet<String> = records
        .iter()
        .map(|(_, raw, tag, _)| tag.clone().unwrap_or_else(|| format!("v{raw}")))
        .collect();
    for t in tagged.difference(&recorded_tags) {
        diagnostics.push(Diagnostic {
            id: "tag-without-record".into(),
            severity: Severity::Warning,
            message: format!(
                "{t} is tagged and the layer holds no release record for it; \
                 the changelog and every baseline read the records, so this release is invisible to them"
            ),
        });
    }
    for t in recorded_tags.difference(&tagged) {
        diagnostics.push(Diagnostic {
            id: "record-without-tag".into(),
            severity: Severity::Warning,
            message: format!(
                "the layer records {t} and no such tag exists in this clone; \
                 either it was never pushed or the tags were not fetched"
            ),
        });
    }

    // The ref to read the surface from. For a record, that is the *commit* it names rather
    // than its tag: a record always carries the commit it published, and a tag is git's copy
    // of that fact — absent from a shallow clone, absent from a fork, and movable. The tag
    // stays the name the reports print, because that is what a reader recognises.
    let (version, reference, read_at) = match since {
        Some(r) => (
            Version::parse(r.trim_start_matches('v')).map_or_else(String::new, |v| v.to_string()),
            r.to_string(),
            r.to_string(),
        ),
        None => match records.last() {
            Some((_, raw, tag, commit)) => (
                raw.clone(),
                tag.clone().unwrap_or_else(|| format!("v{raw}")),
                commit
                    .clone()
                    .or_else(|| tag.clone())
                    .unwrap_or_else(|| format!("v{raw}")),
            ),
            None => match tags.last() {
                Some(t) => {
                    diagnostics.push(Diagnostic {
                        id: "baseline-from-tag".into(),
                        severity: Severity::Warning,
                        message: format!(
                            "the layer records no release, so {t} was taken as the baseline from git's tags alone"
                        ),
                    });
                    (t.trim_start_matches('v').to_string(), t.clone(), t.clone())
                }
                None => return Err(SurfaceError::NothingPublished),
            },
        },
    };

    let surface = Surface::read_ref(root, &read_at)?;
    let commit = match &surface.origin {
        super::surface::Origin::Ref { commit, .. } => commit.clone(),
        super::surface::Origin::WorkingTree => String::new(),
    };

    // The record's own commit against the one the tag points at: a tag moved after the
    // record was written makes every comparison since then measure the wrong tree.
    let recorded = records.iter().find(|(_, raw, _, _)| *raw == version);
    if let Some((_, _, _, Some(record_commit))) = recorded {
        if !commit.is_empty() && record_commit != &commit {
            diagnostics.push(Diagnostic {
                id: "tag-commit-mismatch".into(),
                severity: Severity::Error,
                message: format!(
                    "the release record for {version} names commit {} and {reference} points at {}; \
                     the surface being compared with is not the one that was published",
                    &record_commit[..record_commit.len().min(12)],
                    &commit[..commit.len().min(12)]
                ),
            });
        }
    }

    Ok(Baseline {
        version,
        reference,
        read_at,
        commit,
        recorded: recorded.is_some(),
        atoms: surface.atoms(),
        fingerprint: surface.fingerprint(),
    })
}

// ---------------------------------------------------------------------------- the plan

/// What the commit subjects since the baseline said about themselves.
///
/// Evidence, never authority. It is here so that a person can see the human account and the
/// measured one side by side, and so that [`VersionPlan::understated`] can say when the two
/// disagree — which is the case this whole subsystem exists to catch.
///
/// ```
/// use majordomus_cli::release::compat::{CommitEvidence, Impact};
/// let commits = CommitEvidence {
///     commits: 24,
///     implied: Impact::Patch,
///     breaking: Vec::new(),
/// };
/// // The contract moved by a minor and the subjects claim a patch: the plan reports that
/// // disagreement rather than deferring to the words.
/// assert!(commits.implied < Impact::Minor);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseCommitEvidence")]
pub struct CommitEvidence {
    /// How many commits since the baseline.
    pub commits: usize,
    /// What their conventional-commit types alone would have implied.
    pub implied: Impact,
    /// The subjects of the ones that claimed to be breaking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub breaking: Vec<String>,
}

/// Whether the tree may land or ship as it stands.
///
/// ```
/// use majordomus_cli::release::compat::Status;
/// assert_ne!(Status::Ok, Status::Blocked);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "ReleaseStatus")]
pub enum Status {
    /// The declared version covers what the contract did.
    Ok,
    /// It does not, and the plan says by how much.
    Blocked,
}

/// The whole answer: what moved, what that requires, and what the tree declares.
///
/// One value, and every surface renders it — the command line, `/api/v1/release/analysis`,
/// the MCP tool, the Cockpit panel, the generated document and the CI gate. There is no
/// second computation anywhere, which is the property that makes the number mean something.
///
/// ```
/// use majordomus_cli::release::compat::{Status, VersionPlan};
/// # use majordomus_cli::release::compat::{Baseline, CommitEvidence, Impact, Policy};
/// # use majordomus_cli::release::version::Version;
/// let plan = VersionPlan {
///     policy: Policy::for_version(Version::parse("0.5.0").unwrap()),
///     baseline: Baseline {
///         version: "0.5.0".into(), reference: "v0.5.0".into(), read_at: "3a032c1e".into(),
///         commit: "3a032c1e".into(), recorded: true, atoms: 300, fingerprint: "sha256:a".into(),
///     },
///     declared_version: "0.5.0".into(),
///     tool_version: "0.5.0".into(),
///     writers_agree: true,
///     atoms: 333,
///     fingerprint: "sha256:b".into(),
///     implied: Impact::Minor,
///     required: Impact::Minor,
///     declared: Impact::None,
///     required_version: "0.6.0".into(),
///     status: Status::Blocked,
///     breaking: false,
///     changes: Vec::new(),
///     commits: CommitEvidence { commits: 24, implied: Impact::Patch, breaking: Vec::new() },
///     understated: true,
///     diagnostics: Vec::new(),
/// };
/// // The contract requires a minor and the version declares none: this tree may not land.
/// assert_eq!(plan.status, Status::Blocked);
/// assert!(plan.declared < plan.required);
/// assert!(plan.understated, "the commit subjects claim less than the contract did");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseVersionPlan")]
pub struct VersionPlan {
    /// The policy the verdict was reached under.
    pub policy: Policy,
    /// The release this was measured against.
    pub baseline: Baseline,
    /// The version the crate manifest declares.
    pub declared_version: String,
    /// The version `bin/majordomus` prints, and whether the two agree.
    pub tool_version: String,
    /// Whether the two writers of the version state one value.
    pub writers_agree: bool,
    /// How many public atoms this tree carries.
    pub atoms: usize,
    /// The fingerprint of this tree's surface.
    pub fingerprint: String,
    /// What the surface movement amounts to on its own.
    pub implied: Impact,
    /// What the policy requires of it.
    pub required: Impact,
    /// What the two version numbers already represent.
    pub declared: Impact,
    /// The smallest version this tree may declare.
    pub required_version: String,
    /// Whether it does.
    pub status: Status,
    /// Whether anything a caller could hold is gone.
    pub breaking: bool,
    /// Every movement of the contract, worst first.
    pub changes: Vec<SurfaceChange>,
    /// What the commits said about themselves.
    pub commits: CommitEvidence,
    /// Whether the commit subjects understate what the contract actually did.
    pub understated: bool,
    /// Anything incoherent about the release state, whatever the verdict.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

impl VersionPlan {
    /// How many changes moved the contract in each direction.
    ///
    /// ```
    /// # use majordomus_cli::release::compat::*;
    /// # use majordomus_cli::release::version::Version;
    /// # fn plan(changes: Vec<SurfaceChange>) -> VersionPlan {
    /// #   VersionPlan { policy: Policy::for_version(Version::parse("0.5.0").unwrap()),
    /// #     baseline: Baseline { version: "0.5.0".into(), reference: "v0.5.0".into(),
    /// #       read_at: "c".into(), commit: "c".into(), recorded: true, atoms: 1,
    /// #       fingerprint: "sha256:a".into() },
    /// #     declared_version: "0.5.0".into(), tool_version: "0.5.0".into(), writers_agree: true,
    /// #     atoms: 1, fingerprint: "sha256:b".into(), implied: Impact::Minor,
    /// #     required: Impact::Minor, declared: Impact::None, required_version: "0.6.0".into(),
    /// #     status: Status::Blocked, breaking: false, changes,
    /// #     commits: CommitEvidence { commits: 0, implied: Impact::None, breaking: Vec::new() },
    /// #     understated: false, diagnostics: Vec::new() }
    /// # }
    /// # fn change(direction: Direction) -> SurfaceChange {
    /// #   SurfaceChange { impact: Impact::Minor, direction, surface: SurfaceKind::Capability,
    /// #     capability: "a.one".into(), id: "a.one".into(), detail: String::new() }
    /// # }
    /// let p = plan(vec![change(Direction::Added), change(Direction::Added), change(Direction::Removed)]);
    /// assert_eq!(p.counts(), (2, 0, 1), "added, changed, removed");
    /// ```
    pub fn counts(&self) -> (usize, usize, usize) {
        let n = |d: Direction| self.changes.iter().filter(|c| c.direction == d).count();
        (
            n(Direction::Added),
            n(Direction::Changed),
            n(Direction::Removed),
        )
    }

    /// Everything that is breaking on its own.
    ///
    /// ```
    /// # use majordomus_cli::release::compat::*;
    /// # use majordomus_cli::release::version::Version;
    /// # let breaking = SurfaceChange { impact: Impact::Major, direction: Direction::Removed,
    /// #   surface: SurfaceKind::Http, capability: "a.one".into(), id: "GET /x".into(),
    /// #   detail: String::new() };
    /// # let additive = SurfaceChange { impact: Impact::Minor, ..breaking.clone() };
    /// # let plan = VersionPlan { policy: Policy::for_version(Version::parse("1.0.0").unwrap()),
    /// #   baseline: Baseline { version: "1.0.0".into(), reference: "v1.0.0".into(),
    /// #     read_at: "c".into(), commit: "c".into(), recorded: true, atoms: 1,
    /// #     fingerprint: "sha256:a".into() },
    /// #   declared_version: "1.0.0".into(), tool_version: "1.0.0".into(), writers_agree: true,
    /// #   atoms: 1, fingerprint: "sha256:b".into(), implied: Impact::Major,
    /// #   required: Impact::Major, declared: Impact::None, required_version: "2.0.0".into(),
    /// #   status: Status::Blocked, breaking: true, changes: vec![breaking, additive],
    /// #   commits: CommitEvidence { commits: 0, implied: Impact::None, breaking: Vec::new() },
    /// #   understated: false, diagnostics: Vec::new() };
    /// assert_eq!(plan.breaking_changes().count(), 1, "only what is breaking on its own");
    /// ```
    pub fn breaking_changes(&self) -> impl Iterator<Item = &SurfaceChange> {
        self.changes.iter().filter(|c| c.impact == Impact::Major)
    }

    /// Whether anything makes the plan itself untrustworthy.
    ///
    /// ```
    /// # use majordomus_cli::release::compat::*;
    /// # use majordomus_cli::release::version::Version;
    /// # let mut plan = VersionPlan { policy: Policy::for_version(Version::parse("0.5.0").unwrap()),
    /// #   baseline: Baseline { version: "0.5.0".into(), reference: "v0.5.0".into(),
    /// #     read_at: "c".into(), commit: "c".into(), recorded: true, atoms: 1,
    /// #     fingerprint: "sha256:a".into() },
    /// #   declared_version: "0.5.0".into(), tool_version: "0.5.0".into(), writers_agree: true,
    /// #   atoms: 1, fingerprint: "sha256:b".into(), implied: Impact::None,
    /// #   required: Impact::None, declared: Impact::None, required_version: "0.5.0".into(),
    /// #   status: Status::Ok, breaking: false, changes: Vec::new(),
    /// #   commits: CommitEvidence { commits: 0, implied: Impact::None, breaking: Vec::new() },
    /// #   understated: false, diagnostics: Vec::new() };
    /// assert!(!plan.has_errors());
    /// // A warning leaves the plan standing; only an error makes it untrustworthy.
    /// plan.diagnostics.push(Diagnostic { id: "tag-without-record".into(),
    ///     severity: Severity::Warning, message: "…".into() });
    /// assert!(!plan.has_errors());
    /// plan.diagnostics.push(Diagnostic { id: "writers-disagree".into(),
    ///     severity: Severity::Error, message: "…".into() });
    /// assert!(plan.has_errors());
    /// ```
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

/// The bump two version numbers already represent.
///
/// A number going *down* is not a bump at all, and is reported as [`Impact::None`] so that
/// the comparison with the requirement refuses it rather than ranking it.
///
/// ```
/// use majordomus_cli::release::compat::{declared_impact, Impact};
/// use majordomus_cli::release::version::Version;
/// let v = |s| Version::parse(s).unwrap();
/// assert_eq!(declared_impact(v("0.5.0"), v("0.6.0")), Impact::Minor);
/// assert_eq!(declared_impact(v("0.5.0"), v("1.0.0")), Impact::Major);
/// // A version going down is no bump at all, so it can never satisfy a requirement.
/// assert_eq!(declared_impact(v("0.5.0"), v("0.4.0")), Impact::None);
/// ```
pub fn declared_impact(from: Version, to: Version) -> Impact {
    if to.major > from.major {
        Impact::Major
    } else if to.major < from.major {
        Impact::None
    } else if to.minor > from.minor {
        Impact::Minor
    } else if to.minor < from.minor {
        Impact::None
    } else if to.patch > from.patch {
        Impact::Patch
    } else {
        Impact::None
    }
}

/// The one analysis. Everything that answers a version question calls this.
///
/// ```
/// use majordomus_cli::capability::registry::CapabilityRegistry;
/// use majordomus_cli::release::compat::analyze;
///
/// let registry = CapabilityRegistry::builder().build().expect("an empty registry composes");
/// let dir = tempfile::tempdir().unwrap();
/// // A repository that has published nothing has no baseline, and the analysis refuses
/// // rather than reporting an empty diff — which would read as "nothing changed".
/// assert!(analyze(dir.path(), &registry, &[], None).is_err());
/// ```
pub fn analyze(
    root: &Path,
    registry: &crate::capability::registry::CapabilityRegistry,
    objects: &[Object],
    since: Option<&str>,
) -> Result<VersionPlan, SurfaceError> {
    let mut diagnostics = Vec::new();
    let baseline = resolve_baseline(root, objects, since, &mut diagnostics)?;
    let base = Surface::read_ref(root, &baseline.read_at)?;
    let head = Surface::of_registry(registry);
    let changes = diff(&base, &head);

    let implied = changes
        .iter()
        .map(|c| c.impact)
        .max()
        .unwrap_or(Impact::None);
    let breaking = implied == Impact::Major;

    let declared_version = super::version::declared(root).unwrap_or_default();
    let tool_version = super::version::tool(root).unwrap_or_default();

    let base_v = Version::parse(&baseline.version);
    let policy = Policy::for_version(base_v.unwrap_or(Version {
        major: 0,
        minor: 0,
        patch: 0,
    }));
    let required = policy.required_of(implied);

    if base_v.is_none() {
        diagnostics.push(Diagnostic {
            id: "baseline-version-malformed".into(),
            severity: Severity::Error,
            message: format!(
                "the baseline version '{}' is not three numbers, so the policy that governs it \
                 cannot be decided; pre-1.0 rules were assumed",
                baseline.version
            ),
        });
    }
    let declared_v = Version::parse(&declared_version);
    if declared_v.is_none() {
        diagnostics.push(Diagnostic {
            id: "declared-version-malformed".into(),
            severity: Severity::Error,
            message: format!(
                "the tree declares '{declared_version}', which is not three numbers; \
                 {} states it",
                super::version::MANIFEST
            ),
        });
    }
    let writers_agree = declared_version == tool_version;
    if !writers_agree {
        diagnostics.push(Diagnostic {
            id: "writers-disagree".into(),
            severity: Severity::Error,
            message: format!(
                "{} states '{declared_version}' and {} states '{tool_version}'; \
                 one release has one version — `majordomus release bump --exact {declared_version}` writes both",
                super::version::MANIFEST,
                super::version::ENTRY
            ),
        });
    }

    let (declared, required_version) = match (base_v, declared_v) {
        (Some(b), Some(d)) => {
            let owed = b.raised_to(required);
            // The smallest acceptable version, which is the baseline raised by the
            // requirement — unless the tree already declares something at least that big.
            let required_version = if declared_impact(b, d) >= required && d >= owed {
                d.to_string()
            } else {
                owed.to_string()
            };
            (declared_impact(b, d), required_version)
        }
        _ => (Impact::None, declared_version.clone()),
    };

    // The human account of the same window, kept beside the measured one.
    let commit_changes =
        super::commits::in_range(root, &format!("{}..HEAD", baseline.reference), objects);
    let commits = CommitEvidence {
        commits: commit_changes.len(),
        implied: match super::version::bump_of(&commit_changes) {
            super::version::Bump::None => Impact::None,
            super::version::Bump::Patch => Impact::Patch,
            super::version::Bump::Minor => Impact::Minor,
            super::version::Bump::Major => Impact::Major,
        },
        breaking: commit_changes
            .iter()
            .filter(|c| c.breaking)
            .map(|c| c.subject.clone())
            .collect(),
    };
    let understated = commits.implied < implied;

    let status = if declared < required
        || !writers_agree
        || diagnostics.iter().any(|d| d.severity == Severity::Error)
    {
        Status::Blocked
    } else {
        Status::Ok
    };

    Ok(VersionPlan {
        policy,
        baseline,
        declared_version,
        tool_version,
        writers_agree,
        atoms: head.atoms(),
        fingerprint: head.fingerprint(),
        implied,
        required,
        declared,
        required_version,
        status,
        breaking,
        changes,
        commits,
        understated,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    //! The compatibility matrix, as tests.
    //!
    //! Every row of the policy this repository documents is a case here, and both policy
    //! modes are exercised separately: a rule that is only written down is a rule nobody is
    //! holding to. They live in this file rather than beside it because
    //! `project.rust-command-tested-in-file` asks for that, and because the quality scanner
    //! counts a module as behaviourally tested only from the tests it can see in the file.
    use super::*;
    use crate::release::surface::{normalise, Origin, Surface, SURFACE_SCHEMA};
    use serde_json::json;

    /// A surface from a capability list in the registry's own shape.
    fn surface(caps: Value) -> Surface {
        let manifest = json!({"schema": "majordomus/capability-registry/v1", "capabilities": caps});
        // Reuse the real reader, so these tests exercise the same normalisation the engine does
        // rather than a convenient stand-in for it.
        serde_json::from_value::<Value>(manifest)
            .map(|m| Surface {
                schema: SURFACE_SCHEMA.to_string(),
                origin: Origin::WorkingTree,
                capabilities: crate::release::surface::public_capabilities(&m),
            })
            .expect("the manifest is a value")
    }

    /// One public capability, with whatever of the contract a case needs.
    fn cap(id: &str) -> Value {
        json!({"id": id, "kind": "query", "visibility": "public"})
    }

    /// The impact a diff between two capability lists amounts to.
    fn implied(base: Value, head: Value) -> Impact {
        diff(&surface(base), &surface(head))
            .iter()
            .map(|c| c.impact)
            .max()
            .unwrap_or(Impact::None)
    }

    /// The changes, for cases that assert on what was named rather than only how much.
    fn changes(base: Value, head: Value) -> Vec<SurfaceChange> {
        diff(&surface(base), &surface(head))
    }

    // ------------------------------------------------------------------- identity and bindings

    #[test]
    fn an_unchanged_surface_owes_nothing() {
        assert_eq!(
            implied(json!([cap("a.one")]), json!([cap("a.one")])),
            Impact::None
        );
    }

    #[test]
    fn reordering_is_not_a_change() {
        let a = json!([cap("a.one"), cap("b.two"), cap("c.three")]);
        let b = json!([cap("c.three"), cap("a.one"), cap("b.two")]);
        assert_eq!(
            implied(a, b),
            Impact::None,
            "the registry's enumeration order is not the contract"
        );
    }

    #[test]
    fn diffing_a_surface_with_itself_is_empty() {
        let s = surface(json!([cap("a.one"), cap("b.two")]));
        assert!(diff(&s, &s).is_empty());
    }

    #[test]
    fn adding_a_capability_is_minor_and_removing_one_is_major() {
        assert_eq!(
            implied(json!([cap("a.one")]), json!([cap("a.one"), cap("b.two")])),
            Impact::Minor
        );
        assert_eq!(
            implied(json!([cap("a.one"), cap("b.two")]), json!([cap("a.one")])),
            Impact::Major
        );
    }

    #[test]
    fn renaming_a_capability_stays_breaking_rather_than_becoming_a_rename() {
        let found = changes(json!([cap("a.old")]), json!([cap("a.new")]));
        assert_eq!(found.len(), 2, "a removal and an addition, not one move");
        assert_eq!(found[0].impact, Impact::Major, "worst first");
        assert_eq!(found[0].direction, Direction::Removed);
        assert_eq!(found[0].id, "a.old");
        assert_eq!(found[1].direction, Direction::Added);
        assert_eq!(
            found.iter().map(|c| c.impact).max(),
            Some(Impact::Major),
            "a rename breaks the caller holding the old name"
        );
    }

    #[test]
    fn the_kind_is_part_of_the_contract() {
        let q = json!([{"id": "a.one", "kind": "query", "visibility": "public"}]);
        let c = json!([{"id": "a.one", "kind": "command", "visibility": "public"}]);
        assert_eq!(implied(q, c), Impact::Major);
    }

    #[test]
    fn making_a_capability_internal_is_a_removal() {
        let public = json!([cap("a.one")]);
        let internal = json!([{"id": "a.one", "kind": "query", "visibility": "internal"}]);
        let found = changes(public, internal);
        assert_eq!(found[0].impact, Impact::Major);
        assert_eq!(
            found[0].direction,
            Direction::Removed,
            "withdrawn from the public surface is gone, whatever the code still holds"
        );
    }

    /// Every binding a caller can hold, added and removed, in one table.
    #[test]
    fn every_binding_added_is_minor_and_removed_is_major() {
        let bare = json!([cap("a.one")]);
        let bound = |exposure: Value| json!([{"id": "a.one", "kind": "query", "visibility": "public", "exposure": exposure}]);
        let cases: &[(&str, Value)] = &[
            ("mcp tool", json!({"mcp": {"tool": "t"}})),
            (
                "mcp resource",
                json!({"mcp": {"resource": {"uri": "majordomus://r"}}}),
            ),
            (
                "route",
                json!({"http": {"method": "GET", "path": "/api/v1/x"}}),
            ),
            ("command", json!({"cli": {"path": ["a", "one"]}})),
        ];
        for (what, exposure) in cases {
            assert_eq!(
                implied(bare.clone(), bound(exposure.clone())),
                Impact::Minor,
                "a new {what} breaks nobody"
            );
            assert_eq!(
                implied(bound(exposure.clone()), bare.clone()),
                Impact::Major,
                "a {what} that is gone breaks whoever held it"
            );
        }
    }

    #[test]
    fn a_route_that_changes_method_or_path_is_breaking_both_ways() {
        let route = |m: &str, p: &str| {
            json!([{"id": "a.one", "kind": "query", "visibility": "public",
                    "exposure": {"http": {"method": m, "path": p}}}])
        };
        assert_eq!(
            implied(route("GET", "/api/v1/x"), route("POST", "/api/v1/x")),
            Impact::Major,
            "a caller issuing GET gets a method-not-allowed"
        );
        assert_eq!(
            implied(route("GET", "/api/v1/x"), route("GET", "/api/v1/y")),
            Impact::Major,
            "a caller holding the old path gets a 404"
        );
        let moved = changes(route("GET", "/api/v1/x"), route("GET", "/api/v1/y"));
        assert_eq!(
            moved.len(),
            2,
            "the old one is gone and the new one arrived"
        );
        assert!(moved[0].detail.contains("breaks"), "and it says why");
    }

    // --------------------------------------------------------------------------- input schemas

    /// A capability whose input is the given schema.
    fn with_input(schema: Value) -> Value {
        json!([{"id": "a.one", "kind": "query", "visibility": "public", "input": {"schema": schema}}])
    }

    /// A capability whose output is the given schema.
    fn with_output(schema: Value) -> Value {
        json!([{"id": "a.one", "kind": "query", "visibility": "public", "output": {"schema": schema}}])
    }

    #[test]
    fn an_input_that_demands_more_is_breaking_and_one_that_demands_less_is_not() {
        let optional = json!({"type": "object", "properties": {"x": {"type": "string"}}});
        let required =
            json!({"type": "object", "properties": {"x": {"type": "string"}}, "required": ["x"]});
        assert_eq!(
            implied(with_input(optional.clone()), with_input(required.clone())),
            Impact::Major,
            "a caller that did not send x is refused now"
        );
        assert_eq!(
            implied(with_input(required), with_input(optional)),
            Impact::Minor,
            "dropping a requirement breaks nobody"
        );
    }

    #[test]
    fn an_optional_input_field_added_is_minor_and_removed_is_major() {
        let one = json!({"type": "object", "properties": {"x": {"type": "string"}}});
        let two = json!({"type": "object", "properties": {"x": {"type": "string"}, "y": {"type": "string"}}});
        assert_eq!(
            implied(with_input(one.clone()), with_input(two.clone())),
            Impact::Minor
        );
        // Every input in this repository denies unknown fields, so a property that is gone is
        // refused rather than ignored.
        assert_eq!(implied(with_input(two), with_input(one)), Impact::Major);
    }

    #[test]
    fn an_input_type_that_changes_is_breaking() {
        let s = json!({"type": "object", "properties": {"x": {"type": "string"}}});
        let n = json!({"type": "object", "properties": {"x": {"type": "number"}}});
        let found = changes(with_input(s), with_input(n));
        assert_eq!(found[0].impact, Impact::Major);
        assert!(
            found[0].id.contains("properties.x"),
            "it names where: {}",
            found[0].id
        );
    }

    #[test]
    fn closing_an_open_object_is_breaking_and_opening_a_closed_one_is_not() {
        let open = json!({"type": "object", "additionalProperties": true});
        let closed = json!({"type": "object", "additionalProperties": false});
        assert_eq!(
            implied(with_input(open.clone()), with_input(closed.clone())),
            Impact::Major
        );
        assert_eq!(implied(with_input(closed), with_input(open)), Impact::Minor);
    }

    // -------------------------------------------------------------------------- output schemas

    #[test]
    fn an_output_that_promises_less_is_breaking_and_more_is_not() {
        let one = json!({"type": "object", "properties": {"x": {"type": "string"}}});
        let two = json!({"type": "object", "properties": {"x": {"type": "string"}, "y": {"type": "string"}}});
        assert_eq!(
            implied(with_output(two.clone()), with_output(one.clone())),
            Impact::Major,
            "a caller reading y finds nothing"
        );
        assert_eq!(
            implied(with_output(one), with_output(two)),
            Impact::Minor,
            "a caller ignoring y is unaffected"
        );
    }

    #[test]
    fn the_same_edit_is_minor_in_an_input_and_major_in_an_output() {
        // This is the whole reason the comparator takes a role: a textual diff of the two
        // schemas is identical, and the compatibility answer is opposite.
        let optional = json!({"type": "object", "properties": {"x": {"type": "string"}}});
        let required =
            json!({"type": "object", "properties": {"x": {"type": "string"}}, "required": ["x"]});
        assert_eq!(
            implied(with_output(optional.clone()), with_output(required.clone())),
            Impact::Minor,
            "an output that is always present is a stronger promise"
        );
        assert_eq!(
            implied(with_input(optional), with_input(required)),
            Impact::Major,
            "an input that must be present is a stricter demand"
        );
    }

    #[test]
    fn an_output_that_stops_promising_a_field_is_breaking() {
        let promised =
            json!({"type": "object", "properties": {"x": {"type": "string"}}, "required": ["x"]});
        let maybe = json!({"type": "object", "properties": {"x": {"type": "string"}}});
        let found = changes(with_output(promised), with_output(maybe));
        assert_eq!(found[0].impact, Impact::Major);
        assert!(
            found[0].detail.contains("no longer promised"),
            "{}",
            found[0].detail
        );
    }

    // ---------------------------------------------------------------------------------- enums

    #[test]
    fn narrowing_an_enum_is_breaking_and_widening_it_is_not() {
        let wide = json!({"type": "string", "enum": ["a", "b", "c"]});
        let narrow = json!({"type": "string", "enum": ["a", "b"]});
        assert_eq!(
            implied(with_input(wide.clone()), with_input(narrow.clone())),
            Impact::Major
        );
        assert_eq!(implied(with_input(narrow), with_input(wide)), Impact::Minor);
    }

    #[test]
    fn a_variant_removed_from_a_oneof_is_breaking() {
        let two = json!({"oneOf": [{"const": "a"}, {"const": "b"}]});
        let one = json!({"oneOf": [{"const": "a"}]});
        assert_eq!(
            implied(with_output(two.clone()), with_output(one.clone())),
            Impact::Major
        );
        assert_eq!(implied(with_output(one), with_output(two)), Impact::Minor);
    }

    #[test]
    fn a_nested_definition_is_compared_and_not_skipped() {
        let with = |inner: Value| {
            with_output(json!({
                "type": "object",
                "properties": {"item": {"$ref": "#/$defs/Item"}},
                "$defs": {"Item": inner}
            }))
        };
        let before = with(json!({"type": "object", "properties": {"x": {"type": "string"}}}));
        let after = with(json!({"type": "object", "properties": {}}));
        let found = changes(before, after);
        assert_eq!(
            found[0].impact,
            Impact::Major,
            "a field inside a $def is still a promise"
        );
        assert!(
            found[0].id.contains("$defs.Item"),
            "it names the path: {}",
            found[0].id
        );
    }

    // -------------------------------------------------------------- prose, and what is not one

    #[test]
    fn rewording_a_description_is_not_a_release() {
        let before = with_output(
            json!({"type": "object", "description": "one wording", "properties": {"x": {"type": "string", "description": "a"}}}),
        );
        let after = with_output(
            json!({"type": "object", "description": "a different and much longer wording", "properties": {"x": {"type": "string", "description": "b"}}}),
        );
        assert_eq!(
            implied(before, after),
            Impact::None,
            "a documentation change is not a contract change"
        );
    }

    #[test]
    fn an_unmodelled_constraint_change_is_breaking_rather_than_guessed_at() {
        let loose = json!({"type": "string"});
        let tight = json!({"type": "string", "pattern": "^a+$"});
        let found = changes(with_input(loose), with_input(tight));
        assert_eq!(
            found[0].impact,
            Impact::Major,
            "a constraint this comparator has no rule for is breaking, never a patch"
        );
        assert!(
            found[0].detail.contains("does not model"),
            "and it says that is why: {}",
            found[0].detail
        );
    }

    // --------------------------------------------------------------------------------- policy

    #[test]
    fn the_policy_below_one_zero_floors_a_breaking_change_at_a_minor() {
        let p = Policy::for_version(Version::parse("0.5.0").expect("a version"));
        assert_eq!(p.mode, Mode::Pre1_0Strict);
        assert_eq!(p.required_of(Impact::None), Impact::None);
        assert_eq!(
            p.required_of(Impact::Patch),
            Impact::None,
            "an unchanged surface owes nothing"
        );
        assert_eq!(p.required_of(Impact::Minor), Impact::Minor);
        assert_eq!(
            p.required_of(Impact::Major),
            Impact::Minor,
            "a removal below 1.0 costs a minor, not 1.0.0 — and is still named as breaking"
        );
    }

    #[test]
    fn the_policy_at_one_zero_and_above_is_semver() {
        let p = Policy::for_version(Version::parse("1.4.2").expect("a version"));
        assert_eq!(p.mode, Mode::Semver);
        assert_eq!(p.required_of(Impact::None), Impact::None);
        assert_eq!(p.required_of(Impact::Patch), Impact::None);
        assert_eq!(p.required_of(Impact::Minor), Impact::Minor);
        assert_eq!(p.required_of(Impact::Major), Impact::Major);
    }

    #[test]
    fn the_policy_names_itself_and_says_what_it_does() {
        for v in ["0.5.0", "2.0.0"] {
            let p = Policy::for_version(Version::parse(v).expect("a version"));
            assert_eq!(
                p.schema, POLICY_SCHEMA,
                "a verdict carries the rules it was reached under"
            );
            assert!(!p.statement().is_empty());
        }
    }

    // ------------------------------------------------------------------------ declared impact

    #[test]
    fn the_declared_bump_is_read_from_the_two_numbers() {
        let v = |s: &str| Version::parse(s).expect("a version");
        assert_eq!(declared_impact(v("0.5.0"), v("0.5.0")), Impact::None);
        assert_eq!(declared_impact(v("0.5.0"), v("0.5.1")), Impact::Patch);
        assert_eq!(declared_impact(v("0.5.0"), v("0.6.0")), Impact::Minor);
        assert_eq!(declared_impact(v("0.5.0"), v("1.0.0")), Impact::Major);
        // A version going down is not a small bump; it is no bump, and ranking it as a patch
        // would let a downgrade satisfy a requirement.
        assert_eq!(declared_impact(v("0.5.0"), v("0.4.0")), Impact::None);
        assert_eq!(declared_impact(v("1.0.0"), v("0.9.9")), Impact::None);
    }

    #[test]
    fn an_impact_ranks_above_the_ones_it_supersedes() {
        assert!(Impact::Major > Impact::Minor);
        assert!(Impact::Minor > Impact::Patch);
        assert!(Impact::Patch > Impact::None);
        for i in [Impact::None, Impact::Patch, Impact::Minor, Impact::Major] {
            assert_eq!(
                Impact::parse(i.as_str()),
                Some(i),
                "every impact round-trips through its word"
            );
        }
        assert_eq!(Impact::parse("enormous"), None);
    }

    #[test]
    fn a_version_is_raised_by_a_measured_impact() {
        let v = Version::parse("0.5.0").expect("a version");
        assert_eq!(v.raised_to(Impact::None).to_string(), "0.5.0");
        assert_eq!(v.raised_to(Impact::Patch).to_string(), "0.5.1");
        assert_eq!(v.raised_to(Impact::Minor).to_string(), "0.6.0");
        assert_eq!(v.raised_to(Impact::Major).to_string(), "1.0.0");
    }

    // ------------------------------------------------------------------------- determinism

    #[test]
    fn the_same_surface_diffs_the_same_however_it_was_enumerated() {
        // Property-shaped without a property-testing dependency: the same logical change,
        // presented in every order, must produce the same verdict and the same report.
        let base = json!([cap("a.one"), cap("b.two"), cap("c.three")]);
        let orders = [
            json!([cap("a.one"), cap("b.two"), cap("d.four")]),
            json!([cap("d.four"), cap("a.one"), cap("b.two")]),
            json!([cap("b.two"), cap("d.four"), cap("a.one")]),
        ];
        let first = changes(base.clone(), orders[0].clone());
        for other in &orders[1..] {
            assert_eq!(
                changes(base.clone(), other.clone()),
                first,
                "the report is a function of the surfaces, not of their enumeration"
            );
        }
        // And it is the right verdict: one gone, one arrived.
        assert_eq!(first.iter().map(|c| c.impact).max(), Some(Impact::Major));
    }

    #[test]
    fn normalising_twice_changes_nothing_further() {
        let v = json!({"b": 1, "a": {"d": 2, "c": [{"f": 3, "e": 4}]}, "description": "x"});
        let once = normalise(&v);
        assert_eq!(normalise(&once), once);
    }
}
