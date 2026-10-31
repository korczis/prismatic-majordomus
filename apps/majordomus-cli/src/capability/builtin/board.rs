//! The `board` module: the delivery board's documents, derived by one capability (I4294).
//!
//! `board.snapshot` reads the plan, the roadmap, the intents, the recorded pull request
//! queue and the release currency, and composes the documents the board's page reads
//! through `crate::board`, with the lanes `share/board.yaml` declares. It reads and writes
//! nothing else: the sync step that publishes the documents adds the mesh seats, live work,
//! CI runners and blockers it observes itself, so none of them is here.

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::board::{self, BoardDeclaration, Inputs, Master, PrItem, Queue};
use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::plan::{Plan, PlanMilestone};
use crate::{capability, module};

use super::{get, mcp};

/// The input of `board.snapshot`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoardSnapshotInput {
    /// Who takes the snapshot, as `meta.syncedBy` names it; `board.snapshot` when absent.
    /// Left out of the digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) by: Option<String>,
}

impl BenchmarkCases for BoardSnapshotInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", BoardSnapshotInput::default()),
            NamedCase::new(
                "named",
                BoardSnapshotInput {
                    by: Some("a mesh session".into()),
                },
            ),
        ]
    }
}

/// The answer of `board.snapshot`: every document of the board by id, and the digest that
/// says whether two snapshots describe the same state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct BoardSnapshot {
    /// `sha256:<hex>` over the canonical JSON of the documents, `meta.syncedAt` and
    /// `meta.syncedBy` left out: equal state, equal digest.
    pub(crate) digest: String,
    /// The commit described, empty when the checkout has none.
    pub(crate) master_sha: String,
    /// Its committer time, RFC 3339.
    pub(crate) master_committed_at: String,
    /// The ref it was read from: `refs/remotes/origin/master`, else `HEAD`.
    pub(crate) master_ref: String,
    /// The bound every document is under, in bytes of compact JSON.
    pub(crate) document_limit: usize,
    /// Each document's compact size, in bytes, by id.
    pub(crate) sizes: BTreeMap<String, usize>,
    /// The documents, by id: `meta`, `milestones`, `intents`, `prs`, and `issues-<lane>`
    /// (then `issues-<lane>-2`, ...) for each lane with open work.
    pub(crate) documents: BTreeMap<String, Value>,
    /// What could not be read, in words: an unobserved pull request queue, a lane naming a
    /// milestone the plan lacks. Empty is the ordinary case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) diagnostics: Vec<String>,
}

/// The pull request queue, as the board shows it: the recorded observation, offline.
fn queue(root: &Path) -> Queue {
    match crate::integration::queue_of(root) {
        Ok(q) => Queue::Observed {
            observed_at: q.observed_at.clone(),
            items: q
                .assessments
                .iter()
                .map(|a| PrItem {
                    n: a.number,
                    t: a.title.clone(),
                    b: a.head_ref.clone(),
                    disp: board::word(&a.disposition),
                    lane: board::word(&a.lane),
                    why: a.reasons.iter().take(4).map(board::word).collect(),
                    head: a.evaluated_against.head_sha.chars().take(11).collect(),
                    train: a.head_ref.starts_with("integration/train-"),
                    risk: board::word(&a.risk),
                })
                .collect(),
        },
        Err(reason) => Queue::Unavailable(reason),
    }
}

