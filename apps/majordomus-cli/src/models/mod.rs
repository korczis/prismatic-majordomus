//! The model catalogue: which AI models exist for this tool's world, what each can do,
//! and which one a stated need selects — as declared data, never as a live client. ADR
//! 0044 is the decision; ADR 0032's boundary stands untouched: this crate calls no
//! model, holds no SDK, and reads no credential's value. The catalogue is
//! `share/models.yaml` — one declaration, exactly like `share/providers.yaml` — and
//! `models.list` / `models.route` project it to the CLI, HTTP, OpenAPI, MCP and the
//! Cockpit, so no consumer keeps a model name of its own.
//!
//! Two words are deliberately absent. "Provider" is spent (ADR 0024, 0032): here the
//! party that serves a model is a *vendor*. And "routing" here is a pure, explainable
//! function over the declared data — a decision with the reasons attached, never an
//! opaque dispatcher: ask `models.route` why, and every excluded model answers.
//!
//! ```
//! use majordomus_cli::models::{route, ModelCatalogue, Requirements};
//!
//! let catalogue: ModelCatalogue = majordomus_cli::metadata::yaml::parse_into(
//!     "version: 1\nvendors:\n  - id: acme\n    title: Acme\nmodels:\n  - id: acme-large\n    vendor: acme\n    native_id: acme-large-1\n    context_window: 200000\n    capabilities: [text, tools, vision]\n  - id: acme-mini\n    vendor: acme\n    native_id: acme-mini-1\n    context_window: 32000\n    capabilities: [text]\n",
//! ).unwrap();
//! let decision = route(&catalogue, &Requirements { require: vec!["vision".into()], ..Default::default() });
//! assert_eq!(decision.selected.as_deref(), Some("acme-large"));
//! assert_eq!(decision.excluded[0].reason, "missing capability: vision");
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub mod observed;

use observed::{CapabilityEvidence, Locality, ObservedModel, ObservedRuntime, RuntimeObservation};

/// The file name under the share directory.
pub const MODELS_FILE: &str = "models.yaml";

/// A party that serves models: a remote API vendor or a local runtime. Not a
/// "provider" — that word names the AI client tools this repository already adapts to
/// (ADR 0024), and one word for two registries is how truths fork.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Vendor {
    /// The vendor id, unique in the catalogue.
    pub id: String,
    /// The human name.
    pub title: String,
    /// Where inference runs: `remote` (an API over the network) or `local` (a runtime
    /// on this machine). Typed here so routing's `local_only` is data, not a name test.
    #[serde(default)]
    pub inference: Inference,
    /// The environment variable whose *presence* means a credential is configured.
    /// Only the name lives here, and only presence is ever reported — no code in this
    /// crate reads the value, and the environment cache refuses to hold one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_env: Option<String>,
    /// For a local vendor, the runtime an observation reaches (ADR 0174): which adapter
    /// speaks to it, where it listens by default, and the variable that moves it. Which
    /// models it serves is the machine's, so they are observed, never declared here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeDeclaration>,
}

/// How a local runtime is reached: data the observer reads, so that no consumer keeps a
/// runtime's port or adapter name of its own.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeDeclaration {
    /// The adapter under `scripts/lib/inference/` that speaks the runtime's protocol.
    pub adapter: String,
    /// Where the runtime listens by default: an origin on the loopback interface.
    pub endpoint: String,
    /// The environment variable that names another endpoint, when the runtime has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_env: Option<String>,
}

/// Where a vendor's inference runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Inference {
    /// An API over the network.
    #[default]
    Remote,
    /// A runtime on this machine.
    Local,
}

/// A model's lifecycle standing, as the catalogue declares it. There is no default: a
/// catalogue entry that declares none has an undeclared standing, which is not the same
/// fact as "available", and no reader may render it as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    /// Generally available.
    Available,
    /// Usable, expected to change.
    Preview,
    /// Still served, no longer chosen by routing unless named explicitly.
    Deprecated,
    /// Gone; kept so an old record's reference still resolves to a row with a story.
    Retired,
}

