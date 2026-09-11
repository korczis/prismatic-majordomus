//! The `session_domain` module: the typed session domain, projected.
//!
//! The module is `session_domain` and not `session` because the registry composes builtin
//! modules and declarative kinds into one namespace, and `session` is already a **kind** —
//! the closed episode records under `.ai/repo/sessions/`. The registry refuses the
//! collision rather than letting one shadow the other, which is how this was found: twelve
//! "module \'session\' is composed twice" diagnostics, one per record. The capability ids
//! carry the module\'s name; the tool names and routes are the short ones, because a tool
//! name is a projection and not the identity.
//!
//! Two capabilities, and deliberately only two.
//!
//! `session.machine` answers the lifecycle state machine as data. It exists because the
//! same machine is currently drawn three times — in `docs/CONTINUITY.md`, in
//! `lib/session.sh`'s comments and in the Cockpit — and three drawings of one machine
//! drift. The `lib/session.sh` drawing still had the task guard in it after ADR 0041
//! removed the guard from the code. The answer here is derived from
//! [`crate::session::EpisodeState`] and [`crate::session::Transition`] and from nothing
//! else, so a transition that is added to the type appears in every surface and a diagram
//! that disagrees with the code becomes impossible rather than merely discouraged.
//!
//! `session.identity` answers the six identities of this checkout, each with the canonical
//! value this executable computes and each store's own spelling beside it. It exists
//! because a forensic read on 2026-09-11 found the repository recorded as an absolute
//! `.git` path *and* as a remote URL, the worktree as an absolute path *and* as a digest,
//! and the session as an episode id *and* as a provider's UUID — three words over six
//! things — and no surface anywhere said so.
//!
//! **There is no third episode listing here.** `episodes.*` (the MCP connection's episode)
//! and `lifecycle.*` (session observability) are being added in the same week by two other
//! workers, and a third projection over the same store would be this subsystem's own
//! favourite defect committed by the module written to stop it. What this module adds is
//! the domain those projections should eventually read from; ADR 0044 records that
//! repointing as a step of the cutover, not of this change.
//!
//! ```
//! use majordomus_cli::capability::builtin::session_domain;
//!
//! let module = session_domain::module();
//! let ids: Vec<&str> = module.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["session_domain.machine", "session_domain.identity"]);
//! // every one of them reads: the domain's write path reaches no surface (ADR 0044)
//! assert!(module.capabilities.iter().all(|e| e.capability.kind.is_read_only()));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::session::{self, IdentityFacet, Machine};
use crate::{capability, module};

use super::{get, Empty};

/// The URI under which `session.machine` is read as an MCP resource.
pub const MACHINE_URI: &str = "majordomus://session/machine";

/// The URI under which `session.identity` is read as an MCP resource.
pub const IDENTITY_URI: &str = "majordomus://session/identity";

/// The lifecycle machine, with the decision it implements, so that a reader meeting it in
/// a projection can go and read why it is shaped this way.
///
/// ```
/// use majordomus_cli::capability::builtin::MachineReport;
/// use majordomus_cli::session::Machine;
///
/// // the report is the machine plus one citation; the machine itself is derived from the
/// // types and carries no second list
/// let report = MachineReport { machine: Machine::describe(), decision: "adr-0044".into() };
/// assert!(report.machine.transitions.iter().any(|t| !t.moves_state), "a checkpoint is an event");
/// assert!(report.decision.contains("0044"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MachineReport {
    /// The states, the transitions, and what the machine does not depend on.
    #[serde(flatten)]
    pub machine: Machine,
    /// The decision this machine is the implementation of, so a reader meeting it in a
    /// projection can go and read why it is shaped this way.
    pub decision: String,
}

/// The identities of this checkout: one facet per subject, with a standing note about what
/// a reader must not do with them.
///
/// ```
/// use majordomus_cli::capability::builtin::IdentityReport;
/// use majordomus_cli::session::identities;
///
/// let dir = tempfile::tempdir().unwrap();
/// let report = IdentityReport {
///     checkout: dir.path().display().to_string(),
///     facets: identities(dir.path()),
///     findings: Vec::new(),
/// };
/// let subjects: Vec<&str> = report.facets.iter().map(|f| f.subject.as_str()).collect();
/// assert_eq!(subjects, ["repository", "checkout", "episode", "provider_session"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IdentityReport {
    /// The checkout this answer is about. An absolute path, because this capability is
    /// served to the worker in front of this checkout and never published — the same
    /// exposure rule `continuity.state` follows and for the same reason (ADR 0014).
    pub checkout: String,
    /// One entry per subject, in the order a reader meets them: repository, checkout,
    /// episode, provider session.
    pub facets: Vec<IdentityFacet>,
    /// What a reader should know before comparing any two of these. Empty is not the good
    /// case here — the conflation is a property of the stores, not of this checkout — so
    /// the standing note is always present.
    pub findings: Vec<String>,
}

fn machine(_: &Context, _: Empty) -> Result<MachineReport, CapabilityError> {
    Ok(MachineReport {
        machine: Machine::describe(),
        decision: ".ai/repo/adrs/0041-the-session-lifecycle-is-the-episodes-not-the-tasks.md \
                   and .ai/repo/adrs/0044-the-session-domain-is-typed-and-its-identities-are-not-interchangeable.md"
            .to_string(),
    })
}

