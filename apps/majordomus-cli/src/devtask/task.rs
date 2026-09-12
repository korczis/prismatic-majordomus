//! One issue read as an executable development task.
//!
//! Four groups, one per owner of the facts in it — see the [module documentation] for the
//! table — and every field of every group attested. Nothing is read here: [`DevTaskModel`]
//! carries the already-derived plan, the already-parsed record, the already-derived git
//! trace and the already-resolved session records, and [`DevTask::build`] is a pure
//! function of them. That is what lets the whole model be unit-tested without a repository
//! and what keeps the derivations single: this file composes, it does not compute.
//!
//! [module documentation]: super

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::plan::{Plan, PlanIssue};
use crate::worktree::trace::IssueTrace;

use super::readiness::{Blocker, BlockerKind, TaskReadiness, MILESTONE_BLOCKER};
use super::{AttestationTally, AttestedCount, AttestedList, AttestedText};

/// The adapter that owns the projection of this model onto GitHub.
///
/// Named, not reimplemented. This executable makes no network call —
/// `test/cases/08_no_forbidden_constructs.sh` is the proof — so the live state of the
/// external record is not a fact any field here can carry, and the honest answer is the
/// name of the one thing that can answer it. Restating the adapter's reconciliation
/// vocabulary here would be the repeated semantic definition ADR 0004 forbids.
pub const PROJECTION_ADAPTER: &str = "scripts/github-sync";

/// The commands that answer the external half of a task's state.
pub const RECONCILE_WITH: &[&str] = &["scripts/github-sync --check", "scripts/traceability"];

/// Everything [`DevTask::build`] needs, gathered by the caller that has a repository.
#[derive(Debug)]
pub struct DevTaskModel<'a> {
    /// The derived plan: the one derivation of status, waves, dependents and findings.
    pub plan: &'a Plan,
    /// The canonical record, when the model declares this issue.
    pub record: Option<RecordRef<'a>>,
    /// What git says about the branches and commits that realised it, when git was
    /// consulted. `None` is not "nothing realised it": it is "git was not asked", and the
    /// execution group says which.
    pub trace: Option<&'a IssueTrace>,
    /// The session records whose branch names this issue, in canonical order. Resolved by
    /// the caller because the rule belongs to [`crate::worktree::state::issue_of`] and is
    /// not restated here.
    pub sessions: Vec<SessionRef>,
    /// The repository the external projection targets, as `project.yaml` declares it.
    pub repository: Option<&'a str>,
}

/// The canonical record: where it is, and what it says.
#[derive(Debug, Clone, Copy)]
pub struct RecordRef<'a> {
    /// Repository-relative path of the file the record was read from.
    pub path: &'a str,
    /// The parsed YAML, whole. Key *presence* here is what makes a field explicit, which is
    /// the one thing the flattened plan cannot tell a reader.
    pub metadata: &'a Value,
}

/// One session record that names this issue, and how strongly.
///
/// The two ways a session reaches an issue are not equally good, and the difference is the
/// whole reason this carries a flag rather than being a bare id:
///
/// * the record's own `issues` key. `share/schemas/majordomus/session-record` describes it
///   as "the issues it moved, by id — derived from the ledger's own events for this
///   episode; never authored", so it is a canonical link that a machine worked out from
///   events it can read. That is `FieldProvenance::Derived`.
/// * the branch name. A record always declares the branch it was written on, and
///   [`crate::worktree::state::issue_of`] reads an issue id out of one. It is a rule about a
///   name and it can be wrong — a branch renamed, an id that is a prefix of another — so it
///   is `FieldProvenance::Inferred`.
///
/// Reporting both under one provenance would make the weaker of them look like the
/// stronger, which is the defect this whole module exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionRef {
    /// The episode's id.
    pub session_id: String,
    /// The branch it was written on.
    pub branch: String,
    /// When the record says it was written.
    pub created_at: String,
    /// Repository-relative path of the record.
    pub path: String,
    /// True when the record's own `issues` key names this issue: a canonical link, not a
    /// guess about a branch name.
    pub declares_issue: bool,
}

// ---------------------------------------------------------------- the groups

/// What a person authored in the canonical record. Every field is `FieldProvenance::Explicit`
/// or `FieldProvenance::Unknown` and nothing else: a value here was written down, or it
/// does not exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DevTaskDeclaration {
    /// One line naming the outcome.
    pub title: AttestedText,
    /// What the issue is for.
    pub objective: AttestedText,
    /// Why it is worth doing.
    pub why: AttestedText,
    /// Where the repository stands before it.
    pub current_state: AttestedText,
    /// Where the repository stands once it is real.
    pub desired_state: AttestedText,
    /// What the author says about how far it got. Authored prose, never a status: the
    /// status is derived, and this is the one field where a person may disagree with it.
    pub completion: AttestedText,
    /// The milestone it belongs to.
    pub milestone: AttestedText,
    /// `p0` … `p3`.
    pub priority: AttestedText,
    /// The execution profile it is worked under.
    pub profile: AttestedText,
    /// Whether it may run beside another issue of its wave.
    pub parallel_safe: AttestedText,
    /// Who owns it.
    pub owner: AttestedText,
    /// What could go wrong.
    pub risk: AttestedText,
    /// The slug.
    pub slug: AttestedText,
    /// Whether it was withdrawn.
    pub cancelled: AttestedText,
    /// When the record was created.
    pub created_at: AttestedText,
    /// When it last moved.
    pub updated_at: AttestedText,
    /// When execution began.
    pub started_at: AttestedText,
    /// When implementation was declared finished.
    pub verified_at: AttestedText,
    /// When completion was recorded.
    pub completed_at: AttestedText,
    /// The paths it may touch.
    pub scope: AttestedList,
    /// The paths it deliberately does not touch.
    pub non_scope: AttestedList,
    /// The issues it declares a dependency on, exactly as declared — including one that
    /// does not exist, which is a diagnostic and not a silent omission.
    pub depends_on: AttestedList,
    /// What must be true before it is finished.
    pub acceptance_criteria: AttestedList,
    /// The commands that check it.
    pub validation: AttestedList,
    /// The evidence tokens it requires before it may be complete.
    pub evidence_required: AttestedList,
    /// The evidence tokens it actually carries.
    pub evidence_present: AttestedList,
    /// The commits the record's own evidence entries name. Authored, and therefore a
    /// different fact from the commits git finds on the branches — which are in
    /// [`DevTaskExecution::commits`] and are derived.
    pub evidence_commits: AttestedList,
}

