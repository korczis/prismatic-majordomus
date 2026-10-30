//! The `health` module: whether what this process serves is healthy, answered by the
//! engines that already decide it rather than by checks written a second time here.
//!
//! The module is `health` and not `system` because the benchmark projection reserves
//! `system` for the transports' own targets (`system.http.index`, `system.mcp.ping`) and
//! buckets its coverage by that name. A capability module called `system` would have its
//! lines tallied as transport targets and silently vanish from the per-transport
//! denominators; `reserved_namespace_is_not_a_module` holds that shut.
//!
//! Every check delegates: the index's own diagnostics say whether the layer read cleanly,
//! the benchmark projection's coverage says whether every executable capability is timed,
//! `generate --check` — the same comparison the command runs — says whether the committed
//! projections are stale, and [`crate::capability::builtin::server::standing_at`] — the
//! reading `server.status` answers from — says where the shared server of this checkout
//! stands. A check that computed its own verdict would be a second opinion, and two
//! opinions drift.
//!
//! # Four readings of "ready", and why none of them is this one
//!
//! This crate answers "ready" four times, about four different subjects. The full table,
//! with what asks each one, is in `docs/MCP.md`; in short:
//!
//! - [`crate::http::Served::ready`] — is a *surface's* producer's output on disk? Per
//!   surface, and what the home page renders as `not built`.
//! - [`Readiness`] (`health.ready`, `GET /api/v1/ready`) — can *this process* answer a
//!   request? Local initialisation only, never a dependency: a readiness probe that
//!   contacts another service fails a deployment for something that is not this process.
//! - [`crate::environment::ServiceAvailability`] — does *anything* accept a connection at
//!   the address the lease published? One bounded TCP connect, no DNS, because it runs on
//!   a shell prompt.
//! - [`crate::capability::builtin::server::ServerStanding`] — is what answers at that
//!   address *current*: this checkout's server, from the file on disk, at this version?
//!
//! The `server` check below is the fourth, and it is the only one of them that a health
//! report should carry: the first is per surface, the second is the report's own caller,
//! and the third cannot tell a live server from a socket somebody else holds.

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

use super::server::{self, ServerStanding};
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
    /// use majordomus_cli::capability::builtin::health::HealthStatus;
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
    /// use majordomus_cli::capability::builtin::health::HealthStatus::*;
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

/// The dimensions that describe the process serving a checkout and not the repository: the
/// shared server, its peer board and its mesh runtime. Only a process holding the checkout's
/// lease decides the last two, so in any other they are `unknown` by construction, never by
/// a fault of the repository.
///
/// ```
/// use majordomus_cli::capability::builtin::health::PROCESS_DIMENSIONS;
/// assert!(PROCESS_DIMENSIONS.contains(&"mesh"));
/// assert!(!PROCESS_DIMENSIONS.contains(&"layer"));
/// ```
pub const PROCESS_DIMENSIONS: [&str; 3] = ["server", "peers", "mesh"];

/// The worst status among the checks that describe the repository
/// (every one whose id is not in [`PROCESS_DIMENSIONS`]); `ok` when there is none.
///
/// ```
/// use majordomus_cli::capability::builtin::health::{repository_status, HealthCheck, HealthStatus};
/// let check = |id: &str, status| HealthCheck {
///     id: id.into(),
///     title: id.into(),
///     status,
///     detail: String::new(),
///     decided_by: String::new(),
///     evidence: Vec::new(),
///     findings: Vec::new(),
/// };
/// let checks = [check("layer", HealthStatus::Warn), check("mesh", HealthStatus::Unknown)];
/// assert_eq!(repository_status(&checks), HealthStatus::Warn);
/// ```
pub fn repository_status(checks: &[HealthCheck]) -> HealthStatus {
    checks
        .iter()
        .filter(|c| !PROCESS_DIMENSIONS.contains(&c.id.as_str()))
        .fold(HealthStatus::Ok, |acc, c| acc.worse(c.status))
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
    /// The worst status among the dimensions that describe the repository at this commit:
    /// every check but the ones in [`PROCESS_DIMENSIONS`]. A process that is no server —
    /// the command line, or the reader of a published snapshot (ADR 0141) — cannot decide
    /// who is attached or what the mesh runtime did, and says `unknown` for them; this is
    /// the verdict that is the same whichever process read the repository, and it is what a
    /// publication shows first.
    pub repository: HealthStatus,
    /// How many checks stand where, by status word.
    pub tallies: BTreeMap<String, usize>,
    /// Every dimension, in a stable order.
    pub checks: Vec<HealthCheck>,
    /// When the state a check decided on was read, by check id: the freshness contract
    /// ([`crate::index::AnswerObservation`]). A check absent here decided live, during this call.
    /// `layer` and `git` are here because they judge the picture the index took, which is
    /// as old as the index.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub observed: BTreeMap<String, crate::index::AnswerObservation>,
}

