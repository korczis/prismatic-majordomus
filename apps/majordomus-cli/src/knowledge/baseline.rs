//! The knowledge baseline: the committed record of what the repository accepted about its
//! own knowledge — which evidence it was recorded against, which curated claims were
//! verified and against what, which debt is tolerated, which conflicts are accepted and
//! which canonicality violations are known — and the check that holds a scan against it.
//!
//! A brownfield repository starts with debt. The baseline is how that debt is tolerated
//! without being hidden: it is recorded once, explicitly, and from then on new debt is
//! refused while old debt may only shrink. Recording it again is an explicit act with the
//! diff in the commit; nothing records it silently.
//!
//! The file is YAML in the layer's subset: lists of maps, identifier keys, one schema
//! line. It lives in the knowledge section beside the curated records.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::metadata::yaml;
use crate::repository::Repository;

use super::model::{Freshness, KnowledgeModel};

/// A knowledge file that does not meet its contract: exit 10, with the path and the reason.
pub(crate) fn contract(path: &Path, reason: impl std::fmt::Display) -> Error {
    Error::Refused {
        code: 10,
        reason: format!("{}: {reason}", path.display()),
    }
}

/// The file name, under the knowledge section.
pub const FILE: &str = "baseline.yaml";

/// The schema the file declares.
pub const SCHEMA: &str = "majordomus/knowledge-baseline/v1";

/// How strictly the check holds a scan against the baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "KnowledgeMode")]
pub enum Mode {
    /// Report everything; never fail.
    Observe,
    /// Report new debt as warnings; never fail.
    Warn,
    /// Refuse new debt; tolerate what the baseline records.
    #[default]
    Protect,
    /// Refuse any debt at all.
    Strict,
}

impl Mode {
    /// The word the policy uses.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Observe => "observe",
            Mode::Warn => "warn",
            Mode::Protect => "protect",
            Mode::Strict => "strict",
        }
    }

    /// The mode a word names.
    pub fn parse(word: &str) -> Option<Mode> {
        match word {
            "observe" => Some(Mode::Observe),
            "warn" => Some(Mode::Warn),
            "protect" => Some(Mode::Protect),
            "strict" => Some(Mode::Strict),
            _ => None,
        }
    }
}

/// The `knowledge:` block of the policy, typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
pub struct KnowledgePolicy {
    /// The check's mode; `protect` when unset.
    #[serde(default)]
    pub mode: Mode,
    /// The semantic layer's switches.
    #[serde(default)]
    pub semantic: super::semantic::SemanticPolicy,
}

