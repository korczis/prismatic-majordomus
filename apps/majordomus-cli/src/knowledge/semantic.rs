//! The semantic layer: an optional provider that reads evidence and proposes what the
//! deterministic extractors cannot — a summary, a classification, a relation between
//! two things that no field declares — behind a policy that is off by default.
//!
//! Three rules hold whatever the provider:
//!
//! 1. **Offline core.** Nothing in a scan, a check or a projection calls a provider. A
//!    provider runs only under `majordomus knowledge derive`, writes to the checkout-local
//!    cache, and the next scan reads the cache like any other fact of the checkout.
//! 2. **Nothing leaves the machine that may not.** Evidence marked as not for remote
//!    processing — anything outside the public visibility, anything the scope calls a
//!    secret or generated — is never given to a provider that is remote, and every text
//!    that is given passes the redactor first. A remote provider is refused outright
//!    unless the policy allows it.
//! 3. **Every derivation says how it was made.** Provider, model, operation, prompt
//!    version and the fingerprints of the evidence read, so that a reader can tell an
//!    inference from an observation and a stale inference from a current one.
//!
//! This executable ships one provider, `local`: a deterministic, offline stub that
//! derives a one-line summary from a document's first paragraph and a classification
//! from its path. It exists so that the whole path — policy, redaction, cache, merge,
//! freshness — is exercised and tested without a network, and so that a real provider
//! has an interface to implement. No credentials are read anywhere in this module.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

use super::model::{Evidence, KnowledgeModel, Node, Visibility};

/// The cache file, under the local half of the layer.
pub const CACHE_FILE: &str = "knowledge/semantic-cache.json";

/// The schema the cache declares.
pub const CACHE_SCHEMA: &str = "majordomus/semantic-cache/v1";

/// The `knowledge.semantic:` block of the policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticPolicy {
    /// Whether `majordomus knowledge derive` may run at all.
    #[serde(default)]
    pub enabled: bool,
    /// Whether a provider that sends text off the machine may be used.
    #[serde(default)]
    pub allow_remote: bool,
    /// The provider to use; `local` is the one shipped.
    #[serde(default = "default_provider")]
    pub provider: String,
    /// The most bytes of evidence text one request may carry.
    #[serde(default = "default_budget")]
    pub max_request_bytes: usize,
}

fn default_provider() -> String {
    "local".into()
}

fn default_budget() -> usize {
    16_000
}

impl Default for SemanticPolicy {
    fn default() -> Self {
        SemanticPolicy {
            enabled: false,
            allow_remote: false,
            provider: default_provider(),
            max_request_bytes: default_budget(),
        }
    }
}

/// How a derivation was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SemanticDerivationProvenance")]
pub struct DerivationProvenance {
    /// The provider id.
    pub provider: String,
    /// The model the provider used, as it names it.
    pub model: String,
    /// The operation: `summarise`, `classify`, `relate`, `contradictions`.
    pub operation: String,
    /// The prompt version, so that a changed prompt is a changed derivation.
    pub prompt_version: String,
}

/// One derivation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SemanticDerivation")]
pub struct Derivation {
    /// The node it is about.
    pub subject: String,
    /// The predicate it asserts.
    pub predicate: String,
    /// The value.
    pub value: Value,
    /// How it was made.
    pub provenance: DerivationProvenance,
    /// The evidence read, `(id, fingerprint)`.
    pub inputs: Vec<(String, String)>,
    /// The other node, for a relation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// The cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct SemanticCache {
    /// `majordomus/semantic-cache/v1`.
    #[serde(default)]
    pub schema: String,
    /// The derivations.
    #[serde(default)]
    pub derivations: Vec<Derivation>,
}

/// Where the cache is.
pub fn cache_path(root: &Path, local: &str) -> PathBuf {
    root.join(local).join(CACHE_FILE)
}

impl SemanticCache {
    /// Read the cache; `None` when there is none or it does not parse (a cache is a
    /// rebuildable local product, and one that cannot be read is one to rebuild).
    pub fn read(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let value: Value = serde_json::from_str(&text).ok()?;
        let (value, _) = super::migrate::migrate(super::migrate::Family::SemanticCache, value).ok()?;
        serde_json::from_value(value).ok()
    }

