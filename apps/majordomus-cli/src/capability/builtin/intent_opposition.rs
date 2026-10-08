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

use std::path::Path;

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

/// The record a new critique of `intent` is written to: `critiques/` beside the directory
/// the intent's own record is in, which is where the layer keeps them.
fn new_critique_path(ctx: &Context, intent: &str) -> String {
    let own = path_of(ctx, INTENT, intent).unwrap_or_default();
    Path::new(&own)
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new(".ai/repo/project"))
        .join("critiques")
        .join(format!("{intent}.yaml"))
        .to_string_lossy()
        .into_owned()
}

/// A YAML double-quoted scalar of `s` that both readers of this layer read back as the same
/// text. Neither reader undoes an escape inside double quotes, so nothing is escaped: a
/// double quote becomes a single one, a backslash a slash, and a line break a space. What is
/// lost is punctuation in a name; what is kept is a record every reader agrees about.
fn quoted(s: &str) -> String {
    let plain: String = s
        .chars()
        .map(|c| match c {
            '"' => '\'',
            '\\' => '/',
            '\n' | '\r' => ' ',
            other => other,
        })
        .collect();
    format!("\"{plain}\"")
}

/// The commit a review is stamped with: the first ten characters of HEAD, or nothing when
/// this checkout has no commit to name.
fn head_of(git: &GitState) -> String {
    match git {
        GitState::Available(info) => info
            .head
            .as_deref()
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect(),
        GitState::Unavailable { .. } => String::new(),
    }
}

/// The findings of `intent validate` that say the critique of `intent` does not hold.
fn unsound(intents: &Intents, intent: &str) -> Vec<String> {
    let prefix = format!("{intent}:");
    intents
        .findings
        .iter()
        .filter(|f| UNSOUND.contains(&f.code.as_str()))
        .filter(|f| f.subject == intent || f.subject.starts_with(&prefix))
        .map(|f| format!("{} {}: {}", f.code, f.subject, f.message))
        .collect()
}

/// What a stamp would be, and why it may not be made: every refusal a stamp has, in the
/// order a reader meets them, decided before anything is written.
fn planned_stamp(
    ctx: &Context,
    input: &OppositionRecordInput,
) -> Result<(OppositionRecorded, Option<String>), CapabilityError> {
    let intent = input.intent.trim().to_string();
    let (intents, answer) = opposed(ctx, &intent)?;
    let existing = path_of(ctx, CRITIQUE, &intent);
    let created = existing.is_none();
    let source = existing.unwrap_or_else(|| new_critique_path(ctx, &intent));
    let reviewer = input
        .reviewed_by
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let reviewed_at = head_of(&ctx.index.repository.git);
    let does_not_hold = unsound(&intents, &intent);

    // a record whose own findings do not hold is not stamped
    let refusal = if !does_not_hold.is_empty() {
        Some(CapabilityError::Refused(format!(
            "the critique of {intent} does not hold and is not stamped: {}",
            does_not_hold.join("; ")
        )))
    } else if reviewed_at.is_empty() {
        Some(CapabilityError::Refused(
            "this checkout has no commit to name: a review is stamped with the commit it was run at".into(),
        ))
    } else if created && Path::new(&ctx.index.repository.root).join(&source).exists() {
        // A file where the record would go that the index does not hold as a critique is a
        // critique that does not validate. Writing a fresh one over it would destroy what a
        // reviewer wrote in order to certify that nothing was found.
        Some(CapabilityError::Refused(format!(
            "{source} exists and is not a critique this repository can read, so it is not \
             stamped and not replaced; `majordomus-cli inspect` names what is wrong with it"
        )))
    } else if created && reviewer.is_none() {
        Some(CapabilityError::InvalidInput(format!(
            "{intent} has no critique, and the record this would create must say who reviewed: give reviewed_by"
        )))
    } else {
        None
    };
    match refusal {
        Some(e) => Err(e),
        None => Ok((
            OppositionRecorded {
                intent,
                source,
                reviewed_revision: answer.reviewed_plan,
                reviewed_at,
                reviewed_with: format!("majordomus-cli {}", crate::VERSION),
                disposition: answer.disposition,
                created,
                written: false,
                event: String::new(),
            },
            reviewer,
        )),
    }
}