/// The answer to "is this process alive": the cheapest true thing this executable can
/// say about itself. Deliberately not a summary of anything — a probe that grew an
/// opinion would stop reporting the one fact the platform polls it for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Liveness {
    /// Always `true`. A process that could not answer would not answer at all, which is
    /// the signal.
    pub alive: bool,
    /// This executable's version, so a rolling deployment can tell which build answered.
    pub version: String,
    /// The commit this executable was built from, in full, or `unknown` when the build could
    /// not name one. A version is what a build calls itself and two revisions share it; this
    /// is what a deployment is checked against. Never omitted: an absent field would read as
    /// a server too old to say, and `unknown` is a different, truthful answer.
    pub commit: String,
    /// Whether that commit's tree carried uncommitted changes when this executable was built;
    /// `null` when the build did not know.
    pub dirty: Option<bool>,
}

/// The answer to "can this process serve traffic": the local initialisation a request
/// would need, and nothing beyond this process. A readiness check that grew a dependency
/// probe fails a deployment because an unrelated service is down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Readiness {
    /// Whether every part below is in place.
    pub ready: bool,
    /// This executable's version.
    pub version: String,
    /// The commit this executable was built from, or `unknown`; the same value `health.live`
    /// answers.
    pub commit: String,
    /// Whether the build's tree was dirty; `null` when the build did not know.
    pub dirty: Option<bool>,
    /// How many capabilities the registry holds; zero would mean nothing to serve.
    pub capabilities: usize,
    /// How many objects the index holds. Read from the index this process built at
    /// start-up: it is already in memory, and reading it walks nothing.
    pub objects: usize,
    /// Whether the layer read cleanly. A degraded layer is still served — the diagnostics
    /// are the point — so this reports rather than refuses.
    pub layer: HealthStatus,
    /// Why this process is serving code that is no longer on disk: the executable it was
    /// started from has been replaced or removed since. Present, `ready` is false — a server
    /// answering with yesterday's code says so in the place it says it is ready, to a client
    /// holding nothing but this answer (I1502).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<String>,
    /// Every reason this answer is not a plain yes, as `code: words`. `lease_lost` and
    /// `stale` make `ready` false: another process holds this checkout's lease, or this one
    /// serves code that is gone. `layer_degraded` does not: the layer is served with its
    /// diagnostics, and a client decides whether that is enough.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

/// Liveness. No filesystem, no index traversal, no network: two fields this process can
/// answer with while doing anything else.
fn liveness(_: &Context, _: Empty) -> Result<Liveness, CapabilityError> {
    Ok(Liveness {
        alive: true,
        version: crate::VERSION.into(),
        commit: crate::COMMIT.into(),
        dirty: crate::DIRTY,
    })
}

/// Readiness. Only what this process needs in order to answer a request, all of it already
/// resident: the registry it built, the index it built, and how that index read. Nothing
/// here contacts a provider, a database or another service, and nothing here walks a tree.
fn readiness(ctx: &Context, _: Empty) -> Result<Readiness, CapabilityError> {
    let capabilities = ctx.registry.summary().total;
    let objects = ctx.index.objects.len();
    let stale = crate::lease::serving_replaced_code();
    let lost = crate::lease::was_lost();
    let mut reasons = Vec::new();
    if lost {
        reasons.push("lease_lost: another process holds this checkout's lease; this one serves the sessions it had and takes on nobody new".to_string());
    }
    if let Some(why) = &stale {
        reasons.push(format!("stale: {why}"));
    }
    if ctx.index.state == State::Degraded {
        reasons.push(format!(
            "layer_degraded: the layer was read with {} diagnostic(s); it is served as read",
            ctx.index.diagnostics.len()
        ));
    }
    Ok(Readiness {
        ready: capabilities > 0 && stale.is_none() && !lost,
        reasons,
        version: crate::VERSION.into(),
        commit: crate::COMMIT.into(),
        dirty: crate::DIRTY,
        capabilities,
        objects,
        layer: match ctx.index.state {
            State::Degraded => HealthStatus::Warn,
            State::Ok => HealthStatus::Ok,
        },
        stale,
    })
}

/// Record one dimension, and say on the execution's stream that it was decided.
///
/// The report is the same whoever asked for it: a `GET /api/v1/health`, an MCP tool call
/// and a browser watching an execution all run this function, and the reporting is silent
/// for the two of them that are not executions. That is the whole of what it took to make
/// an existing capability observable — no second implementation, no second route, and no
/// change to what it answers.
fn record(checks: &mut Vec<HealthCheck>, p: &crate::execution::Progress, check: HealthCheck) {
    p.step_done(
        &check.id,
        check.status != HealthStatus::Fail,
        Some(check.detail.clone()),
    );
    checks.push(check);
    p.progress(checks.len() as u64, None, "dimension(s) decided");
}

