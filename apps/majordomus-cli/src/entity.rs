//! Entities: the one projection that makes every object of the layer addressable.
//!
//! An [`crate::model::Object`] already has a kind, an identity and a `majordomus://` URI,
//! and every surface of this executable already reads it. What it did not have was an
//! *address a reader can be given*: the Cockpit showed one object at
//! `/cockpit/object?uri=majordomus%3A%2F%2Frule%2Fproject.x%401`, no page of the public
//! site showed a decision or a project rule at all, and nothing answered the two questions
//! a governance object is read for — what does it point at, and what points at it.
//!
//! This module is that projection and nothing else. It holds no list of kinds, no list of
//! entities and no list of routes:
//!
//! ```text
//!   index (objects)   ─┬─> slug ──> route     one deterministic function, per object
//!   graph::RELATIONS  ─┘   relations          the same table the graph and the
//!                          backlinks          dangling-reference check already read
//!   capability registry ─> surfaces           the exposures the registry already holds
//! ```
//!
//! Adding an object to the layer therefore adds its route, its page, its relations and its
//! backlinks, with no edit here and none in any consumer. That is the invariant
//! `project.entities-are-routable` states and `scripts/ci/entity-check` refuses to let
//! drift.
//!
//! # The slug
//!
//! An identity is not a path segment: `project.a-rule@1` carries an `@`, and a `document`'s
//! identity is a repository-relative path with slashes in it. [`slug`] is the deterministic
//! reduction to one segment — lowercase, every run of anything else a single `-`, trimmed:
//!
//! ```
//! use majordomus_cli::entity::slug;
//! assert_eq!(slug("adr-0004"), "adr-0004");
//! assert_eq!(slug("project.a-diagram-is-drawn-not-typed@1"),
//!            "project-a-diagram-is-drawn-not-typed-1");
//! assert_eq!(slug(".ai/repo/README.md"), "ai-repo-readme-md");
//! assert_eq!(slug("Continue"), "continue");
//! ```
//!
//! Two identities of one kind can in principle reduce to one slug. That is a collision, not
//! a tie-break: [`collisions`] finds every one of them and the gate refuses the tree while
//! any exists, because a route that answers with one of two objects is worse than no route.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::model::Capability;
use crate::capability::registry::CapabilityRegistry;
use crate::graph::{self, Outcome};
use crate::model::Object;

/// Where every entity route of the Cockpit starts. A kind index is this plus the kind; an
/// entity is that plus its slug. Nothing else is under it, which is what makes the two
/// route shapes unambiguous without a list of kinds to check against.
pub const COCKPIT_PREFIX: &str = "/cockpit/objects";

/// The deterministic, URL-safe reduction of an identity to one path segment.
///
/// Lowercase; every maximal run of characters outside `[a-z0-9]` becomes a single `-`;
/// leading and trailing `-` are dropped. An identity that reduces to nothing (there is no
/// such identity in a valid layer, because an identity is non-empty and a path always
/// carries a letter) yields the empty string, and [`collisions`] reports it.
///
/// ```
/// use majordomus_cli::entity::slug;
/// assert_eq!(slug("majordomus.scope-integrity@1"), "majordomus-scope-integrity-1");
/// assert_eq!(slug("---"), "");
/// assert_eq!(slug("A  B"), "a-b");
/// ```
pub fn slug(identity: &str) -> String {
    let mut out = String::with_capacity(identity.len());
    let mut pending = false;
    for ch in identity.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending && !out.is_empty() {
                out.push('-');
            }
            pending = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending = true;
        }
    }
    out
}

/// The Cockpit route of one kind's index.
///
/// ```
/// assert_eq!(majordomus_cli::entity::kind_route("adr"), "/cockpit/objects/adr");
/// ```
pub fn kind_route(kind: &str) -> String {
    format!("{COCKPIT_PREFIX}/{}", slug(kind))
}

/// The Cockpit route of one entity, from its kind and identity.
///
/// ```
/// use majordomus_cli::entity::route;
/// assert_eq!(route("adr", "adr-0004"), "/cockpit/objects/adr/adr-0004");
/// assert_eq!(route("rule", "project.x@1"), "/cockpit/objects/rule/project-x-1");
/// ```
pub fn route(kind: &str, identity: &str) -> String {
    format!("{}/{}", kind_route(kind), slug(identity))
}

