//! Observed local inference: which runtimes answered, what each serves, and how much is
//! known about what each model can do — runtime truth, kept apart from the declared
//! catalogue it extends (ADR 0174). The catalogue says which local runtimes exist and
//! where each listens by default; the transport under `scripts/lib/inference/` reaches
//! them (ADR 0032 keeps HTTP out of this crate); this module admits what it reports,
//! derives each capability's standing from the evidence, and stores the one current
//! observation of the checkout under `.ai/local/state/inference/`.
//!
//! A capability's standing is never higher than its evidence: `inferred` (the runtime's
//! kind of model usually can), `advertised` (the runtime's own metadata says so),
//! `probed` (a bounded request just succeeded). A probe that failed refutes the word,
//! whatever was advertised. An observation older than its time to live is stale, and
//! routing says so rather than trusting yesterday's runtime.
//!
//! ```
//! use majordomus_cli::models::observed::{standing, CapabilityClaim, CapabilityEvidence, Probe, ProbeOutcome};
//!
//! let claims = vec![CapabilityClaim { word: "text".into(), evidence: CapabilityEvidence::Advertised, source: "GET /api/show".into() }];
//! let probes = vec![Probe { word: "text".into(), outcome: ProbeOutcome::Passed, duration_ms: 40, diagnostic: None }];
//! let words = standing(&claims, &probes);
//! assert_eq!(words[0].evidence, Some(CapabilityEvidence::Probed));
//! assert!(!words[0].refuted);
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Inference, ModelCatalogue};

/// Where the checkout's current observation lives: checkout state, never committed.
pub const STATE_FILE: &str = ".ai/local/state/inference/observation.json";
/// The observation document's format version.
pub const VERSION: u32 = 1;
/// How long an observation stands before routing calls it stale, seconds.
pub const DEFAULT_TTL_SECONDS: u64 = 900;

const MAX_RUNTIMES: usize = 64;
const MAX_MODELS: usize = 512;
const MAX_TEXT: usize = 300;
const MAX_ID: usize = 256;

/// How much is known about one capability of one model, weakest first.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityEvidence {
    /// The runtime's kind of model usually can; nothing about this model says so.
    Inferred,
    /// The runtime's own metadata says this model can.
    Advertised,
    /// A bounded request of this kind just succeeded against this model.
    Probed,
}

impl CapabilityEvidence {
    /// The word, as the wire spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityEvidence::Inferred => "inferred",
            CapabilityEvidence::Advertised => "advertised",
            CapabilityEvidence::Probed => "probed",
        }
    }
}

/// Whether an endpoint answered as the runtime its vendor names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    /// It answered with the runtime's own signature.
    Identified,
    /// Something answered, but not as that runtime: a compatible protocol is not an identity.
    Unidentified,
    /// Nothing answered in time.
    Unreachable,
    /// It answered in a way the adapter could not read.
    Failed,
}

/// What a probe concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProbeOutcome {
    /// The request succeeded and its answer had the expected shape.
    Passed,
    /// It did not.
    Failed,
}

/// Which models the transport probed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProbeScope {
    /// None: metadata only, nothing loaded, nothing generated.
    #[default]
    None,
    /// The models already loaded.
    Loaded,
    /// Every model, which may have loaded each one.
    All,
}

/// Whether inference at an endpoint stays on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    /// The loopback interface: nothing leaves the machine.
    Loopback,
    /// Another host, admitted outright by the observer.
    Remote,
}

/// One capability word a runtime's metadata, or the adapter's inference, attaches to a model.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapabilityClaim {
    /// The catalogue's capability word: `text`, `embeddings`, `tools`, `vision`, ...
    pub word: String,
    /// How much the claim rests on.
    pub evidence: CapabilityEvidence,
    /// Where it came from: the request and the field.
    pub source: String,
}

/// One bounded request the transport made to establish a capability.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Probe {
    /// The capability word it tried.
    pub word: String,
    /// What it concluded.
    pub outcome: ProbeOutcome,
    /// How long it took, milliseconds, including any model load it caused.
    pub duration_ms: u64,
    /// Why it failed, as a class and a sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
}

/// A model's context window, as far as it is known. The model's own limit and the one
/// the runtime is configured to serve are different facts, and either may be unknown.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContextWindow {
    /// What the model's metadata says it supports, tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertised: Option<u64>,
    /// What the runtime serves it with now, tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configured: Option<u64>,
}