/// One model: the canonical reference this repository uses, the vendor's own name for
/// it, and its typed capabilities. Capability words are data the catalogue declares —
/// consumers compare against a requirement list, never against a hardcoded name.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    /// The canonical id, unique in the catalogue: what sessions, configuration and
    /// routing name. Stable even when a vendor renames.
    pub id: String,
    /// The vendor that serves it.
    pub vendor: String,
    /// The vendor's own identifier, verbatim.
    pub native_id: String,
    /// Other names that resolve to this model. Aliases live here and nowhere else.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    /// Typed capability words: `text`, `vision`, `tools`, `structured_output`,
    /// `reasoning`, `streaming`, `embeddings` — the declared vocabulary, open for a
    /// catalogue to extend without a code change.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// The context window, tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u64>,
    /// The lifecycle standing, when the catalogue declares one. Absent is undeclared: the
    /// catalogue admits only facts with provenance, and routing still considers the model,
    /// but nothing may say it is available on the catalogue's behalf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ModelStatus>,
    /// One line a person needs when choosing; never pricing — a price the tool cannot
    /// verify is a fact it must not state (nothing here fabricates cost data).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The whole catalogue, as `share/models.yaml` declares it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelCatalogue {
    /// The format version.
    pub version: u32,
    /// The vendors, in declaration order.
    #[serde(default)]
    pub vendors: Vec<Vendor>,
    /// The models, in declaration order — which is also routing's preference order:
    /// the first satisfying model wins, and "why this one" is "it came first among
    /// those that qualify", an answer a person can verify by reading the file.
    #[serde(default)]
    pub models: Vec<ModelEntry>,
}

impl ModelCatalogue {
    /// Load the catalogue from a share directory. No file is an empty catalogue — a
    /// distribution that declares no models has none, which is an answer; a file that
    /// does not parse is an error naming it.
    pub fn load(share: &std::path::Path) -> Result<ModelCatalogue, String> {
        let path = share.join(MODELS_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => crate::metadata::yaml::parse_into(&text)
                .map_err(|reason| format!("{}: {reason}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ModelCatalogue::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The entry a canonical id or an alias resolves to.
    pub fn resolve(&self, name: &str) -> Option<&ModelEntry> {
        self.models
            .iter()
            .find(|m| m.id == name || m.aliases.iter().any(|a| a == name))
    }

    /// The declared vendor of an entry.
    pub fn vendor_of(&self, entry: &ModelEntry) -> Option<&Vendor> {
        self.vendors.iter().find(|v| v.id == entry.vendor)
    }

    /// The catalogue's own consistency: duplicate ids or aliases, and a model naming a
    /// vendor the catalogue does not declare. Reported, never repaired.
    pub fn diagnostics(&self) -> Vec<String> {
        let mut findings = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for model in &self.models {
            for name in std::iter::once(&model.id).chain(model.aliases.iter()) {
                if !seen.insert(name.clone()) {
                    findings.push(format!("'{name}' names two models; an alias is one truth"));
                }
            }
            if self.vendor_of(model).is_none() {
                findings.push(format!(
                    "model '{}' names vendor '{}', which the catalogue does not declare",
                    model.id, model.vendor
                ));
            }
        }
        findings
    }
}

/// What a task needs from a model. Every field empty means "the first available model".
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct Requirements {
    /// Capability words the model must declare, all of them.
    pub require: Vec<String>,
    /// The least context window, tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_context: Option<u64>,
    /// Only this vendor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// Only local inference: nothing that would leave the machine.
    pub local_only: bool,
    /// A model named outright — by canonical id or alias. Still checked against the
    /// other requirements: an override that cannot do the work is an exclusion with
    /// the reason, not a silent selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The least evidence an observed model's required capability must rest on. Declared
    /// models stand on the catalogue's own provenance and are not held to it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_evidence: Option<CapabilityEvidence>,
}

/// The observation routing may also choose from, with how old it is: a separate overlay
/// on the declared catalogue, never written into it.
#[derive(Debug, Clone, Copy)]
pub struct ObservedPool<'a> {
    /// The checkout's current observation.
    pub observation: &'a RuntimeObservation,
    /// Its age now, seconds; `None` when its stamp does not parse, which counts as stale.
    pub age_seconds: Option<u64>,
    /// The age beyond which it is stale, seconds.
    pub ttl_seconds: u64,
}

impl ObservedPool<'_> {
    /// Whether the observation is too old to route on.
    pub fn stale(&self) -> bool {
        self.age_seconds.is_none_or(|age| age > self.ttl_seconds)
    }
}

/// The observation a routing decision considered.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservedBasis {
    /// When the observation was recorded.
    pub recorded_at: String,
    /// Its age when the decision was made, seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_seconds: Option<u64>,
    /// The age beyond which it is stale, seconds.
    pub ttl_seconds: u64,
    /// Whether it was stale, so that no observed model could be selected.
    pub stale: bool,
}

/// One model that did not qualify, and exactly why.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExcludedModel {
    /// The canonical id.
    pub model: String,
    /// The first reason it fell out, in the order the checks run.
    pub reason: String,
}

