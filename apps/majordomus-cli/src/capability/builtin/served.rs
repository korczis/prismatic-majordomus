//! The `served` module: whether a deployment serves the commit it was meant to, observed
//! from outside and recorded as evidence a later reader can re-judge.
//!
//! Two operations, deliberately unequal:
//!
//! - `served.observe` reaches the network and appends a record, so it is a command of the
//!   trusted command line only. A browser or an MCP client that could make this process
//!   probe an arbitrary URL would have been handed a request forger.
//! - `served.show` is a read: it asks no network, writes nothing and re-judges what was
//!   recorded against the commit asked about, which is what makes an old observation go
//!   stale without anyone deciding it has (see [`crate::served`]).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

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
use crate::served::{self, Curl, GitAncestry, Judgement, Observation};
use crate::{capability, module};

use super::get;

/// The URI under which the recorded standing of every deployment is read as a resource.
pub const SERVED_URI: &str = "majordomus://served";

/// The deployment name an observation carries when the caller names none: the public site.
pub const DEFAULT_DEPLOYMENT: &str = "pages";

/// The input of `served.observe`.
///
/// ```
/// use majordomus_cli::capability::builtin::served::ObserveInput;
/// let input: ObserveInput = serde_json::from_str(r#"{"commit":"HEAD"}"#).unwrap();
/// assert_eq!(input.commit.as_deref(), Some("HEAD"));
/// assert!(serde_json::from_str::<ObserveInput>(r#"{"shell":"rm"}"#).is_err());
/// ```
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObserveInput {
    /// The name the observation is recorded under; `pages` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<String>,
    /// The site's base URL; `base_url` of `site/config.toml` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The identity file under the base; `build.json` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    /// The commit expected to be served, any revision git resolves; `HEAD` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// The bound on the probe in seconds; ten when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    /// Judge without recording. Recording is the default, because an observation nobody
    /// kept is a measurement nobody can read.
    #[serde(default)]
    pub dry_run: bool,
}

/// What `served.observe` did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Observed {
    /// The observation, with its judgement.
    pub observation: Observation,
    /// Where it was recorded, repository-relative; absent on a dry run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded: Option<String>,
    /// The exit status the command line reports: 0 served, 10 a measured no, 12 unanswered.
    pub exit_code: i32,
}

/// The input of `served.show`.
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShowInput {
    /// The commit each record is re-judged against, any revision git resolves; `HEAD` when
    /// absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Only this deployment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment: Option<String>,
}

/// One deployment's newest record and what it proves about the commit asked.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentStanding {
    /// The deployment name.
    pub deployment: String,
    /// The newest record of it.
    pub observation: Observation,
    /// That record re-judged against the commit asked about. This, not the recorded
    /// judgement, is what a reader acts on.
    pub now: Judgement,
}

/// Every deployment's standing against one commit.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ServedStanding {
    /// The full commit every record was re-judged against.
    pub expected: String,
    /// The checkout-local file the records were read from.
    pub observations: String,
    /// Lines of that file that were not readable observations.
    pub unreadable: usize,
    /// One entry per deployment observed, in name order.
    pub deployments: Vec<DeploymentStanding>,
}

fn root_of(ctx: &Context) -> PathBuf {
    PathBuf::from(&ctx.index.repository.root)
}

/// Resolves `rev` to a full commit sha, refusing what git does not resolve to a commit.
fn resolve(root: &Path, rev: &str) -> Result<String, CapabilityError> {
    if rev.starts_with('-') {
        return Err(CapabilityError::InvalidInput(format!(
            "`{rev}` is not a revision"
        )));
    }
    let out = crate::git::read_only(root)
        .args(["rev-parse", "--verify", "--quiet", "--end-of-options"])
        .arg(format!("{rev}^{{commit}}"))
        .output()
        .map_err(|e| CapabilityError::Internal(format!("cannot run git: {e}")))?;
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || sha.len() != 40 {
        return Err(CapabilityError::InvalidInput(format!(
            "`{rev}` does not name a commit this clone holds"
        )));
    }
    Ok(sha)
}

/// The `base_url` of the site configuration, when the checkout has one.
fn configured_base(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join("site/config.toml")).ok()?;
    text.lines().find_map(|l| {
        let rest = l
            .trim()
            .strip_prefix("base_url")?
            .trim_start()
            .strip_prefix('=')?;
        Some(rest.trim().trim_matches('"').to_string())
    })
}

