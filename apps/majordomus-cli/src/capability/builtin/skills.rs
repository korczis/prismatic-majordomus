//! The `skills` module: every skill as a proven capability, derived.
//!
//! The module is `skills` and not `skill` because `skill` is the declarative **kind** of the
//! files under `.ai/repo/skills/`, whose resources are `majordomus://skill/<id>`, and the
//! registry composes modules and kinds into one namespace.
//!
//! Every capability here reads [`crate::skill::Skills`], built on demand from the index and the
//! evidence ledger. There is no second catalogue: the skills are the index's objects of kind
//! `skill`, and what this module adds is what the repository can show about each — tested,
//! documented, enforced, used — and the findings when it cannot. All three are read-only.
//!
//! ```
//! use majordomus_cli::capability::builtin::skills;
//! let m = skills::module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["skills.status", "skills.explain", "skills.verify"]);
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::evidence::Ledger;
use crate::skill::{SkillFinding, SkillStatus, Skills, SKILL};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which every skill, derived, is read as an MCP resource.
pub const SKILLS_URI: &str = "majordomus://skills";

// ---------------------------------------------------------------- views

/// Every skill with its derived standing, and how many stand where: what
/// `majordomus skills status`, `GET /api/v1/skills` and the `majordomus_skills` tool answer.
///
/// ```
/// use majordomus_cli::capability::builtin::skills::SkillStatusList;
/// let empty = SkillStatusList {
///     count: 0,
///     standings: std::collections::BTreeMap::new(),
///     skills: vec![],
/// };
/// assert_eq!(empty.count, 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillStatusList {
    /// How many skills.
    pub count: usize,
    /// How many in each standing, keyed by the standing word.
    pub standings: std::collections::BTreeMap<String, usize>,
    /// Every skill, in index order.
    pub skills: Vec<SkillStatus>,
}

/// The verdict over every skill: valid when no finding refuses.
///
/// ```
/// use majordomus_cli::capability::builtin::skills::SkillVerification;
/// let v = SkillVerification { valid: true, skills: 3, failures: 0, warnings: 2, findings: vec![] };
/// // warnings are debts, reported; they do not make the verdict false
/// assert!(v.valid);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkillVerification {
    /// True when no finding is a failure.
    pub valid: bool,
    /// How many skills were examined.
    pub skills: usize,
    /// How many findings refuse.
    pub failures: usize,
    /// How many findings are warnings.
    pub warnings: usize,
    /// Every finding.
    pub findings: Vec<SkillFinding>,
}

// ---------------------------------------------------------------- inputs

/// Which skill to explain, by its id. An id the repository does not hold is a refusal
/// naming what was looked for, never an empty answer.
///
/// ```
/// use majordomus_cli::capability::builtin::skills::SkillExplainInput;
/// let input = SkillExplainInput { id: "implement".into() };
/// assert_eq!(input.id, "implement");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkillExplainInput {
    /// The skill's id, which is also its directory name.
    pub id: String,
}

impl BenchmarkCases for SkillExplainInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        let id = ctx
            .index
            .objects
            .iter()
            .find(|o| o.kind == SKILL)
            .map(|o| o.identity.clone());
        // a repository without a skill still answers the route, with the refusal, so the
        // required parameter keeps an example in the OpenAPI document
        vec![NamedCase::new(
            if id.is_some() {
                "first-skill"
            } else {
                "absent"
            },
            SkillExplainInput {
                id: id.unwrap_or_else(|| "absent".into()),
            },
        )]
    }
}

// ---------------------------------------------------------------- handlers

fn derived(ctx: &Context) -> Result<Skills, CapabilityError> {
    let root = std::path::PathBuf::from(&ctx.index.repository.root);
    let ledger = Ledger::load(&root).map_err(|e| CapabilityError::Internal(e.to_string()))?;
    Ok(Skills::build(&ctx.index, &ledger))
}

fn skills_status(ctx: &Context, _: Empty) -> Result<SkillStatusList, CapabilityError> {
    let skills = derived(ctx)?;
    let mut standings = std::collections::BTreeMap::new();
    for s in &skills.skills {
        *standings.entry(s.standing.label().to_string()).or_insert(0) += 1;
    }
    Ok(SkillStatusList {
        count: skills.skills.len(),
        standings,
        skills: skills.skills,
    })
}

fn skills_explain(ctx: &Context, input: SkillExplainInput) -> Result<SkillStatus, CapabilityError> {
    let skills = derived(ctx)?;
    skills.skill(&input.id).cloned().ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no skill '{}'; `skills.status` names every one this repository holds",
            input.id
        ))
    })
}