/// The attached-clients dimension, from the board this process holds (`None` when it holds
/// none). Live peers only are counted as attached; a peer that is gone with its claims still
/// shown, or one attached that has announced nothing, is worth a look, because the board then
/// understates or overstates who is working here.
fn peers_check(board: Option<&[crate::peers::Peer]>) -> HealthCheck {
    let decided_by = "the peer board of the process holding this checkout's lease, tallied as `environment.preflight` tallies it";
    let Some(peers) = board else {
        return HealthCheck {
            id: "peers".into(),
            title: "Attached clients".into(),
            status: HealthStatus::Unknown,
            detail: "this process holds no peer board: it does not hold this checkout's lease, so who is attached is the server's to say".into(),
            decided_by: decided_by.into(),
            evidence: vec!["majordomus_peers".into(), "majordomus serve status".into()],
            findings: Vec::new(),
        };
    };
    let t = crate::environment::preflight::PeersObservation::of(peers);
    let silent = t.attached - t.announced;
    let mut findings = Vec::new();
    if t.detached > 0 {
        findings.push(format!(
            "{} peer(s) are gone and the board still shows their claims",
            t.detached
        ));
    }
    if silent > 0 {
        findings.push(format!(
            "{silent} attached peer(s) have announced nothing, so the board understates who is here"
        ));
    }
    HealthCheck {
        id: "peers".into(),
        title: "Attached clients".into(),
        status: if findings.is_empty() {
            HealthStatus::Ok
        } else {
            HealthStatus::Warn
        },
        detail: format!(
            "{} attached ({} announced) · {} gone with claims still shown",
            t.attached, t.announced, t.detached
        ),
        decided_by: decided_by.into(),
        evidence: vec!["majordomus_peers".into()],
        findings,
    }
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
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
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
        },
    );

    // --- the registry: it exists, therefore it validated; a registry that does not build
    // stops the process before any transport is bound
    let summary = ctx.registry.summary();
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
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
        },
    );

    // --- the scope: what a worker reads of this repository and what it never reads
    let tally = &index.scoped.tally;
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
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
        },
    );

    // --- git
    let (git_status, git_detail, git_decided_by) = git_verdict(
        &index.repository.git,
        &index.repository.discovery,
        crate::COMMIT,
        crate::DIRTY,
    );
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
            id: "git".into(),
            title: "Version control".into(),
            status: git_status,
            detail: git_detail,
            decided_by: git_decided_by.into(),
            evidence: vec!["git status".into()],
            findings: Vec::new(),
        },
    );

    // --- benchmark coverage, from the projection that defines the targets
    let projection = BenchmarkProjection::from_context(ctx);
    let coverage = Coverage::compute(ctx, &projection);
    let total = coverage.tallies.get("total").cloned().unwrap_or_default();
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
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
        },
    );

    // --- the committed registry manifest against the registry this process built.
    // Only that one artifact: it is derived from the code alone, and rendering it costs
    // nothing but serialising the descriptors already in memory. Rendering the OpenAPI
    // document, the reference and the benchmark matrix on every health call would rebuild
    // canonical state per request, which this executable does not do; `generate --check`
    // is the complete answer and this check names it.
    let root = std::path::Path::new(&index.repository.root);
    let manifest = generate::Document::new(
        "registry",
        generate::REGISTRY_SCHEMA,
        "the canonical capability registry",
        generate::registry_manifest(&ctx.registry),
    )
    .artifacts(crate::VERSION);
    let manifest = manifest
        .iter()
        .find(|a| a.format == generate::ArtifactFormat::Json)
        .map(|a| a.content.clone())
        .unwrap_or_default();
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
        // A repository that commits no manifest has nothing to drift from: that is every
        // repository but the ones that run `majordomus generate`, and reading it as
        // `unknown` made the whole report unknown wherever the layer was merely adopted.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            HealthStatus::Ok,
            format!("{path} is not committed here, so there is nothing to compare"),
            Vec::new(),
        ),
        Err(e) => (
            HealthStatus::Unknown,
            format!("{path} could not be read: {e}"),
            vec![format!("{path} (unreadable)")],
        ),
    };
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
            id: "generated-registry".into(),
            title: "Committed projections".into(),
            status,
            detail,
            decided_by:
                "the same rendering `majordomus generate` writes, compared with what is committed"
                    .into(),
            evidence: vec!["majordomus generate --check".into()],
            findings,
        },
    );

    // --- handovers published from other machines
    //
    // `continuity.status` decides it: every record of the local continuity store admitted
    // or refused, and the lineage read from what was admitted. A refused record (forged,
    // foreign, leaking) or a broken line fails; a diverged line, a dangling parent or a
    // record too new to read warns. A repository that never published is not a fault.
    let continuity = crate::continuity::Machine::open_read(
        root,
        super::mesh::declaration(ctx).and_then(Result::ok),
    )
    .and_then(|m| crate::continuity::status(&m));
    let (status, detail, findings) = continuity_health(continuity);
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
            id: "continuity".into(),
            title: "Handovers published across machines".into(),
            status,
            detail,
            decided_by: "the admission and lineage checks of `continuity.status`".into(),
            evidence: vec!["majordomus-cli continuity status".into()],
            findings,
        },
    );

    // --- the shared server of this checkout
    //
    // Not a second opinion and not a second reading: `server::standing_at` is what answers
    // `GET /api/v1/server`, and this reports what it says. It costs one small file and,
    // when the lease names an address, one probe bounded by the declared probe timeout; the
    // five-second cache on `health.report` is as fresh as a file other processes write can
    // usefully be, and `server.status` is the uncached answer for a caller that needs one.
    let checkout = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let local_half = server::local_half(&checkout);
    let lease_path = crate::lease::lease_file(&checkout, &local_half);
    let (standing, reason, lease) = server::standing_at(&checkout, &local_half);
    let detail = match lease.document() {
        Some(doc) => format!(
            "{}: {}, version {}, pid {}, since {}",
            standing.as_str(),
            doc.url.as_deref().unwrap_or("no address published yet"),
            doc.version.as_deref().unwrap_or("unstated"),
            doc.pid,
            doc.started_at
        ),
        None if standing == ServerStanding::Absent => format!(
            "absent: no lease at {}, so nothing serves this checkout",
            lease_path.display()
        ),
        None => format!(
            "{}: the lease at {} holds no document",
            standing.as_str(),
            lease_path.display()
        ),
    };
    // Whose report this is. Everything above describes the checkout's server, read from the
    // lease — and a process whose lease was taken over describes, accurately, a server that
    // is not itself. The one caller that has reached the wrong process is the one caller
    // who cannot afford to be told that the right server is fine, so this outranks the
    // standing rather than qualifying it.
    let orphaned = crate::lease::was_lost();
    let mut findings: Vec<String> = reason.into_iter().collect();
    if orphaned {
        findings.push(
            "this process no longer holds the lease: it serves the sessions it already had \
             and takes on no new ones; what is described above is another process"
                .into(),
        );
    }
    record(
        &mut checks,
        &ctx.progress,
        HealthCheck {
            id: "server".into(),
            title: "The shared server".into(),
            status: if orphaned {
                HealthStatus::Warn
            } else {
                match standing {
                    // A checkout nobody serves is not an unhealthy one: nothing is running
                    // and nothing claims to be. A lease still binding resolves itself
                    // within the bind grace, and is reported as stale by this same engine
                    // when it does not. Neither is a fault of what this process serves.
                    ServerStanding::Absent | ServerStanding::Starting | ServerStanding::Ready => {
                        HealthStatus::Ok
                    }
                    // A server answering from code this tree no longer has says yesterday's
                    // truth in today's words, and a lease naming one that answers for
                    // nobody sends the next reader at a dead address. Both are somebody's
                    // to clear; neither stops this process from serving, so neither is a
                    // failure.
                    ServerStanding::Outdated | ServerStanding::Stale => HealthStatus::Warn,
                    // It answers, over a share that is not its own: its pages have no
                    // assets and what it reads from `share/` is another release's
                    // (ADR 0163). It serves, so it is not a failure.
                    ServerStanding::Skewed => HealthStatus::Warn,
                }
            },
            detail: if orphaned {
                format!("{detail}; and this process is not it")
            } else {
                detail
            },
            decided_by:
                "the decision `server.status` makes, from this checkout's lease and one probe of the server it names"
                    .into(),
            evidence: vec!["majordomus serve status".into()],
            findings,
        },
    );

    // --- the peers attached to this process
    //
    // Decided from the board and tallied the way preflight tallies it, so the two cards on
    // one page cannot count the same board two ways. Only a process that holds this
    // checkout's lease holds its board: anything else — the command line, a process whose
    // lease was never published — has an empty board of its own, and an empty board it does
    // not own is no evidence that nobody is attached.
    record(
        &mut checks,
        &ctx.progress,
        peers_check(
            crate::lease::held()
                .is_some()
                .then(|| ctx.peers.list())
                .as_deref(),
        ),
    );

    // --- the mesh, as the server running it decided: only a process a shared server runs in
    // decided anything, and in any other the answer is unknown, never a guess
    let mesh = if ctx.mesh.decided() {
        let verdict = crate::mesh::doctor::runtime_verdict(
            super::mesh::declaration(ctx),
            Some(&ctx.mesh.status()),
        );
        HealthCheck {
            id: "mesh".into(),
            title: "The mesh runtime".into(),
            status: if verdict.ok {
                HealthStatus::Ok
            } else {
                HealthStatus::Fail
            },
            detail: verdict.detail,
            decided_by: "mesh doctor's runtime check, over this server's mesh.status".into(),
            evidence: Vec::new(),
            findings: verdict.remediation.into_iter().collect(),
        }
    } else {
        HealthCheck {
            id: "mesh".into(),
            title: "The mesh runtime".into(),
            status: HealthStatus::Unknown,
            detail: "this process runs no shared server, so no mesh was activated or declined here; ask the server (GET /api/v1/health) or run `majordomus mesh doctor`".into(),
            decided_by: "whether a shared server decided the mesh in this process".into(),
            evidence: Vec::new(),
            findings: Vec::new(),
        }
    };
    record(&mut checks, &ctx.progress, mesh);

    let status = checks
        .iter()
        .fold(HealthStatus::Ok, |acc, c| acc.worse(c.status));
    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for c in &checks {
        *tallies.entry(c.status.as_str().into()).or_insert(0) += 1;
    }
    let observed = [
        ("layer", &index.repository.observed.index),
        ("git", &index.repository.observed.git),
    ]
    .into_iter()
    .filter(|(id, o)| !o.observed_at.is_empty() && checks.iter().any(|c| c.id == *id))
    .map(|(id, o)| (id.to_string(), o.clone()))
    .collect();
    Ok(Health {
        status,
        repository: repository_status(&checks),
        tallies,
        checks,
        observed,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "health",
        title: "Health",
        description: "Whether what this process serves is healthy, decided by the engines that already decide it: the index's diagnostics, the registry builder, the benchmark projection's coverage, the comparison `generate --check` makes and the lease reading `server.status` answers from. No check here has an opinion of its own.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "health.report",
                title: "Health of this process",
                description: "Every dimension of what this process serves — the layer as it was read, the registry, the scope, version control, benchmark coverage, the committed registry manifest, the shared server of this checkout and the attached peers — each decided by the engine that owns it, with the command that reproduces the verdict.",
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
                tags: ["health", "introspection"],
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: Some(5) },
                handler: health,
            },
            capability! {
                id: "health.live",
                title: "Liveness",
                description: "Is this process alive: the cheapest true statement this executable can make about itself, with the version that answered. No filesystem traversal, no index build, no network — this is what a hosting platform polls, and it must cost nothing to say.",
                input: Empty,
                output: Liveness,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: None, http: get("/api/v1/live"), cli: None },
                tags: ["health", "deployment"],
                handler: liveness,
            },
            capability! {
                id: "health.ready",
                title: "Readiness",
                description: "Can this process serve traffic: the registry and the index it built at start-up, already resident, and how the layer read. Only local initialisation — never an external provider, a database or another service, because a readiness check that probes a dependency fails a deployment for something that is not this process.",
                input: Empty,
                output: Readiness,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: None, http: get("/api/v1/ready"), cli: None },
                tags: ["health", "deployment"],
                handler: readiness,
            },
        ],
    }
}