/// Where the task sits in the plan graph. Every field is derived; nothing here is authored,
/// and nothing here is stored anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DevTaskPosition {
    /// The canonical status, from the one derivation of it.
    pub status: AttestedText,
    /// The execution wave: one past the longest path to it. Unknown when a cycle prevents
    /// it from ever having one.
    pub wave: AttestedCount,
    /// The dependencies that are not `DONE`, plus the milestone gate when it applies.
    pub blocked_by: AttestedList,
    /// The issues that wait on this one.
    pub dependents: AttestedList,
    /// Every issue that waits on it, directly or through another. The measure of what
    /// finishing it unblocks.
    pub transitive_dependents: AttestedCount,
    /// The status of the milestone it belongs to.
    pub milestone_status: AttestedText,
    /// The milestone's layer in the milestone graph.
    pub milestone_rank: AttestedCount,
    /// The outcomes that milestone still waits on.
    pub milestone_blocked_by: AttestedList,
    /// How many evidence tokens it carries.
    pub evidence_have: AttestedCount,
    /// How many it needs.
    pub evidence_need: AttestedCount,
}

/// Local execution state: what actually happened in this checkout's git history and session
/// records. Derived where the relation was declared, inferred where it was not, and the
/// fields say which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DevTaskExecution {
    /// Whether git was consulted at all. False makes every git-derived field below
    /// `unknown` rather than empty, which is the difference between "nothing realised this"
    /// and "nobody looked".
    pub git_consulted: bool,
    /// The branches whose name carries this issue's id, local first.
    pub branches: AttestedList,
    /// Of those, the ones that reached the trunk.
    pub merged_branches: AttestedList,
    /// How many distinct commits those branches hold that the trunk did not.
    pub commits: AttestedCount,
    /// The trunk every branch was measured against.
    pub trunk: AttestedText,
    /// True when every branch's commits could be derived. False when one reached the trunk
    /// without a merge commit of its own, so part of the work cannot be told from the
    /// trunk's — reported rather than counted as zero.
    pub commits_complete: AttestedText,
    /// The session records whose own `issues` key names this issue. Derived: the key is
    /// worked out from the episode's ledger events, not authored and not guessed.
    pub sessions: AttestedList,
    /// The session records that reach this issue only through their branch name, and which
    /// therefore may be wrong. Never overlaps [`DevTaskExecution::sessions`]: a session that
    /// declared the issue is reported there and not here, so the weaker relation never
    /// stands in for the stronger one.
    pub sessions_by_branch: AttestedList,
}

/// The external projection. Nothing here is ever written by this executable, and the fields
/// that only the adapter can answer say so rather than reading as empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DevTaskSynchronisation {
    /// The adapter that owns the projection. The one thing that can answer the live state.
    pub adapter: String,
    /// The repository the projection targets, from `project.yaml`.
    pub repository: AttestedText,
    /// The reconciliation state of the external record. Always `unknown` from here: it is a
    /// network fact, and this executable makes none. The reason names the command.
    pub state: AttestedText,
    /// The external record's own identifier — a GitHub number — which GitHub assigns and
    /// nothing local records. Always `unknown`, for the same reason.
    pub external_id: AttestedText,
    /// The commands that answer the external half.
    pub reconcile_with: Vec<String>,
    /// Whether this executable may write any of it. Always false; stated as a field so that
    /// a client need not know the rule to respect it.
    pub writable: bool,
}

/// The readiness verdict, with everything it was derived from beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReadinessVerdict {
    /// Where the task stands, as work.
    pub state: TaskReadiness,
    /// The canonical status it refines, so the two can never be read apart.
    pub canonical_status: String,
    /// Why it is in that state, in one line.
    pub reason: String,
    /// May a worker pick it up right now?
    pub startable: bool,
    /// Is there nothing further to do?
    pub terminal: bool,
    /// Everything standing in the way, each with what to do about it.
    pub blockers: Vec<Blocker>,
}

/// One thing wrong with this task's records, in the shape `project.finding-carries-reproduce`
/// asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DevTaskDiagnostic {
    /// `FAIL` or `WARN`, as the plan's own finding levels.
    pub level: String,
    /// The stable code a reader greps for.
    pub code: String,
    /// What is wrong, in one line.
    pub message: String,
    /// The one command that shows it again.
    pub reproduce: String,
}

// ---------------------------------------------------------------- the task

