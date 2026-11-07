//! The `self` module: an installation keeps itself at the published stable release, through
//! the one installer (ADR 0161, I2720).
//!
//! One operation, `self.update`, a command of the trusted command line only: `--check` reads
//! the published metadata and judges it against the running version, writing nothing;
//! `--apply` re-runs the published `install.sh` with the target's tag and this installation's
//! own prefix and launcher directory, then reads the installed version back from the launcher.
//! It reaches the network and, applied, replaces the executable that answers, so no browser
//! or MCP client is handed it. The decision and the installation's places are
//! [`crate::release::self_update`]; the download, verification and swap are the installer's,
//! and exist nowhere else.
//!
//! ```
//! use majordomus_cli::capability::builtin::self_update;
//! use majordomus_cli::capability::model::CapabilityKind;
//! let m = self_update::module();
//! let update = &m.capabilities[0].capability;
//! assert_eq!(update.id.as_str(), "self.update");
//! assert_eq!(update.kind, CapabilityKind::Command);
//! assert!(update.exposure.http.is_none() && update.exposure.mcp.is_none(), "the command line only");
//! assert_eq!(update.exposure.cli.as_ref().unwrap().path, vec!["self", "update"]);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{
    BenchmarkPolicy, CapabilityKind, CliExposure, Exposure, Stability, WaiverReason,
};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::release::self_update::{self, Applied, Decision, Installation, Verdict};
use crate::release::version::Version;
use crate::served::{Curl, Fetch, Fetched};
use crate::{capability, module};

/// The input of `self.update`: report (the default), or apply; the stable release unless a
/// version is named.
///
/// ```
/// use majordomus_cli::capability::builtin::self_update::UpdateInput;
/// let input: UpdateInput = serde_json::from_str(r#"{"apply":true,"version":"v1.0.0"}"#).unwrap();
/// assert!(input.apply && input.version.as_deref() == Some("v1.0.0"));
/// assert!(serde_json::from_str::<UpdateInput>(r#"{"prefix":"/tmp"}"#).is_err(),
///     "the places are the installation's own, never given");
/// ```
#[derive(Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateInput {
    /// Install the target. Without it the command reports and writes nothing.
    #[serde(default)]
    pub apply: bool,
    /// The release to move to (`v1.0.0` or `1.0.0`); the stable pointer when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The release base to read the metadata and the installer from; when absent,
    /// `MAJORDOMUS_RELEASE_BASE_URL`, else the installer base of share/distribution.yaml.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// The bound on each metadata read, in seconds; twenty when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
}

impl BenchmarkCases for UpdateInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // nothing: the capability is waived because it reads the published metadata over
        // the network; timing it in a loop would measure a CDN, not this executable
        Vec::new()
    }
}

/// What `self.update` found and did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateReport {
    /// The version this installation runs: its tree's `share/version.txt`, or the version
    /// this executable was built as when it is not an installed release.
    pub running: String,
    /// The version the stable pointer (`releases/latest.json`) names, when it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable: Option<String>,
    /// The version asked about: the one named, or the stable one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Where the metadata was read.
    pub metadata_url: String,
    /// The judgement: upgrade, current or refused, with its reason.
    pub decision: Decision,
    /// Whether `--apply` would install something.
    pub applies: bool,
    /// The installation this executable belongs to, when it is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation: Option<Installation>,
    /// Why this executable is not an installation `--apply` can update, when it is not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_installed: Option<String>,
    /// What running the installer came to, under `--apply`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied: Option<Applied>,
    /// The exit status the command line reports: 0 nothing to do, an upgrade reported or
    /// installed and read back; 10 refused, or an installer that failed; 12 not answered
    /// (the metadata could not be read, or this is no installation to apply to).
    pub exit_code: i32,
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "self",
        title: "Self-update",
        description: "An installation keeps itself at the published stable release through the one installer: the running version against the version the published metadata names, forward only and stable only, and an apply that re-runs install.sh with the installation's own prefix rather than a second implementation of download, verification and swap (ADR 0161).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "self.update",
                kind: CapabilityKind::Command,
                title: "Report or apply an upgrade to the stable release",
                description: "Reads the release metadata the installation's share/distribution.yaml names (releases/latest.json, or releases/<tag>.json for a named version; MAJORDOMUS_RELEASE_BASE_URL overrides the base as it does for install.sh) and judges it against the running version: an upgrade applies only to a newer, stable, listed release; a downgrade, a pre-release and an unlisted version are refused with the reason. Without --apply it writes nothing. With --apply it fetches install.sh from the same base and runs it with --version <tag>, --prefix and --install-dir of this installation, then reads the installed version back from the launcher. Exit 0 nothing to do or done, 10 refused or the installer failed, 12 not answered.",
                input: UpdateInput,
                output: UpdateReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: None,
                    http: None,
                    cli: Some(CliExposure { path: vec!["self".into(), "update".into()] }),
                },
                tags: ["distribution", "release", "install"],
                cache: CachePolicy::Disabled,
                benchmark: BenchmarkPolicy::Waived { reason: WaiverReason::ExternalDependency },
                handler: update,
            },
        ],
    }
}