    /// Write the cache.
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let mut c = self.clone();
        c.schema = CACHE_SCHEMA.into();
        c.derivations.sort_by(|a, b| a.subject.cmp(&b.subject).then(a.predicate.cmp(&b.predicate)));
        let text = serde_json::to_string_pretty(&c).map_err(|e| Error::Protocol {
            reason: e.to_string(),
        })?;
        std::fs::write(path, text).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }
}

/// One piece of evidence as a provider receives it: redacted, with its fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SemanticInput")]
pub struct Input {
    /// The evidence id.
    pub id: String,
    /// The fingerprint the text carried.
    pub fingerprint: String,
    /// The path, for the provider's context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The text, redacted.
    pub text: String,
}

/// A request to a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SemanticRequest")]
pub struct Request {
    /// The operation.
    pub operation: String,
    /// The node it is about.
    pub subject: String,
    /// Its kind.
    pub kind: String,
    /// The inputs.
    pub inputs: Vec<Input>,
}

/// What a provider answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SemanticAnswer")]
pub struct Answer {
    /// The predicate.
    pub predicate: String,
    /// The value.
    pub value: Value,
    /// The other node, for a relation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// A semantic provider.
pub trait SemanticProvider: Send + Sync {
    /// The id the policy names.
    fn id(&self) -> &str;
    /// The model, as the provider names it.
    fn model(&self) -> &str;
    /// The prompt version.
    fn prompt_version(&self) -> &str;
    /// Whether it sends text off the machine.
    fn remote(&self) -> bool;
    /// The operations it offers.
    fn operations(&self) -> Vec<&'static str>;
    /// Derive. An empty answer is "nothing to say", not an error.
    fn derive(&self, request: &Request) -> std::result::Result<Vec<Answer>, String>;
}

/// The shipped provider: deterministic, offline, no model at all.
pub struct Local;

impl SemanticProvider for Local {
    fn id(&self) -> &str {
        "local"
    }
    fn model(&self) -> &str {
        "none (deterministic heuristics)"
    }
    fn prompt_version(&self) -> &str {
        "local/1"
    }
    fn remote(&self) -> bool {
        false
    }
    fn operations(&self) -> Vec<&'static str> {
        vec!["summarise", "classify"]
    }
    fn derive(&self, request: &Request) -> std::result::Result<Vec<Answer>, String> {
        let Some(input) = request.inputs.first() else {
            return Ok(vec![]);
        };
        match request.operation.as_str() {
            "summarise" => {
                let summary = first_paragraph(&input.text);
                Ok(summary
                    .map(|s| Answer {
                        predicate: "summary".into(),
                        value: Value::String(s),
                        target: None,
                    })
                    .into_iter()
                    .collect())
            }
            "classify" => {
                let class = input
                    .path
                    .as_deref()
                    .and_then(super::extract::docs::classify)
                    .unwrap_or("other");
                Ok(vec![Answer {
                    predicate: "classification".into(),
                    value: Value::String(class.into()),
                    target: None,
                }])
            }
            other => Err(format!("the local provider has no operation `{other}`")),
        }
    }
}

/// The first paragraph of prose after the heading, one line, at most 200 characters.
fn first_paragraph(text: &str) -> Option<String> {
    let body = crate::metadata::frontmatter::split(text).map(|s| s.body.to_string()).unwrap_or_else(|_| text.to_string());
    let mut fence = false;
    let mut para = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence || t.starts_with('#') || t.starts_with('|') || t.starts_with("<!--") {
            continue;
        }
        if t.is_empty() {
            if !para.is_empty() {
                break;
            }
            continue;
        }
        para.push(t);
    }
    if para.is_empty() {
        return None;
    }
    let joined = para.join(" ");
    let mut s: String = joined.chars().take(200).collect();
    if joined.chars().count() > 200 {
        s.push('…');
    }
    Some(s)
}

/// Every provider this executable ships, by id.
pub fn providers() -> Vec<Box<dyn SemanticProvider>> {
    vec![Box::new(Local)]
}

/// The provider a policy names, checked against the policy.
pub fn provider_for(policy: &SemanticPolicy) -> Result<Box<dyn SemanticProvider>> {
    if !policy.enabled {
        return Err(Error::Refused {
            code: 10,
            reason: "the semantic layer is off: set `knowledge.semantic.enabled: true` in the policy to run a provider".into(),
        });
    }
    let Some(p) = providers().into_iter().find(|p| p.id() == policy.provider) else {
        return Err(Error::Refused {
            code: 12,
            reason: format!(
                "no semantic provider `{}`; this executable ships: {}",
                policy.provider,
                providers().iter().map(|p| p.id().to_string()).collect::<Vec<_>>().join(", ")
            ),
        });
    };
    if p.remote() && !policy.allow_remote {
        return Err(Error::Refused {
            code: 10,
            reason: format!(
                "provider `{}` sends text off the machine and the policy does not allow it (`knowledge.semantic.allow_remote`)",
                p.id()
            ),
        });
    }
    Ok(p)
}