impl KnowledgePolicy {
    /// The policy of a repository: the `knowledge:` block of its policy file, or the
    /// defaults when the file carries none or cannot be read.
    pub fn load(repository: &Repository) -> Self {
        let Some(rel) = repository.section_path("policy") else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(repository.root().join(rel)) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    /// The `knowledge:` block of a policy text.
    pub fn parse(text: &str) -> Self {
        yaml::parse_mapping(text)
            .ok()
            .and_then(|m| m.get("knowledge").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }
}

/// A node as the baseline recorded it: its id and its fingerprint (evidence and claims).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeRecordedNode")]
pub struct RecordedNode {
    /// The node id.
    pub id: String,
    /// The fingerprint it carried, sixteen hex characters.
    pub fingerprint: String,
}

/// A claim a person verified, and the evidence fingerprint it was verified against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeVerifiedClaim")]
pub struct VerifiedClaim {
    /// The claim id.
    pub claim: String,
    /// The evidence id.
    pub evidence: String,
    /// The fingerprint the evidence carried when the claim was verified.
    pub fingerprint: String,
}

/// A node whose freshness debt the baseline tolerates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeToleratedDebt")]
pub struct ToleratedDebt {
    /// The node id.
    pub node: String,
    /// The freshness it had when tolerated.
    pub freshness: Freshness,
    /// Why, as recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A conflict a person accepted as known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeAcceptedConflict")]
pub struct AcceptedConflict {
    /// The conflict id.
    pub conflict: String,
    /// Why it stands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A canonicality violation the baseline tolerates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeToleratedViolation")]
pub struct ToleratedViolation {
    /// The violation id.
    pub violation: String,
    /// Why, as recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The baseline, typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "KnowledgeBaseline")]
pub struct Baseline {
    /// `majordomus/knowledge-baseline/v1`.
    #[serde(default)]
    pub schema: String,
    /// The commit the baseline was recorded at, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    /// Every node as recorded, by fingerprint: what "changed since the baseline" is
    /// decided against.
    #[serde(default)]
    pub nodes: Vec<RecordedNode>,
    /// Every verified claim.
    #[serde(default)]
    pub verified: Vec<VerifiedClaim>,
    /// The debt tolerated.
    #[serde(default)]
    pub debt: Vec<ToleratedDebt>,
    /// The conflicts accepted.
    #[serde(default)]
    pub accepted: Vec<AcceptedConflict>,
    /// The canonicality violations tolerated.
    #[serde(default)]
    pub canonicality: Vec<ToleratedViolation>,
}

impl Baseline {
    /// Is this the empty baseline of a repository that never recorded one?
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.verified.is_empty()
            && self.debt.is_empty()
            && self.accepted.is_empty()
            && self.canonicality.is_empty()
    }

    /// Read the baseline at `path`; an absent file is the empty baseline. A file of a
    /// schema this executable does not read is refused with what would read it; a file of
    /// an older schema is migrated in memory.
    pub fn load(path: &Path) -> Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => {
                return Err(Error::Io {
                    path: path.to_path_buf(),
                    source: e,
                })
            }
        };
        Self::parse(path, &text)
    }

    /// Parse a baseline text.
    pub fn parse(path: &Path, text: &str) -> Result<Self> {
        let value: Value = Value::Object(
            yaml::parse_mapping(text).map_err(|reason| contract(path, reason))?,
        );
        let (value, _steps) = super::migrate::migrate(super::migrate::Family::Baseline, value)
            .map_err(|reason| contract(path, reason))?;
        let mut b: Baseline = serde_json::from_value(value).map_err(|e| contract(path, e.to_string()))?;
        b.nodes.sort_by(|a, c| a.id.cmp(&c.id));
        Ok(b)
    }

    /// Render the baseline as the YAML subset the layer reads, sorted so that two
    /// recordings of one state are one text.
    pub fn render(&self) -> String {
        let mut b = self.clone();
        b.schema = SCHEMA.into();
        b.nodes.sort_by(|a, c| a.id.cmp(&c.id));
        b.verified.sort_by(|a, c| a.claim.cmp(&c.claim).then(a.evidence.cmp(&c.evidence)));
        b.debt.sort_by(|a, c| a.node.cmp(&c.node));
        b.accepted.sort_by(|a, c| a.conflict.cmp(&c.conflict));
        b.canonicality.sort_by(|a, c| a.violation.cmp(&c.violation));
        let value = serde_json::to_value(&b).expect("a baseline serialises");
        yaml::render_with_banner(
            &value,
            "The knowledge baseline: what this repository accepted about its own knowledge, recorded by `majordomus knowledge baseline record`. Debt listed here is tolerated and may only shrink; new debt is refused by `majordomus knowledge check`. Edit by recording, not by hand.",
        )
    }

    /// Write the baseline to `path`.
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        std::fs::write(path, self.render()).map_err(|e| Error::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// The recorded fingerprint of a node.
    pub fn node_fingerprint(&self, id: &str) -> Option<&str> {
        self.nodes
            .binary_search_by(|n| n.id.as_str().cmp(id))
            .ok()
            .map(|i| self.nodes[i].fingerprint.as_str())
    }

    /// The fingerprint a claim was verified against for one piece of evidence.
    pub fn verified_fingerprint(&self, claim: &str, evidence: &str) -> Option<&str> {
        self.verified
            .iter()
            .find(|v| v.claim == claim && v.evidence == evidence)
            .map(|v| v.fingerprint.as_str())
    }

    /// Is a node's debt tolerated?
    pub fn tolerates(&self, node: &str) -> Option<&ToleratedDebt> {
        self.debt.iter().find(|d| d.node == node)
    }

    /// Is a conflict accepted?
    pub fn accepts(&self, conflict: &str) -> bool {
        self.accepted.iter().any(|a| a.conflict == conflict)
    }

    /// Is a canonicality violation tolerated?
    pub fn tolerates_violation(&self, id: &str) -> bool {
        self.canonicality.iter().any(|v| v.violation == id)
    }

    /// The baseline a scan would record now: every node's fingerprint, every curated
    /// content claim verified against its present evidence, every remaining debt tolerated,
    /// every open conflict left open (accepting one is a separate, named act), every
    /// canonicality violation tolerated. `previous` carries the acceptances forward.
    pub fn record(model: &KnowledgeModel, previous: &Baseline) -> Baseline {
        let mut b = Baseline {
            schema: SCHEMA.into(),
            recorded_at: model.repository.head.clone(),
            ..Default::default()
        };
        for n in &model.nodes {
            // a node with no evidence and no claim is a name something pointed at; it
            // has nothing to move
            if n.evidence.is_empty() && n.claims.is_empty() {
                continue;
            }
            b.nodes.push(RecordedNode {
                id: n.id.clone(),
                fingerprint: super::model::node_fingerprint(n, model),
            });
        }
        for (_, c) in model.claims() {
            if !super::freshness::needs_verification(c) {
                continue;
            }
            for ev in &c.evidence {
                if let Some(e) = model.evidence(ev) {
                    b.verified.push(VerifiedClaim {
                        claim: c.id.clone(),
                        evidence: ev.clone(),
                        fingerprint: e.fingerprint.value.clone(),
                    });
                }
            }
        }
        // debt that verification alone will not clear: unresolved references, open
        // conflicts, derived knowledge; tolerated with the reason the scan gave
        for n in &model.nodes {
            let after = super::freshness::after_verification(n);
            if after.is_debt() {
                b.debt.push(ToleratedDebt {
                    node: n.id.clone(),
                    freshness: after,
                    reason: n.freshness_reason.clone(),
                });
            }
        }
        // every open conflict present when the baseline is recorded is accepted by that
        // act, with the reason saying so: recording is explicit and its diff is the
        // review. A conflict that appears afterwards is new debt until a person accepts
        // it by name (`majordomus knowledge accept`).
        for c in &model.conflicts {
            let reason = previous
                .accepted
                .iter()
                .find(|a| a.conflict == c.id)
                .and_then(|a| a.reason.clone())
                .unwrap_or_else(|| {
                    format!(
                        "present when the baseline was recorded{}",
                        model.repository.head.as_deref().map(|h| format!(" at {}", &h[..12.min(h.len())])).unwrap_or_default()
                    )
                });
            b.accepted.push(AcceptedConflict {
                conflict: c.id.clone(),
                reason: Some(reason),
            });
        }
        for g in &model.gaps {
            if g.category == super::model::GapCategory::Canonicality {
                b.canonicality.push(ToleratedViolation {
                    violation: g.id.clone(),
                    reason: Some(g.reason.clone()),
                });
            }
        }
        b
    }
}

/// One item of debt the check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeDebtItem")]
pub struct DebtItem {
    /// What kind of debt: `freshness`, `conflict` or `canonicality`.
    pub class: String,
    /// The node, conflict or violation id.
    pub id: String,
    /// The state it is in.
    pub state: String,
    /// Why, as the scan said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// What the check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeCheckReport")]
pub struct CheckReport {
    /// The mode the check ran in.
    pub mode: Mode,
    /// Whether a baseline was recorded at all.
    pub baseline_recorded: bool,
    /// The commit the baseline was recorded at, when it says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    /// Debt the baseline does not tolerate: what fails the check in protect mode.
    pub new_debt: Vec<DebtItem>,
    /// Debt the baseline tolerates, still present.
    pub tolerated: Vec<DebtItem>,
    /// Debt the baseline tolerates that is gone: the baseline should be recorded again
    /// so that it cannot come back.
    pub resolved: Vec<String>,
    /// The verdict: `pass` or `fail`.
    pub verdict: String,
    /// One line a person reads.
    pub summary: String,
}

impl CheckReport {
    /// Did the check pass?
    pub fn passed(&self) -> bool {
        self.verdict == "pass"
    }
}

/// Hold a scanned model against the baseline in a mode.
pub fn check(model: &KnowledgeModel, baseline: &Baseline, mode: Mode) -> CheckReport {
    let mut new_debt = Vec::new();
    let mut tolerated = Vec::new();
    let mut resolved = Vec::new();
    for n in &model.nodes {
        if !n.freshness.is_debt() {
            continue;
        }
        let item = DebtItem {
            class: "freshness".into(),
            id: n.id.clone(),
            state: n.freshness.as_str().into(),
            reason: n.freshness_reason.clone(),
        };
        // a tolerated node that got worse is new debt: tolerance is of a state, not of
        // a node
        match baseline.tolerates(&n.id) {
            Some(t) if n.freshness.worse(t.freshness) == t.freshness => tolerated.push(item),
            _ => new_debt.push(item),
        }
    }
    for c in &model.conflicts {
        let item = DebtItem {
            class: "conflict".into(),
            id: c.id.clone(),
            state: match c.resolution {
                super::model::Resolution::Open => "open".into(),
                super::model::Resolution::Accepted => "accepted".into(),
            },
            reason: Some(c.basis.clone()),
        };
        if c.resolution == super::model::Resolution::Accepted {
            tolerated.push(item);
        } else {
            new_debt.push(item);
        }
    }
    for g in &model.gaps {
        if g.category != super::model::GapCategory::Canonicality {
            continue;
        }
        let item = DebtItem {
            class: "canonicality".into(),
            id: g.id.clone(),
            state: "violation".into(),
            reason: Some(g.reason.clone()),
        };
        if baseline.tolerates_violation(&g.id) {
            tolerated.push(item);
        } else {
            new_debt.push(item);
        }
    }
    let live: std::collections::BTreeSet<&str> = tolerated.iter().map(|d| d.id.as_str()).collect();
    for d in &baseline.debt {
        if !live.contains(d.node.as_str()) {
            resolved.push(d.node.clone());
        }
    }
    for v in &baseline.canonicality {
        if !live.contains(v.violation.as_str()) {
            resolved.push(v.violation.clone());
        }
    }
    let fails = match mode {
        Mode::Observe | Mode::Warn => false,
        Mode::Protect => !new_debt.is_empty(),
        Mode::Strict => !new_debt.is_empty() || !tolerated.is_empty(),
    };
    let summary = format!(
        "{} new, {} tolerated, {} resolved; mode {}",
        new_debt.len(),
        tolerated.len(),
        resolved.len(),
        mode.as_str()
    );
    CheckReport {
        mode,
        baseline_recorded: !baseline.is_empty(),
        recorded_at: baseline.recorded_at.clone(),
        new_debt,
        tolerated,
        resolved,
        verdict: if fails { "fail".into() } else { "pass".into() },
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_baseline_round_trips_through_the_yaml_subset() {
        let b = Baseline {
            schema: SCHEMA.into(),
            recorded_at: Some("abc".into()),
            nodes: vec![RecordedNode {
                id: "document:docs/X.md".into(),
                fingerprint: "0123456789abcdef".into(),
            }],
            verified: vec![VerifiedClaim {
                claim: "knowledge:start-here#reference[verified_against=file:docs/X.md]".into(),
                evidence: "file:docs/X.md".into(),
                fingerprint: "0123".into(),
            }],
            debt: vec![ToleratedDebt {
                node: "document:docs/Y.md".into(),
                freshness: Freshness::Unverified,
                reason: Some("line 3 links to nowhere: a: b".into()),
            }],
            accepted: vec![],
            canonicality: vec![ToleratedViolation {
                violation: "canonicality:orphan:docs/generated/z.json".into(),
                reason: None,
            }],
        };
        let text = b.render();
        let back = Baseline::parse(Path::new("baseline.yaml"), &text).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn a_baseline_without_a_schema_line_is_migrated_and_an_unknown_one_refused() {
        let back = Baseline::parse(Path::new("b.yaml"), "debt: []\n").unwrap();
        assert_eq!(back.schema, SCHEMA);
        let err = Baseline::parse(Path::new("b.yaml"), "schema: majordomus/knowledge-baseline/v9\n");
        assert!(err.is_err());
    }

    #[test]
    fn the_policy_block_defaults_to_protect() {
        assert_eq!(KnowledgePolicy::parse("version: 1\n").mode, Mode::Protect);
        assert_eq!(
            KnowledgePolicy::parse("knowledge:\n  mode: strict\n").mode,
            Mode::Strict
        );
    }
}