/// Write the stamp `stamp` describes and append its event. Every failure here is the
/// filesystem's or the vocabulary's, and each is returned naming the path it was about.
fn write_stamp(
    ctx: &Context,
    stamp: &OppositionRecorded,
    reviewer: Option<&str>,
) -> Result<(), CapabilityError> {
    let root = Path::new(&ctx.index.repository.root);
    let path = root.join(&stamp.source);
    let refused = |what: &str, e: std::io::Error| {
        CapabilityError::Refused(format!("could not {what} {}: {e}", path.display()))
    };
    let vocabulary = ctx
        .index
        .share
        .as_ref()
        .ok_or_else(|| {
            CapabilityError::Refused(
                "this index was built without a share directory, so the event vocabulary cannot be read; a review may not write an event it cannot validate".into(),
            )
        })
        .and_then(|share| {
            crate::ledger::Vocabulary::load(&share.join("events.yaml"))
                .map_err(|e| CapabilityError::Refused(e.to_string()))
        })?;

    let text = if stamp.created {
        // a fixed shape with no findings: what a reviewer adds, a reviewer writes
        Ok(format!(
            "intent: {}\nreviewed_at: {}\nreviewed_by: {}\nfindings: []\n",
            stamp.intent,
            quoted(&stamp.reviewed_at),
            quoted(reviewer.unwrap_or_default())
        ))
    } else {
        std::fs::read_to_string(&path).map_err(|e| refused("read", e))
    }?;
    // the stamp, and nothing else: every other line of the record is left as it is
    // quoted, because a commit's first ten characters can be all digits, and a scalar that
    // reads as a number is not the string the record's schema asks for
    let text = set_field_before(
        &text,
        "reviewed_at",
        &quoted(&stamp.reviewed_at),
        "findings:",
    );
    let text = set_field_before(
        &text,
        "reviewed_revision",
        &stamp.reviewed_revision,
        "findings:",
    );
    let text = set_field_before(
        &text,
        "reviewed_with",
        &quoted(&stamp.reviewed_with),
        "findings:",
    );
    path.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text))
        .map_err(|e| refused("write", e))?;

    crate::ledger::append(
        root,
        &vocabulary,
        &ctx.index.repository.git,
        &crate::ledger::now(),
        EVENT,
        &[
            ("intent", stamp.intent.clone()),
            ("reviewed_revision", stamp.reviewed_revision.clone()),
            ("disposition", stamp.disposition.as_str().to_string()),
            ("state", OppositionReviewState::Current.as_str().to_string()),
        ],
    )
    .map(|_| ())
    .map_err(|e| CapabilityError::Refused(e.to_string()))
}