/// The route of an object, from the object itself.
///
/// [`route`] asks for the kind and the identity separately, because the Cockpit's router
/// has those two strings out of a path and no object yet. This is the same function for
/// every caller that is already holding the object, and it exists so that no consumer
/// reaches for `o.kind` and `o.identity` and spells the address a second time.
///
/// ```
/// # use majordomus_cli::{Object, Provenance};
/// # fn object(kind: &str, identity: &str) -> Object {
/// #     Object { kind: kind.into(), identity: identity.into(),
/// #         uri: format!("majordomus://{kind}/{identity}"), title: None, description: None,
/// #         metadata: serde_json::json!({}), body: String::new(), content: String::new(),
/// #         media_type: "text/markdown",
/// #         provenance: Provenance { path: "x.md".into(), directory: "x".into(),
/// #             source_class: kind.into(), section: None, bytes: 0, member: None } }
/// # }
/// use majordomus_cli::entity::{object_route, route};
///
/// let rule = object("rule", "project.entities-are-routable@1");
/// assert_eq!(
///     object_route(&rule),
///     "/cockpit/objects/rule/project-entities-are-routable-1"
/// );
/// // and it is the same address the router resolves with
/// assert_eq!(object_route(&rule), route(&rule.kind, &rule.identity));
/// ```
pub fn object_route(o: &Object) -> String {
    route(&o.kind, &o.identity)
}

/// Find the object of a kind whose identity has this slug.
///
/// Linear over the kind's objects, which is what the Cockpit needs (a lookup per request,
/// over at most a few hundred objects of one kind) and what keeps this function free of a
/// cache nobody invalidates.
///
/// ```
/// # use majordomus_cli::{Object, Provenance};
/// # fn object(kind: &str, identity: &str) -> Object {
/// #     Object { kind: kind.into(), identity: identity.into(),
/// #         uri: format!("majordomus://{kind}/{identity}"), title: None, description: None,
/// #         metadata: serde_json::json!({}), body: String::new(), content: String::new(),
/// #         media_type: "text/markdown",
/// #         provenance: Provenance { path: "x.md".into(), directory: "x".into(),
/// #             source_class: kind.into(), section: None, bytes: 0, member: None } }
/// # }
/// use majordomus_cli::entity::find;
///
/// let objects = vec![object("rule", "project.x@1"), object("adr", "adr-0056")];
///
/// let found = find(&objects, "rule", "project-x-1").expect("the slug resolves");
/// assert_eq!(found.identity, "project.x@1");
/// // the kind is part of the address, so the same slug under another kind is not it
/// assert!(find(&objects, "adr", "project-x-1").is_none());
/// assert!(find(&objects, "rule", "no-such-rule").is_none());
/// ```
pub fn find<'a>(objects: &'a [Object], kind: &str, slug_wanted: &str) -> Option<&'a Object> {
    objects
        .iter()
        .find(|o| o.kind == kind && slug(&o.identity) == slug_wanted)
}

/// Two objects of one kind whose identities reduce to the same route.
///
/// Carries both identities and both files, because the reader who has to fix it needs to
/// know which two things collided and where they are written — a count would tell them
/// only that the tree is refused. `correction` is the sentence [`collisions`] wrote about
/// this particular pair.
///
/// ```
/// # use majordomus_cli::{Object, Provenance};
/// # fn object(kind: &str, identity: &str) -> Object {
/// #     Object { kind: kind.into(), identity: identity.into(),
/// #         uri: format!("majordomus://{kind}/{identity}"), title: None, description: None,
/// #         metadata: serde_json::json!({}), body: String::new(), content: String::new(),
/// #         media_type: "text/markdown",
/// #         provenance: Provenance { path: format!("{identity}.md"), directory: "x".into(),
/// #             source_class: kind.into(), section: None, bytes: 0, member: None } }
/// # }
/// use majordomus_cli::entity::{collisions, Collision};
///
/// // `a.b` and `a-b` are two identities and one slug
/// let objects = vec![object("rule", "a.b"), object("rule", "a-b")];
/// let found: Vec<Collision> = collisions(&objects);
///
/// assert_eq!(found.len(), 1);
/// assert_eq!(found[0].kind, "rule");
/// assert_eq!(found[0].slug, "a-b");
/// assert_eq!(found[0].identities, ["a.b", "a-b"]);
/// assert!(!found[0].correction.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "RouteCollision")]
pub struct Collision {
    /// The kind they share.
    pub kind: String,
    /// The slug they both reduce to; empty when an identity reduces to nothing.
    pub slug: String,
    /// The identities that collide, in index order.
    pub identities: Vec<String>,
    /// The files that declared them.
    pub paths: Vec<String>,
    /// What to do about it.
    pub correction: String,
}