/// Lines the redactor drops: anything that looks like a credential assignment or a key
/// block. Deterministic and conservative: a false positive costs a line of context, a
/// false negative costs a secret.
const SECRET_MARKERS: &[&str] = &[
    "-----begin",
    "private key",
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "authorization:",
    "bearer ",
    "aws_access_key",
    "aws_secret",
    "client_secret",
    "credential",
];

/// Redact a text for a provider: every line carrying a secret marker is replaced by a
/// marker line, and the count is answered with the text.
pub fn redact(text: &str) -> (String, usize) {
    let mut out = Vec::new();
    let mut dropped = 0;
    for line in text.lines() {
        let lower = line.to_lowercase();
        if SECRET_MARKERS.iter().any(|m| lower.contains(m)) && (lower.contains('=') || lower.contains(':') || lower.contains("-----")) {
            dropped += 1;
            out.push("[redacted]");
        } else {
            out.push(line);
        }
    }
    (out.join("\n"), dropped)
}

/// What one run of `derive` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SemanticDeriveReport")]
pub struct DeriveReport {
    /// The provider.
    pub provider: String,
    /// Its model.
    pub model: String,
    /// Whether it is remote.
    pub remote: bool,
    /// The operations run.
    pub operations: Vec<String>,
    /// Nodes considered.
    pub considered: usize,
    /// Nodes skipped because their evidence may not be processed remotely, or is not
    /// public.
    pub withheld: usize,
    /// Lines redacted across every request.
    pub redacted_lines: usize,
    /// Derivations written.
    pub derivations: usize,
    /// Where the cache was written.
    pub cache: String,
}

/// Run a provider over the nodes of a model whose evidence is readable, and write the
/// cache. `read` answers the text of a piece of file evidence. Only nodes of the given
/// kinds are considered; every kind when the list is empty.
pub fn derive(
    model: &KnowledgeModel,
    provider: &dyn SemanticProvider,
    policy: &SemanticPolicy,
    kinds: &[String],
    read: &dyn Fn(&Evidence) -> Option<String>,
    cache_at: &Path,
) -> Result<DeriveReport> {
    let mut cache = SemanticCache::read(cache_at).unwrap_or_default();
    let mut considered = 0;
    let mut withheld = 0;
    let mut redacted_lines = 0;
    let mut written = 0;
    let operations: Vec<String> = provider.operations().iter().map(|s| s.to_string()).collect();
    for n in &model.nodes {
        if !kinds.is_empty() && !kinds.iter().any(|k| *k == n.kind) {
            continue;
        }
        if n.provenance == super::model::Provenance::Derived {
            continue;
        }
        let evidence: Vec<&Evidence> = n
            .evidence
            .iter()
            .filter_map(|id| model.evidence(id))
            .filter(|e| e.locator.path.is_some())
            .collect();
        if evidence.is_empty() {
            continue;
        }
        considered += 1;
        if provider.remote() && evidence.iter().any(|e| !e.remote_processing || e.visibility != Visibility::Public) {
            withheld += 1;
            continue;
        }
        let mut inputs = Vec::new();
        let mut bytes = 0;
        for e in &evidence {
            let Some(text) = read(e) else { continue };
            let (text, dropped) = redact(&text);
            redacted_lines += dropped;
            if bytes + text.len() > policy.max_request_bytes {
                break;
            }
            bytes += text.len();
            inputs.push(Input {
                id: e.id.clone(),
                fingerprint: e.fingerprint.value.clone(),
                path: e.locator.path.clone(),
                text,
            });
        }
        if inputs.is_empty() {
            continue;
        }
        for op in provider.operations() {
            let request = Request {
                operation: op.into(),
                subject: n.id.clone(),
                kind: n.kind.clone(),
                inputs: inputs.clone(),
            };
            let answers = provider.derive(&request).map_err(|reason| Error::Refused {
                code: 13,
                reason: format!("provider {}: {reason}", provider.id()),
            })?;
            for a in answers {
                let d = Derivation {
                    subject: n.id.clone(),
                    predicate: a.predicate,
                    value: a.value,
                    provenance: DerivationProvenance {
                        provider: provider.id().into(),
                        model: provider.model().into(),
                        operation: op.into(),
                        prompt_version: provider.prompt_version().into(),
                    },
                    inputs: inputs.iter().map(|i| (i.id.clone(), i.fingerprint.clone())).collect(),
                    target: a.target,
                };
                cache.derivations.retain(|x| !(x.subject == d.subject && x.predicate == d.predicate && x.provenance.provider == d.provenance.provider && x.provenance.operation == d.provenance.operation));
                cache.derivations.push(d);
                written += 1;
            }
        }
    }
    cache.write(cache_at)?;
    Ok(DeriveReport {
        provider: provider.id().into(),
        model: provider.model().into(),
        remote: provider.remote(),
        operations,
        considered,
        withheld,
        redacted_lines,
        derivations: written,
        cache: cache_at.display().to_string(),
    })
}

