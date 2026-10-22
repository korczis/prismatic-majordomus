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
//!
//! `deploy.observed` keeps to that: it answers from the record `majordomus served forge` wrote
//! when it last asked the forge ([`crate::deploy::observed`]) — every deployment to every
//! environment and every release — and says so when this checkout has never asked.
//!
//! One operation is not a read: `deploy.slug` assembles the tree a provider receives
//! ([`crate::deploy::slug`], ADR 0219) into a directory outside the checkout. It changes
//! nothing in the repository and reaches no network, and it is still a command of the trusted
//! command line only: an operation that writes wherever its caller names has no business
//! behind a surface a browser or an MCP client can reach.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CapabilityKind, CliExposure, Exposure, McpExposure, McpResource, Stability,
    WaiverReason,
};
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

/// The input of `deploy.slug`: which deployment, at which commit, with which executable, and
/// where the slug is written. Relative paths are read against the working directory.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlugInput {
    /// The deployment's `id`; when absent, the one deployment that names a provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<String>,
    /// The commit every file is read at, any revision git resolves; `HEAD` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The executable built at that commit.
    pub executable: String,
    /// The directory the slug is written into: outside the checkout, absent or empty.
    pub out: String,
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

fn deploy_observed(
    ctx: &Context,
    _: Empty,
) -> Result<crate::deploy::observed::ObservedDeployments, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    Ok(crate::deploy::observed::report(&root))
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