impl ContextWindow {
    /// The window a request can rely on: the configured one when known, else the advertised.
    pub fn usable(&self) -> Option<u64> {
        self.configured.or(self.advertised)
    }
}

/// One model as the transport reported it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelReport {
    /// The runtime's own name for the model, verbatim.
    pub native_id: String,
    /// The content digest, when the runtime reports one: the model's identity across names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// The architecture family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    /// The parameter count, as the runtime spells it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<String>,
    /// The quantization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    /// The weight format: `gguf`, `mlx`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// The size on disk, bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Whether it is loaded now; absent when the runtime did not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded: Option<bool>,
    /// Its context window.
    #[serde(default)]
    pub context_window: ContextWindow,
    /// The capability words attached to it, with their evidence.
    #[serde(default)]
    pub capabilities: Vec<CapabilityClaim>,
    /// The probes made against it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub probes: Vec<Probe>,
}

/// One endpoint as the transport reported it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeReport {
    /// The catalogue vendor whose runtime was expected there.
    pub vendor: String,
    /// The endpoint's origin, `http://127.0.0.1:11434`.
    pub endpoint: String,
    /// Whether it answered as that runtime.
    pub state: RuntimeState,
    /// The runtime's version, when it reports one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The response signature that identified it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identified_by: Option<String>,
    /// Why it is not identified, as a class and a sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    /// How long the observation of this endpoint took, milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// What it serves.
    #[serde(default)]
    pub models: Vec<ModelReport>,
}

/// An endpoint the transport would not reach, and why.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EndpointRefusal {
    /// The vendor it was for.
    pub vendor: String,
    /// The endpoint, as given.
    pub endpoint: String,
    /// Why it was not reached.
    pub reason: String,
}

/// The input of `models.observe`: one observation, as `scripts/inference-observe` reports it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservationReport {
    /// The format version; only [`VERSION`] is admitted.
    pub version: u32,
    /// When the transport finished, RFC 3339 — reported, not trusted for staleness.
    pub observed_at: String,
    /// Which models were probed.
    #[serde(default)]
    pub probe: ProbeScope,
    /// Whether the observer admitted endpoints off the loopback interface.
    #[serde(default)]
    pub remote_admitted: bool,
    /// One report per endpoint, in the observer's plan order.
    #[serde(default)]
    pub runtimes: Vec<RuntimeReport>,
    /// The endpoints it would not reach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused: Vec<EndpointRefusal>,
}

/// A capability word of an observed model, with the standing its evidence gives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CapabilityStanding {
    /// The word.
    pub word: String,
    /// The strongest evidence for it; absent when the only word about it is a failed probe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<CapabilityEvidence>,
    /// A probe failed and none passed: routing does not count the word, whatever was claimed.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub refuted: bool,
    /// Where the evidence came from.
    pub sources: Vec<String>,
}

/// One observed model, as admitted.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservedModel {
    /// The reference routing and records name: `<vendor>/<native_id>`.
    pub id: String,
    /// The instance identity: this model at this runtime instance, digest-bound when known.
    pub instance: String,
    /// The report.
    #[serde(flatten)]
    pub report: ModelReport,
    /// Each capability word with its derived standing, sorted by word.
    pub standing: Vec<CapabilityStanding>,
}

/// A model of a runtime orders by its native id, its instance identity breaking a tie.
impl crate::order::Ordered for ObservedModel {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.report.native_id, &self.instance)
    }
}

/// One observed endpoint, as admitted.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservedRuntime {
    /// The runtime instance identity: the vendor at this endpoint.
    pub id: String,
    /// Whether inference here stays on the machine.
    pub locality: Locality,
    /// The report, its models replaced by the admitted ones below.
    #[serde(flatten)]
    pub report: RuntimeSummary,
    /// The models, sorted by native id.
    pub models: Vec<ObservedModel>,
}

/// A runtime report without its models.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeSummary {
    /// The vendor.
    pub vendor: String,
    /// The endpoint's origin.
    pub endpoint: String,
    /// Whether it answered as that runtime.
    pub state: RuntimeState,
    /// Its version, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The signature that identified it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identified_by: Option<String>,
    /// Why it is not identified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    /// How long it took, milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// The checkout's current observation, as stored.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RuntimeObservation {
    /// The format version.
    pub version: u32,
    /// When this crate admitted it, RFC 3339: the clock staleness is measured from.
    pub recorded_at: String,
    /// When the transport says it finished.
    pub observed_at: String,
    /// Which models were probed.
    pub probe: ProbeScope,
    /// Whether endpoints off the loopback interface were admitted.
    pub remote_admitted: bool,
    /// The endpoints, in the observer's order.
    pub runtimes: Vec<ObservedRuntime>,
    /// The endpoints the observer would not reach.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refused: Vec<EndpointRefusal>,
}

