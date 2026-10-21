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
    /// How a call to this vendor can be paid: the sources the vendor offers, each with
    /// the evidence it was read from. What the vendor *offers* is the distribution's;
    /// which of them the owner *holds*, and any amount, is the owner's and never here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub funding: Vec<FundingSource>,
}

/// One way a vendor lets a call be paid. A kind and a credential, never an amount: a
/// price or a balance this tool cannot verify is a fact it must not state.
///
/// ```
/// use majordomus_cli::models::{FundingKind, FundingSource, ModelCatalogue};
///
/// let c = ModelCatalogue::parse(concat!(
///     "version: 1\nvendors:\n  - id: acme\n    title: Acme\n    funding:\n",
///     "      - id: grant\n        kind: credit_grant\n        credential_env: ACME_API_KEY\n",
///     "        renews: monthly\n        claim_url: https://acme.example/claim\n",
///     "        evidence:\n          url: https://acme.example/docs\n          checked: 2026-10-21\n",
/// )).unwrap();
/// let grant: &FundingSource = &c.vendors[0].funding[0];
/// assert_eq!(grant.kind, FundingKind::CreditGrant);
/// assert_eq!(grant.credential_env.as_deref(), Some("ACME_API_KEY"));
/// assert!(c.diagnostics().is_empty());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FundingSource {
    /// The source id, unique within its vendor.
    pub id: String,
    /// What kind of source it is.
    pub kind: FundingKind,
    /// The environment variable whose presence reaches this source. Two sources may
    /// name the same one: a credit grant and metered billing are both reached through
    /// one API credential, and nothing local tells them apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_env: Option<String>,
    /// The period an allowance renews over, for a source that has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renews: Option<Renewal>,
    /// Where the owner claims or links the source, as the vendor published it. Shown as
    /// evidence; Majordomus never follows it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_url: Option<String>,
    /// What the declaration stands on.
    pub evidence: FundingEvidence,
    /// One line a person needs when choosing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The kinds of funding source, in the order a policy most often prefers them.
///
/// ```
/// use majordomus_cli::models::FundingKind;
///
/// let kind: FundingKind = serde_json::from_str(r#""credit_grant""#).unwrap();
/// assert_eq!(kind, FundingKind::CreditGrant);
/// assert!(serde_json::from_str::<FundingKind>(r#""voucher""#).is_err(), "the set is closed");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FundingKind {
    /// Inference on this machine; no money changes hands.
    Local,
    /// A recurring allowance of API usage that comes with something else the owner
    /// pays for, such as the API credits a subscription plan includes.
    CreditGrant,
    /// A subscription seat, used through the client signed in with it.
    Subscription,
    /// Pay-as-you-go usage, billed by what was used.
    Metered,
}

/// The period an allowance renews over.
///
/// ```
/// use majordomus_cli::models::Renewal;
///
/// assert_eq!(serde_json::from_str::<Renewal>(r#""monthly""#).unwrap(), Renewal::Monthly);
/// assert_eq!(serde_json::to_string(&Renewal::Monthly).unwrap(), r#""monthly""#);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Renewal {
    /// Every calendar month, or every billing month, as the vendor states it.
    Monthly,
}

/// Where a funding declaration was read, and when.
///
/// ```
/// use majordomus_cli::models::FundingEvidence;
///
/// let e: FundingEvidence = serde_json::from_str(
///     r#"{"url":"https://acme.example/docs","checked":"2026-10-21","quote":"Pay as you go."}"#,
/// ).unwrap();
/// assert_eq!(e.checked, "2026-10-21");
/// assert!(serde_json::from_str::<FundingEvidence>(r#"{"url":"x","checked":"y","price":1}"#).is_err(),
///     "no field could carry an amount");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FundingEvidence {
    /// The page or record it was read from.
    pub url: String,
    /// The day it was read, `YYYY-MM-DD`.
    pub checked: String,
    /// The vendor's own words, verbatim, when a sentence settles it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
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
    /// Funding sources [`ModelCatalogue::parse`] could not read, each naming its vendor
    /// and source. They are set aside rather than failing the file: the advisor catalogue
    /// loads the same file, and one bad source must not take every advisor down.
    #[serde(skip)]
    pub unread_funding: Vec<String>,
}

