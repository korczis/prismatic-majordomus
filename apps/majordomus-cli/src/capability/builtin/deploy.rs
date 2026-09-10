//! The `deploy` module: what this repository's deployments are, and whether they would
//! work — read-only, from the canonical objects the index already holds.
//!
//! Every operation here is a read. A deployment is mutated by the trusted command line and
//! by CI, never by an HTTP request and never by an MCP client: an operation that could
//! start or stop a machine has no business behind a surface a browser can reach.
//!
//! No capability here contacts the provider. Live provider state costs a network call, and
//! a network call on a read that a client may poll is not an ordinary read; it belongs to
//! an explicit operation with its own cost, not behind `deploy.list`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::deploy::{Deployment, Refusal, Workspace, KIND};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the deployments are read as one MCP resource.
pub const DEPLOYMENTS_URI: &str = "majordomus://deployments";

/// One deployment as this process reads it, with where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentView {
    /// The repository-relative file the object lives in.
    pub file: String,
    /// The object, typed.
    #[serde(flatten)]
    pub deployment: Deployment,
}

/// A view of a deployment orders as the deployment it carries: the file it was read from
/// is provenance, not identity.
impl crate::order::Ordered for DeploymentView {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        self.deployment.order_key()
    }
}

/// Every deployment the layer declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentList {
    /// How many.
    pub count: usize,
    /// Each, in identity order.
    pub deployments: Vec<DeploymentView>,
    /// Objects of the deployment kind this executable could not read, with why. A
    /// malformed object is reported here rather than hidden by being skipped.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<Refusal>,
}

/// The input of `deploy.get`: which deployment.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetDeploymentInput {
    /// The deployment's `id`.
    pub id: String,
}

/// Whether the deployments this repository declares would work, decided locally: against
/// the capability registry this process built and the workspace it sits in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentCheck {
    /// True when no deployment earned a refusal.
    pub ok: bool,
    /// How many deployments were decided.
    pub decided: usize,
    /// Every refusal, in the order the objects and their keys appear.
    pub refusals: Vec<Refusal>,
}

fn views(ctx: &Context) -> (Vec<DeploymentView>, Vec<Refusal>) {
    let mut out = Vec::new();
    let mut bad = Vec::new();
    for object in ctx.index.objects.iter().filter(|o| o.kind == KIND) {
        match Deployment::parse(object) {
            Ok(deployment) => out.push(DeploymentView {
                file: object.provenance.path.clone(),
                deployment,
            }),
            Err(refusal) => bad.push(refusal),
        }
    }
    crate::order::canonical(&mut out);
    (out, bad)
}

fn deploy_list(ctx: &Context, _: Empty) -> Result<DeploymentList, CapabilityError> {
    let (deployments, unreadable) = views(ctx);
    Ok(DeploymentList {
        count: deployments.len(),
        deployments,
        unreadable,
    })
}

fn deploy_get(ctx: &Context, input: GetDeploymentInput) -> Result<DeploymentView, CapabilityError> {
    let (deployments, _) = views(ctx);
    deployments
        .into_iter()
        .find(|d| d.deployment.id == input.id)
        .ok_or_else(|| {
            CapabilityError::NotFound(format!(
                "no deployment '{}' (the deployments are read from the layer's deployments section)",
                input.id
            ))
        })
}

fn deploy_check(ctx: &Context, _: Empty) -> Result<DeploymentCheck, CapabilityError> {
    let (deployments, unreadable) = views(ctx);
    let routes = ctx
        .registry
        .iter()
        .filter_map(|c| c.exposure.http.as_ref().map(|h| h.path.clone()))
        .collect();
    let ws = Workspace::read(std::path::Path::new(&ctx.index.repository.root), routes);
    let mut refusals = unreadable;
    for view in &deployments {
        refusals.extend(view.deployment.check(&view.file, &ws));
    }
    Ok(DeploymentCheck {
        ok: refusals.is_empty(),
        decided: deployments.len(),
        refusals,
    })
}