fn identity(ctx: &Context, _: Empty) -> Result<IdentityReport, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let facets = session::identities(&root);
    Ok(IdentityReport {
        checkout: root.to_string_lossy().to_string(),
        facets,
        findings: vec![
            "the local half spells the repository as a path and the shared half as a remote \
             URL; the two can never be equal, so comparing them across halves answers `different \
             repository` for two records of one"
                .to_string(),
            "a provider session is an external correlation id and not the session identity: \
             an episode opened by hand has none, and a worker that reconnects keeps its work \
             while the value changes under it"
                .to_string(),
        ],
    })
}

/// The module: two read-only capabilities over the typed session domain, declared once and
/// projected to MCP and HTTP from that declaration alone.
///
/// ```
/// use majordomus_cli::capability::builtin::session_domain::module;
/// assert_eq!(module().id.as_str(), "session_domain");
/// assert_eq!(module().capabilities.len(), 2);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "session_domain",
        title: "The session domain",
        description: "The typed session domain: the lifecycle state machine an episode moves through, declared once in the executable and projected here rather than redrawn per surface, and the identities this checkout records — the repository, the checkout, the episode and the provider's own session — each with the canonical value and each store's own spelling of it, because the stores spell six things as three words and nothing said so.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "session_domain.machine",
                title: "The episode lifecycle, as data",
                description: "Every state an episode can be in, whether it is terminal and what it may move to; every transition — open, resume, checkpoint, detach, close, recover — with the states it runs between and whether it moves the state at all. Derived from the types, so a diagram that disagrees with the code cannot exist. What the machine deliberately does not depend on is stated rather than left to be inferred: task state, which is ADR 0041.",
                input: Empty,
                output: MachineReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_session_machine".into()),
                        resource: Some(McpResource { uri: MACHINE_URI.into(), name: "session-machine".into() }),
                    }),
                    http: get("/api/v1/session/machine"),
                    cli: None,
                },
                tags: ["session", "continuity", "lifecycle"],
                // The machine is a function of the compiled types: it cannot change while
                // the process runs, so the only reason to recompute it is to prove that it
                // is still derived. A long TTL and one entry.
                cache: CachePolicy::Process { max_entries: 1, ttl_seconds: Some(600) },
                handler: machine,
            },
            capability! {
                id: "session_domain.identity",
                title: "What identifies this checkout, and how each store spells it",
                description: "The repository, the checkout, the open episode and the provider session, each with the canonical value this executable computes and each store's own spelling beside it. The stores record the repository sometimes as an absolute .git path and sometimes as a remote URL, the worktree as a path and as a digest, and the session as both an episode id and a provider UUID; this is the surface that says so, so that a reader never compares two spellings of different things and concludes they disagree.",
                input: Empty,
                output: IdentityReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_session_identity".into()),
                        resource: Some(McpResource { uri: IDENTITY_URI.into(), name: "session-identity".into() }),
                    }),
                    http: get("/api/v1/session/identity"),
                    cli: None,
                },
                tags: ["session", "identity", "continuity"],
                // Short-lived, like `continuity.state`: the open episode it reads is
                // written by another process, and a reader that cached it for a minute
                // would answer with an episode that had already closed.
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(2) },
                handler: identity,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place the ids, the tool names, the resource URIs and the
    /// routes exist. A refactor that dropped one of them would still compile and every
    /// suite that tests the report itself would still pass; this is the assertion that
    /// would not — which is what `project.rust-command-tested-in-file` asks.
    #[test]
    fn the_declaration_yields_the_identity_and_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "session_domain");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["session_domain.machine", "session_domain.identity"]);
        assert!(ids.iter().all(|id| id.starts_with("session_domain.")));

        for (n, tool, uri, route) in [
            (
                0usize,
                "majordomus_session_machine",
                MACHINE_URI,
                "/api/v1/session/machine",
            ),
            (
                1,
                "majordomus_session_identity",
                IDENTITY_URI,
                "/api/v1/session/identity",
            ),
        ] {
            let c = &m.capabilities[n].capability;
            let mcp = c.exposure.mcp.as_ref().expect("an MCP projection");
            assert_eq!(mcp.tool.as_deref(), Some(tool));
            assert_eq!(mcp.resource.as_ref().map(|r| r.uri.as_str()), Some(uri));
            assert_eq!(
                c.exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(route)
            );
            // every capability of this module reads; the domain's write path is library
            // API and reaches no surface (ADR 0044)
            assert!(c.kind.is_read_only() && c.kind.is_executable());
        }
    }

    #[test]
    fn the_machine_projection_is_derived_and_carries_no_second_list() {
        let report = Machine::describe();
        assert_eq!(report.states.len(), crate::session::EpisodeState::ALL.len());
        assert_eq!(
            report.transitions.len(),
            crate::session::Transition::ALL.len()
        );
        // the one thing the machine is stated to be independent of
        assert!(report
            .independent_of
            .iter()
            .any(|s| s.contains("task") && s.contains("0041")));
    }
}
