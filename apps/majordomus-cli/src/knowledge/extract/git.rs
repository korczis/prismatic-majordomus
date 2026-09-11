//! The `git` extractor: the repository itself, as version control identifies it. One node,
//! the revision as evidence, and the facts git states — the branch, the commit, whether
//! the working tree is clean — as observed claims.

use serde_json::Value;

use crate::git::GitState;
use crate::knowledge::model::{
    Evidence, EvidenceKind, ExtractorInfo, Fingerprint, Granularity, KindInfo, Locator,
    Ownership, PredicateInfo, Provenance, Verification, Visibility,
};

use super::{claim, Extraction, ExtractionContext, Extractor, NodeSpec};

/// The extractor's id.
pub const ID: &str = "git";

/// The git extractor.
pub struct Git;

/// The extractor.
pub fn extractor() -> Git {
    Git
}

/// The repository node's id for a root.
pub fn repository_node_id(root: &std::path::Path) -> String {
    format!("repository:{}", repository_name(root))
}

/// The repository's name: the last component of its root.
pub fn repository_name(root: &std::path::Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "repository".into())
}

impl Extractor for Git {
    fn info(&self, _: &ExtractionContext<'_>) -> ExtractorInfo {
        ExtractorInfo {
            id: ID.into(),
            version: 1,
            title: "Git".into(),
            description: "The repository as version control identifies it: its name, the branch, the commit and the state of the working tree, read from git and from nothing else.".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![KindInfo {
                kind: "repository".into(),
                meaning: "the repository the model is about".into(),
            }],
            relations: vec![],
            predicates: vec![
                PredicateInfo {
                    name: "branch".into(),
                    meaning: "the branch the working tree is on".into(),
                    functional: true,
                },
                PredicateInfo {
                    name: "head".into(),
                    meaning: "the commit the working tree is at".into(),
                    functional: true,
                },
                PredicateInfo {
                    name: "working_tree".into(),
                    meaning: "clean or dirty".into(),
                    functional: true,
                },
                PredicateInfo {
                    name: "tracked_files".into(),
                    meaning: "how many files git tracks".into(),
                    functional: true,
                },
            ],
        }
    }

    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction {
        let mut out = Extraction::default();
        let id = repository_node_id(ctx.root);
        let mut evidence = Vec::new();
        let (branch, head, working_tree) = match ctx.git {
            GitState::Available(info) => {
                if let Some(head) = &info.head {
                    let ev = format!("commit:{head}");
                    out.evidence.push(Evidence {
                        id: ev.clone(),
                        kind: EvidenceKind::Commit,
                        locator: Locator {
                            id: Some(head.clone()),
                            ..Default::default()
                        },
                        fingerprint: Fingerprint {
                            algorithm: "sha1".into(),
                            value: head.clone(),
                            granularity: Granularity::Revision,
                        },
                        extractor: ID.into(),
                        visibility: Visibility::Public,
                        remote_processing: true,
                    });
                    evidence.push(ev);
                }
                (
                    info.branch.clone(),
                    info.head.clone(),
                    info.working_tree.clone(),
                )
            }
            GitState::Unavailable { .. } => (None, None, "unknown".to_string()),
        };
        let mut node = NodeSpec {
            id: id.clone(),
            title: repository_name(ctx.root),
            summary: Some(match (&branch, &head) {
                (Some(b), Some(h)) => format!("on {b} at {}", &h[..12.min(h.len())]),
                _ => "not a git work tree, or git could not be asked".into(),
            }),
            provenance: Provenance::Observed,
            ownership: Ownership::External,
            visibility: Visibility::Public,
            evidence: evidence.clone(),
            source: None,
            extractor: ID,
        }
        .build();
        if let Some(b) = &branch {
            node.claims.push(claim(
                &id,
                "branch",
                Value::String(b.clone()),
                Provenance::Observed,
                evidence.clone(),
                Verification::Existence,
            ));
        }
        if let Some(h) = &head {
            node.claims.push(claim(
                &id,
                "head",
                Value::String(h.clone()),
                Provenance::Observed,
                evidence.clone(),
                Verification::Existence,
            ));
        }
        node.claims.push(claim(
            &id,
            "working_tree",
            Value::String(working_tree),
            Provenance::Observed,
            evidence.clone(),
            Verification::Existence,
        ));
        node.claims.push(claim(
            &id,
            "tracked_files",
            Value::from(ctx.tracked.len()),
            Provenance::Observed,
            evidence,
            Verification::Existence,
        ));
        out.node(node);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_is_named_after_its_root() {
        assert_eq!(
            repository_node_id(std::path::Path::new("/a/b/prismatic-majordomus")),
            "repository:prismatic-majordomus"
        );
        assert_eq!(repository_name(std::path::Path::new("/")), "repository");
    }
}