impl ModelCatalogue {
    /// Load the catalogue from a share directory. No file is an empty catalogue — a
    /// distribution that declares no models has none, which is an answer; a file that
    /// does not parse is an error naming it.
    pub fn load(share: &std::path::Path) -> Result<ModelCatalogue, String> {
        let path = share.join(MODELS_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => ModelCatalogue::parse(&text)
                .map_err(|reason| format!("{}: {reason}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ModelCatalogue::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// Parse a catalogue. A funding source without evidence, of a kind nobody declared,
    /// or otherwise unreadable is set aside and reported by [`ModelCatalogue::diagnostics`]
    /// naming its vendor and its id — a typed parse alone would fail the whole file and
    /// say only that a field is missing somewhere.
    ///
    /// ```
    /// use majordomus_cli::models::ModelCatalogue;
    ///
    /// let c = ModelCatalogue::parse(concat!(
    ///     "version: 1\nvendors:\n  - id: acme\n    title: Acme\n    funding:\n",
    ///     "      - id: grant\n        kind: credit_grant\n",
    ///     "models: []\n",
    /// )).unwrap();
    /// assert!(c.vendors[0].funding.is_empty(), "the unreadable source is set aside");
    /// assert!(c.diagnostics()[0].contains("vendor 'acme' funding source 'grant' declares no evidence"));
    /// ```
    pub fn parse(text: &str) -> Result<ModelCatalogue, String> {
        let mut map = crate::metadata::yaml::parse_mapping(text)?;
        let unread = set_aside_unreadable_funding(&mut map);
        let mut catalogue: ModelCatalogue =
            serde_json::from_value(serde_json::Value::Object(map)).map_err(|e| e.to_string())?;
        catalogue.unread_funding = unread;
        Ok(catalogue)
    }

    /// Every credential variable the catalogue names, the vendors' and their funding
    /// sources', once each in declaration order: what no CI gate may depend on.
    ///
    /// ```
    /// use majordomus_cli::models::ModelCatalogue;
    ///
    /// let c = ModelCatalogue::parse(concat!(
    ///     "version: 1\nvendors:\n  - id: acme\n    title: Acme\n    credential_env: ACME_KEY\n",
    ///     "    funding:\n      - id: seat\n        kind: subscription\n        credential_env: ACME_SEAT\n",
    ///     "        evidence:\n          url: https://acme.example/docs\n          checked: 2026-10-21\n",
    ///     "      - id: metered\n        kind: metered\n        credential_env: ACME_KEY\n",
    ///     "        evidence:\n          url: https://acme.example/docs\n          checked: 2026-10-21\n",
    /// )).unwrap();
    /// assert_eq!(c.credential_variables(), ["ACME_KEY", "ACME_SEAT"]);
    /// ```
    pub fn credential_variables(&self) -> Vec<&str> {
        let mut names: Vec<&str> = Vec::new();
        for vendor in &self.vendors {
            let sources = vendor
                .funding
                .iter()
                .filter_map(|s| s.credential_env.as_deref());
            for name in vendor.credential_env.as_deref().into_iter().chain(sources) {
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
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
        findings.extend(self.unread_funding.iter().cloned());
        for vendor in &self.vendors {
            findings.extend(funding_findings(vendor));
        }
        findings
    }
}

/// Take out of the raw mapping every funding source that cannot be read — no evidence,
/// evidence without its page or day, a kind outside [`FundingKind`], or anything else the
/// typed parse would refuse — and say why, naming the vendor and the source. Done before
/// the typed parse, so that the rest of the catalogue still loads and the reason names
/// what it means.
fn set_aside_unreadable_funding(
    map: &mut serde_json::Map<String, serde_json::Value>,
) -> Vec<String> {
    let mut unread = Vec::new();
    let vendors = map.get_mut("vendors").and_then(|v| v.as_array_mut());
    for vendor in vendors.into_iter().flatten() {
        let vendor_id = vendor
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        let Some(sources) = vendor.get_mut("funding").and_then(|f| f.as_array_mut()) else {
            continue;
        };
        let mut i = 0;
        sources.retain(|source| {
            i += 1;
            let named = match source.get("id").and_then(|v| v.as_str()) {
                Some(id) => format!("vendor '{vendor_id}' funding source '{id}'"),
                None => format!("vendor '{vendor_id}' funding source #{i}"),
            };
            let reasons = funding_unreadable(source);
            for reason in &reasons {
                unread.push(format!("{named} {reason}; it is set aside"));
            }
            reasons.is_empty()
        });
    }
    unread
}

/// Why one raw funding source cannot be read; empty when it can.
fn funding_unreadable(source: &serde_json::Value) -> Vec<String> {
    let mut reasons = Vec::new();
    match source.get("kind") {
        None => reasons.push("declares no kind".to_string()),
        Some(kind) if serde_json::from_value::<FundingKind>(kind.clone()).is_err() => {
            reasons.push(format!("declares kind {kind}, which is not a funding kind"));
        }
        Some(_) => {}
    }
    match source.get("evidence") {
        None => reasons.push("declares no evidence".to_string()),
        Some(evidence) => {
            for field in ["url", "checked"] {
                let present = evidence
                    .get(field)
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.trim().is_empty());
                if !present {
                    reasons.push(format!("has evidence without its {field}"));
                }
            }
        }
    }
    if reasons.is_empty() {
        if let Err(e) = serde_json::from_value::<FundingSource>(source.clone()) {
            reasons.push(format!("cannot be read: {e}"));
        }
    }
    reasons
}

/// A vendor's funding declarations that parse but do not agree with themselves or with
/// the vendor: reported, never repaired.
fn funding_findings(vendor: &Vendor) -> Vec<String> {
    let mut findings = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for source in &vendor.funding {
        let named = format!("vendor '{}' funding source '{}'", vendor.id, source.id);
        if !seen.insert(source.id.as_str()) {
            findings.push(format!("{named} is declared twice"));
        }
        let local_vendor = vendor.inference == Inference::Local;
        if local_vendor != (source.kind == FundingKind::Local) {
            findings.push(format!(
                "{named} is of kind {:?} on a vendor whose inference is {:?}",
                source.kind, vendor.inference
            ));
        }
        if source.kind == FundingKind::CreditGrant && source.renews.is_none() {
            findings.push(format!(
                "{named} is a credit grant that declares no renewal period"
            ));
        }
        if source.kind != FundingKind::Local && source.credential_env.is_none() {
            findings.push(format!(
                "{named} names no credential variable that reaches it"
            ));
        }
        if !is_iso_day(&source.evidence.checked) {
            findings.push(format!(
                "{named} has evidence checked '{}', which is not a YYYY-MM-DD day",
                source.evidence.checked
            ));
        }
    }
    findings
}

fn is_iso_day(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
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
}

/// Decide. Pure over the catalogue and the requirements; the preference order is the
/// catalogue's declaration order, and every exclusion names its first failing check.
pub fn route(catalogue: &ModelCatalogue, needs: &Requirements) -> RoutingDecision {
    let mut qualified: Vec<&ModelEntry> = Vec::new();
    let mut excluded = Vec::new();
    for model in &catalogue.models {
        match disqualify(catalogue, model, needs) {
            Some(reason) => excluded.push(ExcludedModel {
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

    fn with_funding(source: &str) -> Result<ModelCatalogue, String> {
        ModelCatalogue::parse(&format!(
            "version: 1\nvendors:\n  - id: acme\n    title: Acme\n    funding:\n{source}"
        ))
    }

    const EVIDENCE: &str = "        evidence:\n          url: https://acme.example/docs\n          checked: 2026-10-21\n";

    /// The one diagnostic that names `needle`, after the catalogue still loaded.
    fn set_aside(source: &str, needle: &str) -> ModelCatalogue {
        let c = with_funding(source).expect("one bad source does not fail the file");
        let findings = c.diagnostics();
        assert!(
            findings
                .iter()
                .any(|f| f.contains(needle) && f.ends_with("it is set aside")),
            "{needle} in {findings:?}"
        );
        c
    }

    /// Proves I3660: a source the vendor declares without saying where it read it is
    /// refused, the refusal names the vendor and the source, and the rest of the
    /// catalogue still loads.
    #[test]
    fn test_a_funding_source_without_evidence_is_refused() {
        let c = set_aside(
            &format!(
                "      - id: grant\n        kind: credit_grant\n        credential_env: ACME_API_KEY\n        renews: monthly\n\
                 \x20     - id: metered\n        kind: metered\n        credential_env: ACME_API_KEY\n{EVIDENCE}"
            ),
            "vendor 'acme' funding source 'grant' declares no evidence",
        );
        let kept: Vec<&str> = c.vendors[0].funding.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(kept, ["metered"], "the readable source stays");

        set_aside(
            "      - id: grant\n        kind: credit_grant\n        credential_env: ACME_API_KEY\n        evidence:\n          url: https://acme.example/docs\n",
            "'grant' has evidence without its checked",
        );
    }

    #[test]
    fn a_funding_source_of_an_unknown_kind_is_set_aside_by_name() {
        set_aside(
            &format!("      - id: voucher\n        kind: voucher\n        credential_env: ACME_API_KEY\n{EVIDENCE}"),
            "vendor 'acme' funding source 'voucher' declares kind \"voucher\"",
        );
        set_aside(
            &format!("      - id: grant\n        kind: metered\n        amount: 200\n{EVIDENCE}"),
            "'grant' cannot be read",
        );
    }

    /// No field of a funding declaration could carry what the owner pays or holds: the
    /// catalogue states what a vendor offers, never an amount.
    #[test]
    fn no_field_of_the_catalogue_can_hold_an_amount() {
        let schema = serde_json::to_value(schemars::schema_for!(ModelCatalogue)).unwrap();
        let text = schema.to_string().to_lowercase();
        assert!(
            text.contains("\"claim_url\""),
            "the schema carries the funding fields"
        );
        for word in [
            "amount", "price", "balance", "cost", "currency", "usd", "limit", "cap\"",
        ] {
            assert!(
                !text.contains(&format!("\"{word}")),
                "the catalogue schema declares a field that could hold an amount: {word}"
            );
        }
    }

    #[test]
    fn every_credential_variable_is_named_once() {
        let c = with_funding(&format!(
            "      - id: seat\n        kind: subscription\n        credential_env: ACME_SEAT\n{EVIDENCE}\
             \x20     - id: metered\n        kind: metered\n        credential_env: ACME_API_KEY\n{EVIDENCE}"
        ))
        .unwrap();
        let mut c = c;
        c.vendors[0].credential_env = Some("ACME_API_KEY".into());
        assert_eq!(c.credential_variables(), ["ACME_API_KEY", "ACME_SEAT"]);
    }

    #[test]
    fn funding_that_disagrees_with_itself_is_reported_not_repaired() {
        let c = with_funding(&format!(
            "      - id: grant\n        kind: credit_grant\n        credential_env: ACME_API_KEY\n{EVIDENCE}\
             \x20     - id: grant\n        kind: local\n        evidence:\n          url: x\n          checked: last week\n"
        ))
        .unwrap();
        let findings = c.diagnostics();
        assert!(
            findings
                .iter()
                .any(|f| f.contains("declares no renewal period")),
            "{findings:?}"
        );
        assert!(
            findings.iter().any(|f| f.contains("is declared twice")),
            "{findings:?}"
        );
        assert!(
            findings
                .iter()
                .any(|f| f.contains("kind Local on a vendor whose inference is Remote")),
            "{findings:?}"
        );
        assert!(
            findings
                .iter()
                .any(|f| f.contains("'last week', which is not a YYYY-MM-DD day")),
            "{findings:?}"
        );
    }

    /// One credential may reach two sources: a grant and metered billing both arrive
    /// through the vendor's API key, and the catalogue says so rather than choosing.
    #[test]
    fn two_sources_may_share_one_credential() {
        let c = with_funding(&format!(
            "      - id: grant\n        kind: credit_grant\n        credential_env: ACME_API_KEY\n        renews: monthly\n{EVIDENCE}\
             \x20     - id: metered\n        kind: metered\n        credential_env: ACME_API_KEY\n{EVIDENCE}"
        ))
        .unwrap();
        assert!(c.diagnostics().is_empty(), "{:?}", c.diagnostics());
        let json = serde_json::to_value(&c.vendors[0]).unwrap();
        assert_eq!(json["funding"][0]["kind"], "credit_grant");
        assert_eq!(json["funding"][1]["evidence"]["checked"], "2026-10-21");
    }

    /// The distribution's own catalogue loads, and every source it declares is sound.
    #[test]
    fn the_shipped_catalogue_declares_its_funding_soundly() {
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        let c = ModelCatalogue::load(&share).unwrap();
        assert!(c.diagnostics().is_empty(), "{:?}", c.diagnostics());
        let anthropic = c.vendors.iter().find(|v| v.id == "anthropic").unwrap();
        for kind in [
            FundingKind::Subscription,
            FundingKind::CreditGrant,
            FundingKind::Metered,
        ] {
            assert!(
                anthropic.funding.iter().any(|s| s.kind == kind),
                "anthropic declares {kind:?}"
            );
        }
        for vendor in &c.vendors {
            assert!(
                !vendor.funding.is_empty(),
                "vendor '{}' declares how it is paid",
                vendor.id
            );
        }
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