fn board_snapshot(
    ctx: &Context,
    input: BoardSnapshotInput,
) -> Result<BoardSnapshot, CapabilityError> {
    let root = Path::new(&ctx.index.repository.root);
    let declaration = match &ctx.index.share {
        Some(share) => BoardDeclaration::load(share).map_err(CapabilityError::Internal)?,
        None => BoardDeclaration::default(),
    };
    let plan = Plan::build(&ctx.index);
    let roadmap: Vec<PlanMilestone> = plan
        .roadmap()
        .into_iter()
        .map(|m| PlanMilestone {
            released_in: Some(crate::plan::released_in(&ctx.index, &plan, &m.id)),
            ..m.clone()
        })
        .collect();
    let evidence = crate::intent::RepositoryEvidence::load(&ctx.index)
        .map_err(|e| CapabilityError::Internal(e.to_string()))?;
    let intents = crate::intent::Intents::build(&ctx.index, &plan, &evidence)
        .intents
        .iter()
        .filter_map(|x| serde_json::to_value(x).ok())
        .collect();
    let release = serde_json::to_value(crate::release::currency::measure(
        root,
        &ctx.index.objects,
        std::time::SystemTime::now(),
    ))
    .unwrap_or_default();
    let master = Master::read(root);
    let composed = board::compose(Inputs {
        declaration: &declaration,
        plan: &plan,
        roadmap,
        intents,
        queue: queue(root),
        release,
        master: master.clone(),
        synced_at: crate::peers::rfc3339(std::time::SystemTime::now()),
        synced_by: input.by.unwrap_or_else(|| "board.snapshot".into()),
    })
    .map_err(CapabilityError::Refused)?;
    let master = master.unwrap_or_default();
    Ok(BoardSnapshot {
        digest: board::digest(&composed.documents),
        master_sha: master.sha,
        master_committed_at: master.committed_at,
        master_ref: master.reference,
        document_limit: board::DOCUMENT_LIMIT,
        sizes: composed
            .documents
            .iter()
            .map(|(id, d)| (id.clone(), board::size(d)))
            .collect(),
        documents: composed.documents,
        diagnostics: composed.diagnostics,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "board",
        title: "The delivery board",
        description: "The documents the delivery board reads, derived from the plan, the roadmap, the intents, the recorded pull request queue and the release currency, with the lanes share/board.yaml declares once. Every open milestone is in a lane or the snapshot refuses, naming it.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "board.snapshot",
                title: "The delivery board's documents",
                description: "Every document of the delivery board by id: meta (the commit, the lanes, the issue counts, the release), milestones in roadmap order, intents, the pull request queue from the last recorded observation, and the open issues of each lane as issues-<lane>, split into issues-<lane>-2 and onwards so that no document serializes past 250000 bytes. The digest is a SHA-256 over the canonical JSON of the documents with meta.syncedAt and meta.syncedBy left out, so equal state is an equal digest. A milestone that is not DONE and is in no lane of share/board.yaml is refused by name, never filed under other. An unobserved queue is said in prs.unavailable and the diagnostics, never refused. Mesh seats, live work, CI runners and blockers are the sync step's and are not here. Reads only; nothing is cached.",
                input: BoardSnapshotInput,
                output: BoardSnapshot,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_board_snapshot"),
                    http: get("/api/v1/board/snapshot"),
                    cli: Some(CliExposure { path: vec!["board".into(), "snapshot".into()] }),
                },
                tags: ["board", "plan", "delivery"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::RepositoryDeclaration },
                handler: board_snapshot,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "board");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["board.snapshot"]);
        let c = &m.capabilities[0].capability;
        assert_eq!(
            c.exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
            Some("majordomus_board_snapshot")
        );
        assert_eq!(
            c.exposure.http.as_ref().map(|h| h.path.as_str()),
            Some("/api/v1/board/snapshot")
        );
        assert_eq!(
            c.exposure.cli.as_ref().map(|c| c.path.clone()),
            Some(vec!["board".to_string(), "snapshot".to_string()])
        );
        assert_eq!(c.kind, crate::capability::model::CapabilityKind::Query);
        assert_eq!(c.cache, CachePolicy::Disabled);
    }

    #[test]
    fn a_repository_with_no_open_milestone_composes_and_says_the_queue_is_unobserved() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let a = board_snapshot(&ctx, BoardSnapshotInput::default()).expect("a snapshot");
        for id in ["meta", "milestones", "intents", "prs"] {
            assert!(a.documents.contains_key(id), "{id} is missing");
            assert_eq!(a.sizes[id], board::size(&a.documents[id]));
        }
        assert_eq!(a.documents["meta"]["syncedBy"], "board.snapshot");
        assert!(a.documents["prs"]["unavailable"].is_string());
        assert!(a.diagnostics.iter().any(|d| d.contains("queue")));
        assert_eq!(a.document_limit, board::DOCUMENT_LIMIT);

        let b = board_snapshot(
            &ctx,
            BoardSnapshotInput {
                by: Some("another session".into()),
            },
        )
        .expect("a snapshot");
        assert_eq!(b.documents["meta"]["syncedBy"], "another session");
        assert_eq!(a.digest, b.digest, "who took it is not state");
    }

    #[test]
    fn an_open_milestone_in_no_lane_is_refused() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).expect("sources");
        text.push_str("  - id: milestone\n    kind: milestone\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/project/milestones/*.yaml'\n    required: false\n");
        std::fs::write(&sources, text).expect("sources written");
        let dir = repo.root().join(".ai/repo/project/milestones");
        std::fs::create_dir_all(&dir).expect("milestones");
        std::fs::write(
            dir.join("unlaned-outcome.yaml"),
            "id: unlaned-outcome\ntitle: An outcome no lane holds\nslug: unlaned-outcome\norder: 1\npriority: p1\nproblem: p\noutcome: o\nacceptance_criteria:\n  - it holds\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\n",
        )
        .expect("milestone");
        let ctx = repo.context().expect("a context");
        let plan = Plan::build(&ctx.index);
        assert!(
            plan.milestone("unlaned-outcome").is_some(),
            "the fixture milestone was not read: {:?}",
            ctx.index.diagnostics
        );
        match board_snapshot(&ctx, BoardSnapshotInput::default()) {
            Err(CapabilityError::Refused(m)) => assert!(m.contains("unlaned-outcome"), "{m}"),
            other => panic!("not refused: {other:?}"),
        }
    }
}