/// One issue as an executable development task: what was authored, where it sits, what has
/// happened to it, where its external projection stands, and whether a worker may start it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DevTask {
    /// The issue id, as asked for.
    pub issue: String,
    /// Whether the canonical model declares it. False is answered rather than refused, with
    /// every canonical field `unknown`: a typo that read as "nothing has been authored" is
    /// the one answer this must never give.
    pub declared: bool,
    /// Repository-relative path of the canonical record, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<String>,
    /// What a person authored.
    pub declaration: DevTaskDeclaration,
    /// Where the plan graph puts it.
    pub position: DevTaskPosition,
    /// What happened locally.
    pub execution: DevTaskExecution,
    /// Where the external projection stands.
    pub synchronisation: DevTaskSynchronisation,
    /// Whether it can be worked on, and what is in the way.
    pub readiness: ReadinessVerdict,
    /// Everything wrong with its records.
    pub diagnostics: Vec<DevTaskDiagnostic>,
    /// How much of the answer was authored and how much a machine worked out.
    pub attestation: AttestationTally,
}

/// Is the key present in the record, and does it carry anything? A key present and null is
/// as good as absent: a reader cannot act on it, and calling it explicit would be the
/// defect this module exists to prevent.
fn present<'a>(meta: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    match meta?.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.is_empty() => None,
        Some(v) => Some(v),
    }
}

/// One scalar of the record, attested by whether the record carries the key.
fn authored(record: Option<RecordRef<'_>>, key: &str) -> AttestedText {
    let source = source_of(record, key);
    match present(record.map(|r| r.metadata), key) {
        Some(v) => AttestedText::explicit(crate::plan::scalar(Some(v)), source),
        None if record.is_none() => AttestedText::unknown(
            source,
            "the canonical model declares no issue with this id".to_string(),
        ),
        None => AttestedText::unknown(source, format!("the record declares no `{key}`")),
    }
}

/// One list of the record, attested by whether the record carries the key — so an authored
/// empty list is explicit and an absent key is unknown.
fn authored_list(record: Option<RecordRef<'_>>, key: &str) -> AttestedList {
    let source = source_of(record, key);
    match record.map(|r| r.metadata).and_then(|m| m.get(key)) {
        Some(Value::Array(items)) => AttestedList::explicit(
            items.iter().map(|v| crate::plan::scalar(Some(v))).collect(),
            source,
        ),
        // A scalar where a list was expected is one item, which is what the flattener
        // produces for `scope: apps/` too; the plan reads it the same way.
        Some(other) if !other.is_null() => {
            AttestedList::explicit(vec![crate::plan::scalar(Some(other))], source)
        }
        _ if record.is_none() => {
            AttestedList::unknown(source, "the canonical model declares no issue with this id")
        }
        _ => AttestedList::unknown(source, format!("the record declares no `{key}`")),
    }
}

/// Where a reader goes to check one authored field.
fn source_of(record: Option<RecordRef<'_>>, key: &str) -> String {
    match record {
        Some(r) => format!("{}#{key}", r.path),
        None => format!(".ai/repo/project/issues#{key}"),
    }
}

/// The `covers` token of every evidence entry that carries one, and the `commit` of every
/// entry that carries one. Read from the record rather than from the plan, because the plan
/// counts them and this names them.
fn evidence_field(record: Option<RecordRef<'_>>, key: &str) -> AttestedList {
    let source = source_of(record, &format!("evidence[].{key}"));
    let Some(RecordRef { metadata, .. }) = record else {
        return AttestedList::unknown(source, "the canonical model declares no issue with this id");
    };
    match metadata.get("evidence") {
        Some(Value::Array(items)) => AttestedList::explicit(
            items
                .iter()
                .filter_map(|e| e.get(key))
                .map(|v| crate::plan::scalar(Some(v)))
                .collect(),
            source,
        ),
        _ => AttestedList::unknown(source, "the record carries no `evidence` entries"),
    }
}

impl DevTask {
    /// The capability id every field of the derived half points a reader at.
    const PLAN: &'static str = "plan.model";
    /// The capability id the git-derived half points a reader at.
    const TRACE: &'static str = "trace.issue";

    /// Compose one development task out of derivations that already exist.
    ///
    /// Pure: every input is a value the caller already has, and the only work done here is
    /// attesting each field and deriving the readiness. A repository is never touched.
    #[allow(clippy::too_many_lines)] // one composition, field by field, with each attested
    pub fn build(model: &DevTaskModel<'_>, issue: &str) -> DevTask {
        let record = model.record;
        let declared = record.is_some();
        let derived = model.plan.issue(issue);
        let milestone = derived.and_then(|i| model.plan.milestone(&i.milestone));

        let declaration = DevTaskDeclaration {
            title: authored(record, "title"),
            objective: authored(record, "objective"),
            why: authored(record, "why"),
            current_state: authored(record, "current_state"),
            desired_state: authored(record, "desired_state"),
            completion: authored(record, "completion"),
            milestone: authored(record, "milestone"),
            priority: authored(record, "priority"),
            profile: authored(record, "profile"),
            parallel_safe: authored(record, "parallel_safe"),
            owner: authored(record, "owner"),
            risk: authored(record, "risk"),
            slug: authored(record, "slug"),
            cancelled: authored(record, "cancelled"),
            created_at: authored(record, "created_at"),
            updated_at: authored(record, "updated_at"),
            started_at: authored(record, "started_at"),
            verified_at: authored(record, "verified_at"),
            completed_at: authored(record, "completed_at"),
            scope: authored_list(record, "scope"),
            non_scope: authored_list(record, "non_scope"),
            depends_on: authored_list(record, "depends_on"),
            acceptance_criteria: authored_list(record, "acceptance_criteria"),
            validation: authored_list(record, "validation"),
            evidence_required: authored_list(record, "evidence_required"),
            evidence_present: evidence_field(record, "covers"),
            evidence_commits: evidence_field(record, "commit"),
        };

        let position = Self::position(model.plan, derived, milestone);
        let execution = Self::execution(model.trace, &model.sessions);
        let synchronisation = Self::synchronisation(model.repository);
        let readiness = Self::readiness(
            model.plan,
            derived,
            &declaration,
            milestone.map(|m| m.id.as_str()),
        );
        let diagnostics = Self::diagnostics(model.plan, issue);

        let mut task = DevTask {
            issue: issue.to_string(),
            declared,
            record: record.map(|r| r.path.to_string()),
            declaration,
            position,
            execution,
            synchronisation,
            readiness,
            diagnostics,
            attestation: AttestationTally::default(),
        };
        task.attestation = task.tally();
        task
    }

