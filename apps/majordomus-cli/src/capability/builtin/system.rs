//! The `system` module: whether what this process serves is healthy, answered by the
//! engines that already decide it rather than by checks written a second time here.
//!
//! Every check delegates: the index's own diagnostics say whether the layer read cleanly,
//! the benchmark projection's coverage says whether every executable capability is timed,
//! and `generate --check` — the same comparison the command runs — says whether the
//! committed projections are stale. A check that computed its own verdict would be a
//! second opinion, and two opinions drift.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bench::{BenchmarkProjection, Coverage};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::generate;
use crate::git::GitState;
use crate::index::State;
use crate::model::Severity;
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which `system.health` is read as an MCP resource.
pub const HEALTH_URI: &str = "majordomus://health";

/// Where one dimension of the system stands. Ordered by severity, so the worst check
/// decides the whole.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// The engine that decides this dimension is satisfied.
    Ok,
    /// Nothing is broken and something is worth looking at.
    Warn,
    /// The engine that decides this dimension is not satisfied.
    Fail,
    /// This process cannot decide it, and says so rather than reporting `ok`.
    Unknown,
}

impl HealthStatus {
    /// The word as serialised.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::system::HealthStatus;
    /// assert_eq!(HealthStatus::Warn.as_str(), "warn");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            HealthStatus::Ok => "ok",
            HealthStatus::Warn => "warn",
            HealthStatus::Fail => "fail",
            HealthStatus::Unknown => "unknown",
        }
    }

    /// The worse of two: the whole is as good as its worst check, and `unknown` is worse
    /// than `warn` because an undecided dimension is not a healthy one.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::system::HealthStatus::*;
    /// assert_eq!(Ok.worse(Warn), Warn);
    /// assert_eq!(Fail.worse(Unknown), Unknown);
    /// ```
    pub fn worse(self, other: Self) -> Self {
        if other > self {
            other
        } else {
            self
        }
    }
}

/// One dimension of the system, decided by one engine, with what a reader needs to check
/// it themselves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HealthCheck {
    /// A stable id, `[a-z][a-z0-9-]*`.
    pub id: String,
    /// The short name.
    pub title: String,
    /// Where it stands.
    pub status: HealthStatus,
    /// One line: what the engine decided and on what.
    pub detail: String,
    /// The engine that decided it, named so the reader knows what to fix.
    pub decided_by: String,
    /// What reproduces the verdict: a command, or a path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    /// Findings behind the verdict, when the engine produced any: the diagnostic
    /// messages, the stale files, the missing coverage lines.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

/// The health of what this process serves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Health {
    /// The worst check's status.
    pub status: HealthStatus,
    /// How many checks stand where, by status word.
    pub tallies: BTreeMap<String, usize>,
    /// Every dimension, in a stable order.
    pub checks: Vec<HealthCheck>,
}