fn skills_verify(ctx: &Context, _: Empty) -> Result<SkillVerification, CapabilityError> {
    let skills = derived(ctx)?;
    Ok(SkillVerification {
        valid: skills.failures() == 0,
        skills: skills.skills.len(),
        failures: skills.failures(),
        warnings: skills.warnings(),
        findings: skills.findings,
    })
}

/// The module the registry composes: three read-only capabilities over one derivation, each
/// declared once here and projected to the command line, HTTP, MCP and OpenAPI.
///
/// ```
/// use majordomus_cli::capability::builtin::skills;
/// let m = skills::module();
/// assert_eq!(m.id.as_str(), "skills");
/// assert!(m.capabilities.iter().all(|e| e.capability.id.as_str().starts_with("skills.")));
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "skills",
        title: "Skills",
        description: "Every skill the layer defines as a proven capability: whether a test names it and the evidence ledger holds a current passing run of that test, whether the site projects its page, whether a doctrine and a CI gate hold it, and whether a workflow, prompt, profile, provider template, recipe or CI workflow invokes it. All four are derived on every read and none is authored; an active skill no test names or nothing invokes is an orphan.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "skills.status",
                title: "Every skill, with its derived standing",
                description: "Every skill the index holds, each with the tests that name it and the evidence state of their latest runs, its page projection, the doctrine and gates that hold it, every invocation that references it, and the standing those derive: proven, partial, orphan, invalid, or not_required for a draft or deprecated skill.",
                input: Empty,
                output: SkillStatusList,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_skills".into()),
                        resource: Some(McpResource { uri: SKILLS_URI.into(), name: "skills".into() }),
                    }),
                    http: get("/api/v1/skills"),
                    cli: Some(crate::capability::CliExposure { path: vec!["skills".into(), "status".into()] }),
                },
                tags: ["skill", "evidence"],
                cache: CachePolicy::Disabled,
                handler: skills_status,
            },
            capability! {
                id: "skills.explain",
                title: "One skill, with everything derived about it",
                description: "One skill in full: its authored identity, status and provenance marker, each test that names it with the state of its latest recorded run and the command that reproduces it, its page, its doctrine and gates, every invocation by path and line, its standing and the findings about it. The file itself stays at `majordomus://skill/<id>`.",
                input: SkillExplainInput,
                output: SkillStatus,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_skill_explain"),
                    http: get("/api/v1/skills/explain"),
                    cli: Some(crate::capability::CliExposure { path: vec!["skills".into(), "explain".into()] }),
                },
                tags: ["skill", "evidence"],
                cache: CachePolicy::Disabled,
                handler: skills_explain,
            },
            capability! {
                id: "skills.verify",
                title: "What the skills owe and do not have",
                description: "Every finding over the skills: a file that breaks the skill contract, an active skill no test names or nothing invokes, a test whose latest run failed, a test or an invocation naming a skill that does not exist — each a failure — and evidence that is not current, a missing page or a missing gate, each a warning.",
                input: Empty,
                output: SkillVerification,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_skills_verify"),
                    http: get("/api/v1/skills/verify"),
                    cli: Some(crate::capability::CliExposure { path: vec!["skills".into(), "verify".into()] }),
                },
                tags: ["skill", "evidence", "validation"],
                cache: CachePolicy::Disabled,
                handler: skills_verify,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these names exist; a refactor that dropped an
    /// exposure or renamed a route would still compile, and this would not pass.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        let expected: &[(&str, &str, &str, &[&str])] = &[
            (
                "skills.status",
                "majordomus_skills",
                "/api/v1/skills",
                &["skills", "status"],
            ),
            (
                "skills.explain",
                "majordomus_skill_explain",
                "/api/v1/skills/explain",
                &["skills", "explain"],
            ),
            (
                "skills.verify",
                "majordomus_skills_verify",
                "/api/v1/skills/verify",
                &["skills", "verify"],
            ),
        ];
        assert_eq!(m.capabilities.len(), expected.len());
        for (e, (id, tool, path, cli)) in m.capabilities.iter().zip(expected) {
            let c = &e.capability;
            assert_eq!(c.id.as_str(), *id);
            let exp = &c.exposure;
            assert_eq!(
                exp.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool)
            );
            assert_eq!(exp.http.as_ref().map(|h| h.path.as_str()), Some(*path));
            let words: Vec<&str> = exp
                .cli
                .as_ref()
                .map(|c| c.path.iter().map(String::as_str).collect())
                .unwrap_or_default();
            assert_eq!(words, *cli);
        }
    }
}