fn deploy_slug(
    ctx: &Context,
    input: SlugInput,
) -> Result<crate::deploy::slug::Slug, CapabilityError> {
    let (deployments, _) = views(ctx);
    let view = match input.deployment.as_deref() {
        Some(id) => deployments
            .iter()
            .find(|d| d.deployment.id == id)
            .ok_or_else(|| {
                CapabilityError::NotFound(format!(
                    "no deployment '{id}' (the deployments are read from the layer's deployments section)"
                ))
            })?,
        None => {
            let hosted: Vec<&DeploymentView> = deployments
                .iter()
                .filter(|d| d.deployment.provider.is_some())
                .collect();
            match hosted.as_slice() {
                [one] => *one,
                [] => {
                    return Err(CapabilityError::Refused(
                        "no deployment names a provider, so there is no slug to assemble; add a `provider` block (ADR 0219)".into(),
                    ))
                }
                many => {
                    return Err(CapabilityError::InvalidInput(format!(
                        "{} deployments name a provider ({}); name one with `deployment`",
                        many.len(),
                        many.iter()
                            .map(|d| d.deployment.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )))
                }
            }
        }
    };
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let request = crate::deploy::slug::Request {
        root: &root,
        file: &view.file,
        id: &view.deployment.id,
        commit: input.commit.as_deref().unwrap_or("HEAD"),
        executable: std::path::Path::new(&input.executable),
        out: std::path::Path::new(&input.out),
    };
    crate::deploy::slug::assemble(&request).map_err(|e| match e {
        crate::deploy::slug::SlugError::Refused(m) => CapabilityError::Refused(m),
        crate::deploy::slug::SlugError::Missing(m) => CapabilityError::NotFound(m),
        crate::deploy::slug::SlugError::Failed(m) => CapabilityError::Internal(m),
    })
}

impl BenchmarkCases for SlugInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: the capability is waived because it writes a directory and needs a built
        // executable; timing it in a loop would measure git and the disk, not this executable
        Vec::new()
    }
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
        description: "The deployments this repository declares, read from the canonical objects the index holds, and whether they would work — decided against the capability registry this process built and the workspace it sits in. Every operation over HTTP and MCP is a read; the one command, assembling the tree a provider receives, writes outside the checkout and is a command line only: a deployment is changed by the trusted command line and by CI, never over HTTP and never by an MCP client.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "deploy.list",
                title: "Deployments",
                description: "Every deployment the layer declares, typed: the application, the package and binary shipped, the address the process listens on, the routes a platform polls, the resources, the machine count, the region, the build inputs and the measured budgets. An object of the kind this executable cannot read is reported with the reason rather than skipped.",
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
                id: "deploy.observed",
                title: "What was deployed, and where",
                description: "Every deployment the forge recorded, to every environment it has one in — GitHub Pages among them — with the commit, the ref, who created it, how it ended, where it is served and the run that did it; every environment described by its newest deployment; and every release the forge published. Read from the record `majordomus served forge` wrote when it last asked the forge: nothing here reaches the network, and a checkout that has never asked answers `observed: false` with what to run.",
                input: Empty,
                output: crate::deploy::observed::ObservedDeployments,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_deployments_observed"), http: get("/api/v1/deployments/observed"), cli: None },
                tags: ["deployment", "delivery"],
                handler: deploy_observed,
            },
            capability! {
                id: "deploy.slug",
                kind: CapabilityKind::Command,
                title: "Assemble the tree a provider receives",
                description: "Writes, into a directory outside the checkout that is absent or empty, the tree the deployment's provider receives (ADR 0219): the object's build inputs except the package the executable is compiled from, read from git at one commit (HEAD unless named); the provider's generated files of that commit at the slug's root, each held to what the object renders and the hooks made executable; the provider's buildpack line; the executable built at that commit; and a record naming the deployment, the commit, the executable's digest and the provider. Refuses an out directory inside the checkout or not empty, a deployment that names no provider, a build input the commit does not track, a provider file the object does not render, and two files that would land on one path. Changes nothing in the repository and reaches no network; pushing the slug is the provider workflow's.",
                input: SlugInput,
                output: crate::deploy::slug::Slug,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["deploy".into(), "slug".into()] }),
                },
                tags: ["deployment", "delivery"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::Destructive },
                handler: deploy_slug,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::model::HttpMethod;

    /// Nothing a browser or an MCP client can reach mutates. A deployment is changed by the
    /// trusted command line and by CI; an operation that could start, stop or replace a
    /// machine, or write where its caller names, must not be reachable from either, and
    /// `Query` is what says so to every projection at once — read-only in MCP, `GET` in HTTP
    /// and OpenAPI. The one command, `deploy.slug`, is a command line and nothing else.
    #[test]
    fn every_deployment_operation_is_a_read() {
        for c in module().capabilities {
            if c.capability.kind == CapabilityKind::Command {
                assert!(
                    c.capability.exposure.http.is_none() && c.capability.exposure.mcp.is_none(),
                    "{} is a command reachable beyond the command line",
                    c.capability.id
                );
                continue;
            }
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
    fn the_module_declares_the_reads_and_the_slug_and_nothing_else() {
        let ids: Vec<String> = module()
            .capabilities
            .iter()
            .map(|c| c.capability.id.to_string())
            .collect();
        assert_eq!(
            ids,
            [
                "deploy.list",
                "deploy.get",
                "deploy.check",
                "deploy.observed",
                "deploy.slug"
            ]
        );
    }

    /// What was observed is read from the record and nothing else: a repository nobody has
    /// asked the forge about answers that, with the command that asks.
    #[test]
    fn what_was_deployed_is_read_from_the_record_and_absent_is_said() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().unwrap();
        let answer = deploy_observed(&ctx, Empty {}).unwrap();
        assert!(!answer.observed);
        assert!(answer.reason.unwrap().contains("served forge"));

        let observation = crate::deploy::observed::parse(
            "o/r",
            "2026-10-12T14:00:00Z",
            &serde_json::json!({"data": {"repository": {"url": "https://forge.example/o/r",
                "deployments": {"nodes": [{"databaseId": 1, "environment": "github-pages",
                    "createdAt": "2026-10-12T10:00:00Z", "commitOid": "aaa",
                    "latestStatus": {"state": "SUCCESS"}}]},
                "releases": {"nodes": []}}}}),
        );
        crate::deploy::observed::store(repo.root(), &observation).unwrap();
        let answer = deploy_observed(&ctx, Empty {}).unwrap();
        assert!(answer.observed);
        assert_eq!(answer.environments[0].name, "github-pages");
        assert_eq!(answer.deployments[0].state, "success");
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

    /// The input names a deployment, a commit, an executable and a directory, and nothing
    /// else: a key it does not have is refused, a credential above all.
    #[test]
    fn the_slug_input_refuses_a_key_it_does_not_have() {
        let input: SlugInput =
            serde_json::from_str(r#"{"executable":"target/majordomus","out":"/tmp/slug"}"#)
                .unwrap();
        assert!(input.deployment.is_none() && input.commit.is_none());
        assert!(
            serde_json::from_str::<SlugInput>(r#"{"executable":"e","out":"o","api_key":"x"}"#)
                .is_err()
        );
    }

    /// The one command is a command line and nothing else.
    #[test]
    fn the_slug_is_a_command_line_only() {
        let m = module();
        let slug = m
            .capabilities
            .iter()
            .find(|c| c.capability.id.as_str() == "deploy.slug")
            .unwrap();
        assert_eq!(slug.capability.kind, CapabilityKind::Command);
        assert!(slug.capability.exposure.http.is_none() && slug.capability.exposure.mcp.is_none());
        assert_eq!(
            slug.capability.exposure.cli.as_ref().unwrap().path,
            ["deploy", "slug"]
        );
    }

    /// A slug is assembled for a deployment some provider hosts: with none named and none
    /// hosted there is nothing to assemble, and a named deployment the layer lacks is not
    /// found — and neither writes anything.
    #[test]
    fn a_slug_needs_a_hosted_deployment() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let out = tempfile::tempdir().unwrap();
        let slug = |deployment: Option<&str>| {
            deploy_slug(
                &ctx,
                SlugInput {
                    deployment: deployment.map(String::from),
                    commit: None,
                    executable: "majordomus".into(),
                    out: out.path().join("slug").display().to_string(),
                },
            )
        };
        let e = slug(None).expect_err("refused");
        assert!(
            matches!(&e, CapabilityError::Refused(m) if m.contains("names a provider")),
            "{e}"
        );
        let e = slug(Some("absent")).expect_err("not found");
        assert!(matches!(e, CapabilityError::NotFound(_)), "{e}");
        assert!(!out.path().join("slug").exists());
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

    fn slug_input(
        deployment: Option<&str>,
        commit: Option<&str>,
        executable: &std::path::Path,
        out: &std::path::Path,
    ) -> SlugInput {
        SlugInput {
            deployment: deployment.map(String::from),
            commit: commit.map(String::from),
            executable: executable.display().to_string(),
            out: out.display().to_string(),
        }
    }

    /// A hosted deployment is assembled when it is named, and when it is the only one that
    /// names a provider and none is named: at HEAD unless a commit is given.
    #[test]
    fn a_hosted_deployment_is_assembled_named_or_alone() {
        let repo = crate::deploy::slug::testing::hosted_layer(&["hosted"]);
        let ctx = repo.context().expect("a context");
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("exe");
        std::fs::write(&exe, b"an executable").unwrap();

        let named = elsewhere.path().join("named");
        let slug = deploy_slug(&ctx, slug_input(Some("hosted"), Some("HEAD"), &exe, &named))
            .expect("assembled");
        assert_eq!(slug.deployment, "hosted");
        assert_eq!(slug.provider.app, "hosted-app");
        assert!(
            slug.entries.iter().any(|e| e == "docs"),
            "{:?}",
            slug.entries
        );
        assert!(named.join("Procfile").is_file());

        let alone = elsewhere.path().join("alone");
        let slug = deploy_slug(&ctx, slug_input(None, None, &exe, &alone)).expect("assembled");
        assert_eq!(slug.deployment, "hosted");
        assert!(alone.join(crate::deploy::slug::SOURCE_FILE).is_file());
    }

    /// What the assembler declines, cannot find, or fails at reaches the caller as a
    /// refusal, a not-found and an internal error.
    #[test]
    fn what_the_assembler_says_is_the_slug_capabilitys_error() {
        let repo = crate::deploy::slug::testing::hosted_layer(&["hosted"]);
        let ctx = repo.context().expect("a context");
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = elsewhere.path().join("exe");
        std::fs::write(&exe, b"an executable").unwrap();

        let absent = elsewhere.path().join("absent");
        let e = deploy_slug(
            &ctx,
            slug_input(None, None, &absent, &elsewhere.path().join("a")),
        )
        .expect_err("not found");
        assert!(
            matches!(&e, CapabilityError::NotFound(m) if m.contains("absent")),
            "{e}"
        );

        let full = elsewhere.path().join("full");
        std::fs::create_dir(&full).unwrap();
        std::fs::write(full.join("x"), b"x").unwrap();
        let e = deploy_slug(&ctx, slug_input(None, None, &exe, &full)).expect_err("refused");
        assert!(
            matches!(&e, CapabilityError::Refused(m) if m.contains("not empty")),
            "{e}"
        );

        let e =
            deploy_slug(&ctx, slug_input(None, None, &exe, &exe.join("slug"))).expect_err("failed");
        assert!(
            matches!(&e, CapabilityError::Internal(m) if m.contains("cannot be written")),
            "{e}"
        );
    }

    /// With two hosted deployments and none named, the input is incomplete: both are named
    /// so the caller can choose.
    #[test]
    fn two_hosted_deployments_need_the_slug_input_to_name_one() {
        let repo = crate::deploy::slug::testing::hosted_layer(&["east", "west"]);
        let ctx = repo.context().expect("a context");
        let elsewhere = tempfile::tempdir().unwrap();
        let out = elsewhere.path().join("slug");
        let e = deploy_slug(&ctx, slug_input(None, None, &out, &out)).expect_err("ambiguous");
        assert!(
            matches!(&e, CapabilityError::InvalidInput(m)
                if m.contains("2 deployments name a provider")
                    && m.contains("east")
                    && m.contains("west")),
            "{e}"
        );
        assert!(!out.exists());
    }

    /// The slug is waived from the benchmark: it declares no case to time.
    #[test]
    fn the_deploy_slug_declares_no_benchmark_case() {
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        let ctx = repo.context().expect("a context");
        assert!(SlugInput::benchmark_cases(&CaseContext { index: &ctx.index }).is_empty());
    }
}