/// The routing answer: what was selected, what stands behind it, who fell out and why.
/// The whole decision is derived from the catalogue and the requirements — run it
/// twice, get it twice; there is no state to consult and no health to guess.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutingDecision {
    /// The selected model's canonical id; `None` when nothing qualifies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
    /// Why the selected model was selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The models that also qualify, in preference order: the fallback chain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<String>,
    /// Every model that did not qualify, with its reason.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<ExcludedModel>,
    /// The observation the decision also chose from, when it was asked to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<ObservedBasis>,
}

/// Decide. Pure over the catalogue and the requirements; the preference order is the
/// catalogue's declaration order, and every exclusion names its first failing check.
pub fn route(catalogue: &ModelCatalogue, needs: &Requirements) -> RoutingDecision {
    route_observed(catalogue, None, needs)
}

/// Decide over the catalogue and, when given, an observation of local runtimes. Pure over
/// its inputs: the observation and its age are arguments, not state this reads. Declared
/// models come first, in declaration order; observed models follow, loaded ones before
/// the rest (choosing a loaded model costs no load), then in the observation's order.
pub fn route_observed(
    catalogue: &ModelCatalogue,
    pool: Option<ObservedPool<'_>>,
    needs: &Requirements,
) -> RoutingDecision {
    let mut qualified: Vec<(String, Option<String>)> = Vec::new();
    let mut excluded = Vec::new();
    for model in &catalogue.models {
        match disqualify(catalogue, model, needs) {
            Some(reason) => excluded.push(ExcludedModel {
                model: model.id.clone(),
                reason,
            }),
            None => qualified.push((model.id.clone(), None)),
        }
    }
    if let Some(pool) = pool {
        let loaded = |m: &ObservedModel| m.report.loaded == Some(true);
        let models = pool.observation.models();
        let first = models.clone().filter(|(_, m)| loaded(m));
        let rest = models.filter(|(_, m)| !loaded(m));
        for (runtime, model) in first.chain(rest) {
            match disqualify_observed(runtime, model, &pool, needs) {
                Some(reason) => excluded.push(ExcludedModel {
                    model: model.id.clone(),
                    reason,
                }),
                None => {
                    qualified.push((model.id.clone(), Some(observed_note(model, &pool, needs))))
                }
            }
        }
    }
    let selected = qualified.first().map(|(id, _)| id.clone());
    let reason = qualified.first().map(|(id, note)| {
        let why = match &needs.model {
            Some(named) => {
                format!("named outright as '{named}' and it satisfies every requirement")
            }
            None if note.is_some() => {
                format!("'{id}' is the first observed model satisfying every requirement")
            }
            None => format!("'{id}' is the first declared model satisfying every requirement"),
        };
        match note {
            Some(note) => format!("{why} ({note})"),
            None => why,
        }
    });
    RoutingDecision {
        selected,
        reason,
        fallbacks: qualified.iter().skip(1).map(|(id, _)| id.clone()).collect(),
        excluded,
        observed: pool.map(|p| ObservedBasis {
            recorded_at: p.observation.recorded_at.clone(),
            age_seconds: p.age_seconds,
            ttl_seconds: p.ttl_seconds,
            stale: p.stale(),
        }),
    }
}

/// What a selected observed model rests on, for the decision's reason.
fn observed_note(model: &ObservedModel, pool: &ObservedPool<'_>, needs: &Requirements) -> String {
    let mut parts = Vec::new();
    if let Some(age) = pool.age_seconds {
        parts.push(format!("observed {age} s ago"));
    }
    parts.push(match model.report.loaded {
        Some(true) => "loaded".into(),
        Some(false) => "not loaded".into(),
        None => "load state unknown".into(),
    });
    for word in &needs.require {
        if let Some(evidence) = model
            .standing
            .iter()
            .find(|s| &s.word == word)
            .and_then(|s| s.evidence)
        {
            parts.push(format!("{word} {}", evidence.as_str()));
        }
    }
    parts.join("; ")
}

