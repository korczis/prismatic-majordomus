//! The advisor catalogue: `share/advisors.yaml`, one declaration of every advisory role a
//! session may consult, in the grain of `share/providers.yaml` and `share/models.yaml`.
//!
//! An advisor is not a provider and not a vendor. A *provider* is a client tool that works
//! in this repository (ADR 0024); a *vendor* serves models (ADR 0049); an *advisor* is a
//! role — something a session may ask for an independent opinion — reached through one
//! transport. The catalogue therefore holds only what neither neighbour knows: the
//! transport, the adapter that speaks it, the executable whose presence says it is
//! installed, and the advisory capabilities it offers. Everything else is a reference —
//! a provider id, a vendor id, a model id — and [`AdvisorCatalogue::diagnostics`] refuses
//! a reference that resolves to nothing, so the three tables cannot fork.
//!
//! Declaration order is the preference order. That is where a repository's "ask this one
//! first" lives: as data the policy reads, never as a name the policy knows.
//!
//! # Example
//!
//! A share directory declaring two advisors and the vendor one of them references: load,
//! resolve the references, check the catalogue, then look an advisor up.
//!
//! ```
//! use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, ADVISORS_FILE};
//! use majordomus_cli::share::ProviderDeclarations;
//!
//! let share = tempfile::tempdir().unwrap();
//! std::fs::write(share.path().join(ADVISORS_FILE), "version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: api\n    adapter: a-api\n    vendor: acme\n    capabilities: [code_review]\n  - id: b\n    title: B\n    transport: cli\n    adapter: b-cli\n    executable: b\n    capabilities: [code_review]\n").unwrap();
//! std::fs::write(share.path().join("models.yaml"), "version: 1\nvendors:\n  - id: acme\n    title: Acme\n    credential_env: ACME_KEY\nmodels: []\n").unwrap();
//!
//! let catalogue = AdvisorCatalogue::load(share.path()).unwrap();
//! let refs = AdvisorCatalogue::references(share.path(), &ProviderDeclarations::default());
//! assert!(catalogue.diagnostics(&refs).is_empty());
//!
//! let ids: Vec<&str> = catalogue.advisors.iter().map(|a| a.id.as_str()).collect();
//! assert_eq!(ids, ["a", "b"], "declaration order is preference order");
//! let a = catalogue.get("a").unwrap();
//! assert_eq!(catalogue.credential_of(a, &refs), Some("ACME_KEY"));
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The file name under the share directory.
pub const ADVISORS_FILE: &str = "advisors.yaml";

/// How an advisor is reached. The transport decides how its presence is probed (a
/// credential variable, an executable, a mesh link) and whether `offline` mode admits it.
///
/// ```
/// use majordomus_cli::reasoning::catalogue::AdvisorTransport;
///
/// let t: AdvisorTransport = serde_json::from_str("\"local_runtime\"").unwrap();
/// assert_eq!(t, AdvisorTransport::LocalRuntime);
/// assert!(!t.is_remote());
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum AdvisorTransport {
    /// A remote API; present when its vendor's credential variable is set.
    Api,
    /// A client tool run as a process; present when its executable is on `PATH`.
    Cli,
    /// A model runtime on this machine; present when its executable is on `PATH`.
    LocalRuntime,
    /// A linked mesh runtime carrying the review feature; present while linked.
    Peer,
}

impl AdvisorTransport {
    /// Whether consulting through this transport leaves the machine: an API, a client
    /// tool (which calls its own service) and a mesh peer do; a local runtime does not.
    /// `offline` mode admits only transports that are not remote.
    ///
    /// ```
    /// use majordomus_cli::reasoning::catalogue::AdvisorTransport;
    ///
    /// assert!(AdvisorTransport::Api.is_remote());
    /// assert!(AdvisorTransport::Peer.is_remote());
    /// assert!(!AdvisorTransport::LocalRuntime.is_remote());
    /// ```
    pub fn is_remote(self) -> bool {
        matches!(self, Self::Api | Self::Cli | Self::Peer)
    }
}

