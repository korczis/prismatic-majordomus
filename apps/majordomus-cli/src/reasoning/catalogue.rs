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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The file name under the share directory.
pub const ADVISORS_FILE: &str = "advisors.yaml";

/// How an advisor is reached.
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
    /// Whether consulting through this transport leaves the machine.
    pub fn is_remote(self) -> bool {
        matches!(self, Self::Api | Self::Cli | Self::Peer)
    }
}

/// One advisory capability word of the declared vocabulary.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdvisoryCapability {
    /// The word, e.g. `independent_reasoning`.
    pub id: String,
    /// What asking for it means.
    pub description: String,
}

/// One declared advisor.
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

/// The whole catalogue, as `share/advisors.yaml` declares it.
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

/// The names the catalogue's references must resolve against.
#[derive(Debug, Clone, Default)]
pub struct References {
    /// Provider ids of `share/providers.yaml`.
    pub providers: Vec<String>,
    /// The model catalogue.
    pub models: crate::models::ModelCatalogue,
}

impl AdvisorCatalogue {
    /// Read `<share>/advisors.yaml`. An absent file is an empty catalogue: a distribution
    /// that declares no advisors has none, and reasoning works without them.
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
    /// read, and the model catalogue of the same share.
    pub fn references(
        share: &std::path::Path,
        providers: &crate::share::ProviderDeclarations,
    ) -> References {
        References {
            providers: providers.providers.iter().map(|p| p.id.clone()).collect(),
            models: crate::models::ModelCatalogue::load(share).unwrap_or_default(),
        }
    }

    /// The declared advisor of an id.
    pub fn get(&self, id: &str) -> Option<&AdvisorDeclaration> {
        self.advisors.iter().find(|a| a.id == id)
    }

    /// Whether a word is in the vocabulary.
    pub fn knows_capability(&self, word: &str) -> bool {
        self.capabilities.iter().any(|c| c.id == word)
    }

    /// The credential variable whose presence says an API advisor is configured: its
    /// vendor's, as the model catalogue declares it. Never a value, only a name.
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