/// What the continuity store says about the repository's health: the worst severity among
/// the status's diagnostics, how many records were admitted, refused and are resumable, and
/// every diagnostic that is more than information. A store that could not be read is
/// unknown, never ok.
fn continuity_health(
    status: Result<crate::continuity::Status, String>,
) -> (HealthStatus, String, Vec<String>) {
    match status {
        Ok(st) => {
            let worst = st
                .diagnostics
                .iter()
                .map(|d| match d.severity {
                    Severity::Error => HealthStatus::Fail,
                    Severity::Warning => HealthStatus::Warn,
                    _ => HealthStatus::Ok,
                })
                .fold(HealthStatus::Ok, HealthStatus::worse);
            (
                worst,
                format!(
                    "{} record(s) admitted, {} refused, {} resumable from another device",
                    st.store.records,
                    st.store.refused,
                    st.resumable.len()
                ),
                st.diagnostics
                    .iter()
                    .filter(|d| d.severity != Severity::Info)
                    .map(|d| {
                        format!(
                            "{} {}{}",
                            d.code,
                            d.path
                                .as_deref()
                                .map(|p| format!("{p}: "))
                                .unwrap_or_default(),
                            d.message
                        )
                    })
                    .collect(),
            )
        }
        Err(e) => (
            HealthStatus::Unknown,
            format!("the continuity store could not be read: {e}"),
            Vec::new(),
        ),
    }
}

