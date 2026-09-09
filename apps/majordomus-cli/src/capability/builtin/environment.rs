//! The `environment` module: what this checkout is, as a capability.
//!
//! One capability, one handler, and every surface a projection of it — the MCP tool and
//! resource, the HTTP route, the OpenAPI operation, the Cockpit page and the command line
//! all answer with the value [`crate::environment::resolve()`] built. The direnv banner is
//! the same value rendered for a terminal.
//!
//! The handler runs inside a process that already holds the index, so it resolves in full:
//! nothing here pays the index build, because whoever called it had already paid it. The
//! fast, cached resolution belongs to the command line, which may be starting cold on a
//! shell prompt.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::environment::{
    resolve, EnvironmentQuery, FieldSource, Inputs, RepositoryEnvironment, Resolution,
};
use crate::repository::Repository;
use crate::{capability, module};

use super::{get, mcp};

/// The URI under which the environment is read as an MCP resource.
pub const ENVIRONMENT_URI: &str = "majordomus://environment";

/// What to include in the snapshot.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentInput {
    /// Contact the local address a running server published, to say whether it answers.
    /// Off by default: a served request should not open a socket to another server on
    /// behalf of its caller, and the caller usually is that server.
    #[serde(default)]
    pub probe_services: bool,
}

impl BenchmarkCases for EnvironmentInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "default",
            EnvironmentInput {
                probe_services: false,
            },
        )]
    }
}

/// Which field's provenance to explain.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExplainInput {
    /// The field in dotted form (`services.url`, `capabilities.objects`); every field when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

impl BenchmarkCases for ExplainInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", ExplainInput { field: None }),
            NamedCase::new(
                "one-field",
                ExplainInput {
                    field: Some("project.version".into()),
                },
            ),
        ]
    }
}

/// Where the values of one snapshot came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentProvenance {
    /// How the snapshot behind these entries was resolved.
    pub resolution: Resolution,
    /// One entry per field, in the order the resolver produced them.
    pub fields: Vec<FieldSource>,
}

fn snapshot(
    ctx: &Context,
    input: EnvironmentInput,
) -> Result<RepositoryEnvironment, CapabilityError> {
    let root = std::path::Path::new(&ctx.index.repository.root);
    let repository = Repository::open(root).map_err(|e| {
        CapabilityError::Internal(format!("the repository could not be re-read: {e}"))
    })?;
    // The share directory is not carried on the context; without it the provider
    // projections are still listed from the policy, and their state is `unknown` rather
    // than guessed. `env status` on the command line has it and reports the state.
    Ok(resolve(
        &Inputs {
            repository: &repository,
            share: None,
            index: Some(&ctx.index),
            registry: Some(&ctx.registry),
        },
        &EnvironmentQuery {
            resolution: Resolution::Full,
            probe_services: input.probe_services,
            use_cache: false,
            // A served request writes nothing: the cache belongs to the checkout's own
            // command line, and a request that wrote it would be a request with a side
            // effect on the repository.
            write_cache: false,
        },
    ))
}