    fn position(
        plan: &Plan,
        derived: Option<&PlanIssue>,
        milestone: Option<&crate::plan::PlanMilestone>,
    ) -> DevTaskPosition {
        let absent = "the canonical model declares no issue with this id";
        let Some(i) = derived else {
            return DevTaskPosition {
                status: AttestedText::unknown(Self::PLAN, absent),
                wave: AttestedCount::unknown(Self::PLAN, absent),
                blocked_by: AttestedList::unknown(Self::PLAN, absent),
                dependents: AttestedList::unknown(Self::PLAN, absent),
                transitive_dependents: AttestedCount::unknown(Self::PLAN, absent),
                milestone_status: AttestedText::unknown(Self::PLAN, absent),
                milestone_rank: AttestedCount::unknown(Self::PLAN, absent),
                milestone_blocked_by: AttestedList::unknown(Self::PLAN, absent),
                evidence_have: AttestedCount::unknown(Self::PLAN, absent),
                evidence_need: AttestedCount::unknown(Self::PLAN, absent),
            };
        };
        let cyclic = plan
            .findings
            .iter()
            .any(|f| f.code == "cycle" && f.message.split_whitespace().any(|w| w == i.id));
        DevTaskPosition {
            status: AttestedText::derived(&i.status, Self::PLAN),
            wave: if cyclic {
                AttestedCount::unknown(
                    Self::PLAN,
                    "a dependency cycle prevents it from ever having a wave",
                )
            } else {
                AttestedCount::derived(i.wave, Self::PLAN)
            },
            blocked_by: AttestedList::derived(i.blocked_by.clone(), Self::PLAN),
            dependents: AttestedList::derived(i.dependents.clone(), Self::PLAN),
            transitive_dependents: AttestedCount::derived(
                u32::try_from(super::graph::transitive_dependents(&plan.issues, &i.id).len())
                    .unwrap_or(u32::MAX),
                Self::PLAN,
            ),
            milestone_status: milestone.map_or_else(
                || {
                    AttestedText::unknown(
                        Self::PLAN,
                        "the issue names no milestone the model declares",
                    )
                },
                |m| AttestedText::derived(&m.status, Self::PLAN),
            ),
            milestone_rank: milestone.map_or_else(
                || {
                    AttestedCount::unknown(
                        Self::PLAN,
                        "the issue names no milestone the model declares",
                    )
                },
                |m| AttestedCount::derived(m.rank, Self::PLAN),
            ),
            milestone_blocked_by: milestone.map_or_else(
                || {
                    AttestedList::unknown(
                        Self::PLAN,
                        "the issue names no milestone the model declares",
                    )
                },
                |m| AttestedList::derived(m.blocked_by.clone(), Self::PLAN),
            ),
            evidence_have: AttestedCount::derived(i.evidence_have, Self::PLAN),
            evidence_need: AttestedCount::derived(i.evidence_need, Self::PLAN),
        }
    }

    fn execution(trace: Option<&IssueTrace>, sessions: &[SessionRef]) -> DevTaskExecution {
        let unasked = "git was not consulted for this answer";
        let declared_field = AttestedList::derived(
            sessions
                .iter()
                .filter(|s| s.declares_issue)
                .map(|s| s.session_id.clone())
                .collect(),
            "kind:session#issues",
        );
        let branch_field = AttestedList::inferred(
            sessions
                .iter()
                .filter(|s| !s.declares_issue)
                .map(|s| s.session_id.clone())
                .collect(),
            "kind:session#branch",
            "the record's own `issues` key does not name this issue; the link is the \
             branch-name rule of worktree::state::issue_of, which a rename can break",
        );
        let Some(t) = trace else {
            return DevTaskExecution {
                git_consulted: false,
                branches: AttestedList::unknown(Self::TRACE, unasked),
                merged_branches: AttestedList::unknown(Self::TRACE, unasked),
                commits: AttestedCount::unknown(Self::TRACE, unasked),
                trunk: AttestedText::unknown(Self::TRACE, unasked),
                commits_complete: AttestedText::unknown(Self::TRACE, unasked),
                sessions: declared_field,
                sessions_by_branch: branch_field,
            };
        };
        DevTaskExecution {
            git_consulted: true,
            branches: AttestedList::derived(
                t.branches.iter().map(|b| b.name.clone()).collect(),
                Self::TRACE,
            ),
            merged_branches: AttestedList::derived(
                t.branches
                    .iter()
                    .filter(|b| b.merge_commit.is_some())
                    .map(|b| b.name.clone())
                    .collect(),
                Self::TRACE,
            ),
            commits: AttestedCount::derived(
                u32::try_from(t.commits).unwrap_or(u32::MAX),
                Self::TRACE,
            ),
            trunk: t.trunk.as_ref().map_or_else(
                || AttestedText::unknown(Self::TRACE, "this repository has no trunk ref"),
                |trunk| AttestedText::derived(trunk, Self::TRACE),
            ),
            commits_complete: AttestedText::derived(t.complete.to_string(), Self::TRACE),
            sessions: declared_field,
            sessions_by_branch: branch_field,
        }
    }

