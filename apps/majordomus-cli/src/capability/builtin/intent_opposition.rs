//! The `intent_opposition` module: the opposition to an intent's plan, executed (ADR 0112).
//!
//! Two capabilities over one derivation, [`crate::intent_opposition::oppose`]:
//!
//! - `intent_opposition.review` reads. It answers the structural findings derived now, the
//!   findings a reviewer recorded, the one disposition both derive, the revision of the plan
//!   a review judges and where the critique's stamp stands against it — which is also the
//!   bounded brief a reviewing session works from.
//! - `intent_opposition.record` writes, and writes one thing: the stamp. It sets the
//!   critique's `reviewed_revision`, `reviewed_at` and `reviewed_with`, creates the record
//!   with no findings when the intent has none, and appends `opposition.recorded` to the
//!   ledger. It writes no finding and no disposition: findings are a reviewer's, and the
//!   disposition is derived on every read.
//!
//! The module is its own, beside `intents`, so that "no capability of `intents` writes"
//! stays a sentence a test can hold.
//!
//! ```
//! use majordomus_cli::capability::builtin::intent_opposition;
//! let m = intent_opposition::module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|e| e.capability.id.as_str()).collect();
//! assert_eq!(ids, ["intent_opposition.review", "intent_opposition.record"]);
//! ```

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CapabilityKind, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::git::GitState;
use crate::intent::{Intents, RepositoryEvidence, INTENT};
use crate::intent_opposition::{
    oppose, IntentOpposition, OppositionDisposition, OppositionReviewState,
};
use crate::intent_review::{CritiqueRecord, GapRecord};
use crate::plan::{set_field_before, Plan};
use crate::{capability, module};

use super::{get, mcp, post};

/// The kind of a critique record, as the layer names it.
const CRITIQUE: &str = "critique";
/// The event a recorded review appends to the ledger.
pub const EVENT: &str = "opposition.recorded";

/// The findings of `intent validate` that say a critique's own content does not hold. A
/// record with one of these about it is not stamped: a stamp over findings nobody can read
/// would certify a review that says nothing.
const UNSOUND: [&str; 8] = [
    "critique_unknown_intent",
    "critique_duplicate_finding",
    "critique_unknown_class",
    "critique_unknown_resolution",
    "rejected_without_reason",
    "planned_into_unknown_issue",
    "planned_into_unrelated_issue",
    "planned_into_cancelled_issue",
];

/// The input of `intent_opposition.review`: which intent.
///
/// ```
/// use majordomus_cli::capability::builtin::intent_opposition::OppositionInput;
/// let i: OppositionInput = serde_json::from_str(r#"{"intent":"intent-planning"}"#).unwrap();
/// assert_eq!(i.intent, "intent-planning");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OppositionInput {
    /// The intent whose plan is opposed, by id.
    pub intent: String,
}

fn first_intent(ctx: &CaseContext<'_>) -> Option<String> {
    ctx.index
        .objects
        .iter()
        .find(|o| o.kind == INTENT)
        .map(|o| o.identity.clone())
}

impl BenchmarkCases for OppositionInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "first-intent",
            OppositionInput {
                intent: first_intent(ctx).unwrap_or_else(|| "absent".into()),
            },
        )]
    }
}

/// The input of `intent_opposition.record`: which intent's review to stamp.
///
/// There is no revision field and no disposition field. The revision stamped is the plan's
/// as the executable derives it at that moment, never one a caller supplies, because a
/// stamp a caller can choose is a signature anyone can forge by asking.
///
/// ```
/// use majordomus_cli::capability::builtin::intent_opposition::OppositionRecordInput;
/// let i: OppositionRecordInput =
///     serde_json::from_str(r#"{"intent":"x","check":true}"#).unwrap();
/// assert!(i.check && i.reviewed_by.is_none());
/// assert!(serde_json::from_str::<OppositionRecordInput>(
///     r#"{"intent":"x","reviewed_revision":"abc"}"#).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OppositionRecordInput {
    /// The intent whose review is stamped, by id.
    pub intent: String,
    /// Answer what would be stamped and write nothing.
    #[serde(default)]
    pub check: bool,
    /// Who reviewed, for a critique this call creates; an existing record keeps the
    /// reviewer it names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
}

impl BenchmarkCases for OppositionRecordInput {
    fn benchmark_cases(ctx: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // a check: the whole derivation and the refusals, and nothing written
        vec![NamedCase::new(
            "check-first-intent",
            OppositionRecordInput {
                intent: first_intent(ctx).unwrap_or_else(|| "absent".into()),
                check: true,
                // named, so the case answers for an intent with no critique too
                reviewed_by: Some("a benchmark".into()),
            },
        )]
    }
}

