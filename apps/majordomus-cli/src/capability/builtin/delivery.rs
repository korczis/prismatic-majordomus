//! The `delivery` module: whether each product feature exists, computed on every read.
//!
//! A feature exists only when it is on the trunk, deployed, publicly verified, has its
//! required tests current, has that evidence published and is linked from the interface
//! (ADR 0071). Every one of those is a verdict derived here from git, from the public site's
//! own identity and — from phase 2 — from recorded evidence; none is a field anybody wrote.
//! Unknown is never pass.
//!
//! Both capabilities are reads, and both may reach the network: the public identity is read
//! from the site, and `scripts/pages` asks the site again and asks GitHub. That is why the
//! benchmark waives them as an external dependency, and why nothing here is cached: a
//! publication that changed a second ago is the answer a person asking wants.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CachePolicy, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::delivery::public::Probe;
use crate::delivery::{self, revision, DeliveryReport, FeatureDelivery, Subject};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The input of `delivery.feature`: which feature.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeliveryFeatureInput {
    /// The feature's id, as `product.features` gives it.
    pub id: String,
}

impl BenchmarkCases for DeliveryFeatureInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let id = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == crate::product::FEATURE)
            .map(|o| o.identity.clone())
            .unwrap_or_else(|| "absent".into());
        vec![NamedCase::new("first-feature", DeliveryFeatureInput { id })]
    }
}

fn report_for(ctx: &Context, only: Option<&str>) -> DeliveryReport {
    let root = std::path::Path::new(&ctx.index.repository.root);
    let subjects: Vec<Subject> = ctx
        .product
        .all()
        .iter()
        .filter(|r| only.is_none_or(|id| r.feature.id == id))
        .map(|r| Subject {
            id: r.feature.id.clone(),
            title: r.feature.title.clone(),
            status: r.feature.status.clone(),
            paths: revision::implementation_paths(root, r, &ctx.index),
        })
        .collect();
    delivery::assess(root, subjects, Probe::for_repository(root).observe())
}

fn delivery_report(ctx: &Context, _: Empty) -> Result<DeliveryReport, CapabilityError> {
    Ok(report_for(ctx, None))
}