/// Every route collision in an index. Empty is the healthy answer, and the only one the
/// gate accepts: a route that resolves to one of two objects is not a route.
///
/// ```
/// # use majordomus_cli::entity::collisions;
/// assert!(collisions(&[]).is_empty());
/// ```
pub fn collisions(objects: &[Object]) -> Vec<Collision> {
    let mut by_route: BTreeMap<(String, String), Vec<&Object>> = BTreeMap::new();
    for o in objects {
        by_route
            .entry((o.kind.clone(), slug(&o.identity)))
            .or_default()
            .push(o);
    }
    by_route
        .into_iter()
        .filter(|((_, s), group)| group.len() > 1 || s.is_empty())
        .map(|((kind, s), group)| Collision {
            correction: if s.is_empty() {
                format!(
                    "the identity of an object of kind `{kind}` carries no letter or digit and so has no route; give it an identity that does"
                )
            } else {
                format!(
                    "{} objects of kind `{kind}` share the route {}; rename one of them so that each has an address of its own",
                    group.len(),
                    route(&kind, &s)
                )
            },
            kind,
            slug: s,
            identities: group.iter().map(|o| o.identity.clone()).collect(),
            paths: group.iter().map(|o| o.provenance.path.clone()).collect(),
        })
        .collect()
}

// ---------------------------------------------------------------- relations

/// Which way a relation runs from the entity being read.
///
/// The direction is a property of the *reading*, not of the relation: one edge in
/// [`graph::RELATIONS`] is outgoing on the page of the object that declared it and incoming
/// on the page of the object it names. Nothing in the layer writes `Incoming` down.
///
/// ```
/// use majordomus_cli::entity::Direction;
///
/// // the wire spelling, which every surface shows and the site template reads
/// assert_eq!(
///     serde_json::to_string(&Direction::Outgoing).unwrap(),
///     "\"outgoing\""
/// );
/// assert_eq!(
///     serde_json::to_string(&Direction::Incoming).unwrap(),
///     "\"incoming\""
/// );
/// // declared before derived, which is the order an entity page renders the two sections
/// assert!((Direction::Outgoing as u8) < (Direction::Incoming as u8));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "RelationDirection")]
pub enum Direction {
    /// This entity declared the reference.
    Outgoing,
    /// Another entity named this one. Never declared: derived by reading every other
    /// object's references and keeping the ones that resolve here.
    Incoming,
}

/// One edge, as an entity page reads it.
///
/// Everything a reader needs about the other end is here — what to show, what kind of thing
/// it is, and the address to send them to — so the page renders a relation without going
/// back to the index for it. `route` is `None` exactly when `external` is true: a `file`, a
/// `test` or a `command` is named by the layer and is not an object of it, so there is no
/// entity page to link to and the template shows plain text instead of a dead link.
///
/// ```
/// use majordomus_cli::entity::{Direction, Edge};
/// use majordomus_cli::order::Ordered;
///
/// let edge = Edge {
///     direction: Direction::Incoming,
///     edge: "depends_on".into(),
///     field: "depends_on".into(),
///     uri: Some("majordomus://rule/project.x@1".into()),
///     kind: "rule".into(),
///     label: "project.x@1".into(),
///     title: Some("A rule".into()),
///     route: Some("/cockpit/objects/rule/project-x-1".into()),
///     external: false,
/// };
///
/// // the relation is the group and the direction ranks inside it; see the `Ordered` impl
/// let key = edge.order_key();
/// assert_eq!(key.group, Some("depends_on"));
/// assert_eq!(key.rank, Direction::Incoming as i64);
/// assert_eq!(key.identity, "majordomus://rule/project.x@1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EntityEdge")]
pub struct Edge {
    /// Which way it runs from the entity that was asked for.
    pub direction: Direction,
    /// The relationship, in the graph's vocabulary (`depends_on`, `put_in_force`, ...).
    pub edge: String,
    /// The front matter key the reference was declared under.
    pub field: String,
    /// The object at the other end: its URI when the layer holds it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Its kind — of the layer, or the external vocabulary (`file`, `test`, `command`).
    pub kind: String,
    /// What to show: the identity of an object, or the name of something outside.
    pub label: String,
    /// Its title, when it is an object that has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Its entity route, when it has one; absent for something outside the layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// True when the other end is not an object of this index.
    pub external: bool,
}

