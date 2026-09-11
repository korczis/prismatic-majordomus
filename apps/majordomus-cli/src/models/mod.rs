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

/// A model's lifecycle standing, as the catalogue declares it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    /// Generally available.
    #[default]
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
    /// The lifecycle standing.
    #[serde(default)]
    pub status: ModelStatus,
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
}

/// One model that did not qualify, and exactly why.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Excluded {
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
    pub excluded: Vec<Excluded>,
}

/// Decide. Pure over the catalogue and the requirements; the preference order is the
/// catalogue's declaration order, and every exclusion names its first failing check.
pub fn route(catalogue: &ModelCatalogue, needs: &Requirements) -> RoutingDecision {
    let mut qualified: Vec<&ModelEntry> = Vec::new();
    let mut excluded = Vec::new();
    for model in &catalogue.models {
        match disqualify(catalogue, model, needs) {
            Some(reason) => excluded.push(Excluded {
                model: model.id.clone(),
                reason,
            }),
            None => qualified.push(model),
        }
    }
    let selected = qualified.first().map(|m| m.id.clone());
    let reason = selected.as_ref().map(|id| match &needs.model {
        Some(named) => format!("named outright as '{named}' and it satisfies every requirement"),
        None => format!("'{id}' is the first declared model satisfying every requirement"),
    });
    RoutingDecision {
        selected,
        reason,
        fallbacks: qualified.iter().skip(1).map(|m| m.id.clone()).collect(),
        excluded,
    }
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
    if matches!(model.status, ModelStatus::Deprecated | ModelStatus::Retired)
        && needs.model.is_none()
    {
        return Some(format!(
            "status is {}; routing only selects it when named outright",
            match model.status {
                ModelStatus::Deprecated => "deprecated",
                _ => "retired",
            }
        ));
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

    #[test]
    fn an_absent_file_is_an_empty_catalogue_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let c = ModelCatalogue::load(dir.path()).unwrap();
        assert!(c.models.is_empty() && c.vendors.is_empty());
    }

    #[test]
    fn no_field_of_the_catalogue_can_hold_a_secret_value() {
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