/// The first reason an observed model does not qualify, in the declared check order with
/// the observation's own checks in place: freshness instead of lifecycle, the runtime's
/// locality instead of the vendor's, each capability's evidence beside its presence.
fn disqualify_observed(
    runtime: &ObservedRuntime,
    model: &ObservedModel,
    pool: &ObservedPool<'_>,
    needs: &Requirements,
) -> Option<String> {
    if let Some(named) = &needs.model {
        if model.id != *named && model.report.native_id != *named {
            return Some(format!("not the model named outright ('{named}')"));
        }
    }
    if pool.stale() {
        return Some(match pool.age_seconds {
            Some(age) => format!(
                "the observation is {age} s old, past its {} s; observe again with scripts/inference-observe",
                pool.ttl_seconds
            ),
            None => "the observation's timestamp does not parse; observe again".into(),
        });
    }
    if let Some(vendor) = &needs.vendor {
        if runtime.report.vendor != *vendor {
            return Some(format!(
                "vendor is '{}', not '{vendor}'",
                runtime.report.vendor
            ));
        }
    }
    if needs.local_only && runtime.locality != Locality::Loopback {
        return Some("the runtime is off the loopback interface and the need is local_only".into());
    }
    for capability in &needs.require {
        let Some(found) = model.standing.iter().find(|s| &s.word == capability) else {
            return Some(format!("missing capability: {capability}"));
        };
        if found.refuted {
            return Some(format!("capability {capability} was refuted by a probe"));
        }
        if let (Some(least), Some(has)) = (needs.min_evidence, found.evidence) {
            if has < least {
                return Some(format!(
                    "capability {capability} is {}, under the required {}",
                    has.as_str(),
                    least.as_str()
                ));
            }
        }
    }
    if let Some(least) = needs.min_context {
        match model.report.context_window.usable() {
            Some(window) if window >= least => {}
            Some(window) => {
                return Some(format!(
                    "context window {window} is under the required {least}"
                ))
            }
            None => return Some(format!("no observed context window; {least} required")),
        }
    }
    None
}

