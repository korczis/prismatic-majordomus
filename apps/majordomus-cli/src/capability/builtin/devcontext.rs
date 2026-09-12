//! The `devcontext` module: the context compiler as a capability.
//!
//! Three questions, answered from the index and the composed graph this process already
//! holds: what context does this piece of work need, why is any one thing in or out of that
//! answer, and what are the compiler's own rules. The MCP tools, the resource, the HTTP
//! routes, the OpenAPI operations, the command line and the Cockpit's pages are projections
//! of these three declarations; none of them holds a selection rule of its own, and there is
//! no Cockpit-specific implementation to hold one in.
//!
//! The work is [`crate::devcontext`]; this file is only the declaration. That split is
//! deliberate: the compiler is a domain model with its own tests, and a capability module
//! that carried the model would make the registry the place selection rules live.
//!
//! # Why `compile` is cached for the life of the process
//!
//! The answer is a pure function of the index and the request, and the executor's cache key
//! is the canonical id, the input in canonical form and *the registry fingerprint* — which
//! is a sha-256 over the index fingerprint, itself over every object's path and content
//! (ADR 0004). So a cached answer is invalidated by an actual change to the repository
//! rather than by a clock, and a time-to-live would only throw away answers that are still
//! true. Every answer also carries the fingerprint it was compiled from, so a caller can
//! tell two trees apart without asking a second question.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    CachePolicy, CliExposure, Exposure, McpExposure, McpResource, Stability,
};
use crate::capability::module::ModuleDescriptor;
use crate::devcontext::{self, CompiledContext, CompilerPolicy, Explanation};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the compiler's own rules are read as an MCP resource.
pub const DEVCONTEXT_POLICY_URI: &str = "majordomus://devcontext/policy";

/// The input of `devcontext.compile`: what to compile a context about.
///
/// A newtype over [`devcontext::CompileInput`] so that the schema component the OpenAPI
/// document carries is named for the operation rather than for the domain type, the way
/// every other module's input is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(rename = "DevContextInput")]
pub struct CompileInput(pub devcontext::CompileInput);

impl BenchmarkCases for CompileInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        devcontext::CompileInput::benchmark_cases(ctx)
            .into_iter()
            .map(|c| NamedCase {
                name: c.name,
                input: CompileInput(c.input),
            })
            .collect()
    }
}

/// The input of `devcontext.explain`: one identifier, and the request to judge it under.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[schemars(rename = "DevContextExplainInput")]
pub struct ExplainInput(pub devcontext::ExplainInput);

impl BenchmarkCases for ExplainInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        devcontext::ExplainInput::benchmark_cases(ctx)
            .into_iter()
            .map(|c| NamedCase {
                name: c.name,
                input: ExplainInput(c.input),
            })
            .collect()
    }
}

fn compile(ctx: &Context, input: CompileInput) -> Result<CompiledContext, CapabilityError> {
    devcontext::compile(ctx, input.0)
}

fn explain(ctx: &Context, input: ExplainInput) -> Result<Explanation, CapabilityError> {
    devcontext::explain(ctx, input.0)
}