impl RuntimeObservation {
    /// Every model of every identified runtime, runtime order then model order.
    pub fn models(&self) -> impl Iterator<Item = (&ObservedRuntime, &ObservedModel)> + Clone {
        self.runtimes
            .iter()
            .filter(|r| r.report.state == RuntimeState::Identified)
            .flat_map(|r| r.models.iter().map(move |m| (r, m)))
    }
}

/// Derive each word's standing from what was claimed and what was probed.
pub fn standing(claims: &[CapabilityClaim], probes: &[Probe]) -> Vec<CapabilityStanding> {
    /// What is known of one word while the claims and probes are read.
    #[derive(Default)]
    struct Tally {
        claimed: Option<CapabilityEvidence>,
        passed: bool,
        failed: bool,
        sources: Vec<String>,
    }
    let mut words: BTreeMap<&str, Tally> = BTreeMap::new();
    for c in claims {
        let t = words.entry(&c.word).or_default();
        t.claimed = t.claimed.max(Some(c.evidence));
        t.sources
            .push(format!("{}: {}", c.evidence.as_str(), c.source));
    }
    for p in probes {
        let t = words.entry(&p.word).or_default();
        match p.outcome {
            ProbeOutcome::Passed => {
                t.passed = true;
                t.sources
                    .push(format!("probe passed in {} ms", p.duration_ms));
            }
            ProbeOutcome::Failed => {
                t.failed = true;
                t.sources.push(format!(
                    "probe failed: {}",
                    p.diagnostic.as_deref().unwrap_or("no diagnostic")
                ));
            }
        }
    }
    words
        .into_iter()
        .map(|(word, t)| CapabilityStanding {
            word: word.to_string(),
            evidence: if t.passed {
                Some(CapabilityEvidence::Probed)
            } else {
                t.claimed
            },
            refuted: t.failed && !t.passed,
            sources: t.sources,
        })
        .collect()
}

/// The origin of an endpoint and whether it is loopback, or why it is not an endpoint.
/// Only an `http`/`https` origin without a credential is one.
pub fn locate(endpoint: &str) -> Result<(String, Locality), String> {
    let (scheme, rest) = endpoint
        .split_once("://")
        .ok_or_else(|| format!("'{endpoint}' is not a URL"))?;
    if scheme != "http" && scheme != "https" {
        return Err(format!("'{endpoint}' is not an HTTP endpoint"));
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err("an endpoint may not carry a credential".into());
    }
    let host = if let Some(v6) = authority.strip_prefix('[') {
        format!("[{}]", v6.split(']').next().unwrap_or_default())
    } else {
        authority.split(':').next().unwrap_or_default().to_string()
    };
    if host.is_empty() || host == "[]" {
        return Err(format!("'{endpoint}' names no host"));
    }
    let loopback = host == "localhost"
        || host == "[::1]"
        || host.strip_prefix("127.").is_some_and(|r| {
            r.split('.').count() == 3 && r.split('.').all(|o| o.parse::<u8>().is_ok())
        });
    let locality = if loopback {
        Locality::Loopback
    } else {
        Locality::Remote
    };
    Ok((format!("{scheme}://{authority}"), locality))
}

fn digest_id(prefix: &str, parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0]);
    }
    let hex: String = h
        .finalize()
        .iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{prefix}-{hex}")
}

fn bounded(field: &str, text: &str, max: usize, findings: &mut Vec<String>) -> String {
    if text.chars().count() > max {
        findings.push(format!("{field} is longer than {max} characters"));
    }
    crate::redaction::redact_secrets(text).text
}

fn word_ok(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= 32
        && word.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
}

