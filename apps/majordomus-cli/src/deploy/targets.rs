//! The deployment plan: which surfaces a change reaches, derived from what the repository
//! already declares, and never from a list of deploy commands.
//!
//! # Why targets are derived
//!
//! "Every session deploys" is not "run every deploy command known to humanity". A
//! documentation change reaches the published site and nothing else; a change to the
//! executable's crate moves the public surface and therefore the release; a declared
//! application deployment is reached only when it is active. Each of those is a fact the
//! repository already holds — the CI model's path classes say which paths the site build
//! and the version gate are taken over, the site's configuration says where it is
//! published, the release records say what was last released, the deployment objects say
//! whether anything is running — and the plan is a projection of those facts.
//!
//! Three kinds of target exist here, and a fourth is a deployment object of its own:
//!
//! ```text
//! pages         the published site (GitHub Pages)        identity: <base_url>/build.json
//! release       the published release metadata           identity: <base_url>/releases/latest.json
//! application   a deployment object with status active   identity: <url>/api/v1/distribution/build
//! ```
//!
//! A target that does not apply is in the plan with the reason, because "not applicable
//! because the change touches nothing the site is built from" and "not checked" are
//! different findings and a reader is entitled to the difference.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::discovery::glob::Glob;

/// The three kinds of surface a deployment target can be.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    /// The published static site.
    Pages,
    /// The published release metadata and artifacts.
    Release,
    /// A running instance of the executable, declared as a deployment object.
    Application,
}

impl TargetKind {
    /// The word, as serialised.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pages => "pages",
            Self::Release => "release",
            Self::Application => "application",
        }
    }
}

/// What a live surface is expected to be serving.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DeploymentIdentity")]
pub struct Identity {
    /// The commit, when the surface reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The version, when the surface reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The release tag, when the surface reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl Identity {
    /// Whether nothing at all is expected — a target with no expectation cannot be
    /// verified, only reached.
    pub fn is_empty(&self) -> bool {
        self.commit.is_none() && self.version.is_none() && self.tag.is_none()
    }
}

/// One surface the change may reach.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentTarget {
    /// The target's identity: `pages`, `release`, or the deployment object's id.
    pub id: String,
    /// Which kind of surface it is.
    pub kind: TargetKind,
    /// Where it is served, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The address that states the surface's own identity, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_url: Option<String>,
    /// Whether the change reaches it.
    pub applicable: bool,
    /// Why it applies, or why it does not.
    pub reason: String,
    /// What the surface is expected to serve once the change is deployed.
    #[serde(default)]
    pub expected: Identity,
    /// The pathspecs whose change makes the target applicable, when paths do.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    /// The changed paths that made it applicable, when paths did.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub because: Vec<String>,
}

/// Every target, applicable or not.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentPlan {
    /// The targets, in a stable order: pages, release, then each application by id.
    pub targets: Vec<DeploymentTarget>,
    /// What could not be derived, each as one line.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}

impl DeploymentPlan {
    /// The applicable targets.
    pub fn applicable(&self) -> impl Iterator<Item = &DeploymentTarget> {
        self.targets.iter().filter(|t| t.applicable)
    }
}

/// What the plan is derived from. Every field is a fact something else already holds.
#[derive(Debug, Clone, Default)]
pub struct PlanFacts {
    /// The site's published origin, from `site/config.toml`.
    pub site_base_url: Option<String>,
    /// The pathspecs the site build is taken over (the CI model's, for the site-build gate).
    pub site_inputs: Vec<String>,
    /// The pathspecs the public surface is taken over (the CI model's, for the version gate).
    pub surface_inputs: Vec<String>,
    /// The newest published release: its tag, version and commit.
    pub latest_release: Option<Identity>,
    /// Every declared application deployment.
    pub applications: Vec<ApplicationFact>,
    /// The commit the change is expected to be served as, when known (the head, or the
    /// commit a caller names).
    pub expected_commit: Option<String>,
    /// The version the tree declares.
    pub declared_version: Option<String>,
}

/// One declared application deployment, as the plan reads it.
#[derive(Debug, Clone, Default)]
pub struct ApplicationFact {
    /// The deployment object's id.
    pub id: String,
    /// Its status word: `declared`, `active`, `retired`.
    pub status: String,
    /// Where it answers, when the object states it.
    pub url: Option<String>,
    /// The pathspecs its image is built from, so a change outside them does not reach it.
    pub inputs: Vec<String>,
}