fn policy(ctx: &Context, _: Empty) -> Result<CompilerPolicy, CapabilityError> {
    devcontext::policy(ctx)
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "devcontext",
        title: "Development context",
        description: "The context a development session should be given, derived from the repository rather than composed by hand: given an issue, a milestone, an intent or a set of paths, the canonical objects that bear on the work, why each one is in the answer, what was left out and why, what collapsed into what, and the whole cost against a budget. The selection is structured — every entry keeps its identifier, its provenance, the selector that reached it and its confidence — because rendering a prompt is a projection of the selection and not the selection itself.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "devcontext.compile",
                title: "Compile the context for a piece of work",
                description: "The objects a session working on this issue, milestone, intent or set of paths should be given: each with its canonical identifier, the index's own provenance, every discovery path that reached it with the reason and the confidence, its version, its tier and its cost in estimated tokens — followed by everything reached and not given with the reason for each, everything that collapsed into one entry, every pair that does not agree, and the per-tier spend against the budget. Deterministic for a given tree and request; the index fingerprint it was compiled from is in the answer.",
                input: CompileInput,
                output: CompiledContext,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_devcontext"),
                    http: get("/api/v1/devcontext"),
                    cli: Some(CliExposure { path: vec!["devcontext".into(), "compile".into()] }),
                },
                tags: ["context", "session", "planning", "introspection"],
                cache: CachePolicy::Process { max_entries: 16, ttl_seconds: None },
                handler: compile,
            },
            capability! {
                id: "devcontext.explain",
                title: "Why one thing is or is not in a compiled context",
                description: "One canonical identifier judged under a request: whether it was selected, reached and excluded, folded into another identifier, held by the index and never reached, or unknown — with the entry, the exclusion or the collapse itself, and the budget the judgement was made under so that `excluded for budget` can be acted on without a second call.",
                input: ExplainInput,
                output: Explanation,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_devcontext_explain"),
                    http: get("/api/v1/devcontext/explain"),
                    cli: Some(CliExposure { path: vec!["devcontext".into(), "explain".into()] }),
                },
                tags: ["context", "provenance", "introspection"],
                cache: CachePolicy::Process { max_entries: 8, ttl_seconds: None },
                handler: explain,
            },
            capability! {
                id: "devcontext.policy",
                title: "The compiler's own rules",
                description: "What the compiler decides and how: the tiers in the order the budget spends in with the kinds that land in each, every edge kind the composed graph declares with the relevance multiplier the compiler follows it by in each direction — or the reason it refuses to follow it at all — the selectors with which of them infer rather than read, and the defaults for the budget, the depth and the relevance floor. Read this rather than inferring the policy from an answer.",
                input: Empty,
                output: CompilerPolicy,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_devcontext_policy".into()),
                        resource: Some(McpResource { uri: DEVCONTEXT_POLICY_URI.into(), name: "devcontext-policy".into() }),
                    }),
                    http: get("/api/v1/devcontext/policy"),
                    cli: Some(CliExposure { path: vec!["devcontext".into(), "policy".into()] }),
                },
                tags: ["context", "introspection"],
                handler: policy,
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
        assert_eq!(m.id.as_str(), "devcontext");
        let expected: &[(&str, &str, &str, &[&str])] = &[
            (
                "devcontext.compile",
                "majordomus_devcontext",
                "/api/v1/devcontext",
                &["devcontext", "compile"],
            ),
            (
                "devcontext.explain",
                "majordomus_devcontext_explain",
                "/api/v1/devcontext/explain",
                &["devcontext", "explain"],
            ),
            (
                "devcontext.policy",
                "majordomus_devcontext_policy",
                "/api/v1/devcontext/policy",
                &["devcontext", "policy"],
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _, _)| *id).collect();
        assert_eq!(ids, want);
        for (executable, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost its HTTP route"
            );
            assert_eq!(
                exposure
                    .cli
                    .as_ref()
                    .map(|c| c.path.iter().map(String::as_str).collect::<Vec<_>>())
                    .as_deref(),
                Some(*cli),
                "{id} lost its command line"
            );
        }
        // the compiler's rules are readable as a resource, because they need no input
        let resource = m.capabilities[2]
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref())
            .expect("the policy is readable as a resource");
        assert_eq!(resource.uri, DEVCONTEXT_POLICY_URI);
    }

    #[test]
    fn the_answer_is_cached_against_the_tree_and_not_against_a_clock() {
        let m = module();
        for e in &m.capabilities {
            match e.capability.cache {
                CachePolicy::Process { ttl_seconds, .. } => assert_eq!(
                    ttl_seconds, None,
                    "{} expires by time; the cache key already carries the index fingerprint",
                    e.capability.id
                ),
                CachePolicy::Disabled => {}
            }
        }
    }

    #[test]
    fn every_capability_has_a_benchmark_case_over_this_repository() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let index = repo.index().expect("an index");
        let ctx = CaseContext { index: &index };
        assert!(!CompileInput::benchmark_cases(&ctx).is_empty());
        assert!(!ExplainInput::benchmark_cases(&ctx).is_empty());
        assert!(!Empty::benchmark_cases(&ctx).is_empty());
    }
}