fn health(ctx: &Context, _: Empty) -> Result<Health, CapabilityError> {
    let index = &ctx.index;
    let mut checks = Vec::new();

    // --- the layer, as the index read it
    let errors = index
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    let warnings = index
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    checks.push(HealthCheck {
        id: "layer".into(),
        title: "The layer as it was read".into(),
        status: match (index.state, warnings) {
            (State::Degraded, _) => HealthStatus::Fail,
            (State::Ok, 0) => HealthStatus::Ok,
            (State::Ok, _) => HealthStatus::Warn,
        },
        detail: format!(
            "{} object(s) indexed; {errors} error(s), {warnings} warning(s)",
            index.objects.len()
        ),
        decided_by: "the index's own diagnostics".into(),
        evidence: vec!["majordomus capabilities validate".into()],
        findings: index
            .diagnostics
            .iter()
            .filter(|d| d.severity != Severity::Info)
            .map(|d| {
                format!(
                    "{}: {} ({})",
                    d.code,
                    d.message,
                    d.path.as_deref().unwrap_or("-")
                )
            })
            .collect(),
    });

    // --- the registry: it exists, therefore it validated; a registry that does not build
    // stops the process before any transport is bound
    let summary = ctx.registry.summary();
    checks.push(HealthCheck {
        id: "registry".into(),
        title: "The capability registry".into(),
        status: HealthStatus::Ok,
        detail: format!(
            "{} capabilities ({} builtin, {} declarative) across {} modules; fingerprint {}",
            summary.total,
            summary.builtin,
            summary.declarative,
            summary.modules,
            &ctx.registry.fingerprint()[..12.min(ctx.registry.fingerprint().len())]
        ),
        decided_by:
            "the registry builder; it refuses to build on a duplicate or a malformed exposure"
                .into(),
        evidence: vec!["majordomus capabilities validate".into()],
        findings: Vec::new(),
    });

    // --- the scope: what a worker reads of this repository and what it never reads
    let tally = &index.scoped.tally;
    checks.push(HealthCheck {
        id: "scope".into(),
        title: "The declared scope".into(),
        status: HealthStatus::Ok,
        detail: format!(
            "{} of {} tracked file(s) in scope, {} out, read from {}",
            tally.r#in, tally.files, tally.out, index.repository.scope_path
        ),
        decided_by: "the scope declaration the index was built under".into(),
        evidence: vec!["majordomus scope".into()],
        findings: Vec::new(),
    });

    // --- git
    let (git_status, git_detail) = match &index.repository.git {
        GitState::Available(info) => {
            let head = info.head.as_deref().unwrap_or("(unborn)");
            (
                if info.working_tree == "clean" {
                    HealthStatus::Ok
                } else {
                    HealthStatus::Warn
                },
                format!(
                    "{} at {}, working tree {}",
                    info.branch.as_deref().unwrap_or("(detached)"),
                    &head[..12.min(head.len())],
                    info.working_tree
                ),
            )
        }
        GitState::Unavailable { reason } => (HealthStatus::Unknown, reason.clone()),
    };
    checks.push(HealthCheck {
        id: "git".into(),
        title: "Version control".into(),
        status: git_status,
        detail: git_detail,
        decided_by: "git, as the index asked it once at startup".into(),
        evidence: vec!["git status".into()],
        findings: Vec::new(),
    });

    // --- benchmark coverage, from the projection that defines the targets
    let projection = BenchmarkProjection::from_context(ctx);
    let coverage = Coverage::compute(ctx, &projection);
    let total = coverage.tallies.get("total").cloned().unwrap_or_default();
    checks.push(HealthCheck {
        id: "benchmark-coverage".into(),
        title: "Benchmark coverage".into(),
        status: if total.missing > 0 {
            HealthStatus::Fail
        } else if total.waived > 0 {
            HealthStatus::Warn
        } else {
            HealthStatus::Ok
        },
        detail: format!(
            "{} of {} target(s) covered, {} missing, {} waived",
            total.covered, total.required, total.missing, total.waived
        ),
        decided_by:
            "the benchmark projection's coverage, the same one `bench coverage --check` reads"
                .into(),
        evidence: vec!["majordomus bench coverage --check".into()],
        findings: coverage
            .lines
            .iter()
            .filter(|l| matches!(l.state, crate::bench::CoverageState::Missing))
            .map(|l| format!("{} on {}: no case", l.subject, l.transport.name()))
            .collect(),
    });

    // --- the committed registry manifest against the registry this process built.
    // Only that one artifact: it is derived from the code alone, and rendering it costs
    // nothing but serialising the descriptors already in memory. Rendering the OpenAPI
    // document, the reference and the benchmark matrix on every health call would rebuild
    // canonical state per request, which this executable does not do; `generate --check`
    // is the complete answer and this check names it.
    let root = std::path::Path::new(&index.repository.root);
    let manifest = generate::registry_manifest(&ctx.registry, crate::VERSION);
    let path = format!("{}/registry.json", generate::OUT_DIR);
    let committed = std::fs::read_to_string(root.join(&path));
    let (status, detail, findings) = match &committed {
        Ok(text) if *text == manifest => (
            HealthStatus::Ok,
            format!("{path} matches the registry this process built"),
            Vec::new(),
        ),
        Ok(_) => (
            HealthStatus::Fail,
            format!("{path} differs from the registry this process built"),
            vec![format!("{path} (differs)")],
        ),
        Err(_) => (
            HealthStatus::Unknown,
            format!("{path} could not be read"),
            vec![format!("{path} (missing)")],
        ),
    };
    checks.push(HealthCheck {
        id: "generated-registry".into(),
        title: "Committed projections".into(),
        status,
        detail,
        decided_by:
            "the same rendering `majordomus generate` writes, compared with what is committed"
                .into(),
        evidence: vec!["majordomus generate --check".into()],
        findings,
    });

    // --- the peers attached to this process
    let peers = ctx.peers.list();
    checks.push(HealthCheck {
        id: "peers".into(),
        title: "Attached clients".into(),
        status: HealthStatus::Ok,
        detail: format!("{} peer(s) attached to this process", peers.len()),
        decided_by: "the in-memory peer board of this process".into(),
        evidence: vec!["majordomus_peers".into()],
        findings: Vec::new(),
    });

    let status = checks
        .iter()
        .fold(HealthStatus::Ok, |acc, c| acc.worse(c.status));
    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for c in &checks {
        *tallies.entry(c.status.as_str().into()).or_insert(0) += 1;
    }
    Ok(Health {
        status,
        tallies,
        checks,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "system",
        title: "System",
        description: "Whether what this process serves is healthy, decided by the engines that already decide it: the index's diagnostics, the registry builder, the benchmark projection's coverage and the comparison `generate --check` makes. No check here has an opinion of its own.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "system.health",
                title: "Health of this process",
                description: "Every dimension of what this process serves — the layer as it was read, the registry, the scope, version control, benchmark coverage, the committed registry manifest and the attached peers — each decided by the engine that owns it, with the command that reproduces the verdict.",
                input: Empty,
                output: Health,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_health".into()),
                        resource: Some(McpResource { uri: HEALTH_URI.into(), name: "health".into() }),
                    }),
                    http: get("/api/v1/health"),
                    cli: None,
                },
                tags: ["system", "health", "introspection"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: Some(5) },
                handler: health,
            },
        ],
    }
}
