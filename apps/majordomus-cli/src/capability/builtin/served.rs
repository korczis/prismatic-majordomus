//! The `served` module: whether a deployment serves the commit it was meant to, observed
//! from outside and recorded as evidence a later reader can re-judge.
//!
//! Three operations, deliberately unequal:
//!
//! - `served.observe` reaches the network and appends a record, so it is a command of the
//!   trusted command line only. A browser or an MCP client that could make this process
//!   probe an arbitrary URL would have been handed a request forger.
//! - `served.forge` asks the forge which deployments and releases it recorded, through the
//!   forge's own tool, and keeps the answer for `deploy.observed` to read. It reaches the
//!   network and spends the caller's credential, so it too is a command line and nothing
//!   else.
//! - `served.show` is a read: it asks no network, writes nothing and re-judges what was
//!   recorded against the commit asked about, which is what makes an old observation go
//!   stale without anyone deciding it has (see [`crate::served`]).
//!
//! ```
//! use majordomus_cli::capability::builtin::served;
//! use majordomus_cli::capability::model::CapabilityKind;
//!
//! let m = served::module();
//! let observe = m.capabilities.iter().find(|c| c.capability.id.as_str() == "served.observe").unwrap();
//! // the operation that reaches the network is on no surface a client can call
//! assert_eq!(observe.capability.kind, CapabilityKind::Command);
//! assert!(observe.capability.exposure.http.is_none());
//! assert!(observe.capability.exposure.mcp.is_none());
//!
//! let show = m.capabilities.iter().find(|c| c.capability.id.as_str() == "served.show").unwrap();
//! assert_eq!(show.capability.exposure.http.as_ref().unwrap().path, "/api/v1/served");
//! ```

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

/// The input of `served.observe`: which deployment, where it is, which commit is expected
/// and how long the probe may take. Everything is optional — the defaults are this
/// repository's own site, its `build.json` and `HEAD`.
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
    /// The base URL; when absent, the `provider.url` of the deployment object `deployment`
    /// names, else `base_url` of `site/config.toml` (I3822).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The identity under the base; when absent, the `health.liveness` route of the
    /// deployment object `deployment` names (which answers `commit` and `dirty`), else
    /// `build.json`.
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

/// What `served.observe` did: the observation, where it was recorded, and the exit status
/// the command line reports the verdict with.
///
/// ```
/// use majordomus_cli::capability::builtin::served::Observed;
/// // the shape a caller reads: a dry run judges and records nothing
/// let json = r#"{"observation":{"schema":"majordomus.served-observation/v1",
///   "deployment":"pages","url":"https://majordomus.dev/build.json",
///   "expected":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","verdict":"unreachable",
///   "reason":"nothing was received","at":"1970-01-01T00:00:00Z"},"exit_code":12}"#;
/// let got: Observed = serde_json::from_str(json).unwrap();
/// assert_eq!(got.exit_code, 12);
/// assert!(got.recorded.is_none());
/// ```
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

/// What `served.forge` came to: whether the forge answered, and what its answer held. The
/// answer itself is read through `deploy.observed`; this is the receipt of having asked.
///
/// ```
/// use majordomus_cli::capability::builtin::served::ForgeObserved;
/// // a forge that could not be asked is said, with the reason, and is exit 12: not answered
/// let json = r#"{"observed":false,"reason":"gh repo view failed: no network",
///   "deployments":0,"releases":0,"environments":0,"exit_code":12}"#;
/// let got: ForgeObserved = serde_json::from_str(json).unwrap();
/// assert!(!got.observed);
/// assert_eq!(got.exit_code, 12);
/// assert!(got.repository.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ForgeObserved {
    /// Whether the forge answered and the answer was recorded.
    pub observed: bool,
    /// Why not, in the words of whatever refused; absent when it answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// `owner/name`, as the forge named the repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// When the forge was asked, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<String>,
    /// How many deployments the answer held.
    pub deployments: usize,
    /// How many releases the answer held.
    pub releases: usize,
    /// How many environments those deployments went to.
    pub environments: usize,
    /// The exit status the command line reports: 0 observed, 12 not answered.
    pub exit_code: i32,
}