/// One advisory capability word of the declared vocabulary: what a plan requests and an
/// advisor offers. A word outside the vocabulary is a catalogue finding.
///
/// ```
/// use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, AdvisoryCapability};
///
/// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
///     "version: 1\ncapabilities:\n  - id: code_review\n    description: Critique of a change.\n",
/// ).unwrap();
/// let word: &AdvisoryCapability = &c.capabilities[0];
/// assert_eq!((word.id.as_str(), word.description.as_str()), ("code_review", "Critique of a change."));
/// assert!(c.knows_capability("code_review"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryCapability {
    /// The word, e.g. `independent_reasoning`.
    pub id: String,
    /// What asking for it means.
    pub description: String,
}

/// One declared advisor: its id, how it is reached and through which adapter, what it
/// references in the provider and model tables, the executable that says it is
/// installed, and the advisory capabilities it offers.
///
/// ```
/// use majordomus_cli::reasoning::catalogue::{AdvisorDeclaration, AdvisorTransport};
///
/// let a: AdvisorDeclaration = majordomus_cli::metadata::yaml::parse_into(
///     "id: b\ntitle: B\ntransport: cli\nadapter: b-cli\nexecutable: b\ncapabilities: [code_review]\n",
/// ).unwrap();
/// assert_eq!(a.transport, AdvisorTransport::Cli);
/// assert_eq!(a.executable.as_deref(), Some("b"));
/// assert!(a.vendor.is_none() && a.model.is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisorDeclaration {
    /// The advisor id, unique in the catalogue: what records and configuration name.
    pub id: String,
    /// The human name.
    pub title: String,
    /// How it is reached.
    pub transport: AdvisorTransport,
    /// The transport adapter that speaks to it: a module `scripts/lib/advisors/<adapter>.mjs`.
    pub adapter: String,
    /// The provider it runs as (`share/providers.yaml`), for a client tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// The vendor that serves it (`share/models.yaml`), for an API or a local runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// The model it asks (`share/models.yaml`), when the catalogue declares it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The executable whose presence on `PATH` says it is installed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// The advisory capabilities it offers, from the declared vocabulary.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// One line a person needs when choosing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// How linked mesh runtimes become advisors: those carrying `feature`, with these
/// capabilities, through this adapter.
///
/// ```
/// use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, PeerAdvisors};
///
/// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
///     "version: 1\npeers:\n  feature: reviews\n  adapter: mesh-review\n  capabilities: [code_review]\n",
/// ).unwrap();
/// let peers: &PeerAdvisors = c.peers.as_ref().unwrap();
/// assert_eq!(peers.feature, "reviews");
/// // a capability outside the (here empty) vocabulary is a finding
/// assert!(c.diagnostics(&Default::default()).iter().any(|f| f.contains("'code_review'")));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PeerAdvisors {
    /// The link feature a runtime must carry.
    pub feature: String,
    /// The adapter that carries a request to it.
    pub adapter: String,
    /// The advisory capabilities a reviewing peer offers.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// The whole catalogue, as `share/advisors.yaml` declares it: the capability vocabulary,
/// the advisors in preference order, how mesh peers become advisors, and the claims no
/// document may assert. The default is the empty catalogue a share without the file has.
///
/// ```
/// use majordomus_cli::reasoning::catalogue::AdvisorCatalogue;
///
/// let empty = AdvisorCatalogue::default();
/// assert!(empty.advisors.is_empty() && empty.get("anything").is_none());
/// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
///     "version: 1\nforbidden_claims: [consensus guarantees correctness]\n",
/// ).unwrap();
/// assert_eq!(c.forbidden_claims, ["consensus guarantees correctness"]);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisorCatalogue {
    /// The format version.
    pub version: u32,
    /// The advisory capability vocabulary.
    #[serde(default)]
    pub capabilities: Vec<AdvisoryCapability>,
    /// The advisors, in preference order.
    #[serde(default)]
    pub advisors: Vec<AdvisorDeclaration>,
    /// Linked mesh runtimes as advisors, when declared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peers: Option<PeerAdvisors>,
    /// Phrases no document of the repository may assert: the claims this design
    /// refuses ("consensus guarantees correctness"), checked by `reasoning.check`.
    #[serde(default)]
    pub forbidden_claims: Vec<String>,
}