/// What a stamp did, or — for a check — would do.
///
/// ```
/// use majordomus_cli::capability::builtin::intent_opposition::OppositionRecorded;
/// use majordomus_cli::intent_opposition::OppositionDisposition;
/// let r = OppositionRecorded {
///     intent: "x".into(), source: ".ai/repo/project/critiques/x.yaml".into(),
///     reviewed_revision: "0".repeat(64), reviewed_at: "abc1234".into(),
///     reviewed_with: "majordomus-cli 0.0.0".into(), disposition: OppositionDisposition::Accept,
///     created: true, written: false, event: String::new(),
/// };
/// assert!(r.created && !r.written, "a check creates nothing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OppositionRecorded {
    /// The intent.
    pub intent: String,
    /// The critique record, repository-relative.
    pub source: String,
    /// The plan revision stamped.
    pub reviewed_revision: String,
    /// The commit stamped.
    pub reviewed_at: String,
    /// The tool and version stamped.
    pub reviewed_with: String,
    /// The disposition derived at that moment; reported and appended to the ledger, never
    /// written to the record.
    pub disposition: OppositionDisposition,
    /// Whether the intent had no critique, so the record is new.
    pub created: bool,
    /// Whether anything was written: false for a check.
    pub written: bool,
    /// The ledger event appended; empty for a check.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub event: String,
}

fn derived(ctx: &Context) -> Result<(Plan, Intents), CapabilityError> {
    let plan = Plan::build(&ctx.index);
    let evidence = RepositoryEvidence::load(&ctx.index)
        .map_err(|e| CapabilityError::Internal(e.to_string()))?;
    let intents = Intents::build(&ctx.index, &plan, &evidence);
    Ok((plan, intents))
}

fn opposed(ctx: &Context, intent: &str) -> Result<(Intents, IntentOpposition), CapabilityError> {
    let (plan, intents) = derived(ctx)?;
    let answer = oppose(
        &intents,
        &plan,
        &GapRecord::all(&ctx.index),
        &CritiqueRecord::all(&ctx.index),
        intent,
    )
    .ok_or_else(|| {
        CapabilityError::NotFound(format!(
            "no intent '{intent}'; `intents.list` names every one this repository holds"
        ))
    })?;
    Ok((intents, answer))
}

fn review(ctx: &Context, input: OppositionInput) -> Result<IntentOpposition, CapabilityError> {
    opposed(ctx, input.intent.trim()).map(|(_, answer)| answer)
}

/// Where the record of `kind` and `identity` lives, as the index recorded it.
fn path_of(ctx: &Context, kind: &str, identity: &str) -> Option<String> {
    ctx.index
        .objects
        .iter()
        .find(|o| o.kind == kind && o.identity == identity)
        .map(|o| o.provenance.path.clone())
}

/// The record a new critique of `intent` is written to: beside the others when there are
/// any, else in `critiques/` beside the directory the intent's own record is in.
fn new_critique_path(ctx: &Context, intent: &str) -> Option<String> {
    let dir: PathBuf = ctx
        .index
        .objects
        .iter()
        .find(|o| o.kind == CRITIQUE)
        .and_then(|o| {
            Path::new(&o.provenance.path)
                .parent()
                .map(Path::to_path_buf)
        })
        .or_else(|| {
            let own = path_of(ctx, INTENT, intent)?;
            Some(Path::new(&own).parent()?.parent()?.join("critiques"))
        })?;
    Some(
        dir.join(format!("{intent}.yaml"))
            .to_string_lossy()
            .into_owned(),
    )
}

/// A YAML double-quoted scalar of `s`, in the subset both readers accept: a backslash and a
/// double quote escaped, a line break folded to a space.
fn quoted(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    format!("\"{}\"", flat.replace('\\', "\\\\").replace('"', "\\\""))
}