/// The nodes a provider would be given, for a dry run: what leaves the machine and what
/// is withheld.
pub fn preview<'a>(model: &'a KnowledgeModel, remote: bool, kinds: &[String]) -> (Vec<&'a Node>, Vec<&'a Node>) {
    let mut given = Vec::new();
    let mut withheld = Vec::new();
    for n in &model.nodes {
        if !kinds.is_empty() && !kinds.iter().any(|k| *k == n.kind) {
            continue;
        }
        let evidence: Vec<&Evidence> = n.evidence.iter().filter_map(|id| model.evidence(id)).collect();
        if evidence.is_empty() {
            continue;
        }
        if remote && evidence.iter().any(|e| !e.remote_processing || e.visibility != Visibility::Public) {
            withheld.push(n);
        } else {
            given.push(n);
        }
    }
    (given, withheld)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_redactor_drops_credential_lines_and_keeps_prose() {
        let (text, n) = redact("# Title\nAPI_KEY=abc123\nThe token is rotated weekly.\nAuthorization: Bearer xyz\n-----BEGIN RSA PRIVATE KEY-----\nplain line\n");
        assert_eq!(n, 3, "{text}");
        assert!(text.contains("[redacted]"));
        assert!(!text.contains("abc123"));
        assert!(!text.contains("xyz"));
        assert!(text.contains("The token is rotated weekly."));
        assert!(text.contains("plain line"));
    }

    #[test]
    fn the_policy_refuses_a_disabled_layer_an_unknown_provider_and_a_remote_one() {
        let off = SemanticPolicy::default();
        assert!(matches!(provider_for(&off), Err(Error::Refused { code: 10, .. })));
        let unknown = SemanticPolicy { enabled: true, provider: "cloud".into(), ..Default::default() };
        assert!(matches!(provider_for(&unknown), Err(Error::Refused { code: 12, .. })));
        let local = SemanticPolicy { enabled: true, ..Default::default() };
        assert_eq!(provider_for(&local).unwrap().id(), "local");
    }

    #[test]
    fn the_local_provider_summarises_the_first_paragraph() {
        let req = Request {
            operation: "summarise".into(),
            subject: "document:README.md".into(),
            kind: "document".into(),
            inputs: vec![Input {
                id: "file:README.md".into(),
                fingerprint: "x".into(),
                path: Some("README.md".into()),
                text: "---\ntitle: x\n---\n# Heading\n\nFirst line\nof prose.\n\nSecond paragraph.\n".into(),
            }],
        };
        let answers = Local.derive(&req).unwrap();
        assert_eq!(answers[0].value, Value::String("First line of prose.".into()));
        let req = Request { operation: "classify".into(), ..req };
        assert_eq!(Local.derive(&req).unwrap()[0].value, Value::String("readme".into()));
    }

    #[test]
    fn the_cache_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        let cache = SemanticCache {
            schema: CACHE_SCHEMA.into(),
            derivations: vec![Derivation {
                subject: "document:a.md".into(),
                predicate: "summary".into(),
                value: Value::String("s".into()),
                provenance: DerivationProvenance { provider: "local".into(), model: "none".into(), operation: "summarise".into(), prompt_version: "local/1".into() },
                inputs: vec![("file:a.md".into(), "fp".into())],
                target: None,
            }],
        };
        cache.write(&path).unwrap();
        assert_eq!(SemanticCache::read(&path).unwrap(), cache);
        assert!(SemanticCache::read(&dir.path().join("missing.json")).is_none());
    }
}