/// The input of `served.show`: the commit every record is re-judged against, and
/// optionally a single deployment. `HEAD` and every deployment when absent.
///
/// ```
/// use majordomus_cli::capability::builtin::served::ShowInput;
/// let input: ShowInput = serde_json::from_str(r#"{"deployment":"pages"}"#).unwrap();
/// assert_eq!(input.deployment.as_deref(), Some("pages"));
/// assert!(input.commit.is_none(), "absent means HEAD");
/// // a field this operation does not have is refused rather than ignored
/// assert!(serde_json::from_str::<ShowInput>(r#"{"url":"https://x"}"#).is_err());
/// ```
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

/// One deployment's newest record and what it proves about the commit asked. Both are
/// carried: the recorded judgement is what was seen, `now` is what it still proves.
///
/// ```
/// use majordomus_cli::capability::builtin::served::DeploymentStanding;
/// use majordomus_cli::served::ServedVerdict;
/// let json = r#"{"deployment":"pages","observation":{"schema":"majordomus.served-observation/v1",
///   "deployment":"pages","url":"https://majordomus.dev/build.json",
///   "expected":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","verdict":"served",
///   "reason":"the deployment serves aaaaaaaaaaaa","at":"1970-01-01T00:00:00Z"},
///   "now":{"verdict":"stale","reason":"it does not contain bbbbbbbbbbbb"}}"#;
/// let s: DeploymentStanding = serde_json::from_str(json).unwrap();
/// // seen as served, and no longer proving the commit asked about
/// assert_eq!(s.observation.judgement.verdict, ServedVerdict::Served);
/// assert_eq!(s.now.verdict, ServedVerdict::Stale);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeploymentStanding {
    /// The deployment name.
    pub deployment: String,
    /// The newest record of it.
    pub observation: Observation,
    /// That record re-judged against the commit asked about. This, not the recorded
    /// judgement, is what a reader acts on.
    pub now: Judgement,
    /// For a deployment the layer declares as an object: `active` while `now` is served,
    /// `declared` otherwise, `retired` when the object says so. Derived from the newest
    /// observation, never set by hand (I3822); absent for a name no object carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<crate::deploy::Status>,
}

/// Every deployment's standing against one commit: the answer `served.show` gives on the
/// command line, over HTTP and to an MCP client, from one derivation.
///
/// ```
/// use majordomus_cli::capability::builtin::served::{standing, ServedStanding};
/// use majordomus_cli::served::{Ancestry, OBSERVATIONS_PATH};
/// # struct A; impl Ancestry for A {
/// #     fn has(&self, _: &str) -> Option<bool> { Some(true) }
/// #     fn contains(&self, _: &str, _: &str) -> Option<bool> { Some(true) } }
/// let dir = tempfile::tempdir().unwrap();
/// let s: ServedStanding = standing(dir.path(), &"a".repeat(40), None, &A);
/// assert_eq!(s.observations, OBSERVATIONS_PATH);
/// assert!(s.deployments.is_empty(), "nothing observed yet");
/// assert_eq!(s.unreadable, 0);
/// ```
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

/// The deployment objects the layer declares, by id. One that does not parse is left out
/// here: `deploy.check` is where it is refused, with the key and the correction.
fn declared_deployments(ctx: &Context) -> Vec<crate::deploy::Deployment> {
    ctx.index
        .objects
        .iter()
        .filter(|o| o.kind == crate::deploy::KIND)
        .filter_map(|o| crate::deploy::Deployment::parse(o).ok())
        .collect()
}

/// Where a deployment object says it answers: its provider's URL and its liveness route,
/// which reports the commit the running executable was built from. `None` for an object
/// that names no provider, whose address is the platform's to know.
fn provider_address(deployment: &crate::deploy::Deployment) -> Option<(String, String)> {
    let provider = deployment.provider.as_ref()?;
    let route = deployment.health.liveness.as_str();
    Some((
        provider.url.clone(),
        route.trim_start_matches('/').to_string(),
    ))
}

/// The status a deployment object stands at, judged from what it was last seen serving:
/// `active` only while that is the commit asked about, and a retired object stays retired.
fn derived_status(
    declared: Option<crate::deploy::Status>,
    now: &Judgement,
) -> crate::deploy::Status {
    use crate::deploy::Status;
    match declared {
        Some(Status::Retired) => Status::Retired,
        _ if now.verdict.passes() => Status::Active,
        _ => Status::Declared,
    }
}