fn delivery_feature(
    ctx: &Context,
    input: DeliveryFeatureInput,
) -> Result<FeatureDelivery, CapabilityError> {
    if ctx.product.feature(&input.id).is_none() {
        return Err(CapabilityError::NotFound(format!(
            "no feature '{}'; `product.features` names every one this repository holds",
            input.id
        )));
    }
    report_for(ctx, Some(&input.id))
        .features
        .into_iter()
        .next()
        .ok_or_else(|| CapabilityError::NotFound(format!("no feature '{}'", input.id)))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "delivery",
        title: "Delivery",
        description: "Whether each product feature exists, computed and never recorded: on the trunk, deployed, publicly verified, its required tests current, that evidence published and linked from the interface — each a verdict of pass, fail or unknown with the reason and the remediation, and unknown never a pass. A feature short of any of them is not delivered, and its development stage (implemented on a branch, on master, tested, deployed) is a different type from delivery.",
        stability: Stability::Experimental,
        capabilities: [
            capability! {
                id: "delivery.report",
                title: "Does each feature exist",
                description: "Every product feature of the layer against the delivery invariant, ordered by id: the six dimensions with their verdicts, reasons and remediations; the development stage when it is not delivered; the paths it is implemented by and the revisions that last touched them at HEAD and on the trunk; and the publication — the public site's identity, read once, and whether scripts/pages verified it. Reads git and the network; the site address can be replaced with MAJORDOMUS_DELIVERY_SITE_URL.",
                input: Empty,
                output: DeliveryReport,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_delivery"),
                    http: get("/api/v1/delivery"),
                    cli: Some(CliExposure { path: vec!["delivery".into(), "report".into()] }),
                },
                tags: ["delivery", "features", "verification"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: delivery_report,
            },
            capability! {
                id: "delivery.feature",
                title: "Does this feature exist",
                description: "One product feature against the delivery invariant: every dimension with its verdict, the sentence that decided it and what would change it, whether it exists, and — when it does not — how far development has carried it and which dimensions block it. An id the layer does not declare is not found rather than answered as not delivered.",
                input: DeliveryFeatureInput,
                output: FeatureDelivery,
                stability: Stability::Experimental,
                exposure: Exposure {
                    mcp: mcp("majordomus_delivery_feature"),
                    http: get("/api/v1/delivery/feature"),
                    cli: Some(CliExposure { path: vec!["delivery".into(), "show".into()] }),
                },
                tags: ["delivery", "features", "verification"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: delivery_feature,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::model::CapabilityKind;
    use crate::delivery::{DeliveryState, DevelopmentStage, Verdict};
    use crate::synthetic::{Shape, SyntheticRepository};

    const FEATURE: &str = "---\nschema: feature/v1\nid: shipped\nkind: feature\ntitle: Shipped\nheadline: 'It ships.'\nsummary: 'It is made of one module and one claimless command.'\nstatus: draft\nmodules: [delivery]\ncommands: [start]\n---\n\n## What it does\n\nShips.\n";

    fn git(root: &std::path::Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A synthetic repository that declares one feature, committed and on `origin/master`.
    fn repository() -> SyntheticRepository {
        let repo = SyntheticRepository::new(Shape::default()).unwrap();
        let root = repo.root();
        let sources = root.join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).unwrap();
        text.push_str("  - id: feature\n    kind: feature\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/features/*.md'\n    required: false\n");
        std::fs::write(&sources, text).unwrap();
        std::fs::create_dir_all(root.join(".ai/repo/features")).unwrap();
        std::fs::write(root.join(".ai/repo/features/shipped.md"), FEATURE).unwrap();
        let module = root.join("apps/majordomus-cli/src/capability/builtin/delivery.rs");
        std::fs::create_dir_all(module.parent().unwrap()).unwrap();
        std::fs::write(module, "// the module\n").unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("lib/start.sh"), "# start\n").unwrap();
        git(root, &["init", "-q", "-b", "master", "."]);
        git(root, &["add", "-A"]);
        git(
            root,
            &[
                "-c",
                "user.email=t@e",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                "all",
            ],
        );
        git(root, &["update-ref", "refs/remotes/origin/master", "HEAD"]);
        repo
    }

    /// One capability's id, MCP tool, HTTP path and command-line words.
    type Projection = (String, Option<String>, Option<String>, Vec<String>);

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "delivery");
        let seen: Vec<Projection> = m
            .capabilities
            .iter()
            .map(|e| {
                let c = &e.capability;
                assert_eq!(c.kind, CapabilityKind::Query, "{} is a read", c.id);
                (
                    c.id.to_string(),
                    c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
                    c.exposure.http.as_ref().map(|h| h.path.clone()),
                    c.exposure
                        .cli
                        .as_ref()
                        .map(|c| c.path.clone())
                        .unwrap_or_default(),
                )
            })
            .collect();
        let s = |v: &str| Some(v.to_string());
        assert_eq!(
            seen,
            vec![
                (
                    "delivery.report".into(),
                    s("majordomus_delivery"),
                    s("/api/v1/delivery"),
                    vec!["delivery".to_string(), "report".into()]
                ),
                (
                    "delivery.feature".into(),
                    s("majordomus_delivery_feature"),
                    s("/api/v1/delivery/feature"),
                    vec!["delivery".to_string(), "show".into()]
                ),
            ]
        );
    }

    #[test]
    fn a_merged_feature_with_no_site_is_on_master_and_does_not_exist() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let report = delivery_report(&ctx, Empty {}).expect("a report");
        assert_eq!(report.tallies.features, 1);
        let f = &report.features[0];
        assert_eq!(f.id, "shipped");
        assert_eq!(
            f.implementation.paths,
            [
                "apps/majordomus-cli/src/capability/builtin/delivery.rs",
                "lib/start.sh"
            ]
        );
        assert_eq!(f.dimensions[0].verdict, Verdict::Pass, "{f:?}");
        // a repository with no site and no publication model is not deployed: it is unknown
        assert_eq!(f.dimensions[1].verdict, Verdict::Unknown);
        assert!(!f.exists);
        assert!(matches!(
            f.delivery,
            DeliveryState::NotDelivered {
                stage: DevelopmentStage::OnMaster,
                ..
            }
        ));
        let one = delivery_feature(
            &ctx,
            DeliveryFeatureInput {
                id: "shipped".into(),
            },
        )
        .expect("one feature");
        assert_eq!(&one, f);
    }

    #[test]
    fn an_absent_feature_is_not_found() {
        let repo = repository();
        let ctx = repo.context().expect("a context");
        let e = delivery_feature(
            &ctx,
            DeliveryFeatureInput {
                id: "absent".into(),
            },
        )
        .expect_err("refused");
        assert!(matches!(e, CapabilityError::NotFound(_)), "{e}");
        let cases = DeliveryFeatureInput::benchmark_cases(&CaseContext { index: &ctx.index });
        assert_eq!(cases.len(), 1);
    }
}