    fn synchronisation(repository: Option<&str>) -> DevTaskSynchronisation {
        let network = format!(
            "a live GitHub state; this executable makes no network call, so only \
             `{PROJECTION_ADAPTER} --check` can answer it"
        );
        DevTaskSynchronisation {
            adapter: PROJECTION_ADAPTER.to_string(),
            repository: repository.map_or_else(
                || {
                    // Deliberately not "project.yaml names no repository". In this
                    // repository the file does name one and the header is empty anyway:
                    // `.ai/repo/knowledge/sources.yaml` declares a source class for
                    // `project/milestones/*.yaml` and one for `project/issues/*.yaml` and
                    // none for `project/project.yaml`, so the `project` kind has no objects
                    // and `Plan::project` is blank for every reader of it. Saying the file
                    // is silent would be this module inventing a fact about a file it did
                    // not read, which is the whole thing it exists not to do.
                    AttestedText::unknown(
                        "plan.model#project.repository",
                        "the derived plan's header carries no repository",
                    )
                },
                |r| AttestedText::explicit(r, ".ai/repo/project/project.yaml#repository"),
            ),
            state: AttestedText::unknown(PROJECTION_ADAPTER, network.clone()),
            external_id: AttestedText::unknown(PROJECTION_ADAPTER, network),
            reconcile_with: RECONCILE_WITH.iter().map(|c| (*c).to_string()).collect(),
            writable: false,
        }
    }

    fn readiness(
        plan: &Plan,
        derived: Option<&PlanIssue>,
        declaration: &DevTaskDeclaration,
        milestone: Option<&str>,
    ) -> ReadinessVerdict {
        let Some(i) = derived else {
            return ReadinessVerdict {
                state: TaskReadiness::Undeclared,
                canonical_status: String::new(),
                reason: "the canonical model declares no issue with this id".into(),
                startable: false,
                terminal: false,
                blockers: vec![Blocker::new(
                    BlockerKind::Reference,
                    "",
                    "no canonical record; author one under .ai/repo/project/issues/ \
                     before anything can be executed against it",
                )],
            };
        };
        let completion_recorded = !declaration.completed_at.text().is_empty();
        let evidence_covered = i.evidence_have >= i.evidence_need;
        let state = TaskReadiness::derive(
            &i.status,
            &i.blocked_by,
            completion_recorded,
            evidence_covered,
        );
        let mut blockers: Vec<Blocker> = Vec::new();
        for b in &i.blocked_by {
            if let Some(m) = b.strip_prefix(MILESTONE_BLOCKER) {
                blockers.push(Blocker::new(
                    BlockerKind::Milestone,
                    m,
                    format!("the milestone {m} waits on an outcome of its own"),
                ));
            } else {
                blockers.push(Blocker::new(
                    BlockerKind::Issue,
                    b,
                    format!("{b} is not DONE"),
                ));
            }
        }
        // A dependency the model does not declare cannot appear in `blocked_by` — the plan
        // drops the edge and reports `unknown_dependency` — so it is named here from the
        // declaration, or it would be a blocker nobody could see from this surface.
        for d in &declaration.depends_on.values {
            if plan.issue(d).is_none() && d != &i.id {
                blockers.push(Blocker::new(
                    BlockerKind::Reference,
                    d,
                    format!(
                        "{d} is declared as a dependency and the model has no such issue; \
                         fix the reference or author the record"
                    ),
                ));
            }
        }
        // A cycle is reported once, about the whole graph; an issue in it or downstream of
        // it can never become ready, and that is a blocker of its own kind.
        for f in &plan.findings {
            if f.code == "cycle" && f.message.split_whitespace().any(|w| w == i.id) {
                blockers.push(Blocker::new(
                    BlockerKind::Cycle,
                    &i.id,
                    "a dependency cycle prevents it from ever becoming ready".to_string(),
                ));
            }
        }
        if state == TaskReadiness::CompletionBlocked {
            let have = &declaration.evidence_present.values;
            for need in &declaration.evidence_required.values {
                if !have.contains(need) {
                    blockers.push(Blocker::new(
                        BlockerKind::Evidence,
                        need,
                        format!(
                            "completion is recorded and no evidence entry covers `{need}`; \
                             record it with `majordomus evidence`"
                        ),
                    ));
                }
            }
        }
        let reason = match state {
            TaskReadiness::Ready => {
                "no dependency and no gate holds it; nobody has started it".into()
            }
            TaskReadiness::Blocked => format!(
                "{} of its dependencies {} not DONE",
                blockers
                    .iter()
                    .filter(|b| b.kind == BlockerKind::Issue)
                    .count(),
                if blockers
                    .iter()
                    .filter(|b| b.kind == BlockerKind::Issue)
                    .count()
                    == 1
                {
                    "is"
                } else {
                    "are"
                }
            ),
            TaskReadiness::Waiting => format!(
                "every dependency is satisfied; the milestone {} waits on an outcome of its own",
                milestone.unwrap_or("it belongs to")
            ),
            TaskReadiness::InProgress => "execution began; the record carries started_at".into(),
            TaskReadiness::Review => {
                "implementation is declared finished and completion is not recorded".into()
            }
            TaskReadiness::CompletionBlocked => {
                "completion is recorded and the evidence the record requires is not all there"
                    .into()
            }
            TaskReadiness::Complete => {
                "completion is recorded and every required evidence token is present".into()
            }
            TaskReadiness::Cancelled => "withdrawn".into(),
            TaskReadiness::Undeclared => format!(
                "the plan assigned the status `{}`, which its own vocabulary does not declare",
                i.status
            ),
        };
        ReadinessVerdict {
            state,
            canonical_status: i.status.clone(),
            reason,
            startable: state.is_startable(),
            terminal: state.is_terminal(),
            blockers,
        }
    }