fn record(
    ctx: &Context,
    input: OppositionRecordInput,
) -> Result<OppositionRecorded, CapabilityError> {
    let intent = input.intent.trim().to_string();
    let (intents, answer) = opposed(ctx, &intent)?;
    let existing = path_of(ctx, CRITIQUE, &intent);

    // a record whose own findings do not hold is not stamped
    let prefix = format!("{intent}:");
    let unsound: Vec<String> = intents
        .findings
        .iter()
        .filter(|f| UNSOUND.contains(&f.code.as_str()))
        .filter(|f| f.subject == intent || f.subject.starts_with(&prefix))
        .map(|f| format!("{} {}: {}", f.code, f.subject, f.message))
        .collect();
    if !unsound.is_empty() {
        return Err(CapabilityError::Refused(format!(
            "the critique of {intent} does not hold and is not stamped: {}",
            unsound.join("; ")
        )));
    }

    let head = match &ctx.index.repository.git {
        GitState::Available(info) => info.head.clone().unwrap_or_default(),
        GitState::Unavailable { .. } => String::new(),
    };
    if head.is_empty() {
        return Err(CapabilityError::Refused(
            "this checkout has no commit to name: a review is stamped with the commit it was run at".into(),
        ));
    }
    let reviewed_at: String = head.chars().take(10).collect();
    let reviewed_with = format!("majordomus-cli {}", crate::VERSION);
    let created = existing.is_none();
    let source = match existing {
        Some(p) => p,
        None => new_critique_path(ctx, &intent).ok_or_else(|| {
            CapabilityError::Internal(format!(
                "intent '{intent}' is in the model but no object carries its file"
            ))
        })?,
    };
    // A file where the record would go that the index does not hold as a critique is a
    // critique that does not validate. Writing a fresh one over it would destroy what a
    // reviewer wrote in order to certify that nothing was found.
    if created && Path::new(&ctx.index.repository.root).join(&source).exists() {
        return Err(CapabilityError::Refused(format!(
            "{source} exists and is not a critique this repository can read, so it is not \
             stamped and not replaced; `majordomus-cli inspect` names what is wrong with it"
        )));
    }
    let reviewer = input
        .reviewed_by
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if created && reviewer.is_none() {
        return Err(CapabilityError::InvalidInput(format!(
            "{intent} has no critique, and the record this would create must say who reviewed: give reviewed_by"
        )));
    }

    let mut result = OppositionRecorded {
        intent: intent.clone(),
        source: source.clone(),
        reviewed_revision: answer.reviewed_plan.clone(),
        reviewed_at: reviewed_at.clone(),
        reviewed_with: reviewed_with.clone(),
        disposition: answer.disposition,
        created,
        written: false,
        event: String::new(),
    };
    if input.check {
        return Ok(result);
    }

    let root = Path::new(&ctx.index.repository.root);
    let path = root.join(&source);
    let refused = |what: &str, e: std::io::Error| {
        CapabilityError::Refused(format!("could not {what} {}: {e}", path.display()))
    };
    let events = ctx
        .index
        .share
        .as_ref()
        .map(|s| s.join("events.yaml"))
        .ok_or_else(|| {
            CapabilityError::Refused(
                "this index was built without a share directory, so the event vocabulary cannot be read; a review may not write an event it cannot validate".into(),
            )
        })?;
    let vocabulary = crate::ledger::Vocabulary::load(&events)
        .map_err(|e| CapabilityError::Refused(e.to_string()))?;

    let text = if created {
        // a fixed shape with no findings: what a reviewer adds, a reviewer writes
        format!(
            "intent: {intent}\nreviewed_at: {reviewed_at}\nreviewed_by: {}\nfindings: []\n",
            quoted(reviewer.unwrap_or_default())
        )
    } else {
        std::fs::read_to_string(&path).map_err(|e| refused("read", e))?
    };
    // the stamp, and nothing else: every other line of the record is left as it is
    let text = set_field_before(&text, "reviewed_at", &reviewed_at, "findings:");
    let text = set_field_before(
        &text,
        "reviewed_revision",
        &answer.reviewed_plan,
        "findings:",
    );
    let text = set_field_before(&text, "reviewed_with", &quoted(&reviewed_with), "findings:");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| refused("create the directory of", e))?;
    }
    std::fs::write(&path, text).map_err(|e| refused("write", e))?;

    let at = crate::ledger::now();
    crate::ledger::append(
        root,
        &vocabulary,
        &ctx.index.repository.git,
        &at,
        EVENT,
        &[
            ("intent", intent),
            ("reviewed_revision", answer.reviewed_plan.clone()),
            ("disposition", answer.disposition.as_str().to_string()),
            ("state", OppositionReviewState::Current.as_str().to_string()),
        ],
    )
    .map_err(|e| CapabilityError::Refused(e.to_string()))?;
    result.written = true;
    result.event = EVENT.to_string();
    Ok(result)
}

