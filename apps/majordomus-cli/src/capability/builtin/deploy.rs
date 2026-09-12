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
use crate::capability::model::{BenchmarkPolicy, Exposure, McpExposure, McpResource, Stability, WaiverReason};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::deploy::targets::DeploymentPlan;
use crate::deploy::verify::{self, CurlFetcher, VerificationReport};
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

/// The view orders as the deployment it wraps: the file path it adds is provenance, not a
/// second opinion about the sequence.
impl crate::order::Ordered for DeploymentView {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        self.deployment.order_key()
    }
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
/// The default asks every published surface whether it serves this checkout's HEAD; a
/// caller names another commit, a subset of targets, or the change set to derive
/// applicability from.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// What to verify, and against which revision.
pub struct VerifyInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit every target is expected to serve. Absent means this checkout's HEAD.
    pub expected_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The targets to ask, by id (`pages`, `release`, a deployment's id). Absent means
    /// every target that applies.
    pub targets: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The changed paths to derive applicability from. Absent means every target that
    /// exists is asked, which is the question after a deployment.
    pub changed: Option<Vec<String>>,
}

impl BenchmarkCases for VerifyInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new("every-target", VerifyInput::default())]
    }
}

/// The plan and the live answers, as one document: what would be asked, what was, and
/// what each surface stated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentVerification {
    /// The commit the targets were expected to serve, when one was known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_commit: Option<String>,
    /// Which targets exist and which apply.
    pub plan: DeploymentPlan,
    /// What each was found to serve.
    #[serde(flatten)]
    pub report: VerificationReport,
}

fn deploy_verify(ctx: &Context, input: VerifyInput) -> Result<DeploymentVerification, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let expected = match input.expected_commit {
        Some(c) => Some(c),
        None => crate::gates::head_of(&root),
    };
    let model = crate::gates::GateModel::load(&root).ok();
    let everything = input.changed.is_none();
    let changed = input.changed.unwrap_or_default();
    let mut plan = super::gates::deployment_plan(ctx, model.as_ref(), &changed, expected.clone(), everything);
    if let Some(only) = &input.targets {
        for t in &mut plan.targets {
            if !only.iter().any(|o| o == &t.id) {
                t.applicable = false;
                t.reason = "not among the targets asked for".into();
            }
        }
    }
    let now = crate::peers::rfc3339(std::time::SystemTime::now());
    let report = verify::verify(&plan, &CurlFetcher, &now);
    Ok(DeploymentVerification { expected_commit: expected, plan, report })
}

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
            capability! {
                id: "deploy.verify",
                title: "What the deployed surfaces are serving",
                description: "Live verification: every surface the change reaches — the published site, the published release metadata, each active deployment — is asked for the identity it states (the commit, version or tag at its own address) and compared with what this checkout expects. A surface stating an older identity is stale, one that does not answer is unreachable, and neither is a pass: a deploy command that exited 0 with the old revision still live is exactly what this refuses. The request carries no header and the evidence carries no body beyond the fields compared. The one capability of this executable that reaches the network, and it reaches only addresses the repository itself declares.",
                input: VerifyInput,
                output: DeploymentVerification,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_deploy_verify".into()),
                        resource: None,
                    }),
                    http: get("/api/v1/deployments/verify"),
                    cli: None,
                },
                tags: ["deployments", "verification", "evidence", "live"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: deploy_verify,
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
    fn the_default_verification_asks_everything_against_head() {
        let i = VerifyInput::default();
        assert!(i.expected_commit.is_none() && i.targets.is_none() && i.changed.is_none());
        let json = serde_json::to_string(&i).unwrap();
        assert_eq!(json, "{}", "nothing is serialised that was not asked");
    }

    #[test]
    fn the_module_declares_the_reads_and_the_one_live_verification() {
        let ids: Vec<String> = module()
            .capabilities
            .iter()
            .map(|c| c.capability.id.to_string())
            .collect();
        assert_eq!(ids, ["deploy.list", "deploy.get", "deploy.check", "deploy.verify"]);
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