fn observe(ctx: &Context, input: ObserveInput) -> Result<Observed, CapabilityError> {
    let root = root_of(ctx);
    let expected = resolve(&root, input.commit.as_deref().unwrap_or("HEAD"))?;
    let base = match input.url {
        Some(u) => u,
        None => configured_base(&root).ok_or_else(|| {
            CapabilityError::InvalidInput(
                "no `url` was given and site/config.toml names no base_url".into(),
            )
        })?,
    };
    served::check_base(&base).map_err(CapabilityError::InvalidInput)?;
    let identity = input
        .identity
        .unwrap_or_else(|| served::DEFAULT_IDENTITY.into());
    if identity.contains("..") || identity.contains('?') || identity.contains('#') {
        return Err(CapabilityError::InvalidInput(format!(
            "`{identity}` is not a plain path under the site"
        )));
    }
    let deployment = input
        .deployment
        .unwrap_or_else(|| DEFAULT_DEPLOYMENT.into());
    let fetch = Curl {
        max_seconds: input
            .timeout_seconds
            .unwrap_or(served::DEFAULT_PROBE_SECONDS)
            .max(1),
    };
    let obs = served::observe(
        &deployment,
        &base,
        &identity,
        &expected,
        &fetch,
        &GitAncestry { root: &root },
        SystemTime::now(),
    );
    let recorded = if input.dry_run {
        None
    } else {
        served::append(&root, &obs).map_err(|e| {
            CapabilityError::Internal(format!("cannot record the observation: {e}"))
        })?;
        Some(served::OBSERVATIONS_PATH.to_string())
    };
    Ok(Observed {
        exit_code: obs.judgement.verdict.exit_code(),
        observation: obs,
        recorded,
    })
}

fn show(ctx: &Context, input: ShowInput) -> Result<ServedStanding, CapabilityError> {
    let root = root_of(ctx);
    let expected = resolve(&root, input.commit.as_deref().unwrap_or("HEAD"))?;
    Ok(standing(
        &root,
        &expected,
        input.deployment.as_deref(),
        &GitAncestry { root: &root },
    ))
}

/// The standing of every recorded deployment against `expected`. Public so that a consumer
/// that judges an intent's deployment criterion reads the same derivation as this read.
pub fn standing(
    root: &Path,
    expected: &str,
    only: Option<&str>,
    git: &dyn served::Ancestry,
) -> ServedStanding {
    let (all, unreadable) = served::read_all(root);
    let mut names: Vec<&str> = all.iter().map(|o| o.deployment.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    let deployments = names
        .into_iter()
        .filter(|n| only.is_none_or(|o| o == *n))
        .filter_map(|n| served::latest(&all, n))
        .map(|o| DeploymentStanding {
            deployment: o.deployment.clone(),
            now: o.rejudge(expected, git),
            observation: o.clone(),
        })
        .collect();
    ServedStanding {
        expected: expected.into(),
        observations: served::OBSERVATIONS_PATH.into(),
        unreadable,
        deployments,
    }
}

impl BenchmarkCases for ObserveInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: the capability is waived because it probes the network; timing it in a
        // loop would measure a CDN, not this executable
        Vec::new()
    }
}