impl BenchmarkCases for GetDeploymentInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        ctx.index
            .objects
            .iter()
            .find(|o| o.kind == KIND)
            .map(|o| {
                vec![NamedCase::new(
                    "first-deployment",
                    GetDeploymentInput {
                        id: o.identity.clone(),
                    },
                )]
            })
            .unwrap_or_else(|| {
                vec![NamedCase::new(
                    "absent",
                    GetDeploymentInput {
                        id: "absent".into(),
                    },
                )]
            })
    }
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "deploy",
        title: "Deployment",
        description: "The deployments this repository declares, read from the canonical objects the index holds, and whether they would work — decided against the capability registry this process built and the workspace it sits in. Every operation is a read: a deployment is changed by the trusted command line and by CI, never over HTTP and never by an MCP client.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "deploy.list",
                title: "Deployments",
                description: "Every deployment the layer declares, typed: the application, the package and binary shipped, the address the process listens on, the routes a platform polls, the resources, the machine count, the region, the build inputs, the measured budgets and the provider's own facts. An object of the kind this executable cannot read is reported with the reason rather than skipped.",
                input: Empty,
                output: DeploymentList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_deployments".into()),
                        resource: Some(McpResource { uri: DEPLOYMENTS_URI.into(), name: "deployments".into() }),
                    }),
                    http: get("/api/v1/deployments"),
                    cli: None,
                },
                tags: ["deployment"],
                handler: deploy_list,
            },
            capability! {
                id: "deploy.get",
                title: "One deployment",
                description: "One deployment by its identity, typed, with the repository-relative file it was read from.",
                input: GetDeploymentInput,
                output: DeploymentView,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_deployment"), http: get("/api/v1/deployment"), cli: None },
                tags: ["deployment"],
                handler: deploy_get,
            },
            capability! {
                id: "deploy.check",
                title: "Would these deployments work",
                description: "Every refusal the declared deployments earn locally: a health route no capability registers, a package or binary the workspace does not produce, a build input that does not resolve, more machines running than exist, a hosted process that would bind loopback. Each names the file, the key, the value observed and the correction. Nothing here contacts the provider.",
                input: Empty,
                output: DeploymentCheck,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_deploy_check"), http: get("/api/v1/deployments/check"), cli: None },
                tags: ["deployment", "diagnostics"],
                cache: CachePolicy::Process { max_entries: 2, ttl_seconds: Some(5) },
                handler: deploy_check,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::model::{CapabilityKind, HttpMethod};

    /// Nothing here mutates. A deployment is changed by the trusted command line and by
    /// CI; an operation that could start, stop or replace a machine must not be reachable
    /// from a browser or from an MCP client, and `Query` is what says so to every
    /// projection at once — read-only in MCP, `GET` in HTTP and OpenAPI.
    #[test]
    fn every_deployment_operation_is_a_read() {
        for c in module().capabilities {
            assert_eq!(
                c.capability.kind,
                CapabilityKind::Query,
                "{} is not a read",
                c.capability.id
            );
            if let Some(http) = &c.capability.exposure.http {
                assert_eq!(
                    http.method,
                    HttpMethod::Get,
                    "{} is not a GET",
                    c.capability.id
                );
            }
        }
    }

    /// The declaration is the whole registration: the ids, the routes and the MCP names
    /// exist once, here, and every projection is derived from them. A test that pinned a
    /// second list would be the defect; this one pins that the module composes what it
    /// says it does.
    #[test]
    fn the_module_declares_the_reads_and_nothing_else() {
        let ids: Vec<String> = module()
            .capabilities
            .iter()
            .map(|c| c.capability.id.to_string())
            .collect();
        assert_eq!(ids, ["deploy.list", "deploy.get", "deploy.check"]);
    }

    /// A deployment id the layer does not have is refused by name rather than answered
    /// with an empty result: an absent deployment and a deployment with nothing in it are
    /// different facts.
    #[test]
    fn an_absent_deployment_is_not_found() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let e = deploy_get(
            &ctx,
            GetDeploymentInput {
                id: "absent".into(),
            },
        )
        .expect_err("refused");
        assert!(matches!(e, CapabilityError::NotFound(_)), "{e}");
    }

    /// A layer that declares no deployment is not a failure: the list is empty and the
    /// check is satisfied, because there is nothing that could be wrong.
    #[test]
    fn a_layer_with_no_deployments_lists_none_and_checks_clean() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        assert_eq!(deploy_list(&ctx, Empty {}).expect("listed").count, 0);
        let check = deploy_check(&ctx, Empty {}).expect("checked");
        assert!(check.ok);
        assert_eq!(check.decided, 0);
    }
}