/// The `git` check: the checkout git describes, or — for a deployed build with no checkout
/// to ask — the commit the executable was built from.
///
/// A hosted deployment receives a tree assembled from the deployment object, never a clone,
/// so `git` there has nothing to answer and the check read `unknown` for a process whose
/// commit is known exactly: the build identity names it, and `/api/v1/live` already serves
/// it. Only a process that reads its layer from the filesystem (the deployed command lines
/// pass `--discovery filesystem`) and was built from a named commit falls back to that
/// identity; a version-control checkout git cannot read stays `unknown`, because there the
/// build commit says nothing about the tree being served. A dirty build is a warning, as a
/// dirty working tree is.
///
/// ```
/// use majordomus_cli::capability::builtin::health::git_verdict;
/// use majordomus_cli::capability::builtin::health::HealthStatus;
/// use majordomus_cli::git::GitState;
///
/// let none = GitState::Unavailable { reason: "git: not a git repository".into() };
/// let (s, d, by) = git_verdict(&none, "filesystem", "0123456789abcdef0123", Some(false));
/// assert_eq!(s, HealthStatus::Ok);
/// assert!(d.contains("0123456789ab") && by.contains("build identity"), "{d} / {by}");
///
/// // a checkout git cannot read is still not known, whatever the executable was built from
/// let (s, _, _) = git_verdict(&none, "vcs", "0123456789abcdef0123", Some(false));
/// assert_eq!(s, HealthStatus::Unknown);
/// ```
pub fn git_verdict(
    git: &GitState,
    discovery: &str,
    commit: &str,
    dirty: Option<bool>,
) -> (HealthStatus, String, &'static str) {
    match git {
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
                "git, as the index asked it when it was built",
            )
        }
        GitState::Unavailable { .. }
            if discovery == "filesystem" && !commit.is_empty() && commit != "unknown" =>
        {
            let state = match dirty {
                Some(false) => "clean",
                Some(true) => "dirty",
                None => "of unknown cleanliness",
            };
            (
                if dirty == Some(false) {
                    HealthStatus::Ok
                } else {
                    HealthStatus::Warn
                },
                format!(
                    "no checkout here (a deployed tree): built from commit {}, {state}",
                    &commit[..12.min(commit.len())]
                ),
                "the build identity this executable carries, since there is no checkout to ask",
            )
        }
        GitState::Unavailable { reason } => (
            HealthStatus::Unknown,
            reason.clone(),
            "git, as the index asked it when it was built",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::{Shape, SyntheticRepository};

    /// A deployed tree has no checkout: its commit comes from the build identity, a dirty or
    /// unknown build is a warning, and only a filesystem-discovered layer falls back at all.
    #[test]
    fn a_deployed_build_names_its_commit_and_a_checkout_does_not_borrow_it() {
        let none = GitState::Unavailable {
            reason: "git: not a git repository".into(),
        };
        let sha = "2034d4a3f0b2f00ee0260f291330788448556b3b";
        let (s, d, by) = git_verdict(&none, "filesystem", sha, Some(false));
        assert_eq!(s, HealthStatus::Ok);
        assert!(d.contains("2034d4a3f0b2") && d.contains("clean"), "{d}");
        assert!(by.contains("build identity"), "{by}");
        assert_eq!(
            git_verdict(&none, "filesystem", sha, Some(true)).0,
            HealthStatus::Warn
        );
        assert_eq!(
            git_verdict(&none, "filesystem", sha, None).0,
            HealthStatus::Warn
        );
        // no named commit, or a version-control layer: still not known
        let (s, d, _) = git_verdict(&none, "filesystem", "unknown", Some(false));
        assert_eq!(
            (s, d.as_str()),
            (HealthStatus::Unknown, "git: not a git repository")
        );
        assert_eq!(
            git_verdict(&none, "vcs", sha, Some(false)).0,
            HealthStatus::Unknown
        );
    }

    /// No board is no evidence: a process that holds no lease cannot say who is attached,
    /// and says so rather than reporting an empty board as a healthy one.
    #[test]
    fn attached_clients_are_unknown_without_a_board() {
        let c = peers_check(None);
        assert_eq!(c.status, HealthStatus::Unknown, "{c:?}");
    }

    /// A board with every live peer announced is ok, and the count is of live peers only.
    #[test]
    fn attached_clients_are_ok_when_every_live_peer_announced() {
        let board = crate::peers::PeerBoard::new();
        let a = board.attach(crate::peers::Transport::Http);
        board.announce(&a, "writing docs", vec!["docs".into()]);
        let c = peers_check(Some(&board.list()));
        assert_eq!(c.status, HealthStatus::Ok, "{c:?}");
        assert!(
            c.detail.starts_with("1 attached (1 announced) · 0 gone"),
            "{c:?}"
        );
    }

    /// An attached peer that announced nothing, and a gone peer whose claims the board still
    /// shows, are each worth a look; and a gone peer is not counted as attached.
    #[test]
    fn attached_clients_warn_on_silent_or_gone_peers() {
        let board = crate::peers::PeerBoard::new();
        board.attach(crate::peers::Transport::Stdio);
        let c = peers_check(Some(&board.list()));
        assert_eq!(c.status, HealthStatus::Warn, "a silent peer: {c:?}");

        let board = crate::peers::PeerBoard::new();
        let gone = board.attach(crate::peers::Transport::Http);
        board.announce(&gone, "left", vec!["a".into()]);
        board.detach(&gone);
        let c = peers_check(Some(&board.list()));
        assert_eq!(c.status, HealthStatus::Warn, "a gone peer: {c:?}");
        assert!(
            c.detail.starts_with("0 attached (0 announced) · 1 gone"),
            "{c:?}"
        );
    }

    /// Liveness says one thing and reads nothing: the counters prove it moved no
    /// canonical state, and the answer is the same whatever the layer holds.
    #[test]
    fn liveness_is_the_same_answer_whatever_the_layer_holds() {
        let small = SyntheticRepository::new(Shape {
            rules: 1,
            ..Shape::default()
        })
        .expect("a synthetic repository");
        let large = SyntheticRepository::new(Shape {
            rules: 40,
            ..Shape::default()
        })
        .expect("a synthetic repository");
        let a = liveness(&small.context().expect("a context"), Empty {}).expect("alive");
        let b = liveness(&large.context().expect("a context"), Empty {}).expect("alive");
        assert_eq!(a, b);
        assert!(a.alive);
        assert_eq!(a.version, crate::VERSION);
        assert_eq!(a.commit, crate::COMMIT);
        assert_eq!(a.dirty, crate::DIRTY);
    }

    /// Readiness reads what this process already holds — the registry it built and the
    /// index it built — and reports the layer rather than refusing over it.
    #[test]
    fn readiness_reports_what_this_process_already_holds() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let r = readiness(&ctx, Empty {}).expect("ready");
        assert!(r.ready);
        assert_eq!(r.capabilities, ctx.registry.summary().total);
        assert_eq!(r.objects, ctx.index.objects.len());
        assert_eq!(r.layer, HealthStatus::Ok);
        assert_eq!(r.commit, crate::COMMIT);
        assert_eq!(r.dirty, crate::DIRTY);
    }

    /// Both are registered capabilities, so every projection carries them without a
    /// second registration anywhere: a route the smoke suite and the platform can read
    /// out of the registry rather than out of a list somebody maintains.
    #[test]
    fn both_are_registered_routes() {
        let routes: Vec<String> = module()
            .capabilities
            .iter()
            .filter_map(|c| c.capability.exposure.http.as_ref().map(|h| h.path.clone()))
            .collect();
        assert!(routes.contains(&"/api/v1/live".to_string()), "{routes:?}");
        assert!(routes.contains(&"/api/v1/ready".to_string()), "{routes:?}");
    }
    /// The continuity check reads its verdict from the status: nothing published is ok, a
    /// file the store refused fails or warns by its severity and is named with its path, a
    /// note that is only information is not a finding, and a store that could not be read
    /// is unknown.
    #[test]
    fn the_continuity_check_is_the_worst_of_what_the_store_reports() {
        use crate::continuity::tests_support::{body, handover, machine, World};
        use crate::continuity::{self as domain, PublishRequest};
        let w = World::new();
        let a_root = w.root("a");
        let a = machine(&a_root, w.identity("a", "macbook-pro"), None);

        let (status, detail, findings) = continuity_health(domain::status(&a));
        assert_eq!(status, HealthStatus::Ok);
        assert!(
            detail.starts_with("0 record(s) admitted, 0 refused, 0 resumable"),
            "{detail}"
        );
        assert!(findings.is_empty(), "{findings:?}");

        handover(&a_root, "20261003T120000Z", &body("o", "s", "n"), None);
        domain::publish(&a, &PublishRequest::default()).unwrap();
        crate::continuity::store::add(&a_root, &[("stray".into(), b"{}".to_vec())], "x\n").unwrap();
        let st = domain::status(&a).unwrap();
        let severities: Vec<Severity> = st.diagnostics.iter().map(|d| d.severity).collect();
        let (status, detail, findings) = continuity_health(Ok(st.clone()));
        assert_ne!(status, HealthStatus::Ok, "{severities:?}");
        assert!(
            detail.starts_with("1 record(s) admitted, 1 refused"),
            "{detail}"
        );
        assert!(
            findings.iter().any(|f| f.contains("records/stray.json: ")),
            "{findings:?}"
        );

        // each severity is its own verdict, and information is not a finding
        let with = |severity: Severity, path: Option<&str>| {
            let mut st = st.clone();
            st.diagnostics = vec![crate::model::Diagnostic {
                severity,
                code: "continuity.sample".into(),
                path: path.map(String::from),
                message: "said".into(),
            }];
            continuity_health(Ok(st))
        };
        let (status, _, findings) = with(Severity::Error, Some("records/x.json"));
        assert_eq!(status, HealthStatus::Fail);
        assert_eq!(findings, ["continuity.sample records/x.json: said"]);
        let (status, _, findings) = with(Severity::Warning, None);
        assert_eq!(status, HealthStatus::Warn);
        assert_eq!(findings, ["continuity.sample said"]);
        let (status, _, findings) = with(Severity::Info, None);
        assert_eq!(status, HealthStatus::Ok);
        assert!(findings.is_empty());

        let (status, detail, findings) = continuity_health(Err("no device".into()));
        assert_eq!(status, HealthStatus::Unknown);
        assert_eq!(detail, "the continuity store could not be read: no device");
        assert!(findings.is_empty());
    }

    /// A process that is no server cannot decide who is attached or what the mesh did, and the
    /// whole report says `unknown` for it; the repository's own verdict does not, because none
    /// of its dimensions is the process's.
    #[test]
    fn the_repositorys_verdict_is_decided_where_no_server_runs() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let report = health(&ctx, Empty {}).expect("a report");
        let of = |id: &str| {
            report
                .checks
                .iter()
                .find(|c| c.id == id)
                .unwrap_or_else(|| panic!("no {id} check"))
                .status
        };
        assert_eq!(of("peers"), HealthStatus::Unknown);
        assert_eq!(of("mesh"), HealthStatus::Unknown);
        assert_eq!(report.status, HealthStatus::Unknown);
        for id in PROCESS_DIMENSIONS {
            of(id);
        }
        let expected = report
            .checks
            .iter()
            .filter(|c| !PROCESS_DIMENSIONS.contains(&c.id.as_str()))
            .fold(HealthStatus::Ok, |acc, c| acc.worse(c.status));
        assert_eq!(report.repository, expected);
        assert_eq!(report.repository, repository_status(&report.checks));
    }

    /// The committed registry manifest is judged by what is there: a repository that commits
    /// none has nothing to drift from and is `ok`; one whose manifest exists and cannot be
    /// read is `unknown`, because this process cannot say whether it drifted.
    #[test]
    fn an_uncommitted_registry_manifest_is_ok_and_an_unreadable_one_is_unknown() {
        let check = |repo: &SyntheticRepository| {
            let ctx = repo.context().expect("a context");
            health(&ctx, Empty {})
                .expect("a report")
                .checks
                .into_iter()
                .find(|c| c.id == "generated-registry")
                .expect("the check is reported")
        };
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let manifest = repo.root().join(generate::OUT_DIR).join("registry.json");
        assert!(!manifest.exists(), "the fixture commits no manifest");
        let absent = check(&repo);
        assert_eq!(absent.status, HealthStatus::Ok, "{absent:?}");
        assert!(absent.detail.contains("not committed here"), "{absent:?}");
        assert!(absent.findings.is_empty(), "{absent:?}");

        // a directory where the file belongs: it exists, and it cannot be read as a manifest
        std::fs::create_dir_all(&manifest).expect("a directory at the manifest's path");
        let unreadable = check(&repo);
        assert_eq!(unreadable.status, HealthStatus::Unknown, "{unreadable:?}");
        assert!(
            unreadable.detail.contains("could not be read"),
            "{unreadable:?}"
        );
        assert_eq!(unreadable.findings.len(), 1, "{unreadable:?}");
    }

    /// A layer read with an error diagnostic is served as read: readiness names it as a
    /// reason, reports the layer as a warning, and is still ready.
    #[test]
    fn readiness_names_a_degraded_layer_and_is_still_ready() {
        let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let mut index = repo.index().expect("an index");
        index.diagnostics.push(crate::model::Diagnostic {
            severity: Severity::Error,
            code: "unknown_key".into(),
            path: Some("docs/x.md".into()),
            message: "key(s) not in schema".into(),
        });
        index.state = State::Degraded;
        let mut degraded = (*ctx).clone();
        degraded.index = std::sync::Arc::new(index);
        let r = readiness(&degraded, Empty {}).expect("ready");
        assert!(r.ready, "a degraded layer is served: {r:?}");
        assert_eq!(r.layer, HealthStatus::Warn);
        let expected = format!(
            "layer_degraded: the layer was read with {} diagnostic(s); it is served as read",
            degraded.index.diagnostics.len()
        );
        assert!(r.reasons.contains(&expected), "{:?}", r.reasons);
    }

    /// The variable that makes a run of this test binary the child of the test below.
    const STALE_CHILD: &str = "MAJORDOMUS_HEALTH_TEST_STALE_CHILD";

    /// A process whose executable was removed under it is serving code that is no longer on
    /// disk: readiness says it is not ready, and why. Shown in a copy of this test binary,
    /// run as a process of its own, which removes its own executable once it has pinned it.
    #[test]
    fn readiness_refuses_a_process_whose_executable_was_removed_under_it() {
        if std::env::var_os(STALE_CHILD).is_some() {
            let repo = SyntheticRepository::new(Shape::default()).expect("a synthetic repository");
            let ctx = repo.context().expect("a context");
            assert_eq!(
                crate::lease::serving_replaced_code(),
                None,
                "pinned while the file is there"
            );
            std::fs::remove_file(std::env::current_exe().expect("this executable"))
                .expect("the copy is removed");
            let r = readiness(&ctx, Empty {}).expect("an answer");
            assert!(!r.ready, "{r:?}");
            assert!(r.stale.is_some(), "{r:?}");
            assert!(
                r.reasons
                    .iter()
                    .any(|x| x.starts_with("stale: ") && x.contains("has been removed")),
                "{:?}",
                r.reasons
            );
            return;
        }
        let dir = tempfile::tempdir().expect("a temp dir");
        let copy = dir.path().join("health-tests");
        std::fs::copy(std::env::current_exe().expect("this test binary"), &copy)
            .expect("a copy of this test binary");
        // another test thread forking while the copy was open for writing leaves the copy's
        // write descriptor in that child until it execs, and Linux refuses to execute a file
        // open for writing (ETXTBSY): the race is the harness's, so it is waited out
        let mut attempts = 0;
        let out = loop {
            let run = std::process::Command::new(&copy)
                .args([
                    "--exact",
                    "capability::builtin::health::tests::readiness_refuses_a_process_whose_executable_was_removed_under_it",
                    "--nocapture",
                    "--test-threads",
                    "1",
                ])
                .env(STALE_CHILD, "1")
                .output();
            match run {
                Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 50 => {
                    attempts += 1;
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                other => break other.expect("the copy runs"),
            }
        };
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success() && stdout.contains("1 passed"),
            "{:?}\n{stdout}\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!copy.exists(), "the child removed its own executable");
    }
}