fn observe(ctx: &Context, input: ObserveInput) -> Result<Observed, CapabilityError> {
    let root = root_of(ctx);
    let expected = resolve(&root, input.commit.as_deref().unwrap_or("HEAD"))?;
    let object = input.deployment.as_deref().and_then(|id| {
        declared_deployments(ctx)
            .into_iter()
            .find(|d| d.id == id)
            .and_then(|d| provider_address(&d))
    });
    let base = match (input.url, &object) {
        (Some(u), _) => u,
        (None, Some((url, _))) => url.clone(),
        (None, None) => configured_base(&root).ok_or_else(|| {
            CapabilityError::InvalidInput(
                "no `url` was given, no deployment object with a provider is named, and site/config.toml names no base_url".into(),
            )
        })?,
    };
    served::check_base(&base).map_err(CapabilityError::InvalidInput)?;
    let identity = match (input.identity, object) {
        (Some(i), _) => i,
        (None, Some((_, route))) => route,
        (None, None) => served::DEFAULT_IDENTITY.into(),
    };
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

fn forge(ctx: &Context, _: super::Empty) -> Result<ForgeObserved, CapabilityError> {
    Ok(forge_with(crate::deploy::observed::GH, &root_of(ctx)))
}

/// Ask the forge through `program`, record what it said, and say what that came to. A forge
/// that could not be asked is an answer of its own, never an error: the caller is a job that
/// goes on to read whatever was last recorded.
fn forge_with(program: &str, root: &Path) -> ForgeObserved {
    match crate::deploy::observed::refresh_with(program, root) {
        Ok(o) => ForgeObserved {
            observed: true,
            reason: None,
            environments: crate::deploy::observed::environments(&o).len(),
            deployments: o.deployments.len(),
            releases: o.releases.len(),
            repository: Some(o.repository),
            observed_at: Some(o.observed_at),
            exit_code: 0,
        },
        Err(reason) => ForgeObserved {
            observed: false,
            reason: Some(reason),
            repository: None,
            observed_at: None,
            deployments: 0,
            releases: 0,
            environments: 0,
            exit_code: 12,
        },
    }
}

fn show(ctx: &Context, input: ShowInput) -> Result<ServedStanding, CapabilityError> {
    let root = root_of(ctx);
    let expected = resolve(&root, input.commit.as_deref().unwrap_or("HEAD"))?;
    let mut answer = standing(
        &root,
        &expected,
        input.deployment.as_deref(),
        &GitAncestry { root: &root },
    );
    let objects = declared_deployments(ctx);
    for d in &mut answer.deployments {
        if let Some(object) = objects.iter().find(|o| o.id == d.deployment) {
            d.status = Some(derived_status(object.status, &d.now));
        }
    }
    Ok(answer)
}

/// The standing of every recorded deployment against `expected`. Public so that a consumer
/// that judges an intent's deployment criterion reads the same derivation as this read.
///
/// ```
/// use majordomus_cli::capability::builtin::served::standing;
/// use majordomus_cli::served::{append, observe, Ancestry, Fetch, Fetched, ServedVerdict};
/// use std::time::SystemTime;
/// struct Linear;  // a <- b
/// impl Ancestry for Linear {
///     fn has(&self, c: &str) -> Option<bool> { Some(c.starts_with('a') || c.starts_with('b')) }
///     fn contains(&self, d: &str, a: &str) -> Option<bool> { Some(d >= a) }
/// }
/// struct Serves(String);
/// impl Fetch for Serves {
///     fn fetch(&self, _: &str) -> Fetched {
///         Fetched::Body(format!(r#"{{"commit":"{}","dirty":false}}"#, self.0).into_bytes())
///     }
/// }
/// let (a, b) = ("a".repeat(40), "b".repeat(40));
/// let dir = tempfile::tempdir().unwrap();
/// let o = observe("pages", "https://s", "build.json", &a, &Serves(a.clone()), &Linear,
///                 SystemTime::UNIX_EPOCH);
/// append(dir.path(), &o).unwrap();
///
/// // the record still proves the commit it was taken against ...
/// let now = standing(dir.path(), &a, None, &Linear);
/// assert_eq!(now.deployments[0].now.verdict, ServedVerdict::Served);
/// // ... and stops proving once the commit asked about moves past it
/// let later = standing(dir.path(), &b, None, &Linear);
/// assert_eq!(later.deployments[0].now.verdict, ServedVerdict::Stale);
/// ```
pub fn standing(
    root: &Path,
    expected: &str,
    only: Option<&str>,
    git: &dyn served::Ancestry,
) -> ServedStanding {
    let (all, unreadable) = served::read_all(root);
    let mut names: Vec<&str> = all.iter().map(|o| o.deployment.as_str()).collect();
    // the canonical order every other collection is shown in; natural_cmp is total, so
    // equal names are adjacent and dedup still removes every repeat
    crate::order::canonical_strings(&mut names);
    names.dedup();
    let deployments = names
        .into_iter()
        .filter(|n| only.is_none_or(|o| o == *n))
        .filter_map(|n| served::latest(&all, n))
        .map(|o| DeploymentStanding {
            deployment: o.deployment.clone(),
            now: o.rejudge(expected, git),
            observation: o.clone(),
            status: None,
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

/// The module: `served.observe` and `served.forge` (commands of the trusted command line)
/// and `served.show` (a read on every surface).
///
/// ```
/// use majordomus_cli::capability::builtin::served;
/// let ids: Vec<_> = served::module().capabilities.iter()
///     .map(|c| c.capability.id.as_str().to_string()).collect();
/// assert_eq!(ids, vec!["served.observe", "served.forge", "served.show"]);
/// ```
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
                description: "Fetches the build identity a deployment serves (one bounded probe, no shell): for a `deployment` naming an object with a provider, its liveness route at the provider's URL; otherwise `build.json` under the configured base URL. It judges it against the expected commit (HEAD unless named) by containment and appends the observation to the checkout-local record. Exit 0 when the deployment serves a build containing the commit, 10 when it measurably does not (behind, or built dirty), 12 when the question could not be answered (unreachable, unreadable, or a served commit this clone lacks).",
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
                id: "served.forge",
                kind: CapabilityKind::Command,
                title: "Ask the forge what it recorded as deployed and released",
                description: "Asks the forge, through its own command-line tool and with the caller's credential, for the newest deployments to every environment and the newest releases, in one query, and records the answer in the checkout-local state `deploy.observed` reads. Exit 0 when the forge answered and the answer was recorded, 12 when it could not be asked (no tool, no credential, no network, a repository the forge does not know): what was recorded before then stands, with its age.",
                input: super::Empty,
                output: ForgeObserved,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["served".into(), "forge".into()] }),
                },
                tags: ["deployment", "evidence", "forge"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: forge,
            },
            capability! {
                id: "served.show",
                title: "What each deployment was last seen serving",
                description: "The newest recorded observation of each deployment, re-judged against a commit (HEAD unless named), and for a deployment the layer declares as an object its status derived from that: `active` only while it serves the commit. A record keeps proving every commit its served build contains and stops proving the moment the commit asked about is not among them; a record that received nothing proves nothing. Asks no network and writes nothing.",
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

    /// Asking the forge is a receipt either way: what it held when it answered, and the
    /// reason when it could not be asked, which is exit 12 and never an error.
    #[cfg(unix)]
    #[test]
    fn asking_the_forge_says_what_it_held_or_why_it_could_not_be_asked() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("fake-gh");
        std::fs::write(
            &program,
            "#!/bin/sh\ncase \"$1\" in\n  repo) echo '{\"nameWithOwner\": \"o/r\"}' ;;\n  api) cat \"$(dirname \"$0\")/answer.json\" ;;\nesac\n",
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(
            dir.path().join("answer.json"),
            serde_json::json!({"data": {"repository": {
                "url": "https://forge.example/o/r",
                "deployments": {"nodes": [
                    {"databaseId": 2, "environment": "github-pages", "createdAt": "2026-10-12T21:00:00Z",
                     "commitOid": A, "latestStatus": {"state": "SUCCESS"}},
                    {"databaseId": 1, "environment": "preview", "createdAt": "2026-10-11T21:00:00Z",
                     "commitOid": B, "latestStatus": {"state": "SUCCESS"}}
                ]},
                "releases": {"nodes": [{"tagName": "v1.0.0", "name": "1.0.0",
                     "publishedAt": "2026-10-10T00:00:00Z", "url": "https://forge.example/o/r/releases/tag/v1.0.0"}]}
            }}})
            .to_string(),
        )
        .unwrap();
        let root = tempfile::tempdir().unwrap();
        let asked = forge_with(&program.to_string_lossy(), root.path());
        assert!(asked.observed, "{asked:?}");
        assert_eq!(asked.repository.as_deref(), Some("o/r"));
        assert_eq!(
            (
                asked.deployments,
                asked.environments,
                asked.releases,
                asked.exit_code
            ),
            (2, 2, 1, 0)
        );
        assert!(asked.observed_at.is_some() && asked.reason.is_none());
        // recorded: the read every surface has answers from it
        assert!(crate::deploy::observed::report(root.path()).observed);

        let refused = forge_with("/no/such/program", root.path());
        assert!(!refused.observed);
        assert_eq!(refused.exit_code, 12);
        assert!(refused.reason.unwrap().contains("could not run"));
        // and what was recorded before stands
        assert!(crate::deploy::observed::report(root.path()).observed);
    }

    /// The capability itself, in a repository the forge does not know: the same receipt.
    #[test]
    fn the_capability_answers_a_receipt_where_the_forge_knows_nothing() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let asked = forge(&ctx, super::super::Empty {}).unwrap();
        assert!(!asked.observed, "{asked:?}");
        assert_eq!(asked.exit_code, 12);
        assert!(asked.reason.is_some());
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
        let forge = &by_id("served.forge").capability;
        assert_eq!(forge.kind, CapabilityKind::Command);
        assert!(forge.exposure.http.is_none() && forge.exposure.mcp.is_none());
        assert_eq!(
            forge.exposure.cli.as_ref().unwrap().path,
            ["served", "forge"]
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

    /// The deployments are one collection with one order: the canonical one, which reads
    /// `mirror-10` after `mirror-2`, where a byte sort puts it first.
    #[test]
    fn deployments_are_listed_once_each_in_canonical_order() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["pages", "mirror-10", "mirror-2", "mirror-2"] {
            served::append(dir.path(), &record(name, A, ServedVerdict::Served)).unwrap();
        }
        let s = standing(dir.path(), A, None, &Linear);
        let names: Vec<_> = s
            .deployments
            .iter()
            .map(|d| d.deployment.as_str())
            .collect();
        assert_eq!(names, ["mirror-2", "mirror-10", "pages"]);
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

    fn deployment(provider: Option<serde_json::Value>) -> crate::deploy::Deployment {
        let mut v = serde_json::json!({
            "schema": "deployment/v1", "kind": "deployment", "id": "hosted",
            "title": "The hosted deployment", "application": "hosted",
            "build": { "package": "majordomus-cli", "binary": "majordomus",
                       "profile": "release", "inputs": ["apps/majordomus-cli"] },
            "listen": { "port": 8080, "interface": "all" },
            "health": { "liveness": "/api/v1/live", "readiness": "/api/v1/ready" },
            "resources": { "cpu_kind": "shared", "cpus": 1, "memory_mb": 256 },
            "machines": { "count": 1, "min_running": 0, "autostart": true, "autostop": true },
            "region": "fra",
        });
        if let Some(p) = provider {
            v["provider"] = p;
        }
        serde_json::from_value(v).expect("a deployment")
    }

    /// I3822: an object that names a provider is observed where it says it answers, at the
    /// route that reports its commit; one that names none has no address of its own.
    #[test]
    fn a_provider_deployment_is_addressed_by_its_object() {
        let hosted = deployment(Some(serde_json::json!({
            "name": "gigalixir", "app": "hosted-app", "stack": "gigalixir-24",
            "url": "https://hosted-app.gigalixirapp.com",
        })));
        let (base, identity) = provider_address(&hosted).expect("an address");
        assert_eq!(base, "https://hosted-app.gigalixirapp.com");
        assert_eq!(identity, "api/v1/live");
        assert_eq!(
            served::identity_url(&base, &identity),
            "https://hosted-app.gigalixirapp.com/api/v1/live"
        );
        assert_eq!(provider_address(&deployment(None)), None);
    }

    /// I3822: the status is the newest observation's, re-judged; it is active only while
    /// that serves the commit, and a retired object is never revived by an observation.
    #[test]
    fn the_status_is_active_only_while_the_commit_is_served() {
        use crate::deploy::Status;
        let judged = |verdict| Judgement {
            verdict,
            reason: String::new(),
        };
        let served = judged(ServedVerdict::Served);
        assert_eq!(
            derived_status(Some(Status::Declared), &served),
            Status::Active
        );
        assert_eq!(derived_status(None, &served), Status::Active);
        for v in [
            ServedVerdict::Stale,
            ServedVerdict::Unreachable,
            ServedVerdict::Malformed,
            ServedVerdict::Dirty,
        ] {
            assert_eq!(
                derived_status(Some(Status::Active), &judged(v)),
                Status::Declared,
                "{v:?}"
            );
        }
        assert_eq!(
            derived_status(Some(Status::Retired), &served),
            Status::Retired
        );
    }

    /// A committed layer that declares the deployment `hosted`, answering at `url`.
    fn hosted_at(url: &str) -> crate::synthetic::SyntheticRepository {
        use crate::deploy::slug::testing::{commit, init, object_yaml, write_object};
        let repo = crate::synthetic::SyntheticRepository::small().unwrap();
        init(repo.root(), &[]);
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).unwrap();
        text.push_str(
            "  - id: deployment\n    kind: deployment\n    discovery: vcs\n    \
             pathspec: ':(glob).ai/repo/deployments/*.yaml'\n    required: false\n",
        );
        std::fs::write(&sources, text).unwrap();
        let yaml = object_yaml("hosted", &["docs"], true)
            .replace("https://hosted-app.gigalixirapp.com", url);
        write_object(repo.root(), ".ai/repo/deployments/hosted.yaml", &yaml);
        commit(repo.root(), "the layer");
        repo
    }

    /// A loopback port nothing listens on: bound by the system's choice, then let go.
    fn closed_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    /// I3822: a named deployment object is observed where it says it answers, at the route
    /// that reports its commit, unless the caller names the address and the identity; with
    /// no address from anywhere, the refusal names every place one could have come from.
    #[test]
    fn an_observation_is_addressed_by_its_object_unless_the_caller_names_it() {
        let base = format!("https://127.0.0.1:{}", closed_port());
        let repo = hosted_at(&base);
        let ctx = repo.context().unwrap();
        let input =
            |url: Option<&str>, identity: Option<&str>, deployment: Option<&str>| ObserveInput {
                deployment: deployment.map(str::to_string),
                url: url.map(str::to_string),
                identity: identity.map(str::to_string),
                commit: None,
                timeout_seconds: Some(1),
                dry_run: true,
            };

        let by_object = observe(&ctx, input(None, None, Some("hosted"))).unwrap();
        assert_eq!(by_object.observation.url, format!("{base}/api/v1/live"));
        assert_eq!(by_object.observation.deployment, "hosted");
        assert_eq!(
            by_object.observation.judgement.verdict,
            ServedVerdict::Unreachable
        );
        assert_eq!(by_object.recorded, None, "a dry run records nothing");

        let elsewhere = format!("https://127.0.0.1:{}", closed_port());
        let named = observe(
            &ctx,
            input(Some(&elsewhere), Some("build.json"), Some("hosted")),
        )
        .unwrap();
        assert_eq!(named.observation.url, format!("{elsewhere}/build.json"));

        match observe(&ctx, input(None, None, None)) {
            Err(CapabilityError::InvalidInput(m)) => {
                assert!(m.contains("no deployment object with a provider"), "{m}");
                assert!(m.contains("site/config.toml names no base_url"), "{m}");
            }
            other => panic!("an observation with no address was made: {other:?}"),
        }
    }

    /// I3822: the standing of a deployment the layer declares carries the status its newest
    /// observation derives, and a name no object carries has none.
    #[test]
    fn a_declared_deployment_stands_at_the_status_its_observation_derives() {
        use crate::deploy::Status;
        let repo = hosted_at(&format!("https://127.0.0.1:{}", closed_port()));
        let ctx = repo.context().unwrap();
        let head = resolve(repo.root(), "HEAD").unwrap();
        for name in ["hosted", "pages"] {
            served::append(repo.root(), &record(name, &head, ServedVerdict::Served)).unwrap();
        }
        let s = show(
            &ctx,
            ShowInput {
                commit: None,
                deployment: None,
            },
        )
        .unwrap();
        let standing: Vec<_> = s
            .deployments
            .iter()
            .map(|d| (d.deployment.as_str(), d.status))
            .collect();
        assert_eq!(
            standing,
            [("hosted", Some(Status::Active)), ("pages", None)]
        );
    }

    /// A process that cannot tell how it was built answers `dirty: null`; that proves no
    /// commit, and the reason says so rather than calling the value not a boolean.
    #[test]
    fn an_unknown_dirtiness_proves_no_commit_and_says_why() {
        let body = format!(r#"{{"alive":true,"commit":"{A}","dirty":null}}"#);
        let why = BuildIdentity::parse(body.as_bytes()).unwrap_err().0;
        assert!(why.contains("dirty: null"), "{why}");
    }
}