/// Admit a report against the catalogue, or refuse it with every finding. Admission
/// checks shape and policy, never truth: a runtime may lie about its models, and the
/// standing says how far each claim was checked.
pub fn admit(
    report: ObservationReport,
    catalogue: &ModelCatalogue,
    recorded_at: String,
) -> Result<RuntimeObservation, Vec<String>> {
    let mut findings = Vec::new();
    if report.version != VERSION {
        findings.push(format!(
            "observation version {} is not {VERSION}",
            report.version
        ));
    }
    if report.runtimes.len() > MAX_RUNTIMES {
        findings.push(format!("more than {MAX_RUNTIMES} runtimes"));
    }
    let mut seen = BTreeSet::new();
    let mut runtimes = Vec::new();
    for r in report.runtimes {
        let declared = catalogue.vendors.iter().find(|v| v.id == r.vendor);
        match declared {
            None => {
                findings.push(format!("vendor '{}' is not declared", r.vendor));
                continue;
            }
            Some(v) if v.inference != Inference::Local || v.runtime.is_none() => {
                findings.push(format!(
                    "vendor '{}' declares no local runtime to observe",
                    r.vendor
                ));
                continue;
            }
            Some(_) => {}
        }
        let (endpoint, locality) = match locate(&r.endpoint) {
            Ok(found) => found,
            Err(reason) => {
                findings.push(format!("{}: {reason}", r.vendor));
                continue;
            }
        };
        if locality == Locality::Remote && !report.remote_admitted {
            findings.push(format!(
                "{} at {endpoint} is off the loopback interface and the observer did not admit remote endpoints",
                r.vendor
            ));
            continue;
        }
        if !seen.insert((r.vendor.clone(), endpoint.clone())) {
            findings.push(format!("{} at {endpoint} is reported twice", r.vendor));
            continue;
        }
        if r.models.len() > MAX_MODELS {
            findings.push(format!(
                "{} reports more than {MAX_MODELS} models",
                r.vendor
            ));
        }
        if r.state != RuntimeState::Identified && !r.models.is_empty() {
            findings.push(format!(
                "{} at {endpoint} is not identified, yet reports models",
                r.vendor
            ));
        }
        let id = digest_id("rt", &[&r.vendor, &endpoint]);
        let mut models: Vec<ObservedModel> = Vec::new();
        let mut names = BTreeSet::new();
        for mut m in r.models {
            if m.native_id.is_empty() || m.native_id.len() > MAX_ID {
                findings.push(format!("{}: a model id is empty or too long", r.vendor));
                continue;
            }
            if !names.insert(m.native_id.clone()) {
                findings.push(format!(
                    "{}: model '{}' is reported twice",
                    r.vendor, m.native_id
                ));
                continue;
            }
            for c in &mut m.capabilities {
                if !word_ok(&c.word) {
                    findings.push(format!(
                        "{}: '{}' is not a capability word",
                        m.native_id, c.word
                    ));
                }
                c.source = bounded("a capability source", &c.source, MAX_TEXT, &mut findings);
            }
            for p in &mut m.probes {
                if !word_ok(&p.word) {
                    findings.push(format!(
                        "{}: '{}' is not a capability word",
                        m.native_id, p.word
                    ));
                }
                p.diagnostic = p
                    .diagnostic
                    .as_deref()
                    .map(|d| bounded("a probe diagnostic", d, MAX_TEXT, &mut findings));
            }
            let instance = digest_id(
                "mi",
                &[&id, &m.native_id, m.digest.as_deref().unwrap_or_default()],
            );
            models.push(ObservedModel {
                id: format!("{}/{}", r.vendor, m.native_id),
                instance,
                standing: standing(&m.capabilities, &m.probes),
                report: m,
            });
        }
        crate::order::canonical(&mut models);
        runtimes.push(ObservedRuntime {
            id,
            locality,
            report: RuntimeSummary {
                vendor: r.vendor,
                endpoint,
                state: r.state,
                version: r
                    .version
                    .map(|v| bounded("a version", &v, MAX_ID, &mut findings)),
                identified_by: r
                    .identified_by
                    .map(|v| bounded("a signature", &v, MAX_TEXT, &mut findings)),
                diagnostic: r
                    .diagnostic
                    .map(|v| bounded("a diagnostic", &v, MAX_TEXT, &mut findings)),
                duration_ms: r.duration_ms,
            },
            models,
        });
    }
    if !findings.is_empty() {
        return Err(findings);
    }
    Ok(RuntimeObservation {
        version: VERSION,
        recorded_at,
        observed_at: report.observed_at,
        probe: report.probe,
        remote_admitted: report.remote_admitted,
        runtimes,
        refused: report
            .refused
            .into_iter()
            .map(|r| EndpointRefusal {
                reason: crate::redaction::redact_secrets(&r.reason).text,
                ..r
            })
            .collect(),
    })
}

