//! The `workflows` extractor: the continuous-integration workflows and the deployment
//! manifests a repository carries, as the files they are. A workflow is a node with its
//! declared name; the file is its evidence. Nothing here interprets a job.

use serde_json::Value;

use crate::knowledge::model::{
    ExtractorInfo, KindInfo, Ownership, PredicateInfo, Provenance, Verification, Visibility,
};

use super::{claim, tracked_where, Extraction, ExtractionContext, Extractor, NodeSpec};

/// The extractor's id.
pub const ID: &str = "workflows";

/// The workflows extractor.
pub struct Workflows;

/// The extractor.
pub fn extractor() -> Workflows {
    Workflows
}

/// Is a path a CI workflow?
fn is_workflow(path: &str) -> bool {
    path.starts_with(".github/workflows/") && (path.ends_with(".yml") || path.ends_with(".yaml"))
}

/// Is a path a deployment manifest this extractor recognises by convention?
fn is_deployment_manifest(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    matches!(name, "Dockerfile" | "fly.toml" | "docker-compose.yml" | "docker-compose.yaml")
        || name.starts_with("Dockerfile.")
}

/// The `name:` a workflow declares at its top level, when it declares one.
fn workflow_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("name:"))
        .map(|n| n.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|n| !n.is_empty())
}

impl Extractor for Workflows {
    fn info(&self, _: &ExtractionContext<'_>) -> ExtractorInfo {
        ExtractorInfo {
            id: ID.into(),
            version: 1,
            title: "Workflows and deployment manifests".into(),
            description: "Every CI workflow under .github/workflows/ and every deployment manifest a convention names (Dockerfile, fly.toml, docker-compose): one node each, the file as evidence, the declared name as a claim. Nothing here runs or interprets a job.".into(),
            deterministic: true,
            reads_sensitive: false,
            kinds: vec![
                KindInfo { kind: "workflow".into(), meaning: "a continuous-integration workflow".into() },
                KindInfo { kind: "deployment_manifest".into(), meaning: "a file that says how the repository is built into a running thing".into() },
            ],
            relations: vec![],
            predicates: vec![
                PredicateInfo { name: "name".into(), meaning: "the name the file declares".into(), functional: true },
                PredicateInfo { name: "path".into(), meaning: "where the file is".into(), functional: true },
            ],
        }
    }

    fn extract(&self, ctx: &ExtractionContext<'_>) -> Extraction {
        let mut out = Extraction::default();
        let paths: Vec<String> = tracked_where(ctx, |p| is_workflow(p) || is_deployment_manifest(p))
            .cloned()
            .collect();
        for path in paths {
            let Some(text) = ctx.read(&path) else {
                continue;
            };
            let ev = out.file_evidence(ID, &path, text.as_bytes(), Visibility::Public);
            let (kind, title) = if is_workflow(&path) {
                (
                    "workflow",
                    workflow_name(&text).unwrap_or_else(|| {
                        path.rsplit('/').next().unwrap_or(&path).to_string()
                    }),
                )
            } else {
                ("deployment_manifest", path.clone())
            };
            let id = format!("{kind}:{path}");
            let mut node = NodeSpec {
                id: id.clone(),
                title,
                summary: None,
                provenance: Provenance::Observed,
                ownership: Ownership::External,
                visibility: Visibility::Public,
                evidence: vec![ev.clone()],
                source: Some(path.clone()),
                extractor: ID,
            }
            .build();
            node.claims.push(claim(
                &id,
                "path",
                Value::String(path.clone()),
                Provenance::Observed,
                vec![ev.clone()],
                Verification::Existence,
            ));
            if let Some(name) = workflow_name(&text) {
                node.claims.push(claim(
                    &id,
                    "name",
                    Value::String(name),
                    Provenance::Observed,
                    vec![ev],
                    Verification::Content,
                ));
            }
            out.node(node);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workflow_is_recognised_by_its_path_and_named_by_its_declaration() {
        assert!(is_workflow(".github/workflows/validate.yml"));
        assert!(!is_workflow(".github/actions/setup/action.yml"));
        assert!(is_deployment_manifest("deploy/Dockerfile"));
        assert!(is_deployment_manifest("fly.toml"));
        assert_eq!(workflow_name("# c\nname: validate\non: push\n").as_deref(), Some("validate"));
        assert_eq!(workflow_name("on: push\n"), None);
    }
}
