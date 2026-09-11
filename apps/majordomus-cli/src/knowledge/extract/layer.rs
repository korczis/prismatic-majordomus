//! The `layer` extractor: every object of the repository's AI layer, as the index read
//! and validated it. A rule, a decision, a use case, a policy, a claim of the claims
//! matrix — each is a declared node whose front matter is its evidence, and every typed
//! reference its front matter carries (the same table the composed graph reads) is a
//! relation, or a gap when it resolves to nothing.
//!
//! Curated knowledge records are the one kind that is `curated` rather than `declared`,
//! and the one place a person states a fact the model can check: the `asserts` list of a
//! record becomes claims about other nodes, and a claim that contradicts what an extractor
//! observed is a conflict, never a silent overwrite of either side.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::graph::{resolved_relations, ResolvedTarget};
use crate::knowledge::model::{
    ClaimState, Evidence, EvidenceKind, ExtractorInfo, Fingerprint, GapCategory, Granularity,
    KindInfo, Locator, Ownership, PredicateInfo, Provenance, Relation, RelationInfo,
    Verification, Visibility,
};
use crate::model::Object;

use super::{claim, claim_with, Extraction, ExtractionContext, Extractor, NodeSpec};

/// The extractor's id.
pub const ID: &str = "layer";

/// The layer extractor.
pub struct Layer;

/// The extractor.
pub fn extractor() -> Layer {
    Layer
}

/// The node id of an object of the index: its kind and its identity.
pub fn node_id(o: &Object) -> String {
    format!("{}:{}", o.kind, o.identity)
}

/// The kinds of the layer whose files are read as text with no metadata: they are observed
/// to exist, not declared.
fn is_text_kind(o: &Object) -> bool {
    o.media_type == "text/plain" && o.metadata.as_object().is_some_and(|m| m.is_empty())
}

/// The relation a front matter field of a curated record asserts, by the record's own
/// vocabulary (`share/schemas/majordomus/knowledge/knowledge.v1.proto`).
const CURATED_RELATIONS: &[&str] = &[
    "relates_to",
    "depends_on",
    "documents",
    "supports",
    "contradicts",
    "supersedes",
    "derived_from",
];

fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    match v.get(key) {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// The node id a typed reference of a curated record names: `file:<path>`, `test:<path>`,
/// `decision:<task>`, `session:<id>`, `commit:<sha>`, `issue:<id>`, or a bare identity
/// of a node of the model.
fn reference_node(
    reference: &str,
    by_path: &BTreeMap<&str, String>,
    ctx: Option<(&ExtractionContext<'_>, &std::collections::BTreeSet<String>)>,
) -> Option<String> {
    let (prefix, rest) = reference.split_once(':')?;
    match prefix {
        "file" | "test" => {
            let rest = rest.trim_end_matches('/');
            if let Some(layer) = by_path.get(rest) {
                return Some(layer.clone());
            }
            if let Some((ctx, generated)) = ctx {
                if generated.contains(rest) {
                    return Some(format!("artifact:{rest}"));
                }
                if ctx.is_directory(rest) {
                    return Some(format!("directory:{rest}"));
                }
            }
            Some(format!("file:{rest}"))
        }
        "issue" => Some(format!("issue:{rest}")),
        "commit" => Some(format!("commit:{rest}")),
        "session" => Some(format!("session:{rest}")),
        "decision" => Some(format!("decision:{rest}")),
        _ => Some(reference.to_string()),
    }
}

impl Extractor for Layer {
    fn info(&self, ctx: &ExtractionContext<'_>) -> ExtractorInfo {
        // the kinds are the layer's own: read off what was indexed, never listed here
        let mut kinds: Vec<KindInfo> = ctx
            .index
            .kinds()
            .into_keys()
            .map(|k| KindInfo {
                kind: k.to_string(),
                meaning: format!("an object of the layer's kind `{k}`"),
            })
            .collect();
        kinds.push(KindInfo {
            kind: "file".into(),
            meaning: "a tracked file the layer names and nothing else models".into(),
        });
        kinds.push(KindInfo {
            kind: "directory".into(),
            meaning: "a directory git tracks files under, named by a reference".into(),
        });
        kinds.push(KindInfo {
            kind: "commit".into(),
            meaning: "a commit a curated record names as evidence".into(),
        });
        kinds.push(KindInfo {
            kind: "decision".into(),
            meaning: "a local decision record a curated record names as evidence".into(),
        });
        kinds.sort_by(|a, b| a.kind.cmp(&b.kind));
        kinds.dedup_by(|a, b| a.kind == b.kind);
        let mut relations: Vec<RelationInfo> = crate::graph::relation_edges()
            .into_iter()
            .map(|(edge, meaning)| RelationInfo {
                kind: edge.to_string(),
                meaning: meaning.to_string(),
                propagates: !matches!(edge, "belongs_to" | "serves" | "governed_by"),
            })
            .collect();
        for r in CURATED_RELATIONS {
            if !relations.iter().any(|x| x.kind == *r) {
                relations.push(RelationInfo {
                    kind: (*r).into(),
                    meaning: format!("the curated record states it {} the target", r.replace('_', " ")),
                    propagates: matches!(*r, "derived_from" | "depends_on" | "documents" | "supports"),
                });
            }
        }
        relations.push(RelationInfo {
            kind: "external".into(),
            meaning: "the object names something outside the layer".into(),
            propagates: false,
        });
        relations.sort_by(|a, b| a.kind.cmp(&b.kind));
        ExtractorInfo {
            id: ID.into(),
            version: 1,
            title: "The AI layer".into(),
            description: "Every object the layer declares, as the index read and validated it: one declared node per object with its front matter as evidence, every typed reference of the front matter as a relation or a gap, and every curated knowledge record as curated knowledge with the assertions it makes about other nodes.".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds,
            relations,
            predicates: vec![
                PredicateInfo { name: "status".into(), meaning: "the status the object declares".into(), functional: true },
                PredicateInfo { name: "reference".into(), meaning: "a typed reference the front matter carries, and whether it resolves".into(), functional: false },
                PredicateInfo { name: "class".into(), meaning: "the class the object declares (a rule's enforcement class, a curated record's class)".into(), functional: true },
                PredicateInfo { name: "epistemics".into(), meaning: "how a curated record says it knows: observed, inferred or decided".into(), functional: true },
                PredicateInfo { name: "origin".into(), meaning: "whether a curated record was authored or extracted".into(), functional: true },
                PredicateInfo { name: "path".into(), meaning: "where the object is".into(), functional: true },
                PredicateInfo { name: "asserts".into(), meaning: "a statement a curated record makes about another node: subject, predicate, value".into(), functional: false },
            ],
        }
    }

    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction {
        let mut out = Extraction::default();
        let objects = &ctx.index.objects;
        let by_uri: BTreeMap<&str, String> = objects.iter().map(|o| (o.uri.as_str(), node_id(o))).collect();
        let by_path: BTreeMap<&str, String> = objects
            .iter()
            .filter(|o| o.provenance.member.is_none())
            .map(|o| (o.provenance.path.as_str(), node_id(o)))
            .collect();
        let generated = ctx.generated_paths();

        for o in objects {
            let id = node_id(o);
            let (provenance, ownership) = if o.kind == "knowledge" {
                (Provenance::Curated, Ownership::External)
            } else if is_text_kind(o) {
                (Provenance::Observed, Ownership::External)
            } else if o.provenance.path.starts_with(crate::generate::OUT_DIR) {
                (Provenance::Declared, Ownership::Majordomus)
            } else {
                (Provenance::Declared, Ownership::External)
            };
            let visibility = match string(&o.metadata, "visibility").as_deref() {
                Some("internal") => Visibility::Internal,
                Some("restricted") => Visibility::Restricted,
                _ => Visibility::Public,
            };
            // the object's evidence: its metadata, fingerprinted apart from its prose, so
            // a reworded paragraph does not move a claim made against a field
            let ev_id = format!("object:{}", o.uri);
            let metadata_text = serde_json::to_string(&o.metadata).unwrap_or_default();
            out.evidence.push(Evidence {
                id: ev_id.clone(),
                kind: EvidenceKind::Object,
                locator: Locator {
                    path: Some(o.provenance.path.clone()),
                    uri: Some(o.uri.clone()),
                    member: o.provenance.member.clone(),
                    ..Default::default()
                },
                fingerprint: Fingerprint::sha256(metadata_text.as_bytes(), Granularity::Object),
                extractor: ID.into(),
                visibility,
                remote_processing: visibility == Visibility::Public,
            });
            let file_ev = if o.provenance.member.is_none() {
                Some(out.file_evidence(ID, &o.provenance.path, o.content.as_bytes(), visibility))
            } else {
                None
            };
            let mut evidence = vec![ev_id.clone()];
            evidence.extend(file_ev.iter().cloned());
            let mut node = NodeSpec {
                id: id.clone(),
                title: o.title.clone().unwrap_or_else(|| o.identity.clone()),
                summary: o.description.clone(),
                provenance,
                ownership,
                visibility,
                evidence: evidence.clone(),
                source: Some(o.provenance.path.clone()),
                extractor: ID,
            }
            .build();
            node.route = Some(crate::graph::routes(o));
            node.claims.push(claim(
                &id,
                "path",
                Value::String(o.provenance.path.clone()),
                Provenance::Observed,
                evidence.clone(),
                Verification::Existence,
            ));
            for key in ["status", "class", "epistemics"] {
                if let Some(v) = string(&o.metadata, key) {
                    node.claims.push(claim(
                        &id,
                        key,
                        Value::String(v),
                        provenance,
                        vec![ev_id.clone()],
                        Verification::Content,
                    ));
                }
            }
            if let Some(origin) = o.metadata.get("provenance").and_then(|p| string(p, "origin")) {
                node.claims.push(claim(
                    &id,
                    "origin",
                    Value::String(origin),
                    provenance,
                    vec![ev_id.clone()],
                    Verification::Content,
                ));
            }

            if o.kind == "knowledge" {
                self.curated(ctx, o, &id, &ev_id, &by_path, &generated, &mut node, &mut out);
            }
            out.node(node);
        }

        // the typed references of every object, through the one table the graph reads
        for r in resolved_relations(ctx.registry, objects) {
            let Some(source) = by_uri.get(r.source.as_str()).cloned() else {
                continue;
            };
            let ev = format!("object:{}", r.source);
            match r.target {
                ResolvedTarget::Object(uri) => {
                    let target = by_uri.get(uri.as_str()).cloned().unwrap_or(uri);
                    push_reference(&mut out, &source, &r.field, &r.reference, &ev, ClaimState::Asserted, None);
                    let (from, to) = if r.inverted { (target, source) } else { (source, target) };
                    out.relation(Relation {
                        source: from,
                        target: to,
                        kind: r.edge,
                        provenance: Provenance::Declared,
                        evidence: vec![ev],
                    });
                }
                ResolvedTarget::External { kind, name, path } => {
                    let target = match &path {
                        Some(p) if super::resolve_path(&mut out, ctx, &by_path, &generated, p, ID).is_some() => {
                            super::resolve_path(&mut out, ctx, &by_path, &generated, p, ID)
                        }
                        Some(p) => {
                            push_reference(
                                &mut out,
                                &source,
                                &r.field,
                                &r.reference,
                                &ev,
                                ClaimState::Unverified,
                                Some(format!("{p} is not a tracked path")),
                            );
                            out.gap(
                                GapCategory::UnresolvedReference,
                                &format!("{source}#{}", r.field),
                                &format!("{} names {} under `{}`, and git tracks no such path", r.source, r.reference, r.field),
                                "track the file, or correct the reference",
                            );
                            continue;
                        }
                        None => None,
                    };
                    push_reference(&mut out, &source, &r.field, &r.reference, &ev, ClaimState::Asserted, None);
                    if let Some(target) = target {
                        let (from, to) = if r.inverted { (target, source) } else { (source, target) };
                        out.relation(Relation {
                            source: from,
                            target: to,
                            kind: r.edge,
                            provenance: Provenance::Declared,
                            evidence: vec![ev],
                        });
                    } else {
                        // a command, a claim outside the index, a doctrine of the vendored
                        // package: named, not modelled; the claim records it and no edge
                        // is drawn to a phantom
                        let _ = (kind, name);
                    }
                }
                ResolvedTarget::Missing(correction) => {
                    push_reference(
                        &mut out,
                        &source,
                        &r.field,
                        &r.reference,
                        &ev,
                        ClaimState::Unverified,
                        Some(correction.clone()),
                    );
                    out.gap(
                        GapCategory::UnresolvedReference,
                        &format!("{source}#{}", r.field),
                        &format!("{} names {} under `{}`: {correction}", r.source, r.reference, r.field),
                        "correct the reference, or register what it names",
                    );
                }
            }
        }
        out
    }
}

fn push_reference(
    out: &mut Extraction,
    source: &str,
    field: &str,
    reference: &str,
    evidence: &str,
    state: ClaimState,
    reason: Option<String>,
) {
    let mut c = claim_with(
        source,
        "reference",
        &format!("{field}={reference}"),
        Value::String(format!("{field}: {reference}")),
        Provenance::Declared,
        vec![evidence.to_string()],
        Verification::Existence,
    );
    if state == ClaimState::Unverified {
        c.state = state;
        c.confidence = c.confidence.unresolved();
    }
    c.reason = reason;
    if let Some(n) = out.nodes.iter_mut().find(|n| n.id == source) {
        if !n.claims.iter().any(|x| x.id == c.id) {
            n.claims.push(c);
        }
    }
}

impl Layer {
    /// A curated record: its typed relations, its evidence references and its assertions.
    #[allow(clippy::too_many_arguments)]
    fn curated(
        &self,
        ctx: &ExtractionContext<'_>,
        o: &Object,
        id: &str,
        ev_id: &str,
        by_path: &BTreeMap<&str, String>,
        generated: &std::collections::BTreeSet<String>,
        node: &mut crate::knowledge::model::Node,
        out: &mut Extraction,
    ) {
        // what it was derived from: the evidence a verified record rests on
        for reference in o
            .metadata
            .get("provenance")
            .map(|p| strings(p, "derived_from"))
            .unwrap_or_default()
        {
            let Some(target) = reference_node(&reference, by_path, Some((ctx, generated))) else {
                continue;
            };
            let resolves = match reference.split_once(':') {
                Some(("file" | "test", p)) => ctx.is_tracked(p) || ctx.is_directory(p) || generated.contains(p),
                _ => true,
            };
            let mut c = claim_with(
                id,
                "reference",
                &format!("derived_from={reference}"),
                Value::String(format!("derived_from: {reference}")),
                Provenance::Curated,
                vec![ev_id.to_string()],
                Verification::Existence,
            );
            if !resolves {
                c.state = ClaimState::Unverified;
                c.confidence = c.confidence.unresolved();
                c.reason = Some(format!("{reference} does not resolve to a tracked path"));
                out.gap(
                    GapCategory::UnresolvedReference,
                    &format!("{id}#derived_from"),
                    &format!("{} is derived from {reference}, which git does not track", o.provenance.path),
                    "track the file, or correct the reference",
                );
            }
            node.claims.push(c);
            if resolves {
                if let Some(("file" | "test", p)) = reference.split_once(':') {
                    let _ = super::resolve_path(out, ctx, by_path, generated, p, ID);
                    // the content the record was verified against: file evidence at content
                    // granularity, so a change to it makes the record stale
                    if let Some(text) = ctx.read(p) {
                        let fev = out.file_evidence(ID, p, text.as_bytes(), Visibility::Public);
                        node.evidence.push(fev.clone());
                        node.claims.push({
                            let mut vc = claim_with(
                                id,
                                "reference",
                                &format!("verified_against={reference}"),
                                Value::String(format!("verified against the content of {p}")),
                                Provenance::Curated,
                                vec![fev],
                                Verification::Content,
                            );
                            vc.reason = Some("a curated record holds while what it was derived from is what it was verified against".into());
                            vc
                        });
                    }
                }
                out.relation(Relation {
                    source: id.to_string(),
                    target,
                    kind: "derived_from".into(),
                    provenance: Provenance::Curated,
                    evidence: vec![ev_id.to_string()],
                });
            }
        }
        // its typed relations
        if let Some(relations) = o.metadata.get("relations").and_then(Value::as_array) {
            for r in relations {
                let (Some(kind), Some(target)) = (string(r, "type"), string(r, "target")) else {
                    continue;
                };
                if !CURATED_RELATIONS.contains(&kind.as_str()) {
                    continue;
                }
                let Some(target_id) = reference_node(&target, by_path, Some((ctx, generated))) else {
                    continue;
                };
                out.relation(Relation {
                    source: id.to_string(),
                    target: target_id,
                    kind,
                    provenance: Provenance::Curated,
                    evidence: vec![ev_id.to_string()],
                });
            }
        }
        // its assertions about other nodes: the one place a person states a fact the
        // model can hold against what the extractors observed
        if let Some(asserts) = o.metadata.get("asserts").and_then(Value::as_array) {
            for (i, a) in asserts.iter().enumerate() {
                let (Some(subject), Some(predicate)) = (string(a, "subject"), string(a, "predicate")) else {
                    continue;
                };
                let value = a.get("value").cloned().unwrap_or(Value::Null);
                let mut c = claim_with(
                    &subject,
                    &predicate,
                    &format!("{id}#{i}"),
                    value.clone(),
                    Provenance::Curated,
                    vec![ev_id.to_string()],
                    Verification::Content,
                );
                c.reason = Some(format!("asserted by {}", o.provenance.path));
                // the assertion is a claim about its subject and lives on the subject; the
                // record itself keeps a claim naming what it asserted, so both are found
                node.claims.push(claim_with(
                    id,
                    "asserts",
                    &format!("{i}"),
                    Value::String(format!("{subject} {predicate} = {value}")),
                    Provenance::Curated,
                    vec![ev_id.to_string()],
                    Verification::Content,
                ));
                out.node(
                    NodeSpec {
                        id: subject.clone(),
                        title: subject.clone(),
                        summary: None,
                        provenance: Provenance::Curated,
                        ownership: Ownership::External,
                        visibility: Visibility::Public,
                        evidence: vec![ev_id.to_string()],
                        source: None,
                        extractor: ID,
                    }
                    .build(),
                )
                .claims
                .push(c);
                out.relation(Relation {
                    source: id.to_string(),
                    target: subject,
                    kind: "documents".into(),
                    provenance: Provenance::Curated,
                    evidence: vec![ev_id.to_string()],
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_reference_names_a_node_of_the_model() {
        let mut by_path = BTreeMap::new();
        by_path.insert("docs/CLI.md", "document:docs/CLI.md".to_string());
        assert_eq!(reference_node("file:docs/CLI.md", &by_path, None).as_deref(), Some("document:docs/CLI.md"));
        assert_eq!(reference_node("file:lib/x.sh", &by_path, None).as_deref(), Some("file:lib/x.sh"));
        assert_eq!(reference_node("issue:I0001", &by_path, None).as_deref(), Some("issue:I0001"));
        assert_eq!(reference_node("component:x", &by_path, None).as_deref(), Some("component:x"));
        assert_eq!(reference_node("bare", &by_path, None), None);
    }
}