/// An edge is ordered by the relation it is, then by which way it runs, then by what a
/// reader sees.
///
/// The relation is the group, because that is the bucket the reader is actually scanning —
/// every `depends_on` together, every `put_in_force` together — and the direction is a rank
/// inside it rather than the most significant part, because every consumer of this
/// collection filters to one direction before it renders (the entity page draws the
/// declared references and the backlinks as two sections). The URI is the tie-breaker, and
/// an external end that has none falls back to its label; `edges` dedups, so two edges that
/// compare equal here were the same edge twice.
///
/// This is the whole order of the collection. There was a `sort_by` here beside the
/// building of it, which is the second opinion [`crate::order`] exists to remove — the
/// natural comparison it brings is also the one every other surface of this repository
/// shows, so `case-2` precedes `case-10` on an entity page as it does everywhere else.
impl crate::order::Ordered for Edge {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::grouped(
            &self.edge,
            &self.label,
            self.uri.as_deref().unwrap_or(&self.label),
        )
        .ranked(self.direction as i64)
    }
}

/// Every edge of one object: the references it declares, and the references that resolve
/// to it.
///
/// Both directions come from one table — [`graph::RELATIONS`] — so a relation added there
/// appears as a forward edge on one page and as a backlink on the other, with nothing
/// declared twice and no bidirectional bookkeeping in the layer. A reference that resolves
/// to nothing is not an edge; it is a finding, and
/// [`graph::unresolved_relations`](crate::graph::unresolved_relations) is what reports it.
///
/// A rule that depends on another produces one edge on each of the two pages, from the one
/// `depends_on` the first of them declared:
///
/// ```
/// # use majordomus_cli::{Object, Provenance};
/// # fn rule(identity: &str, metadata: serde_json::Value) -> Object {
/// #     Object { kind: "rule".into(), identity: identity.into(),
/// #         uri: format!("majordomus://rule/{identity}"), title: None, description: None,
/// #         metadata, body: String::new(), content: String::new(),
/// #         media_type: "text/markdown",
/// #         provenance: Provenance { path: format!("{identity}.md"), directory: "x".into(),
/// #             source_class: "rule".into(), section: None, bytes: 0, member: None } }
/// # }
/// use majordomus_cli::capability::builtin;
/// use majordomus_cli::capability::registry::CapabilityRegistry;
/// use majordomus_cli::entity::{edges, Direction};
///
/// let registry = CapabilityRegistry::builder()
///     .with_builtin(builtin::all())
///     .build()
///     .unwrap();
///
/// let dependant = rule("project.a@1", serde_json::json!({ "depends_on": ["project.b@1"] }));
/// let dependency = rule("project.b@1", serde_json::json!({}));
/// let objects = vec![dependant.clone(), dependency.clone()];
///
/// // the declaring end sees it run outwards
/// let out = edges(&registry, &objects, &dependant);
/// assert!(out.iter().any(|e| e.direction == Direction::Outgoing
///     && e.edge == "depends_on"
///     && e.label == "project.b@1"));
///
/// // the named end sees the same relation as a backlink, declared nowhere
/// let back = edges(&registry, &objects, &dependency);
/// assert!(back.iter().any(|e| e.direction == Direction::Incoming
///     && e.edge == "depends_on"
///     && e.label == "project.a@1"));
/// ```
pub fn edges(registry: &CapabilityRegistry, objects: &[Object], subject: &Object) -> Vec<Edge> {
    let resolver = graph::Resolver::new(registry, objects);
    let by_uri: BTreeMap<&str, &Object> = objects.iter().map(|o| (o.uri.as_str(), o)).collect();
    let mut out: Vec<Edge> = Vec::new();

    for declarer in objects {
        for rel in graph::RELATIONS {
            if !rel.kinds.is_empty() && !rel.kinds.contains(&declarer.kind.as_str()) {
                continue;
            }
            for reference in graph::metadata_strings(&declarer.metadata, rel.field) {
                if graph::is_absence(&reference) {
                    continue;
                }
                let (from, to, target) = match resolver.resolve(rel, &reference) {
                    Outcome::Node(target) => {
                        if rel.inverted {
                            (Some(target.clone()), Some(declarer.uri.clone()), target)
                        } else {
                            (Some(declarer.uri.clone()), Some(target.clone()), target)
                        }
                    }
                    Outcome::External(node) => {
                        // an external end has no URI, so it can only ever be the far end of
                        // an edge this object declared
                        if declarer.uri != subject.uri {
                            continue;
                        }
                        out.push(Edge {
                            direction: Direction::Outgoing,
                            edge: rel.edge.to_string(),
                            field: rel.field.to_string(),
                            uri: None,
                            kind: node.kind.clone(),
                            label: reference.clone(),
                            title: None,
                            route: None,
                            external: true,
                        });
                        continue;
                    }
                    Outcome::Missing(_) => continue,
                };
                let (Some(from), Some(to)) = (from, to) else {
                    continue;
                };
                let direction = if from == subject.uri {
                    Direction::Outgoing
                } else if to == subject.uri {
                    Direction::Incoming
                } else {
                    continue;
                };
                // the far end is whichever of the two is not the subject
                let other = if direction == Direction::Outgoing {
                    to
                } else {
                    from
                };
                let _ = &target;
                out.push(match by_uri.get(other.as_str()) {
                    Some(o) => Edge {
                        direction,
                        edge: rel.edge.to_string(),
                        field: rel.field.to_string(),
                        uri: Some(o.uri.clone()),
                        kind: o.kind.clone(),
                        label: o.identity.clone(),
                        title: o.title.clone(),
                        route: Some(object_route(o)),
                        external: false,
                    },
                    // a capability node: the graph names it `capability:<id>`, and it is
                    // not an object of the index
                    None => Edge {
                        direction,
                        edge: rel.edge.to_string(),
                        field: rel.field.to_string(),
                        uri: None,
                        kind: "capability".into(),
                        label: other.trim_start_matches("capability:").to_string(),
                        title: None,
                        route: Some(format!(
                            "/cockpit/capabilities/{}",
                            crate::http::router::percent_encode(
                                other.trim_start_matches("capability:")
                            )
                        )),
                        external: false,
                    },
                });
            }
        }
    }
    crate::order::canonical(&mut out);
    out.dedup();
    out
}