/// The stored observation of a checkout, if any; a file that does not parse is an error.
pub fn load(root: &Path) -> Result<Option<RuntimeObservation>, String> {
    let path = root.join(STATE_FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Replace the stored observation, atomically: a reader sees the old one or the new one.
pub fn save(root: &Path, observation: &RuntimeObservation) -> std::io::Result<PathBuf> {
    let path = root.join(STATE_FILE);
    let dir = path.parent().expect("the state file has a directory");
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".observation.{}.tmp", std::process::id()));
    let text = serde_json::to_string_pretty(observation).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, format!("{text}\n"))?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// How old an observation is at `now`, seconds; `None` when its stamp does not parse.
pub fn age_seconds(observation: &RuntimeObservation, now_epoch: i64) -> Option<u64> {
    crate::reasoning::epoch_seconds(&observation.recorded_at)
        .map(|at| now_epoch.saturating_sub(at).max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> ModelCatalogue {
        crate::metadata::yaml::parse_into(
            "version: 1
vendors:
  - id: cloud
    title: Cloud
  - id: bench
    title: Bench
    inference: local
    runtime:
      adapter: bench
      endpoint: http://127.0.0.1:9
  - id: bare
    title: Bare
    inference: local
",
        )
        .unwrap()
    }

    fn report(json: serde_json::Value) -> ObservationReport {
        serde_json::from_value(json).unwrap()
    }

    fn one(endpoint: &str, models: serde_json::Value) -> ObservationReport {
        report(serde_json::json!({
            "version": 1, "observed_at": "2026-10-17T12:00:00Z",
            "runtimes": [{ "vendor": "bench", "endpoint": endpoint, "state": "identified", "models": models }]
        }))
    }

    #[test]
    fn a_failed_probe_refutes_what_was_advertised_and_a_passed_one_proves_it() {
        let claims = vec![
            CapabilityClaim {
                word: "text".into(),
                evidence: CapabilityEvidence::Advertised,
                source: "meta".into(),
            },
            CapabilityClaim {
                word: "tools".into(),
                evidence: CapabilityEvidence::Inferred,
                source: "kind".into(),
            },
        ];
        let probes = vec![
            Probe {
                word: "text".into(),
                outcome: ProbeOutcome::Failed,
                duration_ms: 5,
                diagnostic: Some("timeout: slow".into()),
            },
            Probe {
                word: "embeddings".into(),
                outcome: ProbeOutcome::Passed,
                duration_ms: 3,
                diagnostic: None,
            },
        ];
        let s = standing(&claims, &probes);
        let by = |w: &str| s.iter().find(|c| c.word == w).unwrap().clone();
        assert!(by("text").refuted);
        assert_eq!(by("text").evidence, Some(CapabilityEvidence::Advertised));
        assert_eq!(by("tools").evidence, Some(CapabilityEvidence::Inferred));
        assert!(!by("tools").refuted);
        assert_eq!(by("embeddings").evidence, Some(CapabilityEvidence::Probed));
        let words: Vec<&str> = s.iter().map(|c| c.word.as_str()).collect();
        assert_eq!(
            words,
            ["embeddings", "text", "tools"],
            "sorted, not report order"
        );
    }

    #[test]
    fn evidence_is_ordered_weakest_first() {
        assert!(CapabilityEvidence::Inferred < CapabilityEvidence::Advertised);
        assert!(CapabilityEvidence::Advertised < CapabilityEvidence::Probed);
    }

    #[test]
    fn locate_admits_origins_and_tells_loopback_from_remote() {
        assert_eq!(
            locate("http://127.0.0.1:11434/v1").unwrap(),
            ("http://127.0.0.1:11434".into(), Locality::Loopback)
        );
        assert_eq!(
            locate("http://localhost:1234").unwrap().1,
            Locality::Loopback
        );
        assert_eq!(locate("http://[::1]:8080").unwrap().1, Locality::Loopback);
        assert_eq!(
            locate("http://192.168.1.5:11434").unwrap().1,
            Locality::Remote
        );
        assert_eq!(
            locate("http://127.example.com").unwrap().1,
            Locality::Remote
        );
        assert!(locate("http://user:pw@127.0.0.1:1").is_err());
        assert!(locate("file:///etc/passwd").is_err());
        assert!(locate("127.0.0.1:1").is_err());
    }

    #[test]
    fn a_remote_endpoint_is_refused_unless_the_observer_admitted_it() {
        let c = catalogue();
        let refused = admit(
            one("http://10.0.0.7:11434", serde_json::json!([])),
            &c,
            "t".into(),
        )
        .unwrap_err();
        assert!(
            refused[0].contains("off the loopback interface"),
            "{refused:?}"
        );
        let mut admitted = one("http://10.0.0.7:11434", serde_json::json!([]));
        admitted.remote_admitted = true;
        let o = admit(admitted, &c, "t".into()).unwrap();
        assert_eq!(o.runtimes[0].locality, Locality::Remote);
    }

    #[test]
    fn only_a_declared_local_runtime_is_observed() {
        let c = catalogue();
        for vendor in ["cloud", "bare", "ghost"] {
            let r = report(serde_json::json!({
                "version": 1, "observed_at": "x",
                "runtimes": [{ "vendor": vendor, "endpoint": "http://127.0.0.1:9", "state": "unreachable" }]
            }));
            assert!(admit(r, &c, "t".into()).is_err(), "{vendor} was admitted");
        }
    }

    #[test]
    fn identities_are_deterministic_and_models_are_sorted() {
        let c = catalogue();
        let models = serde_json::json!([
            { "native_id": "zeta", "capabilities": [{ "word": "text", "evidence": "inferred", "source": "s" }] },
            { "native_id": "alpha", "digest": "sha256:ab", "capabilities": [] }
        ]);
        let a = admit(one("http://127.0.0.1:9", models.clone()), &c, "t".into()).unwrap();
        let b = admit(one("http://127.0.0.1:9/", models), &c, "t".into()).unwrap();
        assert_eq!(a.runtimes[0].id, b.runtimes[0].id);
        let names: Vec<&str> = a.runtimes[0]
            .models
            .iter()
            .map(|m| m.report.native_id.as_str())
            .collect();
        assert_eq!(names, ["alpha", "zeta"]);
        assert_eq!(a.runtimes[0].models[0].id, "bench/alpha");
        assert_eq!(
            a.runtimes[0].models[0].instance,
            b.runtimes[0].models[0].instance
        );
        assert!(a.runtimes[0].models[0].instance.starts_with("mi-"));
    }

    #[test]
    fn a_secret_in_a_diagnostic_is_redacted_before_it_is_stored() {
        let c = catalogue();
        let r = report(serde_json::json!({
            "version": 1, "observed_at": "x",
            "runtimes": [{ "vendor": "bench", "endpoint": "http://127.0.0.1:9", "state": "failed",
                           "diagnostic": "auth_failed: key sk-ant-REDACTMEREDACTMEREDACTME0123 refused" }]
        }));
        let o = admit(r, &c, "t".into()).unwrap();
        let d = o.runtimes[0].report.diagnostic.as_deref().unwrap();
        assert!(!d.contains("sk-ant-REDACTME"), "{d}");
    }

    #[test]
    fn a_malformed_word_or_an_unidentified_runtime_with_models_is_refused() {
        let c = catalogue();
        let bad_word = one(
            "http://127.0.0.1:9",
            serde_json::json!([
                { "native_id": "m", "capabilities": [{ "word": "Text!", "evidence": "advertised", "source": "s" }] }
            ]),
        );
        assert!(admit(bad_word, &c, "t".into()).is_err());
        let r = report(serde_json::json!({
            "version": 1, "observed_at": "x",
            "runtimes": [{ "vendor": "bench", "endpoint": "http://127.0.0.1:9", "state": "unidentified",
                           "models": [{ "native_id": "m" }] }]
        }));
        assert!(admit(r, &c, "t".into()).is_err());
    }

    #[test]
    fn the_store_round_trips_and_an_absent_file_is_no_observation() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load(dir.path()).unwrap().is_none());
        let o = admit(
            one("http://127.0.0.1:9", serde_json::json!([])),
            &catalogue(),
            "2026-10-17T12:00:00Z".into(),
        )
        .unwrap();
        let path = save(dir.path(), &o).unwrap();
        assert!(path.ends_with(STATE_FILE));
        let back = load(dir.path()).unwrap().unwrap();
        assert_eq!(back.runtimes[0].id, o.runtimes[0].id);
        let at = crate::reasoning::epoch_seconds("2026-10-17T12:00:00Z").unwrap();
        assert_eq!(age_seconds(&back, at + 90), Some(90));
    }
}