/// The names the catalogue's references must resolve against: the provider ids of
/// `share/providers.yaml` and the model catalogue of `share/models.yaml`.
///
/// ```
/// use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, References};
///
/// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
///     "version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: cli\n    adapter: a-cli\n    provider: tool\n    executable: a\n    capabilities: [code_review]\n",
/// ).unwrap();
/// assert!(!c.diagnostics(&References::default()).is_empty());
/// let refs = References { providers: vec!["tool".into()], ..Default::default() };
/// assert!(c.diagnostics(&refs).is_empty());
/// ```
#[derive(Debug, Clone, Default)]
pub struct References {
    /// Provider ids of `share/providers.yaml`.
    pub providers: Vec<String>,
    /// The model catalogue.
    pub models: crate::models::ModelCatalogue,
}

impl AdvisorCatalogue {
    /// Read `<share>/advisors.yaml`. An absent file is an empty catalogue: a distribution
    /// that declares no advisors has none, and reasoning works without them. A file that
    /// does not parse is an error naming it.
    ///
    /// ```
    /// use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, ADVISORS_FILE};
    ///
    /// let share = tempfile::tempdir().unwrap();
    /// assert!(AdvisorCatalogue::load(share.path()).unwrap().advisors.is_empty());
    /// std::fs::write(share.path().join(ADVISORS_FILE), "version: one\n").unwrap();
    /// let refused = AdvisorCatalogue::load(share.path()).unwrap_err();
    /// assert!(refused.contains(ADVISORS_FILE));
    /// ```
    pub fn load(share: &std::path::Path) -> Result<Self, String> {
        let path = share.join(ADVISORS_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => crate::metadata::yaml::parse_into(&text)
                .map_err(|reason| format!("{}: {reason}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The references the catalogue resolves against: the provider ids the index already
    /// read, and the model catalogue of the same share (empty when it cannot be read).
    ///
    /// ```
    /// use majordomus_cli::reasoning::catalogue::AdvisorCatalogue;
    /// use majordomus_cli::share::{ProviderDeclaration, ProviderDeclarations};
    ///
    /// let share = tempfile::tempdir().unwrap();
    /// let providers = ProviderDeclarations {
    ///     providers: vec![ProviderDeclaration { id: "tool".into(), ..Default::default() }],
    ///     ..Default::default()
    /// };
    /// let refs = AdvisorCatalogue::references(share.path(), &providers);
    /// assert_eq!(refs.providers, ["tool"]);
    /// assert!(refs.models.vendors.is_empty());
    /// ```
    pub fn references(
        share: &std::path::Path,
        providers: &crate::share::ProviderDeclarations,
    ) -> References {
        References {
            providers: providers.providers.iter().map(|p| p.id.clone()).collect(),
            models: crate::models::ModelCatalogue::load(share).unwrap_or_default(),
        }
    }

    /// The declared advisor of an id; `None` for an id the catalogue does not declare,
    /// which includes every `peer:` id.
    ///
    /// ```
    /// use majordomus_cli::reasoning::catalogue::AdvisorCatalogue;
    ///
    /// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
    ///     "version: 1\nadvisors:\n  - id: a\n    title: A\n    transport: cli\n    adapter: a-cli\n",
    /// ).unwrap();
    /// assert_eq!(c.get("a").map(|a| a.title.as_str()), Some("A"));
    /// assert!(c.get("peer:n1-r1").is_none());
    /// ```
    pub fn get(&self, id: &str) -> Option<&AdvisorDeclaration> {
        self.advisors.iter().find(|a| a.id == id)
    }

    /// Whether a word is in the declared capability vocabulary: what `reasoning.plan`
    /// checks a requested capability against before it plans.
    ///
    /// ```
    /// use majordomus_cli::reasoning::catalogue::AdvisorCatalogue;
    ///
    /// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
    ///     "version: 1\ncapabilities:\n  - id: code_review\n    description: x\n",
    /// ).unwrap();
    /// assert!(c.knows_capability("code_review"));
    /// assert!(!c.knows_capability("telepathy"));
    /// ```
    pub fn knows_capability(&self, word: &str) -> bool {
        self.capabilities.iter().any(|c| c.id == word)
    }

    /// The credential variable whose presence says an API advisor is configured: its
    /// vendor's, as the model catalogue declares it. Never a value, only a name. `None`
    /// for an advisor with no vendor, or whose vendor declares no variable.
    ///
    /// ```
    /// use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, References};
    ///
    /// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into("version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: api\n    adapter: a-api\n    vendor: acme\n    capabilities: [code_review]\n  - id: b\n    title: B\n    transport: cli\n    adapter: b-cli\n    executable: b\n    capabilities: [code_review]\n",
    /// ).unwrap();
    /// let refs = References {
    ///     models: majordomus_cli::metadata::yaml::parse_into("version: 1\nvendors:\n  - id: acme\n    title: Acme\n    credential_env: ACME_KEY\nmodels: []\n").unwrap(),
    ///     ..Default::default()
    /// };
    /// assert_eq!(c.credential_of(c.get("a").unwrap(), &refs), Some("ACME_KEY"));
    /// assert_eq!(c.credential_of(c.get("b").unwrap(), &refs), None);
    /// ```
    pub fn credential_of<'a>(
        &self,
        advisor: &AdvisorDeclaration,
        refs: &'a References,
    ) -> Option<&'a str> {
        let vendor = advisor.vendor.as_deref()?;
        refs.models
            .vendors
            .iter()
            .find(|v| v.id == vendor)
            .and_then(|v| v.credential_env.as_deref())
    }

    /// The catalogue's own consistency: duplicate ids, words outside the vocabulary, a
    /// reference resolving to nothing, an advisor whose presence cannot be probed.
    /// Reported, never repaired; empty is the ordinary case.
    ///
    /// ```
    /// use majordomus_cli::reasoning::catalogue::{AdvisorCatalogue, References};
    ///
    /// let c: AdvisorCatalogue = majordomus_cli::metadata::yaml::parse_into(
    ///     "version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: cli\n    adapter: a-cli\n    provider: nowhere\n    executable: a\n    capabilities: [code_review, telepathy]\n",
    /// ).unwrap();
    /// let found = c.diagnostics(&References::default());
    /// assert!(found.iter().any(|f| f.contains("provider 'nowhere'")));
    /// assert!(found.iter().any(|f| f.contains("'telepathy'")));
    /// ```
    pub fn diagnostics(&self, refs: &References) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for a in &self.advisors {
            if !seen.insert(a.id.as_str()) {
                out.push(format!("advisor '{}' is declared twice", a.id));
            }
            if a.capabilities.is_empty() {
                out.push(format!("advisor '{}' offers no capability", a.id));
            }
            for word in &a.capabilities {
                if !self.knows_capability(word) {
                    out.push(format!(
                        "advisor '{}' offers '{word}', which the vocabulary does not declare",
                        a.id
                    ));
                }
            }
            if let Some(p) = &a.provider {
                if !refs.providers.iter().any(|known| known == p) {
                    out.push(format!(
                        "advisor '{}' names provider '{p}', which share/providers.yaml does not declare",
                        a.id
                    ));
                }
            }
            if let Some(v) = &a.vendor {
                if !refs.models.vendors.iter().any(|known| &known.id == v) {
                    out.push(format!(
                        "advisor '{}' names vendor '{v}', which share/models.yaml does not declare",
                        a.id
                    ));
                }
            }
            if let Some(m) = &a.model {
                match refs.models.resolve(m) {
                    None => out.push(format!(
                        "advisor '{}' names model '{m}', which share/models.yaml does not declare",
                        a.id
                    )),
                    Some(entry) if a.vendor.as_deref().is_some_and(|v| v != entry.vendor) => out
                        .push(format!(
                            "advisor '{}' names model '{m}' of vendor '{}', not of its own vendor",
                            a.id, entry.vendor
                        )),
                    Some(_) => {}
                }
            }
            match a.transport {
                AdvisorTransport::Api => {
                    if self.credential_of(a, refs).is_none() {
                        out.push(format!(
                            "advisor '{}' is an API advisor whose vendor declares no credential variable, so its presence cannot be probed",
                            a.id
                        ));
                    }
                }
                AdvisorTransport::Cli | AdvisorTransport::LocalRuntime => {
                    if a.executable.is_none() {
                        out.push(format!(
                            "advisor '{}' declares no executable, so its presence cannot be probed",
                            a.id
                        ));
                    }
                }
                AdvisorTransport::Peer => out.push(format!(
                    "advisor '{}' is declared with transport peer; peers are discovered from the mesh, never listed",
                    a.id
                )),
            }
        }
        if let Some(p) = &self.peers {
            for word in &p.capabilities {
                if !self.knows_capability(word) {
                    out.push(format!(
                        "peer advisors offer '{word}', which the vocabulary does not declare"
                    ));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue(yaml: &str) -> AdvisorCatalogue {
        crate::metadata::yaml::parse_into(yaml).unwrap()
    }

    #[test]
    fn an_absent_file_is_an_empty_catalogue_and_a_present_one_is_read() {
        let share = tempfile::tempdir().unwrap();
        let empty = AdvisorCatalogue::load(share.path()).unwrap();
        assert!(empty.advisors.is_empty() && empty.diagnostics(&References::default()).is_empty());
        std::fs::write(
            share.path().join(ADVISORS_FILE),
            "version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: local_runtime\n    adapter: a-rt\n    executable: rt\n    capabilities: [code_review]\n",
        )
        .unwrap();
        let read = AdvisorCatalogue::load(share.path()).unwrap();
        assert_eq!(
            read.get("a").map(|a| a.transport),
            Some(AdvisorTransport::LocalRuntime)
        );
        assert!(read.diagnostics(&References::default()).is_empty());
    }

    #[test]
    fn an_unknown_field_is_refused_rather_than_ignored() {
        let share = tempfile::tempdir().unwrap();
        std::fs::write(share.path().join(ADVISORS_FILE), "version: 1\nquorum: 2\n").unwrap();
        assert!(AdvisorCatalogue::load(share.path()).is_err());
    }

    #[test]
    fn diagnostics_name_every_unprobeable_or_unresolved_advisor() {
        let c = catalogue(
            "version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: api\n    adapter: a-api\n    vendor: ghost\n    capabilities: [code_review]\n  - id: a\n    title: A again\n    transport: cli\n    adapter: a-cli\n    capabilities: []\n  - id: p\n    title: P\n    transport: peer\n    adapter: mesh\n    capabilities: [code_review]\n",
        );
        let found = c.diagnostics(&References::default());
        for needle in [
            "vendor 'ghost'",
            "declares no credential variable",
            "'a' is declared twice",
            "'a' offers no capability",
            "'a' declares no executable",
            "transport peer",
        ] {
            assert!(
                found.iter().any(|f| f.contains(needle)),
                "{needle} missing from {found:?}"
            );
        }
    }

    #[test]
    fn a_model_of_another_vendor_is_a_finding() {
        let c = catalogue(
            "version: 1\ncapabilities:\n  - id: code_review\n    description: x\nadvisors:\n  - id: a\n    title: A\n    transport: api\n    adapter: a-api\n    vendor: acme\n    model: other-large\n    capabilities: [code_review]\n",
        );
        let refs = References {
            providers: vec![],
            models: crate::metadata::yaml::parse_into(
                "version: 1\nvendors:\n  - id: acme\n    title: Acme\n    credential_env: ACME_KEY\n  - id: other\n    title: Other\nmodels:\n  - id: other-large\n    vendor: other\n    native_id: other-large-1\n    context_window: 1000\n    capabilities: [text]\n",
            )
            .unwrap(),
        };
        let found = c.diagnostics(&refs);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("of vendor 'other'"));
    }
}