// ---------------------------------------------------------------- surfaces

/// One place this entity can be reached, and how.
///
/// A row of the "where else this is answered" table on an entity page. `surface` is the
/// interface's own name, so a reader who works on the command line and a reader who works
/// over MCP each find their own line rather than a route they have to translate.
///
/// ```
/// use majordomus_cli::entity::Surface;
///
/// let s = Surface {
///     surface: "mcp".into(),
///     address: "majordomus://rule/project.x@1".into(),
///     detail: "the file as read".into(),
/// };
/// assert_eq!(
///     serde_json::to_value(&s).unwrap()["surface"],
///     serde_json::json!("mcp")
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "EntitySurface")]
pub struct Surface {
    /// Which surface: `mcp`, `http`, `cli`, `cockpit`, `site`.
    pub surface: String,
    /// The address on it: a URI, a route, a command line.
    pub address: String,
    /// What a reader does with it.
    pub detail: String,
}

/// Every surface that answers for one object, derived from the registry's exposures rather
/// than listed here.
///
/// The MCP line is the object's own resource exposure; the HTTP and command-line lines are
/// the exposures of the capability that reads an object by URI, whichever capability that
/// is and whatever route it is mounted at, so a route that moves moves here too.
///
/// ```
/// # use majordomus_cli::{Object, Provenance};
/// # fn object(kind: &str, identity: &str) -> Object {
/// #     Object { kind: kind.into(), identity: identity.into(),
/// #         uri: format!("majordomus://{kind}/{identity}"), title: None, description: None,
/// #         metadata: serde_json::json!({}), body: String::new(), content: String::new(),
/// #         media_type: "text/markdown",
/// #         provenance: Provenance { path: "x.md".into(), directory: "x".into(),
/// #             source_class: kind.into(), section: None, bytes: 0, member: None } }
/// # }
/// use majordomus_cli::capability::builtin;
/// use majordomus_cli::capability::registry::CapabilityRegistry;
/// use majordomus_cli::entity::surfaces;
///
/// let registry = CapabilityRegistry::builder()
///     .with_builtin(builtin::all())
///     .build()
///     .unwrap();
///
/// let found = surfaces(&registry, &object("rule", "project.x@1"));
/// // the Cockpit always answers, because an address is derived and never registered
/// assert!(found.iter().any(|s| s.surface == "cockpit"
///     && s.address == "/cockpit/objects/rule/project-x-1"));
/// // and nothing here is written down: every line came out of the registry's exposures
/// assert!(found.iter().all(|s| !s.address.is_empty()));
/// ```
pub fn surfaces(registry: &CapabilityRegistry, o: &Object) -> Vec<Surface> {
    let mut out = Vec::new();
    let id = format!("{}.{}", o.kind, o.identity);
    if let Some(c) = registry.get(&id) {
        if let Some(m) = &c.exposure.mcp {
            if let Some(r) = &m.resource {
                out.push(Surface {
                    surface: "mcp".into(),
                    address: r.uri.clone(),
                    detail: format!(
                        "resources/read, or the tool majordomus_get with uri={}",
                        o.uri
                    ),
                });
            }
        }
    }
    let reader: Option<&Capability> = registry.get("entity.show").or(registry.get("objects.get"));
    if let Some(c) = reader {
        if let Some(http) = &c.exposure.http {
            out.push(Surface {
                surface: "http".into(),
                address: format!(
                    "{} {}?uri={}",
                    http.method.as_str(),
                    http.path,
                    crate::http::router::percent_encode(&o.uri)
                ),
                detail: format!("the typed entity as {} answers it", c.id),
            });
        }
        if let Some(cli) = &c.exposure.cli {
            // the address a person types is the route's two segments, which is shorter than
            // the URI and is the same thing the Cockpit's address bar holds
            out.push(Surface {
                surface: "cli".into(),
                address: format!(
                    "majordomus {} {}/{}",
                    cli.path.join(" "),
                    o.kind,
                    slug(&o.identity)
                ),
                detail: "the same answer on the command line; --format json for the typed form"
                    .into(),
            });
        }
    }
    out.push(Surface {
        surface: "cockpit".into(),
        address: object_route(o),
        detail: "this page".into(),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(kind: &str, identity: &str) -> Object {
        Object {
            kind: kind.into(),
            identity: identity.into(),
            uri: crate::model::uri_for(kind, identity),
            title: None,
            description: None,
            metadata: serde_json::Value::Null,
            body: String::new(),
            content: String::new(),
            media_type: "text/markdown",
            provenance: crate::model::Provenance {
                path: format!(".ai/{kind}/{identity}.md"),
                directory: format!(".ai/{kind}"),
                source_class: kind.into(),
                section: None,
                bytes: 0,
                member: None,
            },
        }
    }

    #[test]
    fn a_slug_is_one_segment_lowercase_and_stable() {
        assert_eq!(slug("adr-0004"), "adr-0004");
        assert_eq!(slug("project.x@1"), "project-x-1");
        assert_eq!(slug("a/b/c.md"), "a-b-c-md");
        assert_eq!(
            slug("ÜBER"),
            "ber",
            "non-ascii is a separator, not a letter"
        );
        assert!(!slug("a/b").contains('/'), "a slug is one path segment");
        // deterministic
        assert_eq!(slug("project.x@1"), slug("project.x@1"));
    }

    #[test]
    fn a_route_is_derived_from_kind_and_identity() {
        let o = object("rule", "project.x@1");
        assert_eq!(object_route(&o), "/cockpit/objects/rule/project-x-1");
        assert_eq!(kind_route("rule"), "/cockpit/objects/rule");
        assert!(object_route(&o).starts_with(&kind_route("rule")));
    }

    #[test]
    fn an_entity_is_found_by_its_own_route() {
        let objects = vec![object("rule", "project.x@1"), object("adr", "adr-0004")];
        let found = find(&objects, "rule", "project-x-1").expect("the rule");
        assert_eq!(found.identity, "project.x@1");
        assert!(
            find(&objects, "rule", "adr-0004").is_none(),
            "kind is part of the address"
        );
        assert!(find(&objects, "rule", "nothing").is_none());
    }

    #[test]
    fn two_identities_that_reduce_to_one_route_are_a_collision() {
        let objects = vec![object("rule", "project.x@1"), object("rule", "project-x-1")];
        let found = collisions(&objects);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].slug, "project-x-1");
        assert_eq!(found[0].identities.len(), 2);
        assert!(found[0]
            .correction
            .contains("/cockpit/objects/rule/project-x-1"));
        // and a healthy index has none
        assert!(collisions(&[object("rule", "a@1"), object("rule", "b@1")]).is_empty());
    }

    #[test]
    fn an_identity_with_no_route_is_reported_rather_than_routed() {
        let found = collisions(&[object("rule", "---")]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slug, "");
        assert!(found[0].correction.contains("no route"));
    }
}