/// Which of `inputs` a changed path falls under.
fn hits(inputs: &[String], changed: &[String]) -> Vec<String> {
    changed
        .iter()
        .filter(|p| inputs.iter().any(|spec| Glob::new(spec).matches(p)))
        .cloned()
        .collect()
}

/// Derive the plan. `changed` is the change set; an empty one, with `everything` set,
/// means "every target that exists" — what a verifier asks after a deployment, when the
/// question is no longer which targets the change reaches but whether each is serving it.
pub fn plan(facts: &PlanFacts, changed: &[String], everything: bool) -> DeploymentPlan {
    let mut targets = Vec::new();
    let mut findings = Vec::new();

    // pages
    match &facts.site_base_url {
        Some(base) => {
            let because = hits(&facts.site_inputs, changed);
            let applicable = everything || !because.is_empty();
            targets.push(DeploymentTarget {
                id: "pages".into(),
                kind: TargetKind::Pages,
                url: Some(base.clone()),
                identity_url: Some(format!("{}/build.json", base.trim_end_matches('/'))),
                applicable,
                reason: if everything {
                    "every published surface is asked after a deployment".into()
                } else if applicable {
                    format!(
                        "the change touches {} file(s) the site is built from",
                        because.len()
                    )
                } else if facts.site_inputs.is_empty() {
                    "the CI model declares no site-build gate, so nothing says what the site is \
                     built from"
                        .into()
                } else {
                    "the change touches nothing the site is built from".into()
                },
                expected: Identity {
                    commit: facts.expected_commit.clone(),
                    version: facts.declared_version.clone(),
                    tag: None,
                },
                inputs: facts.site_inputs.clone(),
                because,
            });
        }
        None => findings.push(
            "site/config.toml states no base_url, so the published site is not a target".into(),
        ),
    }

    // release
    match (&facts.site_base_url, &facts.latest_release) {
        (Some(base), Some(latest)) => {
            let because = hits(&facts.surface_inputs, changed);
            let applicable = everything || !because.is_empty();
            targets.push(DeploymentTarget {
                id: "release".into(),
                kind: TargetKind::Release,
                url: Some(format!("{}/releases/", base.trim_end_matches('/'))),
                identity_url: Some(format!(
                    "{}/releases/latest.json",
                    base.trim_end_matches('/')
                )),
                applicable,
                reason: if everything {
                    "every published surface is asked after a deployment".into()
                } else if applicable {
                    format!(
                        "the change touches {} file(s) the public surface is taken over; a \
                         release carries it",
                        because.len()
                    )
                } else {
                    "the change touches nothing the public surface is taken over".into()
                },
                // what the metadata must state is the newest record: a release that was
                // cut is published or it was not, and the tree's declared version is what
                // the next one will state
                expected: latest.clone(),
                inputs: facts.surface_inputs.clone(),
                because,
            });
        }
        (_, None) => findings.push(
            "no release record exists, so the published release metadata is not a target".into(),
        ),
        (None, _) => {}
    }

    // applications
    for app in &facts.applications {
        let active = app.status == "active";
        let because = hits(&app.inputs, changed);
        let reached = everything || !because.is_empty();
        let applicable = active && app.url.is_some() && reached;
        targets.push(DeploymentTarget {
            id: app.id.clone(),
            kind: TargetKind::Application,
            url: app.url.clone(),
            identity_url: app
                .url
                .as_ref()
                .map(|u| format!("{}/api/v1/distribution/build", u.trim_end_matches('/'))),
            applicable,
            reason: if applicable && everything {
                "the deployment is active and every published surface is asked after a \
                 deployment"
                    .into()
            } else if applicable {
                format!(
                    "the deployment is active and the change touches {} file(s) its image \
                     is built from",
                    because.len()
                )
            } else if !active {
                format!(
                    "the deployment object is `{}`, not active: nothing is running it, and \
                     nothing here pretends to verify it",
                    app.status
                )
            } else if app.url.is_none() {
                "the deployment object is active but states no url to ask".into()
            } else {
                "the change touches nothing the deployment's image is built from".into()
            },
            expected: Identity {
                commit: facts.expected_commit.clone(),
                version: facts.declared_version.clone(),
                tag: None,
            },
            inputs: app.inputs.clone(),
            because,
        });
    }

    DeploymentPlan { targets, findings }
}

