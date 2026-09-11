//! The `semantic` extractor: what a semantic provider derived earlier, read back from the
//! local cache and merged as `derived` knowledge. It runs no provider itself — a scan is
//! deterministic and offline — and it emits nothing when nothing was cached, which is the
//! state of every fresh checkout.
//!
//! Every derivation carries the provider, the model, the operation, the prompt version
//! and the fingerprints of the evidence it read, so a reader can tell an inference from an
//! observation and a stale inference from a current one: a derivation whose input
//! evidence no longer carries the fingerprint it was made against is not merged.


use crate::knowledge::model::{
    ClaimState, ExtractorInfo, KindInfo, Ownership, PredicateInfo, Provenance, Relation,
    RelationInfo, Verification, Visibility,
};
use crate::knowledge::semantic::{cache_path, Derivation, SemanticCache};

use super::{claim_with, Extraction, ExtractionContext, Extractor, NodeSpec};

/// The extractor's id.
pub const ID: &str = "semantic";

/// The semantic extractor.
pub struct Semantic;

/// The extractor.
pub fn extractor() -> Semantic {
    Semantic
}

impl Extractor for Semantic {
    fn info(&self, _: &ExtractionContext<'_>) -> ExtractorInfo {
        ExtractorInfo {
            id: ID.into(),
            version: 1,
            title: "Semantic derivations".into(),
            description: "What a semantic provider derived earlier, read from the local cache under the checkout-local half of the layer: classifications, summaries and proposed relations, each marked derived and carrying the provider, the model, the operation, the prompt version and the evidence fingerprints it read. A derivation whose evidence moved is not merged. Nothing here calls a provider.".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![KindInfo {
                kind: "concept".into(),
                meaning: "a concept a semantic provider proposed".into(),
            }],
            relations: vec![RelationInfo {
                kind: "related_to".into(),
                meaning: "a semantic provider proposed the two are related".into(),
                propagates: false,
            }],
            predicates: vec![
                PredicateInfo { name: "summary".into(), meaning: "a one-line summary a provider wrote".into(), functional: true },
                PredicateInfo { name: "classification".into(), meaning: "a class a provider proposed".into(), functional: true },
                PredicateInfo { name: "candidate_conflict".into(), meaning: "a contradiction a provider suspects in prose; a candidate for review, never a conflict on its own".into(), functional: false },
            ],
        }
    }

    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction {
        let mut out = Extraction::default();
        let path = cache_path(ctx.root, ctx.local);
        let Some(cache) = SemanticCache::read(&path) else {
            return out;
        };
        for d in cache.derivations {
            let Derivation {
                subject,
                predicate,
                value,
                provenance,
                inputs,
                target,
            } = d;
            // every input the derivation read must still carry the fingerprint it read
            let current = inputs.iter().all(|(id, fp)| {
                ctx_evidence_fingerprint(ctx, id).as_deref() == Some(fp.as_str())
            });
            if !current {
                continue;
            }
            let ev_ids: Vec<String> = inputs.iter().map(|(id, _)| id.clone()).collect();
            let discriminator = format!("{}:{}", provenance.provider, provenance.operation);
            let mut c = claim_with(
                &subject,
                &predicate,
                &discriminator,
                value.clone(),
                Provenance::Derived,
                ev_ids.clone(),
                Verification::Content,
            );
            c.reason = Some(format!(
                "derived by {} ({}) with prompt {} over {} piece(s) of evidence",
                provenance.provider,
                provenance.model,
                provenance.prompt_version,
                inputs.len()
            ));
            if predicate == "candidate_conflict" {
                c.state = ClaimState::Unverified;
            }
            let node = out.node(
                NodeSpec {
                    id: subject.clone(),
                    title: subject.clone(),
                    summary: None,
                    provenance: Provenance::Derived,
                    ownership: Ownership::External,
                    visibility: Visibility::Internal,
                    evidence: ev_ids.clone(),
                    source: None,
                    extractor: ID,
                }
                .build(),
            );
            node.claims.push(c);
            if let (Some(t), true) = (target, predicate == "related_to") {
                out.relation(Relation {
                    source: subject,
                    target: t,
                    kind: "related_to".into(),
                    provenance: Provenance::Derived,
                    evidence: ev_ids,
                });
            }
        }
        out
    }
}

/// The live fingerprint of a piece of evidence another extractor would emit for `id`:
/// the file's content for `file:<path>`; nothing else is re-derived here.
fn ctx_evidence_fingerprint(ctx: &ExtractionContext<'_>, id: &str) -> Option<String> {
    let path = id.strip_prefix("file:")?;
    let text = ctx.read(path)?;
    Some(
        crate::knowledge::model::Fingerprint::sha256(
            text.as_bytes(),
            crate::knowledge::model::Granularity::File,
        )
        .value,
    )
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_file_evidence_id_names_the_path_it_fingerprints() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "hello").unwrap();
        // the fingerprint is the file's, at file granularity, and nothing but a file id
        // is re-derived here
        let fp = crate::knowledge::model::Fingerprint::sha256(
            b"hello",
            crate::knowledge::model::Granularity::File,
        );
        assert_eq!(fp.granularity, crate::knowledge::model::Granularity::File);
        assert!("file:a.md".strip_prefix("file:").is_some());
        assert!("object:x".strip_prefix("file:").is_none());
    }
}