/// The module the registry composes: the review of one intent's plan as a read-only
/// capability, and the stamp of which plan was reviewed as the one capability here that
/// writes a tracked record. Both are projected to the command line, HTTP and MCP from this
/// declaration and from nowhere else.
///
/// ```
/// use majordomus_cli::capability::builtin::intent_opposition;
/// use majordomus_cli::capability::Effect;
/// let m = intent_opposition::module();
/// assert_eq!(m.id.as_str(), "intent_opposition");
/// // one of the two writes, and it is the one named for it
/// let writers: Vec<&str> = m.capabilities.iter()
///     .filter(|e| e.capability.execution.effect == Effect::RepositoryMutation)
///     .map(|e| e.capability.id.as_str())
///     .collect();
/// assert_eq!(writers, ["intent_opposition.record"]);
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "intent_opposition",
        title: "Opposition",
        description: "The opposition to an intent's plan, executed (ADR 0112): the structural findings the coverage, the plan and the gap review derive about one intent now, the findings a reviewer recorded with their resolutions, the one disposition both derive, and the revision of the plan a review judges against the stamp its critique carries. Reading is the brief a reviewer works from; recording stamps which plan a review was run over, and writes nothing else.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "intent_opposition.review",
                title: "The opposition to one intent's plan, and the brief a reviewer works from",
                description: "For one intent: its statement, invariants, non-goals and criteria with the state of their evidence; every live issue serving it with its links, dependencies, scope and required evidence; the recorded gap's conditions; `structural`, every finding the intent engine and the plan derive about it now, blocking where that derivation calls it a failure and advisory where a warning; `recorded`, every finding a reviewer wrote in its critique with its resolution, source and resolver; `disposition` — `reject` while a structural finding is blocking or a recorded blocking finding is open, `accept_with_required_changes` when recorded blocking findings are each planned or rejected with a reason, `accept` otherwise — with the findings that reject it; `reviewed_plan`, the revision of what a review judges; and `review`, the critique's stamp and whether it is `none`, `not_stamped`, `current` or `stale` against that revision. Nothing structural and no disposition is stored: both are derived on every call, with no advisor, model or network.",
                input: OppositionInput,
                output: IntentOpposition,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_opposition"),
                    http: get("/api/v1/intents/opposition"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "oppose".into()] }),
                },
                tags: ["intent", "project", "planning", "governance"],
                cache: CachePolicy::Disabled,
                handler: review,
            },
            capability! {
                id: "intent_opposition.record",
                kind: CapabilityKind::Command,
                title: "Stamp the review of an intent's plan with the plan it was run over",
                description: "Derive the opposition to one intent's plan and stamp its critique with what was reviewed: `reviewed_revision`, the plan's revision as derived at that moment and never one a caller supplies; `reviewed_at`, the commit; and `reviewed_with`, the tool and its version. An intent with no critique gets a record with no findings, which needs `reviewed_by`. The stamp is three top-level lines and every other line of the record is left as it is: findings are a reviewer's to write. A critique whose own findings do not hold — an unknown class or resolution, a duplicate, a rejection without a reason, a resolution planned into an issue that does not exist, serves another intent or is cancelled — is refused and nothing is written. The event `opposition.recorded` is appended to the ledger with the intent, the revision and the disposition derived then; the disposition is not written to the record. With `check`, the answer is what would be stamped and nothing is written.",
                input: OppositionRecordInput,
                output: OppositionRecorded,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_intent_opposition_record"),
                    http: post("/api/v1/intents/opposition/record"),
                    cli: Some(crate::capability::CliExposure { path: vec!["intent".into(), "stamp".into()] }),
                },
                tags: ["intent", "project", "planning", "lifecycle"],
                cache: CachePolicy::Disabled,
                handler: record,
            }
            // The one thing its kind cannot say: this writes a tracked record. The exposure
            // ceiling of every surface is derived from the effect.
            .writes_repository(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reviewer_with_a_quote_or_a_line_break_is_one_scalar() {
        assert_eq!(quoted("a \"b\"\nc\\d"), "\"a \\\"b\\\" c\\\\d\"");
    }

    #[test]
    fn exactly_one_capability_of_the_module_writes() {
        let writers: Vec<String> = module()
            .capabilities
            .into_iter()
            .filter(|e| {
                e.capability.execution.effect == crate::capability::Effect::RepositoryMutation
            })
            .map(|e| e.capability.id.to_string())
            .collect();
        assert_eq!(writers, ["intent_opposition.record"]);
    }
}
