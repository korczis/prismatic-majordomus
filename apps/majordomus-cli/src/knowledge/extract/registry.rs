//! The `registry` extractor: the executable's own capability registry and the generation
//! plan, as knowledge. Every capability is an observed node with its exposures as claims;
//! every module is a node composing them; every generated artifact the manifest declares
//! is a Majordomus-owned node with a `derived_from` relation to the canonical source it
//! was rendered from. This is the evidence the canonicality audit reads: a projection that
//! names no source, or a file in a generated tree that no plan declares, is an orphan.

use serde_json::Value;

use crate::capability::{CapabilityKind, Provenance as CapabilityProvenance};
use crate::knowledge::model::{
    Evidence, EvidenceKind, ExtractorInfo, Fingerprint, Granularity, KindInfo, Locator,
    Ownership, PredicateInfo, Provenance, Relation, RelationInfo, Verification, Visibility,
};

use super::{claim, claim_with, Extraction, ExtractionContext, Extractor, NodeSpec};

/// The extractor's id.
pub const ID: &str = "registry";

/// The registry extractor.
pub struct Registry;

/// The extractor.
pub fn extractor() -> Registry {
    Registry
}

/// The node id of a capability.
pub fn capability_node_id(id: &str) -> String {
    format!("capability:{id}")
}

/// The node id of a generated artifact.
pub fn artifact_node_id(path: &str) -> String {
    format!("artifact:{path}")
}

/// One entry of the committed generation manifest, as this extractor reads it.
#[derive(Debug, serde::Deserialize)]
struct ManifestEntry {
    path: String,
    document: String,
    format: String,
    #[serde(default)]
    schema: Option<String>,
    source: String,
    #[serde(default)]
    derived_from: Vec<String>,
    #[serde(default)]
    sha256: Option<String>,
    #[serde(default)]
    describes_itself: bool,
}

#[derive(Debug, serde::Deserialize)]
struct Manifest {
    schema: String,
    artifacts: Vec<ManifestEntry>,
}