    /// Every plan finding whose subject is this issue, plus the graph-wide ones that name
    /// it. The plan is the only validator; this filters its findings, and adds none.
    fn diagnostics(plan: &Plan, issue: &str) -> Vec<DevTaskDiagnostic> {
        plan.findings
            .iter()
            .filter(|f| {
                f.subject == issue
                    || (f.subject == "graph" && f.message.split_whitespace().any(|w| w == issue))
            })
            .map(|f| DevTaskDiagnostic {
                level: f.level.clone(),
                code: f.code.clone(),
                message: f.message.clone(),
                reproduce: "majordomus plan validate".into(),
            })
            .collect()
    }

    /// Count every attested field of this task.
    fn tally(&self) -> AttestationTally {
        let mut t = AttestationTally::default();
        let d = &self.declaration;
        for f in [
            &d.title,
            &d.objective,
            &d.why,
            &d.current_state,
            &d.desired_state,
            &d.completion,
            &d.milestone,
            &d.priority,
            &d.profile,
            &d.parallel_safe,
            &d.owner,
            &d.risk,
            &d.slug,
            &d.cancelled,
            &d.created_at,
            &d.updated_at,
            &d.started_at,
            &d.verified_at,
            &d.completed_at,
            &self.position.status,
            &self.position.milestone_status,
            &self.execution.trunk,
            &self.execution.commits_complete,
            &self.synchronisation.repository,
            &self.synchronisation.state,
            &self.synchronisation.external_id,
        ] {
            t.count(f.provenance);
        }
        for f in [
            &d.scope,
            &d.non_scope,
            &d.depends_on,
            &d.acceptance_criteria,
            &d.validation,
            &d.evidence_required,
            &d.evidence_present,
            &d.evidence_commits,
            &self.position.blocked_by,
            &self.position.dependents,
            &self.position.milestone_blocked_by,
            &self.execution.branches,
            &self.execution.merged_branches,
            &self.execution.sessions,
            &self.execution.sessions_by_branch,
        ] {
            t.count(f.provenance);
        }
        for f in [
            &self.position.wave,
            &self.position.transitive_dependents,
            &self.position.milestone_rank,
            &self.position.evidence_have,
            &self.position.evidence_need,
            &self.execution.commits,
        ] {
            t.count(f.provenance);
        }
        t
    }