/// The release base: the operator's override, as install.sh reads it, else the model's.
fn release_base(declared: &str) -> String {
    std::env::var("MAJORDOMUS_RELEASE_BASE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| declared.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// A base the metadata may be read from: https, or plain http only when the operator said so
/// with the installer's own switch (`MAJORDOMUS_INSECURE_BASE_URL=1`, meant for tests).
fn admit_base(base: &str) -> Result<(), String> {
    crate::served::check_base(base)?;
    if base.starts_with("http://") && std::env::var("MAJORDOMUS_INSECURE_BASE_URL").as_deref() != Ok("1")
    {
        return Err(format!(
            "{base} is plain http; set MAJORDOMUS_INSECURE_BASE_URL=1 as well if this is deliberate"
        ));
    }
    Ok(())
}

fn tag_of(word: &str) -> String {
    let w = word.trim();
    if w.starts_with('v') {
        w.to_string()
    } else {
        format!("v{w}")
    }
}

fn update(ctx: &Context, input: UpdateInput) -> Result<UpdateReport, CapabilityError> {
    let model = ctx.index.distribution.as_ref().ok_or_else(|| {
        CapabilityError::NotFound(
            "this installation carries no share/distribution.yaml, so it names no release metadata"
                .into(),
        )
    })?;
    let base = match input.base_url.as_deref() {
        Some(b) => b.trim_end_matches('/').to_string(),
        None => release_base(&model.installer.base_url),
    };
    admit_base(&base).map_err(CapabilityError::InvalidInput)?;

    let exe = std::env::current_exe().ok();
    let launcher = std::env::var_os("MAJORDOMUS_LAUNCHER").map(std::path::PathBuf::from);
    let located = exe
        .as_deref()
        .ok_or_else(|| "the path of this executable could not be read".to_string())
        .and_then(|e| Installation::locate(e, launcher.as_deref()));
    let running = match &located {
        Ok(at) => at.version.clone(),
        Err(_) => env!("CARGO_PKG_VERSION").to_string(),
    };
    let running_v = Version::parse(&running).ok_or_else(|| {
        CapabilityError::Internal(format!("the running version {running:?} is not a version"))
    })?;

    let fetch = Curl {
        max_seconds: input.timeout_seconds.unwrap_or(20),
    };
    let read = |url: &str| -> Result<Option<(String, String)>, String> {
        match fetch.fetch(url) {
            Fetched::Body(b) => {
                let v: serde_json::Value = serde_json::from_slice(&b)
                    .map_err(|e| format!("{url} is not release metadata: {e}"))?;
                let version = v["version"].as_str().unwrap_or_default().to_string();
                let tag = v["tag"].as_str().unwrap_or_default().to_string();
                if Version::parse(&version).is_none() || tag != format!("v{version}") {
                    return Err(format!("{url} names no usable version (version {version:?}, tag {tag:?})"));
                }
                Ok(Some((version, tag)))
            }
            // curl -f exits 22 on an HTTP error status: the project's answer that nothing is
            // published there, which is the unlisted refusal and not a failure to reach it
            Fetched::Unreachable(why) if why.contains("exited 22") => Ok(None),
            Fetched::Unreachable(why) => Err(why),
        }
    };

    let latest_url = format!("{base}/releases/latest.json");
    let unanswered = |metadata_url: String, reason: String| UpdateReport {
        running: running.clone(),
        stable: None,
        target: None,
        metadata_url,
        decision: Decision {
            verdict: Verdict::Unanswered,
            refusal: None,
            reason,
        },
        applies: false,
        installation: located.clone().ok(),
        not_installed: located.clone().err(),
        applied: None,
        exit_code: 12,
    };
    let stable = match read(&latest_url) {
        Ok(found) => found.map(|(v, _)| v),
        Err(why) => {
            return Ok(unanswered(
                latest_url,
                format!("the stable pointer could not be read: {why}"),
            ))
        }
    };
    let (metadata_url, target) = match input.version.as_deref() {
        None => (latest_url.clone(), stable.clone()),
        Some(word) => {
            let url = format!("{base}/releases/{}.json", tag_of(word));
            match read(&url) {
                Ok(found) => (url, found.map(|(v, _)| v)),
                Err(why) => {
                    return Ok(unanswered(url, format!("the release metadata could not be read: {why}")))
                }
            }
        }
    };
    let target_v = target.as_deref().and_then(Version::parse);
    let asked = input.version.as_deref().map(tag_of);
    let decision = self_update::decide(&running_v, target_v.as_ref(), asked.as_deref());
    let applies = decision.verdict == Verdict::Upgrade;

    let mut report = UpdateReport {
        running: running.clone(),
        stable,
        target: target.clone(),
        metadata_url,
        exit_code: match decision.verdict {
            Verdict::Refused => 10,
            _ => 0,
        },
        decision,
        applies,
        installation: located.clone().ok(),
        not_installed: located.clone().err(),
        applied: None,
    };
    if !input.apply || !applies {
        return Ok(report);
    }
    let Ok(at) = located else {
        report.exit_code = 12;
        return Ok(report);
    };
    let tag = format!("v{}", target.unwrap_or_default());
    let installer_url = format!("{base}/{}", model.installer.script);
    let script = match fetch.fetch(&installer_url) {
        Fetched::Body(b) => b,
        Fetched::Unreachable(why) => {
            report.exit_code = 12;
            report.decision.verdict = Verdict::Unanswered;
            report.decision.reason = format!("the installer could not be read: {why}");
            return Ok(report);
        }
    };
    // the installer is written to a directory of this run's own and removed after it
    let dir = std::env::temp_dir().join(format!("majordomus-self-update.{}", std::process::id()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| CapabilityError::Internal(format!("{}: {e}", dir.display())))?;
    let path = dir.join(&model.installer.script);
    std::fs::write(&path, script)
        .map_err(|e| CapabilityError::Internal(format!("{}: {e}", path.display())))?;
    let applied = self_update::apply(&path, &tag, &at);
    let _ = std::fs::remove_dir_all(&dir);
    let wanted = tag.trim_start_matches('v');
    report.exit_code = match (applied.status, applied.installed.as_deref()) {
        (Some(0), Some(v)) if v == wanted => 0,
        _ => 10,
    };
    report.applied = Some(applied);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_spelled_with_its_v_once() {
        assert_eq!(tag_of("1.0.0"), "v1.0.0");
        assert_eq!(tag_of("v1.0.0"), "v1.0.0");
    }

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "self");
        let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
        assert_eq!(ids, vec!["self.update"]);
        let e = &m.capabilities[0].capability.exposure;
        assert!(e.mcp.is_none() && e.http.is_none(), "installing is the command line's alone");
    }
}