impl Extractor for Registry {
    fn info(&self, _: &ExtractionContext<'_>) -> ExtractorInfo {
        ExtractorInfo {
            id: ID.into(),
            version: 1,
            title: "The capability registry and the generation plan".into(),
            description: "Every capability of this executable as an observed node with its module, kind, stability and exposures; every module composing them; and every generated artifact the committed manifest declares, owned by Majordomus, with a derived_from relation to the canonical source it was rendered from. What the canonicality audit reads.".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![
                KindInfo { kind: "capability".into(), meaning: "one query or command of the executable's registry; a declarative resource is the layer's object it reads, with the exposure as a claim on that object".into() },
                KindInfo { kind: "module".into(), meaning: "a capability module composed at the root".into() },
                KindInfo { kind: "artifact".into(), meaning: "a file the generation plan writes; owned by Majordomus, never edited".into() },
                KindInfo { kind: "file".into(), meaning: "a source file a capability was declared in or an artifact was derived from".into() },
                KindInfo { kind: "directory".into(), meaning: "a directory an artifact was derived from".into() },
            ],
            relations: vec![
                RelationInfo { kind: "composes".into(), meaning: "the module composes the capability".into(), propagates: false },
                RelationInfo { kind: "declared_in".into(), meaning: "the capability is declared in that file".into(), propagates: true },
                RelationInfo { kind: "derived_from".into(), meaning: "the artifact is generated from that source".into(), propagates: true },
            ],
            predicates: vec![
                PredicateInfo { name: "kind".into(), meaning: "query, command or resource".into(), functional: true },
                PredicateInfo { name: "stability".into(), meaning: "where the capability stands".into(), functional: true },
                PredicateInfo { name: "module".into(), meaning: "the module that composes it".into(), functional: true },
                PredicateInfo { name: "mcp_tool".into(), meaning: "the MCP tool name it is exposed as".into(), functional: true },
                PredicateInfo { name: "mcp_resource".into(), meaning: "the MCP resource URI it is exposed as".into(), functional: true },
                PredicateInfo { name: "http_route".into(), meaning: "the HTTP route it is exposed on".into(), functional: true },
                PredicateInfo { name: "cli_path".into(), meaning: "the command-line path it is exposed as".into(), functional: true },
                PredicateInfo { name: "source_path".into(), meaning: "the file it was declared in".into(), functional: true },
                PredicateInfo { name: "document".into(), meaning: "the document a generated artifact projects".into(), functional: true },
                PredicateInfo { name: "format".into(), meaning: "the encoding a generated artifact is written in".into(), functional: true },
                PredicateInfo { name: "schema".into(), meaning: "the schema a generated artifact's content satisfies".into(), functional: true },
                PredicateInfo { name: "generated_from".into(), meaning: "the source line a generated artifact carries".into(), functional: true },
                PredicateInfo { name: "derived_from".into(), meaning: "a canonical source a generated artifact was derived from, as the manifest declares it".into(), functional: false },
                PredicateInfo { name: "describes_itself".into(), meaning: "the artifact is the manifest's own encoding".into(), functional: true },
            ],
        }
    }

    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction {
        let mut out = Extraction::default();
        // the modules
        for m in ctx.registry.modules() {
            let id = format!("module:{}", m.id);
            out.node(
                NodeSpec {
                    id,
                    title: m.title.clone(),
                    summary: Some(m.description.clone()),
                    provenance: Provenance::Observed,
                    ownership: Ownership::External,
                    visibility: Visibility::Public,
                    evidence: vec![],
                    source: None,
                    extractor: ID,
                }
                .build(),
            );
        }
        // the capabilities, builtin ones with their declaration file as evidence. A
        // declarative resource is the layer's object projected, not a thing of its own:
        // it adds an exposure claim to the object's node and stands no second node
        for c in ctx.registry.iter() {
            if c.kind == CapabilityKind::Resource {
                let Some(res) = c.exposure.mcp.as_ref().and_then(|m| m.resource.as_ref()) else {
                    continue;
                };
                let Some(o) = ctx.index.get(&res.uri) else {
                    continue;
                };
                let oid = super::layer::node_id(o);
                let ev = format!("capability:{}", c.id);
                out.evidence.push(Evidence {
                    id: ev.clone(),
                    kind: EvidenceKind::Capability,
                    locator: Locator {
                        id: Some(c.id.to_string()),
                        uri: Some(res.uri.clone()),
                        ..Default::default()
                    },
                    fingerprint: Fingerprint::sha256(
                        serde_json::to_string(c).unwrap_or_default().as_bytes(),
                        Granularity::Object,
                    ),
                    extractor: ID.into(),
                    visibility: Visibility::Public,
                    remote_processing: true,
                });
                let mut node = NodeSpec {
                    id: oid.clone(),
                    title: o.title.clone().unwrap_or_else(|| o.identity.clone()),
                    summary: None,
                    provenance: Provenance::Declared,
                    ownership: Ownership::External,
                    visibility: Visibility::Public,
                    evidence: vec![ev.clone()],
                    source: Some(o.provenance.path.clone()),
                    extractor: ID,
                }
                .build();
                node.claims.push(claim(&oid, "mcp_resource", Value::String(res.uri.clone()), Provenance::Observed, vec![ev.clone()], Verification::Content));
                if let Some(h) = &c.exposure.http {
                    node.claims.push(claim(&oid, "http_route", Value::String(format!("{} {}", h.method.as_str(), h.path)), Provenance::Observed, vec![ev.clone()], Verification::Content));
                }
                out.node(node);
                continue;
            }
            let id = capability_node_id(c.id.as_str());
            let mut evidence = Vec::new();
            let descriptor = serde_json::to_string(c).unwrap_or_default();
            let ev = format!("capability:{}", c.id);
            out.evidence.push(Evidence {
                id: ev.clone(),
                kind: EvidenceKind::Capability,
                locator: Locator {
                    id: Some(c.id.to_string()),
                    path: Some(c.provenance.source_path()),
                    ..Default::default()
                },
                fingerprint: Fingerprint::sha256(descriptor.as_bytes(), Granularity::Object),
                extractor: ID.into(),
                visibility: Visibility::Public,
                remote_processing: true,
            });
            evidence.push(ev.clone());
            let source_path = c.provenance.source_path();
            let builtin = matches!(c.provenance, CapabilityProvenance::Builtin { .. });
            if builtin {
                if let Some(text) = ctx.read(&source_path) {
                    let fev = out.file_evidence(ID, &source_path, text.as_bytes(), Visibility::Public);
                    evidence.push(fev);
                }
            }
            // a declarative resource is the object it reads; it is listed as a capability
            // because the registry projects it, and it links to the layer's node
            let mut node = NodeSpec {
                id: id.clone(),
                title: c.title.clone(),
                summary: (!c.description.is_empty()).then(|| c.description.clone()),
                provenance: Provenance::Observed,
                ownership: Ownership::External,
                visibility: Visibility::Public,
                evidence: evidence.clone(),
                source: Some(source_path.clone()),
                extractor: ID,
            }
            .build();
            node.route = Some(crate::graph::capability_page(c));
            let word = |v: &dyn erased::Serialize| v.word();
            node.claims.push(claim(&id, "kind", Value::String(word(&c.kind)), Provenance::Observed, evidence.clone(), Verification::Content));
            node.claims.push(claim(&id, "stability", Value::String(word(&c.stability)), Provenance::Observed, evidence.clone(), Verification::Content));
            node.claims.push(claim(&id, "module", Value::String(c.module.to_string()), Provenance::Observed, evidence.clone(), Verification::Content));
            node.claims.push(claim(&id, "source_path", Value::String(source_path.clone()), Provenance::Observed, evidence.clone(), Verification::Existence));
            if let Some(tool) = c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()) {
                node.claims.push(claim(&id, "mcp_tool", Value::String(tool), Provenance::Observed, evidence.clone(), Verification::Content));
            }
            if let Some(res) = c.exposure.mcp.as_ref().and_then(|m| m.resource.as_ref()) {
                node.claims.push(claim(&id, "mcp_resource", Value::String(res.uri.clone()), Provenance::Observed, evidence.clone(), Verification::Content));
            }
            if let Some(h) = &c.exposure.http {
                node.claims.push(claim(&id, "http_route", Value::String(format!("{} {}", h.method.as_str(), h.path)), Provenance::Observed, evidence.clone(), Verification::Content));
            }
            if let Some(cli) = &c.exposure.cli {
                node.claims.push(claim(&id, "cli_path", Value::String(format!("majordomus {}", cli.path.join(" "))), Provenance::Observed, evidence.clone(), Verification::Content));
            }
            out.node(node);
            out.relation(Relation {
                source: format!("module:{}", c.module),
                target: id.clone(),
                kind: "composes".into(),
                provenance: Provenance::Observed,
                evidence: vec![ev.clone()],
            });
            if builtin {
                let fid = format!("file:{source_path}");
                out.node(
                    NodeSpec {
                        id: fid.clone(),
                        title: source_path.clone(),
                        summary: Some("a Rust module of the executable".into()),
                        provenance: Provenance::Observed,
                        ownership: Ownership::External,
                        visibility: Visibility::Public,
                        evidence: vec![],
                        source: Some(source_path.clone()),
                        extractor: ID,
                    }
                    .build(),
                );
                out.relation(Relation {
                    source: id,
                    target: fid,
                    kind: "declared_in".into(),
                    provenance: Provenance::Observed,
                    evidence: vec![ev],
                });
            }
        }
        // the generation plan: every artifact the committed manifest declares
        let manifest_path = format!("{}/{}.json", crate::generate::OUT_DIR, crate::generate::MANIFEST_ID);
        let by_path: std::collections::BTreeMap<&str, String> = ctx
            .index
            .objects
            .iter()
            .filter(|o| o.provenance.member.is_none())
            .map(|o| (o.provenance.path.as_str(), super::layer::node_id(o)))
            .collect();
        let generated = ctx.generated_paths();
        if let Some(text) = ctx.read_generated(&manifest_path) {
            match serde_json::from_str::<Manifest>(&text) {
                Ok(m) if m.schema == crate::generate::MANIFEST_SCHEMA => {
                    let mev = out.file_evidence(ID, &manifest_path, text.as_bytes(), Visibility::Public);
                    for a in m.artifacts {
                        let id = artifact_node_id(&a.path);
                        let mut evidence = vec![mev.clone()];
                        let aev = format!("artifact:{}", a.path);
                        out.evidence.push(Evidence {
                            id: aev.clone(),
                            kind: EvidenceKind::Artifact,
                            locator: Locator {
                                path: Some(a.path.clone()),
                                id: Some(a.document.clone()),
                                ..Default::default()
                            },
                            fingerprint: Fingerprint {
                                algorithm: "sha256".into(),
                                value: a.sha256.clone().unwrap_or_else(|| "self".into()),
                                granularity: Granularity::File,
                            },
                            extractor: ID.into(),
                            visibility: Visibility::Public,
                            remote_processing: true,
                        });
                        evidence.push(aev.clone());
                        let mut node = NodeSpec {
                            id: id.clone(),
                            title: a.path.clone(),
                            summary: Some(format!("{} as {}", a.document, a.format)),
                            provenance: Provenance::Observed,
                            ownership: Ownership::Majordomus,
                            visibility: Visibility::Public,
                            evidence: evidence.clone(),
                            source: Some(a.path.clone()),
                            extractor: ID,
                        }
                        .build();
                        node.claims.push(claim(&id, "document", Value::String(a.document.clone()), Provenance::Observed, evidence.clone(), Verification::Content));
                        node.claims.push(claim(&id, "format", Value::String(a.format.clone()), Provenance::Observed, evidence.clone(), Verification::Content));
                        node.claims.push(claim(&id, "generated_from", Value::String(a.source.clone()), Provenance::Observed, evidence.clone(), Verification::Content));
                        if let Some(schema) = &a.schema {
                            node.claims.push(claim(&id, "schema", Value::String(schema.clone()), Provenance::Observed, evidence.clone(), Verification::Content));
                        }
                        if a.describes_itself {
                            // the manifest indexes every other artifact: it is derived
                            // from the plan, which is to say from all of them
                            node.claims.push(claim(&id, "describes_itself", Value::Bool(true), Provenance::Observed, evidence.clone(), Verification::Content));
                            out.relation(Relation {
                                source: id.clone(),
                                target: format!("directory:{}", crate::generate::OUT_DIR),
                                kind: "derived_from".into(),
                                provenance: Provenance::Observed,
                                evidence: vec![aev.clone()],
                            });
                            out.node(super::directory_node(crate::generate::OUT_DIR, ID));
                        }
                        for (i, src) in a.derived_from.iter().enumerate() {
                            node.claims.push(claim_with(&id, "derived_from", &format!("{i}"), Value::String(src.clone()), Provenance::Observed, vec![aev.clone()], Verification::Existence));
                            let resolved = super::resolve_path(&mut out, ctx, &by_path, &generated, src, ID)
                                .or_else(|| {
                                    // a source written and not yet committed: the artifact
                                    // was derived from it all the same, and saying so
                                    // beats calling it an orphan until the commit lands
                                    let abs = ctx.abs(src);
                                    (abs.is_file() || abs.is_dir()).then(|| {
                                        let mut n = super::directory_node(src, ID);
                                        if abs.is_file() {
                                            n.id = format!("file:{src}");
                                            n.title = src.clone();
                                        }
                                        n.summary = Some("present in the working tree and not yet tracked".into());
                                        let nid = n.id.clone();
                                        out.node(n);
                                        nid
                                    })
                                });
                            let Some(target) = resolved else {
                                continue;
                            };
                            out.relation(Relation {
                                source: id.clone(),
                                target,
                                kind: "derived_from".into(),
                                provenance: Provenance::Observed,
                                evidence: vec![aev.clone()],
                            });
                        }
                        out.node(node);
                    }
                }
                Ok(m) => out.diagnostics.push(crate::model::Diagnostic::warning(
                    "knowledge_manifest_schema",
                    Some(manifest_path.clone()),
                    format!("the manifest carries schema '{}', and this executable reads {}", m.schema, crate::generate::MANIFEST_SCHEMA),
                )),
                Err(e) => out.diagnostics.push(crate::model::Diagnostic::warning(
                    "knowledge_manifest_unreadable",
                    Some(manifest_path.clone()),
                    format!("the manifest is not the document this executable writes: {e}"),
                )),
            }
        }
        out
    }
}

/// The serialised word of a serde enum, for a claim value.
mod erased {
    pub trait Serialize {
        fn word(&self) -> String;
    }
    impl<T: serde::Serialize> Serialize for T {
        fn word(&self) -> String {
            serde_json::to_value(self)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_ids_are_the_capability_and_artifact_identities() {
        assert_eq!(capability_node_id("objects.get"), "capability:objects.get");
        assert_eq!(artifact_node_id("docs/generated/registry.json"), "artifact:docs/generated/registry.json");
    }
}