/// The site's published origin, from `site/config.toml`: the one `base_url = "..."` line
/// Zola reads. Read here rather than restated, so a moved site moves every target.
pub fn site_base_url(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("site/config.toml")).ok()?;
    text.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("base_url")?.trim_start().strip_prefix('=')?.trim();
        let value = rest.trim_matches('"').trim_matches('\'');
        (!value.is_empty()).then(|| value.trim_end_matches('/').to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> PlanFacts {
        PlanFacts {
            site_base_url: Some("https://example.test".into()),
            site_inputs: vec!["docs/**".into(), "site/**".into()],
            surface_inputs: vec!["apps/**".into()],
            latest_release: Some(Identity {
                commit: Some("abc".into()),
                version: Some("0.5.0".into()),
                tag: Some("v0.5.0".into()),
            }),
            applications: vec![
                ApplicationFact { id: "declared-only".into(), status: "declared".into(), url: None, inputs: vec!["apps/**".into()] },
                ApplicationFact { id: "live".into(), status: "active".into(), url: Some("https://app.test".into()), inputs: vec!["apps/**".into()] },
            ],
            expected_commit: Some("def".into()),
            declared_version: Some("0.6.0".into()),
        }
    }

    #[test]
    fn a_documentation_change_reaches_the_site_and_not_the_release() {
        let p = plan(&facts(), &["docs/CLI.md".into()], false);
        let by = |id: &str| p.targets.iter().find(|t| t.id == id).unwrap().clone();
        assert!(by("pages").applicable);
        assert_eq!(by("pages").because, ["docs/CLI.md"]);
        assert!(!by("release").applicable);
        assert!(by("release").reason.contains("touches nothing"));
        assert_eq!(
            by("pages").identity_url.as_deref(),
            Some("https://example.test/build.json")
        );
    }

    #[test]
    fn a_crate_change_reaches_the_release_and_the_expected_identity_is_the_newest_record() {
        let p = plan(&facts(), &["apps/majordomus-cli/src/lib.rs".into()], false);
        let r = p.targets.iter().find(|t| t.id == "release").unwrap();
        assert!(r.applicable);
        assert_eq!(r.expected.tag.as_deref(), Some("v0.5.0"));
    }

    #[test]
    fn a_declared_deployment_is_in_the_plan_and_not_applicable_with_the_reason() {
        let p = plan(&facts(), &[], true);
        let d = p.targets.iter().find(|t| t.id == "declared-only").unwrap();
        assert!(!d.applicable);
        assert!(d.reason.contains("`declared`"));
        let live = p.targets.iter().find(|t| t.id == "live").unwrap();
        assert!(live.applicable);
        assert_eq!(
            live.identity_url.as_deref(),
            Some("https://app.test/api/v1/distribution/build")
        );
        assert_eq!(live.expected.commit.as_deref(), Some("def"));
    }

    #[test]
    fn an_active_deployment_is_reached_only_by_a_change_to_what_its_image_is_built_from() {
        let docs = plan(&facts(), &["docs/x.md".into()], false);
        let live = docs.targets.iter().find(|t| t.id == "live").unwrap();
        assert!(!live.applicable);
        assert!(live.reason.contains("touches nothing"));
        let code = plan(&facts(), &["apps/majordomus-cli/src/lib.rs".into()], false);
        assert!(code.targets.iter().find(|t| t.id == "live").unwrap().applicable);
    }

    #[test]
    fn everything_asks_every_published_surface_and_the_order_is_stable() {
        let p = plan(&facts(), &[], true);
        let ids: Vec<&str> = p.targets.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["pages", "release", "declared-only", "live"]);
        assert_eq!(p.applicable().count(), 3);
    }

    #[test]
    fn a_site_without_an_origin_is_a_finding_not_a_target() {
        let mut f = facts();
        f.site_base_url = None;
        let p = plan(&f, &["docs/x.md".into()], false);
        assert!(p.targets.iter().all(|t| t.kind == TargetKind::Application));
        assert!(p.findings.iter().any(|f| f.contains("base_url")));
    }

    #[test]
    fn the_base_url_is_read_from_the_site_configuration() {
        let dir = std::env::temp_dir().join(format!("mj-targets-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("site")).unwrap();
        std::fs::write(
            dir.join("site/config.toml"),
            "title = \"x\"\nbase_url = \"https://majordomus.test/\"\n",
        )
        .unwrap();
        assert_eq!(site_base_url(&dir).as_deref(), Some("https://majordomus.test"));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(site_base_url(&dir), None);
    }
}