/// The first reason a model does not qualify, in a fixed check order; `None` qualifies.
fn disqualify(
    catalogue: &ModelCatalogue,
    model: &ModelEntry,
    needs: &Requirements,
) -> Option<String> {
    if let Some(named) = &needs.model {
        let resolves = model.id == *named || model.aliases.iter().any(|a| a == named);
        if !resolves {
            return Some(format!("not the model named outright ('{named}')"));
        }
    }
    if let Some(status @ (ModelStatus::Deprecated | ModelStatus::Retired)) = model.status {
        if needs.model.is_none() {
            return Some(format!(
                "status is {}; routing only selects it when named outright",
                match status {
                    ModelStatus::Deprecated => "deprecated",
                    _ => "retired",
                }
            ));
        }
    }
    if let Some(vendor) = &needs.vendor {
        if model.vendor != *vendor {
            return Some(format!("vendor is '{}', not '{vendor}'", model.vendor));
        }
    }
    if needs.local_only {
        let local = catalogue
            .vendor_of(model)
            .is_some_and(|v| v.inference == Inference::Local);
        if !local {
            return Some("inference is remote and the need is local_only".into());
        }
    }
    for capability in &needs.require {
        if !model.capabilities.iter().any(|c| c == capability) {
            return Some(format!("missing capability: {capability}"));
        }
    }
    if let Some(least) = needs.min_context {
        match model.context_window {
            Some(window) if window >= least => {}
            Some(window) => {
                return Some(format!(
                    "context window {window} is under the required {least}"
                ))
            }
            None => return Some(format!("no declared context window; {least} required")),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> ModelCatalogue {
        crate::metadata::yaml::parse_into(
            "version: 1
vendors:
  - id: acme
    title: Acme
    credential_env: ACME_API_KEY
  - id: bench
    title: Bench
    inference: local
models:
  - id: acme-large
    vendor: acme
    native_id: acme-large-1
    aliases: [large]
    context_window: 200000
    capabilities: [text, tools, vision, reasoning]
  - id: acme-old
    vendor: acme
    native_id: acme-old-9
    status: deprecated
    context_window: 100000
    capabilities: [text, tools]
  - id: bench-local
    vendor: bench
    native_id: bench.gguf
    context_window: 8000
    capabilities: [text]
",
        )
        .unwrap()
    }

    /// An entry that declares no status has none: it is not read as "available".
    #[test]
    fn an_undeclared_status_is_undeclared_and_not_available() {
        let c = catalogue();
        let large = c.models.iter().find(|m| m.id == "acme-large").unwrap();
        assert_eq!(large.status, None);
        let old = c.models.iter().find(|m| m.id == "acme-old").unwrap();
        assert_eq!(old.status, Some(ModelStatus::Deprecated));
        let json = serde_json::to_value(large).unwrap();
        assert!(json.get("status").is_none(), "{json}");
    }

    #[test]
    fn the_first_satisfying_model_wins_and_every_exclusion_says_why() {
        let c = catalogue();
        let d = route(&c, &Requirements::default());
        assert_eq!(d.selected.as_deref(), Some("acme-large"));
        assert_eq!(d.fallbacks, vec!["bench-local"]);
        let old = d.excluded.iter().find(|e| e.model == "acme-old").unwrap();
        assert!(old.reason.contains("deprecated"), "{}", old.reason);
    }

    #[test]
    fn requirements_narrow_and_the_reasons_name_the_first_failing_check() {
        let c = catalogue();
        let d = route(
            &c,
            &Requirements {
                require: vec!["vision".into()],
                min_context: Some(150_000),
                ..Default::default()
            },
        );
        assert_eq!(d.selected.as_deref(), Some("acme-large"));
        assert!(d.fallbacks.is_empty());
        let local = d
            .excluded
            .iter()
            .find(|e| e.model == "bench-local")
            .unwrap();
        assert_eq!(local.reason, "missing capability: vision");
    }

    #[test]
    fn local_only_selects_local_inference_or_nothing() {
        let c = catalogue();
        let d = route(
            &c,
            &Requirements {
                local_only: true,
                ..Default::default()
            },
        );
        assert_eq!(d.selected.as_deref(), Some("bench-local"));
        let d = route(
            &c,
            &Requirements {
                local_only: true,
                require: vec!["vision".into()],
                ..Default::default()
            },
        );
        assert_eq!(
            d.selected, None,
            "nothing qualifies, and that is the answer"
        );
        assert!(!d.excluded.is_empty());
    }

    #[test]
    fn naming_a_model_outright_still_checks_the_requirements() {
        let c = catalogue();
        let d = route(
            &c,
            &Requirements {
                model: Some("large".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            d.selected.as_deref(),
            Some("acme-large"),
            "an alias resolves"
        );
        let d = route(
            &c,
            &Requirements {
                model: Some("bench-local".into()),
                require: vec!["vision".into()],
                ..Default::default()
            },
        );
        assert_eq!(
            d.selected, None,
            "an override that cannot do the work is refused"
        );
    }

    #[test]
    fn a_deprecated_model_named_outright_is_still_selectable() {
        let c = catalogue();
        let d = route(
            &c,
            &Requirements {
                model: Some("acme-old".into()),
                ..Default::default()
            },
        );
        assert_eq!(d.selected.as_deref(), Some("acme-old"));
    }

    #[test]
    fn the_catalogue_reports_duplicates_and_undeclared_vendors() {
        let c: ModelCatalogue = crate::metadata::yaml::parse_into(
            "version: 1
models:
  - id: a
    vendor: ghost
    native_id: a1
  - id: a
    vendor: ghost
    native_id: a2
",
        )
        .unwrap();
        let findings = c.diagnostics();
        assert!(findings.iter().any(|f| f.contains("names two models")));
        assert!(findings.iter().any(|f| f.contains("ghost")));
    }

    fn observed_catalogue() -> ModelCatalogue {
        crate::metadata::yaml::parse_into(
            "version: 1
vendors:
  - id: acme
    title: Acme
  - id: bench
    title: Bench
    inference: local
    runtime:
      adapter: bench
      endpoint: http://127.0.0.1:9
models:
  - id: acme-large
    vendor: acme
    native_id: acme-large-1
    capabilities: [text, tools]
",
        )
        .unwrap()
    }

    fn observation(remote: bool) -> RuntimeObservation {
        let all = serde_json::json!([
                { "vendor": "bench", "endpoint": "http://127.0.0.1:9", "state": "identified", "models": [
                    { "native_id": "cold", "loaded": false, "context_window": { "advertised": 32768 },
                      "capabilities": [{ "word": "text", "evidence": "advertised", "source": "meta" }] },
                    { "native_id": "warm", "loaded": true, "context_window": { "advertised": 32768, "configured": 4096 },
                      "capabilities": [{ "word": "text", "evidence": "inferred", "source": "kind" }],
                      "probes": [{ "word": "text", "outcome": "passed", "duration_ms": 12 }] },
                    { "native_id": "embed", "loaded": true,
                      "capabilities": [{ "word": "embeddings", "evidence": "advertised", "source": "type" }],
                      "probes": [{ "word": "text", "outcome": "failed", "duration_ms": 3, "diagnostic": "error: no chat" }] }
                ] },
                { "vendor": "bench", "endpoint": "http://10.1.2.3:9", "state": "identified", "models": [
                    { "native_id": "far", "loaded": true,
                      "capabilities": [{ "word": "text", "evidence": "advertised", "source": "meta" }] }
                ] }
        ]);
        let runtimes: Vec<serde_json::Value> = all
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| remote || r["endpoint"] == "http://127.0.0.1:9")
            .cloned()
            .collect();
        let report: observed::ObservationReport = serde_json::from_value(serde_json::json!({
            "version": 1, "observed_at": "2026-10-17T12:00:00Z", "remote_admitted": remote,
            "runtimes": runtimes,
        }))
        .unwrap();
        observed::admit(report, &observed_catalogue(), "2026-10-17T12:00:00Z".into()).unwrap()
    }

    fn pool(o: &RuntimeObservation, age: u64) -> ObservedPool<'_> {
        ObservedPool {
            observation: o,
            age_seconds: Some(age),
            ttl_seconds: 900,
        }
    }

    #[test]
    fn without_an_observation_routing_is_the_catalogue_alone() {
        let c = observed_catalogue();
        let d = route(
            &c,
            &Requirements {
                local_only: true,
                ..Default::default()
            },
        );
        assert_eq!(d.selected, None);
        assert!(d.observed.is_none());
    }

    #[test]
    fn local_only_selects_a_loaded_observed_model_and_says_what_it_rests_on() {
        let c = observed_catalogue();
        let o = observation(false);
        let needs = Requirements {
            local_only: true,
            require: vec!["text".into()],
            ..Default::default()
        };
        let d = route_observed(&c, Some(pool(&o, 30)), &needs);
        assert_eq!(d.selected.as_deref(), Some("bench/warm"), "{d:?}");
        let reason = d.reason.unwrap();
        assert!(
            reason.contains("observed 30 s ago") && reason.contains("text probed"),
            "{reason}"
        );
        assert_eq!(d.fallbacks, vec!["bench/cold"]);
        let embed = d
            .excluded
            .iter()
            .find(|e| e.model == "bench/embed")
            .unwrap();
        assert_eq!(embed.reason, "capability text was refuted by a probe");
        assert!(!d.observed.unwrap().stale);
    }

    #[test]
    fn min_evidence_holds_observed_models_to_what_was_checked() {
        let c = observed_catalogue();
        let o = observation(false);
        let needs = Requirements {
            require: vec!["text".into()],
            min_evidence: Some(CapabilityEvidence::Probed),
            local_only: true,
            ..Default::default()
        };
        let d = route_observed(&c, Some(pool(&o, 30)), &needs);
        assert_eq!(d.selected.as_deref(), Some("bench/warm"));
        let cold = d.excluded.iter().find(|e| e.model == "bench/cold").unwrap();
        assert_eq!(
            cold.reason,
            "capability text is advertised, under the required probed"
        );
    }

    #[test]
    fn a_stale_observation_selects_nothing_and_says_to_observe_again() {
        let c = observed_catalogue();
        let o = observation(false);
        let needs = Requirements {
            local_only: true,
            ..Default::default()
        };
        let d = route_observed(&c, Some(pool(&o, 901)), &needs);
        assert_eq!(d.selected, None);
        assert!(
            d.excluded
                .iter()
                .all(|e| e.model == "acme-large" || e.reason.contains("past its 900 s")),
            "{d:?}"
        );
        assert!(d.observed.unwrap().stale);
    }

    #[test]
    fn declared_models_come_first_and_a_remote_runtime_is_not_local() {
        let c = observed_catalogue();
        let o = observation(true);
        let d = route_observed(
            &c,
            Some(pool(&o, 1)),
            &Requirements {
                require: vec!["text".into()],
                ..Default::default()
            },
        );
        assert_eq!(d.selected.as_deref(), Some("acme-large"));
        assert!(d.fallbacks.contains(&"bench/far".to_string()));
        let d = route_observed(
            &c,
            Some(pool(&o, 1)),
            &Requirements {
                local_only: true,
                ..Default::default()
            },
        );
        let far = d.excluded.iter().find(|e| e.model == "bench/far").unwrap();
        assert!(
            far.reason.contains("off the loopback interface"),
            "{}",
            far.reason
        );
    }

    #[test]
    fn min_context_reads_the_configured_window_before_the_advertised() {
        let c = observed_catalogue();
        let o = observation(false);
        let needs = Requirements {
            local_only: true,
            min_context: Some(8000),
            require: vec!["text".into()],
            ..Default::default()
        };
        let d = route_observed(&c, Some(pool(&o, 1)), &needs);
        assert_eq!(d.selected.as_deref(), Some("bench/cold"));
        let warm = d.excluded.iter().find(|e| e.model == "bench/warm").unwrap();
        assert_eq!(
            warm.reason,
            "context window 4096 is under the required 8000"
        );
    }

    #[test]
    fn the_order_of_the_report_does_not_change_the_decision() {
        let c = observed_catalogue();
        let o = observation(false);
        let mut shuffled = o.clone();
        shuffled.runtimes[0].models.reverse();
        let needs = Requirements {
            local_only: true,
            require: vec!["text".into()],
            ..Default::default()
        };
        let a = route_observed(&c, Some(pool(&o, 5)), &needs);
        let b = route_observed(&c, Some(pool(&shuffled, 5)), &needs);
        assert_eq!(a.selected, b.selected);
        // admission sorts models; a stored observation is always in sorted order
        let admitted_names: Vec<&str> = o.runtimes[0]
            .models
            .iter()
            .map(|m| m.report.native_id.as_str())
            .collect();
        assert_eq!(admitted_names, ["cold", "embed", "warm"]);
    }

    #[test]
    fn an_absent_file_is_an_empty_catalogue_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let c = ModelCatalogue::load(dir.path()).unwrap();
        assert!(c.models.is_empty() && c.vendors.is_empty());
    }

    #[test]
    fn no_field_of_the_catalogue_can_hold_a_secret_value() {
        // Proves `models-no-secret-fields`: the catalogue reports whether a vendor's
        // named credential variable is set and has no field a value could hide in.
        // The vendor declares the *name* of a credential's environment variable; the
        // schema has no field for a value, so a leak would need a new field to exist.
        // Property names are what is checked — prose may say "tokens" all it likes.
        fn property_names(value: &serde_json::Value, out: &mut Vec<String>) {
            if let Some(properties) = value.get("properties").and_then(|p| p.as_object()) {
                for (name, nested) in properties {
                    out.push(name.to_lowercase());
                    property_names(nested, out);
                }
            }
            for key in ["items", "$defs", "definitions"] {
                if let Some(nested) = value.get(key) {
                    if let Some(map) = nested.as_object() {
                        for entry in map.values() {
                            property_names(entry, out);
                        }
                    } else {
                        property_names(nested, out);
                    }
                }
            }
        }
        let schema = serde_json::to_value(schemars::schema_for!(ModelCatalogue)).unwrap();
        let mut names = Vec::new();
        property_names(&schema, &mut names);
        assert!(
            names.iter().any(|n| n == "credential_env"),
            "the walk sees the fields"
        );
        for name in names {
            for word in ["secret", "token", "password", "key_value", "bearer"] {
                assert!(
                    !name.contains(word),
                    "the catalogue schema declares a field that smells like a credential: {name}"
                );
            }
        }
    }
}
