//! Change inspection: what a change set means for the knowledge, before it is merged.
//! The paths that changed, what they touch (the impact), every capability the change
//! adds with the checklist of surfaces derived for it, and the debt the change
//! introduces — freshness the baseline does not tolerate, canonicality violations neither
//! tolerated nor excepted. What a pull request is inspected with.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::handler::{CapabilityError, Context};
use crate::git::ChangedPath;

use super::baseline::{check, DebtItem, Mode};
use super::canonicality::{Audit, Surface, Violation};
use super::impact::ImpactReport;
use super::Scanned;

/// A capability the change set adds, with the surfaces derived for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeAddedCapability")]
pub struct AddedCapability {
    /// The capability id.
    pub id: String,
    /// The file it is declared in: its canonical source.
    pub declared_in: String,
    /// Every surface derived from the declaration, ticked.
    pub surfaces: Vec<Surface>,
    /// Its manual maintenance surface.
    pub mms: usize,
    /// Hand-written files that already name it.
    pub mentions: usize,
}

/// The inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "KnowledgeInspection")]
pub struct Inspection {
    /// The base the change set was cut against.
    pub base: String,
    /// The impact of the change set on the model.
    pub impact: ImpactReport,
    /// Every capability the change set adds.
    pub added_capabilities: Vec<AddedCapability>,
    /// Debt the baseline does not tolerate: what the change must fix or record.
    pub new_debt: Vec<DebtItem>,
    /// Canonicality violations that count.
    pub violations: Vec<Violation>,
    /// The check's mode.
    pub mode: Mode,
    /// `pass` or `fail`.
    pub verdict: String,
    /// One line.
    pub summary: String,
}

/// Inspect a change set: the working tree against `base` (`HEAD` when unset).
pub fn inspect(
    ctx: &Context,
    s: &Scanned,
    impact: ImpactReport,
    audit: &Audit,
    base: &str,
) -> Result<Inspection, CapabilityError> {
    let root = s.root.clone();
    // the capabilities the change adds: every query or command declared in a changed
    // file of the registry whose id the base revision's copy of that file does not name
    let mut added = Vec::new();
    for c in ctx.registry.iter() {
        if c.kind == crate::capability::CapabilityKind::Resource {
            continue;
        }
        let path = c.provenance.source_path();
        let Some(change) = impact.changed.iter().find(|p| p.path == path) else {
            continue;
        };
        let is_new = match change.status.as_str() {
            "added" | "untracked" => true,
            _ => crate::git::show(&root, base, &path)
                .map(|before| !before.contains(&format!("\"{}\"", c.id)))
                .unwrap_or(true),
        };
        if !is_new {
            continue;
        }
        let row = audit.capabilities.iter().find(|a| a.id == c.id.as_str());
        added.push(AddedCapability {
            id: c.id.to_string(),
            declared_in: path,
            surfaces: row.map(|r| r.surfaces.clone()).unwrap_or_default(),
            mms: row.map(|r| r.mms).unwrap_or(1),
            mentions: row.map(|r| r.mentions.len()).unwrap_or(0),
        });
    }
    added.sort_by(|a, b| a.id.cmp(&b.id));
    let report = check(&s.model, &s.baseline, s.policy.mode);
    let violations: Vec<Violation> = audit
        .violations
        .iter()
        .filter(|v| !v.tolerated && v.excepted_by.is_none())
        .cloned()
        .collect();
    let fails = match s.policy.mode {
        Mode::Observe | Mode::Warn => false,
        Mode::Protect => !report.new_debt.is_empty() || !violations.is_empty(),
        Mode::Strict => !report.new_debt.is_empty() || !report.tolerated.is_empty() || !violations.is_empty(),
    };
    let summary = format!(
        "{} path(s) changed against {base}; {} node(s) touched directly, {} reached; {} capability(ies) added; {} new debt item(s), {} counting canonicality violation(s); mode {}",
        impact.changed.len(),
        impact.direct.len(),
        impact.transitive.len(),
        added.len(),
        report.new_debt.len(),
        violations.len(),
        s.policy.mode.as_str()
    );
    Ok(Inspection {
        base: base.to_string(),
        impact,
        added_capabilities: added,
        new_debt: report.new_debt,
        violations,
        mode: s.policy.mode,
        verdict: if fails { "fail".into() } else { "pass".into() },
        summary,
    })
}

/// The changed paths of the working tree against a base, or an empty set where git
/// cannot answer (a repository with no history).
pub fn changed(root: &std::path::Path, base: &str) -> Vec<ChangedPath> {
    crate::git::changed_paths(root, Some(base)).unwrap_or_default()
}