impl BenchmarkCases for ShowInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("head", ShowInput::default()),
            NamedCase::new(
                "pages-against-head",
                ShowInput {
                    commit: Some("HEAD".into()),
                    deployment: Some(DEFAULT_DEPLOYMENT.into()),
                },
            ),
        ]
    }
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "served",
        title: "Served deployments",
        description: "Whether a deployment serves the commit it was meant to: the build identity a site serves, read from outside the way a visitor reads it, judged by commit containment and recorded so that a later reader can re-judge it. Only `served` passes; a site that could not be reached, an identity that could not be read, a build from an uncommitted tree and a served commit this clone does not hold are each an unanswered question, never a yes. Observing reaches the network and is a command of the trusted command line; reading the recorded standing is a read on every surface.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "served.observe",
                kind: CapabilityKind::Command,
                title: "Observe what a deployment serves",
                description: "Fetches the build identity the site serves (`build.json` under the configured base URL, one bounded probe, no shell), judges it against the expected commit (HEAD unless named) by containment and appends the observation to the checkout-local record. Exit 0 when the deployment serves a build containing the commit, 10 when it measurably does not (behind, or built dirty), 12 when the question could not be answered (unreachable, unreadable, or a served commit this clone lacks).",
                input: ObserveInput,
                output: Observed,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["served".into(), "observe".into()] }),
                },
                tags: ["deployment", "evidence", "verification"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: observe,
            },
            capability! {
                id: "served.show",
                title: "What each deployment was last seen serving",
                description: "The newest recorded observation of each deployment, re-judged against a commit (HEAD unless named). A record keeps proving every commit its served build contains and stops proving the moment the commit asked about is not among them; a record that received nothing proves nothing. Asks no network and writes nothing.",
                input: ShowInput,
                output: ServedStanding,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_served".into()),
                        resource: Some(McpResource { uri: SERVED_URI.into(), name: "served".into() }),
                    }),
                    http: get("/api/v1/served"),
                    cli: Some(CliExposure { path: vec!["served".into(), "show".into()] }),
                },
                tags: ["deployment", "evidence", "verification"],
                cache: CachePolicy::Disabled,
                handler: show,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::model::HttpMethod;
    use crate::served::{BuildIdentity, ServedVerdict, OBSERVATION_SCHEMA};

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    struct Linear;
    impl served::Ancestry for Linear {
        fn has(&self, c: &str) -> Option<bool> {
            Some(c == A || c == B)
        }
        fn contains(&self, d: &str, a: &str) -> Option<bool> {
            Some(d == a || (d == B && a == A))
        }
    }

    fn record(deployment: &str, commit: &str, verdict: ServedVerdict) -> Observation {
        Observation {
            schema: OBSERVATION_SCHEMA.into(),
            deployment: deployment.into(),
            url: "https://s/build.json".into(),
            expected: commit.into(),
            served: (verdict == ServedVerdict::Served).then(|| BuildIdentity {
                commit: commit.into(),
                dirty: false,
                source_version: None,
                source_hash: None,
            }),
            judgement: Judgement {
                verdict,
                reason: "r".into(),
            },
            at: "1970-01-01T00:00:00Z".into(),
        }
    }

    /// The asymmetry is the security property: the operation that reaches the network is on
    /// no surface a browser or an MCP client can call.
    #[test]
    fn only_the_read_is_reachable_from_http_and_mcp() {
        let m = module();
        assert_eq!(m.id.as_str(), "served");
        let by_id = |id: &str| {
            m.capabilities
                .iter()
                .find(|c| c.capability.id.as_str() == id)
                .unwrap()
        };
        let observe = &by_id("served.observe").capability;
        assert_eq!(observe.kind, CapabilityKind::Command);
        assert!(observe.exposure.http.is_none() && observe.exposure.mcp.is_none());
        assert_eq!(
            observe.exposure.cli.as_ref().unwrap().path,
            ["served", "observe"]
        );
        let show = &by_id("served.show").capability;
        assert_eq!(show.kind, CapabilityKind::Query);
        assert_eq!(show.exposure.http.as_ref().unwrap().method, HttpMethod::Get);
        assert_eq!(show.exposure.http.as_ref().unwrap().path, "/api/v1/served");
        assert_eq!(
            show.exposure.mcp.as_ref().unwrap().tool.as_deref(),
            Some("majordomus_served")
        );
    }

    #[test]
    fn standing_rejudges_the_newest_record_of_each_deployment() {
        let dir = tempfile::tempdir().unwrap();
        for o in [
            record("pages", A, ServedVerdict::Served),
            record("fly", A, ServedVerdict::Unreachable),
            record("pages", B, ServedVerdict::Served),
        ] {
            served::append(dir.path(), &o).unwrap();
        }
        let s = standing(dir.path(), B, None, &Linear);
        let rows: Vec<_> = s
            .deployments
            .iter()
            .map(|d| {
                (
                    d.deployment.as_str(),
                    d.observation.expected.as_str(),
                    d.now.verdict,
                )
            })
            .collect();
        assert_eq!(
            rows,
            vec![
                ("fly", A, ServedVerdict::Unreachable),
                ("pages", B, ServedVerdict::Served)
            ]
        );
        assert_eq!(s.unreadable, 0);

        let older = standing(dir.path(), A, Some("pages"), &Linear);
        assert_eq!(older.deployments.len(), 1);
        assert_eq!(
            older.deployments[0].now.verdict,
            ServedVerdict::Served,
            "B contains A"
        );
    }

    #[test]
    fn a_served_record_is_stale_against_a_commit_it_does_not_contain() {
        let dir = tempfile::tempdir().unwrap();
        served::append(dir.path(), &record("pages", A, ServedVerdict::Served)).unwrap();
        let s = standing(dir.path(), B, None, &Linear);
        assert_eq!(
            s.deployments[0].observation.judgement.verdict,
            ServedVerdict::Served
        );
        assert_eq!(s.deployments[0].now.verdict, ServedVerdict::Stale);
    }

    #[test]
    fn revisions_resolve_to_full_commits_and_option_shaped_input_is_refused() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(resolve(root, "HEAD").unwrap().len(), 40);
        for bad in ["--output=/tmp/x", "no-such-revision-anywhere"] {
            assert!(
                matches!(resolve(root, bad), Err(CapabilityError::InvalidInput(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn the_configured_base_is_read_from_the_site_configuration() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(configured_base(dir.path()), None);
        std::fs::create_dir_all(dir.path().join("site")).unwrap();
        std::fs::write(
            dir.path().join("site/config.toml"),
            "title = \"x\"\nbase_url = \"https://example.test\"\n",
        )
        .unwrap();
        assert_eq!(
            configured_base(dir.path()).as_deref(),
            Some("https://example.test")
        );
    }
}