fn record(
    ctx: &Context,
    input: OppositionRecordInput,
) -> Result<OppositionRecorded, CapabilityError> {
    planned_stamp(ctx, &input).and_then(|(stamp, reviewer)| {
        if input.check {
            Ok(stamp)
        } else {
            write_stamp(ctx, &stamp, reviewer.as_deref()).map(|()| OppositionRecorded {
                written: true,
                event: EVENT.to_string(),
                ..stamp
            })
        }
    })
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
    use crate::capability::{builtin, CapabilityRegistry};
    use crate::git::GitInfo;
    use crate::intent_binding::tests::planned;
    use crate::synthetic::{crate_share, SyntheticRepository};
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use std::sync::Arc;

    const CRITIQUE_FILE: &str = ".ai/repo/project/critiques/x.yaml";

    fn write(repo: &SyntheticRepository, rel: &str, text: &str) {
        let path = repo.root().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// The fixture's intent, with a criterion whose test this repository holds, so that
    /// nothing structural stands in the way unless a test puts it there.
    fn sound(repo: &SyntheticRepository) {
        write(repo, "test/cases/01_x.sh", "true\n");
        write(
            repo,
            ".ai/repo/project/intents/x.yaml",
            "id: x\ntitle: The x is true\nstatement: \"x holds.\"\ninvariants:\n  - Nothing else breaks\nmilestones:\n  - m\nsatisfaction:\n  - id: case\n    criterion: The case passes\n    evidence: test\n    ref: test/cases/01_x.sh\n",
        );
    }

    /// A context over the repository as it stands, at commit `head` (none when empty), with
    /// the distribution's share so that the event vocabulary can be read.
    fn context(repo: &SyntheticRepository, head: &str, share: Option<PathBuf>) -> Arc<Context> {
        let mut index = repo.index().unwrap();
        if !head.is_empty() {
            index.repository.git = GitState::Available(GitInfo {
                toplevel: repo.root().to_path_buf(),
                head: Some(head.to_string()),
                branch: Some("master".into()),
                working_tree: "clean".into(),
            });
        }
        index.share = share;
        let registry = CapabilityRegistry::builder()
            .with_modules(builtin::modules())
            .with_index(&index)
            .build()
            .unwrap();
        Arc::new(Context::new(Arc::new(index), Arc::new(registry)))
    }

    fn at_head(repo: &SyntheticRepository) -> Arc<Context> {
        context(repo, "0123456789abcdef", Some(crate_share()))
    }

    fn review_of(ctx: &Context) -> Value {
        ctx.execute("intent_opposition.review", json!({ "intent": "x" }))
            .unwrap()
    }

    fn stamp(ctx: &Context, input: Value) -> Result<Value, CapabilityError> {
        ctx.execute("intent_opposition.record", input)
    }

    fn refused(result: Result<Value, CapabilityError>) -> String {
        match result {
            Err(CapabilityError::Refused(why)) => why,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_reviewer_with_a_quote_or_a_line_break_is_one_scalar() {
        assert_eq!(quoted("a \"b\"\nc\\d"), "\"a 'b' c/d\"");
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

    #[test]
    fn the_review_is_derived_and_a_stamp_names_the_plan_it_was_run_over() {
        let repo = planned();
        sound(&repo);
        let ctx = at_head(&repo);

        // nobody reviewed x: the brief is whole, nothing is recorded, and nothing rejects
        let brief = review_of(&ctx);
        assert_eq!(brief["review"]["state"], "none");
        assert_eq!(brief["disposition"], "accept", "{brief}");
        assert_eq!(brief["issues"][0]["id"], "I0001");
        assert_eq!(brief["invariants"][0], "Nothing else breaks");
        let revision = brief["reviewed_plan"].as_str().unwrap().to_string();
        assert_eq!(revision.len(), 64);
        // an intent this repository does not hold is not found, never answered empty
        assert!(matches!(
            ctx.execute("intent_opposition.review", json!({ "intent": "absent" })),
            Err(CapabilityError::NotFound(_))
        ));

        // a record this would create must say who reviewed; a check writes nothing
        assert!(matches!(
            stamp(&ctx, json!({ "intent": "x" })),
            Err(CapabilityError::InvalidInput(_))
        ));
        let check = stamp(
            &ctx,
            json!({ "intent": "x", "check": true, "reviewed_by": "t" }),
        )
        .unwrap();
        assert_eq!(check["written"], false);
        assert_eq!(check["created"], true);
        assert_eq!(check["reviewed_revision"], revision.as_str());
        assert!(!repo.root().join(CRITIQUE_FILE).exists());

        // the stamp creates the record, with no finding in it, and the ledger carries the run
        let made = stamp(
            &ctx,
            json!({ "intent": "x", "reviewed_by": "a \"careful\" reviewer" }),
        )
        .unwrap();
        assert_eq!(made["written"], true);
        assert_eq!(made["event"], EVENT);
        assert_eq!(made["source"], CRITIQUE_FILE);
        let text = std::fs::read_to_string(repo.root().join(CRITIQUE_FILE)).unwrap();
        assert!(
            text.contains(&format!("reviewed_revision: {revision}\n")),
            "{text}"
        );
        assert!(text.contains("reviewed_at: \"0123456789\"\n"), "{text}");
        assert!(
            text.contains("reviewed_by: \"a 'careful' reviewer\"\n"),
            "{text}"
        );
        assert!(text.ends_with("findings: []\n"), "{text}");
        let ledger =
            std::fs::read_to_string(repo.root().join(".ai/local/state/ledger.jsonl")).unwrap();
        assert!(
            ledger.contains("\"event\":\"opposition.recorded\""),
            "{ledger}"
        );
        assert!(ledger.contains(&revision), "{ledger}");

        // read again, the review is of the plan as it stands
        let ctx = at_head(&repo);
        let after = review_of(&ctx);
        assert_eq!(after["review"]["state"], "current");
        assert_eq!(after["review"]["reviewed_by"], "a 'careful' reviewer");
        assert!(after["review"]["reviewed_with"]
            .as_str()
            .unwrap()
            .starts_with("majordomus-cli "));

        // a reviewer's findings are stamped around, never through
        write(
            &repo,
            CRITIQUE_FILE,
            "intent: x\nreviewed_at: old\nreviewed_by: a reviewer\nfindings:\n  - id: thin\n    class: invariant_conflict\n    subject: x#case\n    finding: \"One case: it may not be enough\"\n    source: a second session\n    blocking: true\n    resolution:\n      state: planned\n      issue: I0001\n      resolved_by: the author\n",
        );
        let ctx = at_head(&repo);
        let before = review_of(&ctx);
        assert_eq!(before["review"]["state"], "not_stamped");
        assert_eq!(before["disposition"], "accept_with_required_changes");
        assert_eq!(before["recorded"][0]["class"], "invariant_conflict");
        assert_eq!(before["recorded"][0]["source"], "a second session");
        assert_eq!(before["recorded"][0]["resolved_by"], "the author");
        let stamped = stamp(&ctx, json!({ "intent": "x" })).unwrap();
        assert_eq!(stamped["created"], false);
        assert_eq!(stamped["disposition"], "accept_with_required_changes");
        let text = std::fs::read_to_string(repo.root().join(CRITIQUE_FILE)).unwrap();
        let findings = &text[text.find("findings:").unwrap()..];
        assert!(
            findings.contains("finding: \"One case: it may not be enough\"\n"),
            "{text}"
        );
        assert!(
            text.contains("reviewed_by: a reviewer\n"),
            "an existing reviewer is kept: {text}"
        );
        assert!(text.contains("reviewed_at: \"0123456789\"\n"), "{text}");

        // the plan changes — the issue reaches further — and the review is of another plan
        let issue = std::fs::read_to_string(repo.root().join(".ai/repo/project/issues/I0001.yaml"))
            .unwrap();
        write(
            &repo,
            ".ai/repo/project/issues/I0001.yaml",
            &issue.replace("scope:\n  - lib\n", "scope:\n  - lib\n  - docs\n"),
        );
        let ctx = at_head(&repo);
        assert_eq!(review_of(&ctx)["review"]["state"], "stale");
        let bound = ctx
            .execute("intents.binding", json!({ "issue": "I0001", "paths": "" }))
            .unwrap();
        assert_eq!(bound["standing"], "refused");
        assert_eq!(bound["refusals"][0]["cause"], "critique_stale");
        assert_eq!(bound["reviews"][0]["state"], "stale");
        let validation = ctx.execute("intents.validate", json!({})).unwrap();
        let finding = |code: &str| {
            validation["findings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["code"] == code)
                .cloned()
        };
        assert_eq!(
            finding("critique_stale").expect("stale is named")["level"],
            "WARN"
        );
    }

    #[test]
    fn a_required_opposition_refuses_what_nobody_ran_and_what_nobody_resolved() {
        let repo = planned();
        sound(&repo);
        write(
            &repo,
            ".ai/repo/policy.yaml",
            "version: 1\ncontext:\n  always_loaded_budget_lines: 150\nintent:\n  binding: required\n  opposition: required\n",
        );
        write(
            &repo,
            CRITIQUE_FILE,
            "intent: x\nreviewed_at: old\nreviewed_by: a reviewer\nfindings:\n  - id: thin\n    class: dependency_order\n    subject: x#case\n    finding: Wrong order\n    blocking: true\n    resolution:\n      state: planned\n      issue: I0001\n",
        );
        let ctx = at_head(&repo);
        let validation = ctx.execute("intents.validate", json!({})).unwrap();
        let level_of = |code: &str| {
            validation["findings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["code"] == code)
                .map(|f| f["level"].clone())
        };
        assert_eq!(
            level_of("critique_not_stamped"),
            Some(json!("FAIL")),
            "{validation}"
        );
        assert_eq!(
            level_of("resolution_names_no_resolver"),
            Some(json!("FAIL"))
        );
        let bound = ctx
            .execute("intents.binding", json!({ "issue": "I0001", "paths": "" }))
            .unwrap();
        let causes: Vec<&str> = bound["refusals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["cause"].as_str().unwrap())
            .collect();
        assert!(causes.contains(&"opposition_not_executed"), "{bound}");
        // a failure about the intent is a structural finding, and it rejects the plan
        assert!(causes.contains(&"plan_rejected"), "{bound}");
        assert_eq!(review_of(&ctx)["disposition"], "reject");

        // a dismissal nobody is named as having made is no more accepted than a plan
        write(
            &repo,
            CRITIQUE_FILE,
            "intent: x\nreviewed_at: old\nreviewed_by: a reviewer\nfindings:\n  - id: thin\n    class: insufficient_work\n    subject: x#case\n    finding: Thin\n    blocking: true\n    resolution:\n      state: rejected\n      because: Not this time\n  - id: later\n    class: insufficient_work\n    subject: x#case\n    finding: Later\n    blocking: false\n    resolution:\n      state: open\n",
        );
        let ctx = at_head(&repo);
        let dismissed = ctx.execute("intents.validate", json!({})).unwrap();
        let unnamed: Vec<&str> = dismissed["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["code"] == "resolution_names_no_resolver")
            .map(|f| f["subject"].as_str().unwrap())
            .collect();
        assert_eq!(
            unnamed,
            ["x:thin"],
            "an open finding has no resolver to name"
        );

        // a resolution into a cancelled issue resolves nothing, and is not stamped
        let issue = std::fs::read_to_string(repo.root().join(".ai/repo/project/issues/I0001.yaml"))
            .unwrap();
        write(
            &repo,
            ".ai/repo/project/issues/I0003.yaml",
            &(issue.replace("I0001", "I0003") + "cancelled: true\n"),
        );
        write(
            &repo,
            CRITIQUE_FILE,
            "intent: x\nreviewed_at: old\nreviewed_by: a reviewer\nfindings:\n  - id: thin\n    class: insufficient_work\n    subject: x#case\n    finding: Thin\n    blocking: true\n    resolution:\n      state: planned\n      issue: I0003\n      resolved_by: the author\n",
        );
        let ctx = at_head(&repo);
        let why = refused(stamp(&ctx, json!({ "intent": "x" })));
        assert!(why.contains("planned_into_cancelled_issue"), "{why}");
        assert!(!std::fs::read_to_string(repo.root().join(CRITIQUE_FILE))
            .unwrap()
            .contains("reviewed_revision"));
    }

    #[test]
    fn a_stamp_that_cannot_be_made_or_written_says_which_step_failed() {
        // no commit to name
        let repo = planned();
        sound(&repo);
        let headless = context(&repo, "", Some(crate_share()));
        let why = refused(stamp(
            &headless,
            json!({ "intent": "x", "reviewed_by": "t" }),
        ));
        assert!(why.contains("no commit to name"), "{why}");

        // a file where the record would go that is not a readable critique is not replaced
        write(
            &repo,
            CRITIQUE_FILE,
            "intent: x\nfindings: \"not a list\"\nsurprise: true\n",
        );
        let ctx = at_head(&repo);
        if review_of(&ctx)["review"]["state"] == "none" {
            let why = refused(stamp(&ctx, json!({ "intent": "x", "reviewed_by": "t" })));
            assert!(
                why.contains("is not a critique this repository can read"),
                "{why}"
            );
        }
        std::fs::remove_file(repo.root().join(CRITIQUE_FILE)).unwrap();

        // no share directory, and a share with no vocabulary: no event can be validated
        let shareless = context(&repo, "0123456789abcdef", None);
        let why = refused(stamp(
            &shareless,
            json!({ "intent": "x", "reviewed_by": "t" }),
        ));
        assert!(why.contains("without a share directory"), "{why}");
        let empty = repo.root().join("empty-share");
        std::fs::create_dir_all(&empty).unwrap();
        let no_vocabulary = context(&repo, "0123456789abcdef", Some(empty));
        refused(stamp(
            &no_vocabulary,
            json!({ "intent": "x", "reviewed_by": "t" }),
        ));
        assert!(
            !repo.root().join(CRITIQUE_FILE).exists(),
            "a refused stamp wrote"
        );

        // the record's directory cannot be made: something else stands where it would be
        let ctx = at_head(&repo);
        let _ = std::fs::remove_dir_all(repo.root().join(".ai/repo/project/critiques"));
        std::fs::write(repo.root().join(".ai/repo/project/critiques"), "a file").unwrap();
        let why = refused(stamp(&ctx, json!({ "intent": "x", "reviewed_by": "t" })));
        assert!(why.contains("could not write"), "{why}");
        std::fs::remove_file(repo.root().join(".ai/repo/project/critiques")).unwrap();

        // an existing record that vanished between the read of the index and the stamp
        write(
            &repo,
            CRITIQUE_FILE,
            "intent: x\nreviewed_at: old\nreviewed_by: a reviewer\nfindings: []\n",
        );
        let ctx = at_head(&repo);
        std::fs::remove_file(repo.root().join(CRITIQUE_FILE)).unwrap();
        let why = refused(stamp(&ctx, json!({ "intent": "x" })));
        assert!(why.contains("could not read"), "{why}");

        // the ledger cannot be appended to: the record is written and the failure is said
        write(
            &repo,
            CRITIQUE_FILE,
            "intent: x\nreviewed_at: old\nreviewed_by: a reviewer\nfindings: []\n",
        );
        let ctx = at_head(&repo);
        let _ = std::fs::remove_dir_all(repo.root().join(".ai/local"));
        std::fs::write(repo.root().join(".ai/local"), "a file").unwrap();
        refused(stamp(&ctx, json!({ "intent": "x" })));
        std::fs::remove_file(repo.root().join(".ai/local")).unwrap();

        // an evidence ledger nobody can read: the opposition cannot be derived at all
        write(&repo, ".ai/repo/evidence/ledger.json", "{ not json");
        let ctx = at_head(&repo);
        assert!(matches!(
            ctx.execute("intent_opposition.review", json!({ "intent": "x" })),
            Err(CapabilityError::Internal(_))
        ));
        assert!(stamp(&ctx, json!({ "intent": "x" })).is_err());
    }
}