fn explain(ctx: &Context, input: ExplainInput) -> Result<EnvironmentProvenance, CapabilityError> {
    let environment = snapshot(
        ctx,
        EnvironmentInput {
            probe_services: false,
        },
    )?;
    let fields = match &input.field {
        None => environment.provenance.clone(),
        Some(field) => {
            let found: Vec<FieldSource> = environment
                .provenance
                .iter()
                .filter(|p| p.field == *field || p.field.starts_with(&format!("{field}.")))
                .cloned()
                .collect();
            if found.is_empty() {
                return Err(CapabilityError::NotFound(format!(
                    "no field named '{field}'; `environment.explain` with no field lists every one"
                )));
            }
            found
        }
    };
    Ok(EnvironmentProvenance {
        resolution: environment.resolution,
        fields,
    })
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "environment",
        title: "Repository environment",
        description: "What this checkout is right now: the project and its version, the repository and its layer, version control, the toolchains it declares, what the layer holds, the workflows a person can run, the provider projections and the local services — one typed snapshot, with a provenance entry for every value in it. The direnv banner, the Cockpit's overview and this route are renderings of the same value.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "environment.status",
                title: "The repository environment",
                description: "One snapshot of this checkout: project identity, repository identity, version control, declared toolchains, what the layer holds counted per kind, the workflows the runner describes, the provider projections against the policy that renders them, and the local services with the address a running server published. Every value carries where it came from.",
                input: EnvironmentInput,
                output: RepositoryEnvironment,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_environment".into()),
                        resource: Some(McpResource { uri: ENVIRONMENT_URI.into(), name: "environment".into() }),
                    }),
                    http: get("/api/v1/environment"),
                    cli: None,
                },
                tags: ["environment", "repository", "introspection"],
                // The snapshot runs `git status` and reads the lease, so two calls a
                // second apart may legitimately differ; a short time to live makes a
                // dashboard polling it cheap without letting it show yesterday's branch.
                cache: CachePolicy::Process { max_entries: 4, ttl_seconds: Some(3) },
                handler: snapshot,
            },
            capability! {
                id: "environment.explain",
                title: "Where an environment value came from",
                description: "The provenance of the snapshot: for each field, what decided it — a compile-time constant, a file, a command, or the cache — which resolver read it, and how far it can be trusted. Narrow it to one field, or to a prefix, by name.",
                input: ExplainInput,
                output: EnvironmentProvenance,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_environment_explain"),
                    http: get("/api/v1/environment/explain"),
                    cli: None,
                },
                tags: ["environment", "provenance", "introspection"],
                handler: explain,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist, and every projection — the MCP
    /// tool, the resource, the HTTP route, the OpenAPI operation, the benchmark target —
    /// is derived from it. A refactor that dropped an exposure would still compile.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "environment");
        let expected: &[(&str, &str, &str)] = &[
            (
                "environment.status",
                "majordomus_environment",
                "/api/v1/environment",
            ),
            (
                "environment.explain",
                "majordomus_environment_explain",
                "/api/v1/environment/explain",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
        }
        let resource = m.capabilities[0]
            .capability
            .exposure
            .mcp
            .as_ref()
            .and_then(|m| m.resource.as_ref())
            .expect("the snapshot is readable as a resource");
        assert_eq!(resource.uri, ENVIRONMENT_URI);
    }

    /// A served request must not write to the checkout it is serving. The whole cache is
    /// the command line's, and a handler that wrote it would give an HTTP GET a side
    /// effect on the repository.
    #[test]
    fn a_served_snapshot_writes_nothing() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let cache = crate::environment::Cache::path(repo.root(), ".ai/local");
        let environment = snapshot(
            &ctx,
            EnvironmentInput {
                probe_services: false,
            },
        )
        .expect("a snapshot");
        assert_eq!(environment.resolution, Resolution::Full);
        assert!(
            !cache.exists(),
            "a served request wrote the checkout's cache"
        );
    }

    #[test]
    fn a_served_snapshot_counts_the_layer_it_is_serving() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let environment = snapshot(&ctx, EnvironmentInput::default()).expect("a snapshot");
        assert_eq!(environment.layer.objects, Some(ctx.index.objects.len()));
        assert_eq!(
            environment.layer.capabilities,
            Some(ctx.registry.summary().total)
        );
    }

    #[test]
    fn explaining_a_field_that_does_not_exist_says_so_rather_than_answering_emptily() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        match explain(
            &ctx,
            ExplainInput {
                field: Some("nothing.like.this".into()),
            },
        ) {
            Err(CapabilityError::NotFound(message)) => {
                assert!(message.contains("nothing.like.this"))
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn explaining_a_prefix_gives_every_field_under_it() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        let all = explain(&ctx, ExplainInput { field: None }).expect("every field");
        let some = explain(
            &ctx,
            ExplainInput {
                field: Some("project".into()),
            },
        )
        .expect("the project fields");
        assert!(!some.fields.is_empty());
        assert!(some.fields.len() < all.fields.len());
        assert!(some.fields.iter().all(|f| f.field.starts_with("project.")));
    }
}