    /// Every attested field of this task, so a check can walk them without knowing the
    /// shape. The invariant is the one [`AttestedText::is_consistent`] states.
    pub fn is_consistent(&self) -> bool {
        let d = &self.declaration;
        [
            &d.title,
            &d.objective,
            &d.milestone,
            &d.completed_at,
            &self.position.status,
            &self.synchronisation.state,
        ]
        .iter()
        .all(|f| f.is_consistent())
            && self.attestation.total() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devtask::FieldProvenance;
    use crate::plan::{PlanCounts, PlanMilestone, PlanProject, PlanVocabulary};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn issue(id: &str, milestone: &str, status: &str) -> PlanIssue {
        PlanIssue {
            id: id.into(),
            milestone: milestone.into(),
            status: status.into(),
            wave: 0,
            priority: "p1".into(),
            profile: "implementation".into(),
            parallel_safe: true,
            title: format!("{id} title"),
            slug: String::new(),
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            dependents: Vec::new(),
            scope: Vec::new(),
            objective: String::new(),
            evidence_have: 0,
            evidence_need: 0,
            started_at: String::new(),
            verified_at: String::new(),
            completed_at: String::new(),
        }
    }

    fn milestone(id: &str, status: &str) -> PlanMilestone {
        PlanMilestone {
            id: id.into(),
            status: status.into(),
            order: 0,
            priority: "p1".into(),
            title: format!("{id} title"),
            slug: String::new(),
            version: String::new(),
            rank: 0,
            depends_on: Vec::new(),
            blocked_by: Vec::new(),
            dependents: Vec::new(),
            claims: Vec::new(),
            counts: PlanCounts {
                total: 0,
                required: 0,
                by_status: BTreeMap::new(),
            },
            issues: Vec::new(),
            outcome: String::new(),
        }
    }

    pub(super) fn plan_of(milestones: Vec<PlanMilestone>, issues: Vec<PlanIssue>) -> Plan {
        Plan {
            project: PlanProject {
                name: "fixture".into(),
                repository: "owner/fixture".into(),
                default_branch: "master".into(),
                active_milestone: String::new(),
            },
            statuses: PlanVocabulary {
                issue: crate::plan::ISSUE_STATUSES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
                milestone: crate::plan::MILESTONE_STATUSES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect(),
            },
            milestones,
            issues,
            waves: Vec::new(),
            edges: Vec::new(),
            milestone_edges: Vec::new(),
            findings: Vec::new(),
        }
    }

    /// An id the model does not declare is answered, not refused — and every canonical
    /// field of that answer is `unknown` rather than an empty string that reads as authored.
    #[test]
    fn an_undeclared_issue_is_answered_with_nothing_claiming_to_be_authored() {
        let plan = plan_of(Vec::new(), Vec::new());
        let model = DevTaskModel {
            plan: &plan,
            record: None,
            trace: None,
            sessions: Vec::new(),
            repository: Some("owner/fixture"),
        };
        let t = DevTask::build(&model, "I9999");
        assert!(!t.declared);
        assert_eq!(t.readiness.state, TaskReadiness::Undeclared);
        assert_eq!(t.declaration.title.provenance, FieldProvenance::Unknown);
        assert!(t.declaration.title.value.is_none());
        assert_eq!(t.position.status.provenance, FieldProvenance::Unknown);
        assert_eq!(t.attestation.explicit, 1, "only the repository is authored");
        assert!(t.is_consistent());
    }

    /// A key the record does not carry is `unknown`; a key it carries is `explicit`. The
    /// distinction the flattened plan cannot make, and the one this module is for.
    #[test]
    fn an_absent_key_is_unknown_and_a_present_one_is_explicit() {
        let meta = json!({
            "id": "I0001",
            "milestone": "m1",
            "title": "Ship it",
            "depends_on": [],
        });
        let plan = plan_of(
            vec![milestone("m1", "ACTIVE")],
            vec![issue("I0001", "m1", "READY")],
        );
        let model = DevTaskModel {
            plan: &plan,
            record: Some(RecordRef {
                path: ".ai/repo/project/issues/I0001.yaml",
                metadata: &meta,
            }),
            trace: None,
            sessions: Vec::new(),
            repository: None,
        };
        let t = DevTask::build(&model, "I0001");
        assert_eq!(t.declaration.title.provenance, FieldProvenance::Explicit);
        assert_eq!(t.declaration.title.text(), "Ship it");
        assert_eq!(
            t.declaration.title.source,
            ".ai/repo/project/issues/I0001.yaml#title"
        );
        // authored as an empty list: explicit and empty, not unknown
        assert_eq!(
            t.declaration.depends_on.provenance,
            FieldProvenance::Explicit
        );
        assert!(t.declaration.depends_on.is_empty());
        // absent entirely
        assert_eq!(t.declaration.objective.provenance, FieldProvenance::Unknown);
        assert!(t.declaration.objective.reason.is_some());
    }

    /// The status is never re-derived here: it is the plan's, marked derived, pointing at
    /// the capability that owns it.
    #[test]
    fn the_status_is_the_plans_and_says_so() {
        let plan = plan_of(
            vec![milestone("m1", "ACTIVE")],
            vec![issue("I0001", "m1", "ACTIVE")],
        );
        let meta = json!({ "id": "I0001", "milestone": "m1" });
        let model = DevTaskModel {
            plan: &plan,
            record: Some(RecordRef {
                path: "p.yaml",
                metadata: &meta,
            }),
            trace: None,
            sessions: Vec::new(),
            repository: None,
        };
        let t = DevTask::build(&model, "I0001");
        assert_eq!(t.position.status.text(), "ACTIVE");
        assert_eq!(t.position.status.provenance, FieldProvenance::Derived);
        assert_eq!(t.position.status.source, "plan.model");
        assert_eq!(t.readiness.state, TaskReadiness::InProgress);
        assert_eq!(t.readiness.canonical_status, "ACTIVE");
    }

    /// Git not consulted is `unknown`, not an empty list. "Nobody looked" and "nothing
    /// realised it" are different answers and a work surface must not merge them.
    #[test]
    fn git_not_consulted_is_unknown_rather_than_empty() {
        let plan = plan_of(
            vec![milestone("m1", "ACTIVE")],
            vec![issue("I0001", "m1", "READY")],
        );
        let meta = json!({ "id": "I0001", "milestone": "m1" });
        let model = DevTaskModel {
            plan: &plan,
            record: Some(RecordRef {
                path: "p.yaml",
                metadata: &meta,
            }),
            trace: None,
            sessions: Vec::new(),
            repository: None,
        };
        let t = DevTask::build(&model, "I0001");
        assert!(!t.execution.git_consulted);
        assert_eq!(t.execution.branches.provenance, FieldProvenance::Unknown);
        assert_eq!(t.execution.commits.provenance, FieldProvenance::Unknown);
    }

    /// A session is linked by a branch name and never by a declaration, so the field is
    /// inferred and carries the rule. Several sessions of one issue is the ordinary case.
    #[test]
    fn a_declared_session_link_is_derived_and_a_branch_guess_is_inferred() {
        let plan = plan_of(
            vec![milestone("m1", "ACTIVE")],
            vec![issue("I0001", "m1", "ACTIVE")],
        );
        let meta = json!({ "id": "I0001", "milestone": "m1" });
        let model = DevTaskModel {
            plan: &plan,
            record: Some(RecordRef {
                path: "p.yaml",
                metadata: &meta,
            }),
            trace: None,
            sessions: vec![
                SessionRef {
                    session_id: "s-1".into(),
                    branch: "feature/I0001-a".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    path: "a.md".into(),
                    declares_issue: true,
                },
                SessionRef {
                    session_id: "s-2".into(),
                    branch: "feature/I0001-b".into(),
                    created_at: "2026-01-02T00:00:00Z".into(),
                    path: "b.md".into(),
                    declares_issue: false,
                },
            ],
            repository: None,
        };
        let t = DevTask::build(&model, "I0001");
        // the record that declared the issue is a canonical link
        assert_eq!(t.execution.sessions.provenance, FieldProvenance::Derived);
        assert_eq!(t.execution.sessions.values, ["s-1"]);
        // the one reached only by its branch name is not allowed to look the same
        assert_eq!(
            t.execution.sessions_by_branch.provenance,
            FieldProvenance::Inferred
        );
        assert_eq!(t.execution.sessions_by_branch.values, ["s-2"]);
        assert!(
            t.execution.sessions_by_branch.reason.is_some(),
            "an inferred field states its rule"
        );
        // and neither list stands in for the other
        assert!(t.execution.sessions.values.iter().all(|s| !t
            .execution
            .sessions_by_branch
            .values
            .contains(s)));
    }

    /// The external half is never claimed and never writable, and the reason names the one
    /// thing that can answer it.
    #[test]
    fn the_external_projection_is_unknown_and_not_writable() {
        let plan = plan_of(Vec::new(), Vec::new());
        let model = DevTaskModel {
            plan: &plan,
            record: None,
            trace: None,
            sessions: Vec::new(),
            repository: Some("owner/fixture"),
        };
        let t = DevTask::build(&model, "I0001");
        assert!(!t.synchronisation.writable);
        assert_eq!(t.synchronisation.adapter, PROJECTION_ADAPTER);
        assert_eq!(t.synchronisation.state.provenance, FieldProvenance::Unknown);
        assert!(t
            .synchronisation
            .state
            .reason
            .as_deref()
            .unwrap()
            .contains(PROJECTION_ADAPTER));
        // the repository the projection targets is authored, in project.yaml
        assert_eq!(
            t.synchronisation.repository.provenance,
            FieldProvenance::Explicit
        );
    }

    /// Completion recorded without the evidence the record requires is its own state, and
    /// each missing token is a blocker naming what to do.
    #[test]
    fn completion_without_evidence_is_completion_blocked_with_the_tokens_named() {
        let mut i = issue("I0001", "m1", "VERIFY");
        i.evidence_have = 1;
        i.evidence_need = 2;
        let plan = plan_of(vec![milestone("m1", "ACTIVE")], vec![i]);
        let meta = json!({
            "id": "I0001",
            "milestone": "m1",
            "completed_at": "2026-01-01",
            "evidence_required": ["a", "b"],
            "evidence": [{ "covers": "a" }],
        });
        let model = DevTaskModel {
            plan: &plan,
            record: Some(RecordRef {
                path: "p.yaml",
                metadata: &meta,
            }),
            trace: None,
            sessions: Vec::new(),
            repository: None,
        };
        let t = DevTask::build(&model, "I0001");
        assert_eq!(t.readiness.state, TaskReadiness::CompletionBlocked);
        assert_eq!(t.readiness.canonical_status, "VERIFY");
        let evidence: Vec<&str> = t
            .readiness
            .blockers
            .iter()
            .filter(|b| b.kind == BlockerKind::Evidence)
            .map(|b| b.subject.as_str())
            .collect();
        assert_eq!(evidence, ["b"]);
    }

    /// Held back only by the milestone gate is `waiting`, and the blocker says which
    /// milestone rather than naming a peer issue that is not the problem.
    #[test]
    fn only_the_milestone_gate_is_waiting() {
        let mut i = issue("I0001", "m1", "BLOCKED");
        i.blocked_by = vec!["milestone:m1".into()];
        let mut m = milestone("m1", "BLOCKED");
        m.blocked_by = vec!["m0".into()];
        let plan = plan_of(vec![m], vec![i]);
        let meta = json!({ "id": "I0001", "milestone": "m1" });
        let model = DevTaskModel {
            plan: &plan,
            record: Some(RecordRef {
                path: "p.yaml",
                metadata: &meta,
            }),
            trace: None,
            sessions: Vec::new(),
            repository: None,
        };
        let t = DevTask::build(&model, "I0001");
        assert_eq!(t.readiness.state, TaskReadiness::Waiting);
        assert_eq!(t.readiness.blockers.len(), 1);
        assert_eq!(t.readiness.blockers[0].kind, BlockerKind::Milestone);
        assert_eq!(t.readiness.blockers[0].subject, "m1");
    }

    /// An issue that names no milestone still answers, with the milestone-derived fields
    /// unknown and the reason saying why — the plan's own `no_milestone` failure reaches
    /// the diagnostics.
    #[test]
    fn an_issue_without_a_milestone_answers_with_the_milestone_fields_unknown() {
        let mut plan = plan_of(Vec::new(), vec![issue("I0001", "", "READY")]);
        plan.findings.push(crate::plan::PlanFinding {
            level: "FAIL".into(),
            code: "no_milestone".into(),
            subject: "I0001".into(),
            message: "names no milestone".into(),
        });
        let meta = json!({ "id": "I0001" });
        let model = DevTaskModel {
            plan: &plan,
            record: Some(RecordRef {
                path: "p.yaml",
                metadata: &meta,
            }),
            trace: None,
            sessions: Vec::new(),
            repository: None,
        };
        let t = DevTask::build(&model, "I0001");
        assert!(t.declared);
        assert_eq!(t.declaration.milestone.provenance, FieldProvenance::Unknown);
        assert_eq!(
            t.position.milestone_status.provenance,
            FieldProvenance::Unknown
        );
        assert_eq!(t.readiness.state, TaskReadiness::Ready);
        assert!(t.diagnostics.iter().any(|d| d.code == "no_milestone"));
        assert!(t.diagnostics.iter().all(|d| !d.reproduce.is_empty()));
    }
}
