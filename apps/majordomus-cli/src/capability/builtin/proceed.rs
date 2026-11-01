//! The `proceed` module: what a session proceeds with, and why (ADR 0177). Nothing here
//! writes.
//!
//! # Why one capability decides
//!
//! Every fact a session needs before it takes work has an owner already: the plan's
//! hand-out order (`plan.next`), the boards of every checkout (`peers.list`) and the live
//! claims of every linked machine (`mesh.state`). Before this capability a session composed
//! them by hand — on 2026-10-17 the sessions of this repository chose issues by grepping the
//! board, and two of them took the same identifier three times in one day. `proceed.plan`
//! asks each owner once and answers one decision: the issue to take, every READY issue it
//! passed over with the constraint that issue failed and the hold that failed it, and every
//! term the order was decided by. It ranks nothing of its own: the order is the plan's, and
//! a claim removes an issue from it without reordering what remains.
//!
//! # The three constraints
//!
//! An issue is *claimed* when a live mesh claim names it as its issue or any hold claims the
//! issue's own record: a structured statement that it is somebody else's work. It is *named*
//! when a hold's words name its id as a word of their own — how most work is still taken on
//! the board ("I2972 TAKEN"), and also how a hand-off note or "stacked on I2783" reads — so
//! it refuses, because two workers on one issue cost more than one issue left for later, but
//! under its own word, so that a reader can tell a claim from a mention and check the words
//! quoted beside it. Its scope is *held* when a live exclusive mesh claim meets it; taking it
//! would be refused at the first `mesh.claim`. An advisory hold that meets the scope refuses
//! nothing and is reported beside the issue as `shared_with`: a cost of coordination, not a
//! reason to walk away.
//!
//! The caller's own holds are never held against it: the mesh claims of its session on this
//! runtime (`<stream>/<session>`, the key `mesh.claim` writes and admits; the same name on
//! another runtime is another session, and holds) and its board announcements (the peer whose
//! board session `board-<peer>` is the session, or the calling MCP peer).
//!
//! # A decision over a source it could not read
//!
//! A decision that could not read the board, or the mesh of a repository that declares one,
//! still names the issue it would select, and says it is `provisional`: the claims it did not
//! read may hold that issue. Its `next` is then empty and the command line exits 11, so that
//! neither a person nor an acting command takes work on the strength of a partial read. A
//! repository that declares no enabled mesh has no mesh to miss, and that source is
//! `undeclared`, not unread.
//!
//! # Who answers
//!
//! As for `brief.session`: the board and the mesh live in the memory of the server that holds
//! this checkout's lease, so the command line asks that server for the decision (`local:
//! true`, so it never asks back) and composes it itself only when none answers — saying so in
//! `sources`, because a decision taken over a mesh it could not read is not the same decision.
//!
//! # One step, as one work order
//!
//! `proceed.step` (I4005, [`step`]) turns the decision into what a loop acts on: the one
//! capability to run next (`devtask.implement` for the selected issue, under its own
//! idempotency key), the issues that may run beside it, what to announce and when to step
//! again; and, for an intent, `done` once its realisation is met. It decides nothing this
//! module does not: the order is `proceed.plan`'s, read again, and `/proceed` is its skill.

mod step;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CachePolicy, CliExposure, Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::lease;
use crate::plan::{Plan, PlanIssue};
use crate::{capability, module};

use super::peers::PeerList;
use super::server::{self, ServerStanding};
use super::{get, mcp};

/// How long the command line waits for the leaseholder's decision: the board's gather probes
/// every checkout's server, as for the brief.
const FORWARD_TIMEOUT: Duration = Duration::from_secs(20);
/// The statuses an issue leaves the plan with; it is neither work nor waiting.
const FINISHED: [&str; 2] = ["DONE", "CANCELLED"];

// ---------------------------------------------------------------- input

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What to ask of `proceed.plan`.
pub struct ProceedPlanInput {
    /// The milestone whose READY issues come first; the active milestone when absent. The rest of the plan follows it, as for `plan.next`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The mesh session the decision is for, whose own claims are not held against it. Absent, the calling MCP session's board session (`board-<peer>`); over the command line, none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    /// Compose in the process that answers and never ask the checkout's leaseholder: what a forwarded request says, so that it is never forwarded again.
    #[serde(default)]
    pub local: bool,
}

impl BenchmarkCases for ProceedPlanInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("default", Self::default()),
            NamedCase::new(
                "milestone_and_session",
                Self {
                    milestone: Some("a-session-proceeds".into()),
                    session: Some("board-p1".into()),
                    local: true,
                },
            ),
        ]
    }
}

// ---------------------------------------------------------------- output

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What a session proceeds with: the selected issue, every READY issue in the order work is handed out with what refused it, and the sources the decision was read from.
pub struct ProceedDecision {
    /// The first READY issue nothing refuses; absent when there is none, and `reason` says why.
    pub selected: Option<ProceedCandidate>,
    /// One sentence a reader can check against `candidates` and `sources`.
    pub reason: String,
    /// Whether a source the decision needs could not be read (see `sources`): the selection is then what this reading would choose, and another worker's unread claim may hold it.
    pub provisional: bool,
    /// What to do with the selection, as the commands that do it; empty when nothing is selected or the decision is provisional.
    pub next: Vec<String>,
    /// The milestone whose issues were looked at first.
    pub milestone: String,
    /// Every READY issue in hand-out order (the plan's: asked milestone first, then wave, priority and id), each with the constraints it failed.
    pub candidates: Vec<ProceedCandidate>,
    /// How many unfinished issues are not READY, by their derived status: the work that waits on a dependency, a gate or a worker.
    pub waiting: BTreeMap<String, usize>,
    /// What the decision was read from, and whether each source could be read.
    pub sources: Vec<ProceedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One READY issue as the decision saw it.
pub struct ProceedCandidate {
    /// The issue's id.
    pub issue: String,
    /// Its title.
    pub title: String,
    /// Its milestone.
    pub milestone: String,
    /// The terms its place in the order was decided by.
    pub terms: ProceedTerms,
    /// The constraints it failed, each with the hold that failed it; empty for an eligible issue.
    pub refused: Vec<ProceedRefusal>,
    /// Advisory holds that meet its scope: refused by none of them, shared with each.
    pub shared_with: Vec<String>,
    /// The paths it touches.
    pub scope: Vec<String>,
    /// What finishes it: the objective and its evidence tally.
    pub finishes_with: ProceedEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// The terms of a candidate's place in the hand-out order, most significant first, and what taking it unblocks.
pub struct ProceedTerms {
    /// Whether it belongs to the milestone looked at first.
    pub in_milestone: bool,
    /// Its wave: one past the longest dependency path to it.
    pub wave: u32,
    /// Its priority, `p0` first.
    pub priority: String,
    /// The unfinished issues that depend on it, directly or through others, each counted once. Shown, not ranked by: the order is the plan's.
    pub unblocks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One constraint a candidate failed.
pub struct ProceedRefusal {
    /// Which constraint.
    pub constraint: ProceedConstraint,
    /// The hold that failed it: a mesh claim's key, or `board:<checkout>:<peer>[:<claim>]`.
    pub hold: String,
    /// Who holds it, in words.
    pub holder: String,
    /// What about the hold fails the constraint.
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// A constraint an issue must meet before a session may take it.
pub enum ProceedConstraint {
    /// Another worker holds the issue itself: a live mesh claim names it as its issue, or a hold claims its record.
    Claimed,
    /// A hold's words name the issue's id as a word of their own: how the board takes work, and how a mention reads.
    Named,
    /// A live exclusive mesh claim of another session meets the issue's scope, so claiming it would be refused.
    ScopeHeld,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// What finishes an issue.
pub struct ProceedEvidence {
    /// The issue's objective.
    pub objective: String,
    /// The evidence it has.
    pub have: u32,
    /// The evidence it needs.
    pub need: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
/// One source of holds and whether it could be read.
pub struct ProceedSource {
    /// `board` (every checkout's peers) or `mesh` (every linked machine's live claims).
    pub source: String,
    /// Whether it was read whole, in part, or not at all.
    pub state: ProceedSourceState,
    /// How many holds of others it contributed.
    pub holds: usize,
    /// What was read, or why not.
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// How much of a source a decision could read.
pub enum ProceedSourceState {
    /// Read whole.
    Read,
    /// Read, with parts that could not be (a board that did not answer).
    Partial,
    /// Not read: the decision holds nothing of it.
    Unread,
    /// Nothing to read: the repository declares no enabled mesh.
    Undeclared,
}

// ---------------------------------------------------------------- the decision

/// Something another worker holds: a live mesh claim or a board announcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hold {
    pub(crate) key: String,
    pub(crate) holder: String,
    pub(crate) exclusive: bool,
    pub(crate) issue: Option<String>,
    pub(crate) words: String,
    pub(crate) scope: Vec<String>,
}

/// The decision over a plan and the holds of others. Pure: everything it reads is passed in.
pub(crate) fn decide(
    plan: &Plan,
    milestone: Option<&str>,
    session: Option<&str>,
    holds: &[Hold],
    sources: Vec<ProceedSource>,
) -> ProceedDecision {
    let asked = milestone
        .unwrap_or(plan.project.active_milestone.as_str())
        .to_string();
    let candidates: Vec<ProceedCandidate> = plan
        .ready_in_order(milestone)
        .into_iter()
        .map(|issue| candidate(plan, &asked, issue, holds))
        .collect();
    let mut waiting = BTreeMap::new();
    for i in &plan.issues {
        if i.status != "READY" && !FINISHED.contains(&i.status.as_str()) {
            *waiting.entry(i.status.clone()).or_insert(0) += 1;
        }
    }
    let selected = candidates.iter().find(|c| c.refused.is_empty()).cloned();
    let passed = candidates
        .iter()
        .take_while(|c| !c.refused.is_empty())
        .count();
    let unread: Vec<&str> = sources
        .iter()
        .filter(|s| {
            matches!(
                s.state,
                ProceedSourceState::Partial | ProceedSourceState::Unread
            )
        })
        .map(|s| s.source.as_str())
        .collect();
    let provisional = !unread.is_empty();
    let mut reason = match &selected {
        Some(c) => format!(
            "{} is the first READY issue that nothing refuses{} (wave {}, {}){}; it unblocks {} issue(s)",
            c.issue,
            if c.terms.in_milestone {
                format!(" in {asked}")
            } else {
                format!(", from outside {asked}, which has none free")
            },
            c.terms.wave,
            c.terms.priority,
            if passed == 0 {
                String::new()
            } else {
                format!(", after {passed} held by others")
            },
            c.terms.unblocks.len()
        ),
        None if candidates.is_empty() && waiting.is_empty() => {
            "the plan declares no unfinished issue to hand out".to_string()
        }
        None if candidates.is_empty() => {
            "no issue is READY; every unfinished one waits on a dependency, its milestone's gate or a worker".to_string()
        }
        None => format!(
            "every one of the {} READY issue(s) is held by another worker",
            candidates.len()
        ),
    };
    if provisional {
        reason.push_str(&format!(
            "; provisional, decided without all of: {} (see sources)",
            unread.join(", ")
        ));
    }
    let next = match &selected {
        Some(c) if !provisional => steps(c, session),
        _ => Vec::new(),
    };
    ProceedDecision {
        selected,
        reason,
        provisional,
        next,
        milestone: asked,
        candidates,
        waiting,
        sources,
    }
}

fn candidate(plan: &Plan, asked: &str, issue: &PlanIssue, holds: &[Hold]) -> ProceedCandidate {
    let record = format!(".ai/repo/project/issues/{}.yaml", issue.id);
    let mut refused = Vec::new();
    let mut shared_with = Vec::new();
    for h in holds {
        let held = if h.issue.as_deref() == Some(issue.id.as_str()) {
            Some((
                ProceedConstraint::Claimed,
                format!("claims {} as its issue", issue.id),
            ))
        } else if h.scope.contains(&record) {
            Some((
                ProceedConstraint::Claimed,
                format!("claims the record {record}"),
            ))
        } else if names(&h.words, &issue.id) {
            Some((
                ProceedConstraint::Named,
                format!("names {}: {}", issue.id, clip(&h.words, 120)),
            ))
        } else {
            None
        };
        if let Some((constraint, detail)) = held {
            refused.push(ProceedRefusal {
                constraint,
                hold: h.key.clone(),
                holder: h.holder.clone(),
                detail,
            });
        } else if crate::mesh::state::scopes_meet(&h.scope, &issue.scope) {
            if h.exclusive {
                refused.push(ProceedRefusal {
                    constraint: ProceedConstraint::ScopeHeld,
                    hold: h.key.clone(),
                    holder: h.holder.clone(),
                    detail: format!("holds {} exclusively", h.scope.join(", ")),
                });
            } else {
                shared_with.push(h.key.clone());
            }
        }
    }
    ProceedCandidate {
        issue: issue.id.clone(),
        title: issue.title.clone(),
        milestone: issue.milestone.clone(),
        terms: ProceedTerms {
            in_milestone: asked.is_empty() || issue.milestone == asked,
            wave: issue.wave,
            priority: issue.priority.clone(),
            unblocks: plan.unblocks(&issue.id),
        },
        refused,
        shared_with,
        scope: issue.scope.clone(),
        finishes_with: ProceedEvidence {
            objective: issue.objective.clone(),
            have: issue.evidence_have,
            need: issue.evidence_need,
        },
    }
}

/// Whether `words` name `id` as a word of their own: `I2972,` does, `I29720` does not.
fn names(words: &str, id: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    words.match_indices(id).any(|(at, _)| {
        !word(words[..at].chars().next_back()) && !word(words[at + id.len()..].chars().next())
    })
}

/// The commands that take the selection: claim it, give it a worktree, start its task. The
/// branch is the worker's to name (`project.worktree-topology`); everything else is filled in,
/// each value one shell word that carries it literally: a title or a path is data, and a
/// command copied from here never substitutes, splits or expands it.
fn steps(c: &ProceedCandidate, session: Option<&str>) -> Vec<String> {
    // `mesh claim` takes the paths as words, `start --scope` as one comma-separated value
    let words: Vec<String> = c.scope.iter().map(|s| word(s)).collect();
    let listed = word(&c.scope.join(","));
    let what = word(&format!("{}: {}", c.issue, c.title));
    let issue = word(&c.issue);
    let session = session.map_or_else(|| "<your mesh session>".to_string(), word);
    vec![
        format!(
            "majordomus mesh claim {} --session {session} --issue {issue} --intent {what}",
            words.join(" ")
        ),
        "majordomus worktree create <branch>".to_string(),
        format!("majordomus start {what} --issue {issue} --scope {listed}"),
    ]
}

/// One shell word that means `s`: bare when no character of it is special to a shell, single
/// quoted otherwise.
fn word(s: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_-./:,=@+%".contains(c);
    if !s.is_empty() && s.chars().all(plain) {
        s.to_string()
    } else {
        crate::worktree::uncommitted::shell_quote(s)
    }
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n).collect();
    out.push('…');
    out
}

// ---------------------------------------------------------------- the handler

fn proceed_plan(
    ctx: &Context,
    input: ProceedPlanInput,
) -> Result<ProceedDecision, CapabilityError> {
    let root = PathBuf::from(&ctx.index.repository.root);
    let root = root.canonicalize().unwrap_or(root);
    let (standing, _, read) = server::standing_at(&root, &server::local_half(&root));
    let doc = read.document();
    let own = doc.is_some_and(lease::is_own);
    let url = doc.and_then(|d| d.url.clone());
    let answering = matches!(standing, ServerStanding::Ready | ServerStanding::Outdated);
    let mut note = None;
    // the board of this checkout is the leaseholder's: when it answers and cannot be asked,
    // this process's own (empty) board says nothing about who is there
    let mut unasked = None;
    if !own && !input.local && ctx.caller.is_none() {
        match (answering, url.as_deref()) {
            (true, Some(url)) => match forward(ctx, url, &input) {
                Ok(d) => return Ok(d),
                Err(why) => {
                    unasked = Some(why.clone());
                    note = Some(why)
                }
            },
            _ => {
                note = Some(format!(
                    "no server answers for this checkout (its server is {}); read by this process, which holds no mesh",
                    standing.as_str()
                ))
            }
        }
    }
    let session = input
        .session
        .clone()
        .or_else(|| ctx.caller.as_ref().map(|p| format!("board-{}", p.as_str())));
    let mut holds = Vec::new();
    let mut board = board_holds(ctx, &root, session.as_deref(), &mut holds);
    if let Some(why) = unasked {
        unasked_board(&mut board, &why);
    }
    let mut mesh = mesh_holds(ctx, session.as_deref(), &mut holds);
    if let Some(n) = note {
        mesh.summary = format!("{}; {n}", mesh.summary);
    }
    Ok(decide(
        &Plan::build(&ctx.index),
        input.milestone.as_deref(),
        session.as_deref(),
        &holds,
        vec![plan_source(ctx), board, mesh],
    ))
}

/// Where the repository declares its mesh: the source class `mesh-declaration`'s pathspec,
/// the directory `guard::mesh_declared_enabled` and the shell's `mj_mesh_declared_enabled` read.
const MESH_DECLARATIONS: &str = ".ai/repo/mesh/";

/// The error diagnostics of the index on the files under `dir`, one line each: the records
/// that are there and did not load.
fn refused_records(ctx: &Context, dir: &str) -> Vec<String> {
    ctx.index
        .diagnostics
        .iter()
        .filter(|d| d.severity == crate::Severity::Error)
        .filter_map(|d| {
            let path = d.path.as_deref()?;
            path.starts_with(dir)
                .then(|| format!("{path}: {} ({})", d.message, d.code))
        })
        .collect()
}

/// The plan the decision orders: read whole, or partial when a record of it did not load,
/// because an issue dropped from the index is an issue the order silently skips.
fn plan_source(ctx: &Context) -> ProceedSource {
    let refused = refused_records(ctx, ".ai/repo/project/");
    ProceedSource {
        source: "plan".into(),
        state: if refused.is_empty() {
            ProceedSourceState::Read
        } else {
            ProceedSourceState::Partial
        },
        holds: 0,
        summary: if refused.is_empty() {
            "every record of the plan loaded".into()
        } else {
            format!(
                "{} record(s) of the plan did not load: {}",
                refused.len(),
                refused.join("; ")
            )
        },
    }
}

/// A board read while this checkout's server answered and could not be asked: what the
/// server's board holds was not read, so the source is at most partial.
fn unasked_board(board: &mut ProceedSource, why: &str) {
    if board.state == ProceedSourceState::Read {
        board.state = ProceedSourceState::Partial;
    }
    board.summary = format!(
        "{}; this checkout's own board is its server's: {why}",
        board.summary
    );
}

/// The announcements of every attached peer but the caller's own, on every checkout's board.
fn board_holds(
    ctx: &Context,
    root: &std::path::Path,
    session: Option<&str>,
    holds: &mut Vec<Hold>,
) -> ProceedSource {
    let list: Result<PeerList, String> = ctx
        .execute("peers.list", json!({ "checkouts": "repository" }))
        .map_err(|e| e.to_string())
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
    let list = match list {
        Ok(l) => l,
        Err(e) => {
            return ProceedSource {
                source: "board".into(),
                state: ProceedSourceState::Unread,
                holds: 0,
                summary: format!("peers.list did not answer ({e})"),
            }
        }
    };
    let before = holds.len();
    for p in list.peers.iter().filter(|p| p.attached) {
        let here = p
            .checkout
            .as_ref()
            .is_some_and(|c| c.worktree.canonicalize().unwrap_or(c.worktree.clone()) == root);
        let own_session = session == Some(format!("board-{}", p.id.as_str()).as_str());
        if here && (ctx.caller.as_ref() == Some(&p.id) || own_session) {
            continue;
        }
        let worktree = p
            .checkout
            .as_ref()
            .map(|c| c.worktree.display().to_string())
            .unwrap_or_default();
        let branch = p
            .checkout
            .as_ref()
            .and_then(|c| c.branch.clone())
            .unwrap_or_else(|| "?".into());
        for a in &p.claims {
            let mut key = format!("board:{worktree}:{}", p.id.as_str());
            if let Some(n) = &a.name {
                key.push(':');
                key.push_str(n);
            }
            holds.push(Hold {
                key,
                holder: format!("{} {} @{branch}", p.id.as_str(), p.client.name),
                exclusive: false,
                issue: None,
                words: a.intent.clone(),
                scope: a.scope.clone(),
            });
        }
    }
    let unread = list.boards.iter().filter(|b| b.reason.is_some()).count();
    ProceedSource {
        source: "board".into(),
        state: if unread == 0 {
            ProceedSourceState::Read
        } else {
            ProceedSourceState::Partial
        },
        holds: holds.len() - before,
        summary: format!("{} board(s), {} of them unread", list.boards.len(), unread),
    }
}

/// The live claims of every session but `session`, on every runtime this one has heard.
fn mesh_holds(ctx: &Context, session: Option<&str>, holds: &mut Vec<Hold>) -> ProceedSource {
    let status = ctx.mesh.cooperation_status();
    let declared = match super::mesh::resolved_declaration(ctx).declaration {
        Some(Ok(c)) => c.enabled,
        Some(Err(e)) => {
            return ProceedSource {
                source: "mesh".into(),
                state: ProceedSourceState::Unread,
                holds: 0,
                summary: format!("the mesh declaration could not be read ({e})"),
            }
        }
        // a declaration the index refused (a schema violation) never reaches the resolver,
        // so its absence there is not the repository's word that it declares no mesh
        None => match refused_records(ctx, MESH_DECLARATIONS).first() {
            Some(why) => {
                return ProceedSource {
                    source: "mesh".into(),
                    state: ProceedSourceState::Unread,
                    holds: 0,
                    summary: format!("the mesh declaration could not be read ({why})"),
                }
            }
            None => false,
        },
    };
    if !declared {
        return ProceedSource {
            source: "mesh".into(),
            state: ProceedSourceState::Undeclared,
            holds: 0,
            summary: "the repository declares no enabled mesh: no claim to read".into(),
        };
    }
    let Some(cooperation) = ctx.mesh.cooperation() else {
        return ProceedSource {
            source: "mesh".into(),
            state: ProceedSourceState::Unread,
            holds: 0,
            summary: format!(
                "not active here: {}",
                status
                    .reason
                    .as_deref()
                    .unwrap_or("cooperation is not running in this process")
            ),
        };
    };
    let before = holds.len();
    // the caller is a session of this runtime, named as mesh.claim names it: its own stream,
    // then its name. The same name on another runtime is another session, and holds.
    let own = session.map(|s| format!("{}/{s}", cooperation.journal().own_stream()));
    for c in cooperation
        .state()
        .claims
        .iter()
        .filter(|c| c.state.is_live())
    {
        if own.as_deref() == Some(c.session.as_str()) {
            continue;
        }
        holds.push(Hold {
            key: c.key.clone(),
            holder: format!("{} on {}", c.session, c.runtime),
            exclusive: matches!(c.mode, crate::mesh::journal::ClaimMode::Exclusive),
            issue: c.issue.clone(),
            words: c.intent.clone().unwrap_or_default(),
            scope: c.scope.clone(),
        });
    }
    ProceedSource {
        source: "mesh".into(),
        state: ProceedSourceState::Read,
        holds: holds.len() - before,
        summary: format!("{} live claim(s) of others", holds.len() - before),
    }
}

/// Ask the leaseholder over the route the registry declares for this capability.
fn forward(ctx: &Context, url: &str, input: &ProceedPlanInput) -> Result<ProceedDecision, String> {
    let path = ctx
        .registry
        .get("proceed.plan")
        .and_then(|c| c.exposure.http.as_ref())
        .map(|h| h.path.clone())
        .ok_or_else(|| "proceed.plan declares no HTTP route to ask over".to_string())?;
    let mut query = format!("{path}?local=true");
    for (k, v) in [("milestone", &input.milestone), ("session", &input.session)] {
        if let Some(v) = v {
            query.push_str(&format!("&{k}={}", crate::http::router::percent_encode(v)));
        }
    }
    let reply = crate::mcp::bridge::request(url, "GET", &query, &[], None, FORWARD_TIMEOUT)
        .map_err(|e| format!("the leaseholder at {url} did not answer ({e})"))?;
    if reply.status != 200 {
        return Err(format!(
            "the leaseholder at {url} answered {} (a server older than this executable has no proceed.plan)",
            reply.status
        ));
    }
    serde_json::from_str(&reply.body).map_err(|e| {
        format!("the leaseholder at {url} answered a decision this executable cannot read ({e})")
    })
}

// ---------------------------------------------------------------- module

pub fn module() -> ModuleDescriptor {
    module! {
        id: "proceed",
        title: "What a session proceeds with",
        description: "The decision a session starts from (ADR 0177): the issue to take, every READY issue it passed over with the constraint that issue failed and the hold that failed it, and the terms of the order, composed from the plan, every checkout's board and every linked machine's live claims. Writes nothing.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "proceed.plan",
                title: "What to proceed with, and why",
                description: "The READY issues in the plan's hand-out order (the asked milestone first, then wave, priority and id, as `plan.next`), each refused when another worker holds it: a live mesh claim names it as its issue or a hold claims its record (`claimed`), a board or mesh announcement names its id as a word of its own (`named`, quoted beside it), or a live exclusive mesh claim of another session meets its scope (`scope_held`); advisory holds over its scope are listed as `shared_with` and refuse nothing. The first issue nothing refuses is `selected`, with what it unblocks, what finishes it and the commands that take it; `reason` says why in one sentence, and `sources` whether the board and the mesh could be read. A decision that could not read the board, or a declared mesh, is `provisional` and gives no commands. The caller's own session (its mesh claims on this runtime, and its board announcements) is never held against it; a session of the same name on another runtime is another worker. Composed by the server holding this checkout's lease, which holds the board and the mesh; the command line asks it. Writes nothing.",
                input: ProceedPlanInput,
                output: ProceedDecision,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_proceed_plan"),
                    http: get("/api/v1/proceed/plan"),
                    cli: Some(CliExposure { path: vec!["proceed".into(), "plan".into()] }),
                },
                tags: ["proceed", "plan", "coordination", "session"],
                // the board and the mesh are live memory
                cache: CachePolicy::Disabled,
                handler: proceed_plan,
            },
            capability! {
                id: "proceed.step",
                title: "Proceed: the next step, as one work order",
                description: "One step of a session that proceeds, as a work order a loop acts on: `do`, the capability to run next and its input — `devtask.implement` for the issue `proceed.plan` selects, under the idempotency key `devtask.implement` defaults to, so that a repeated order is one execution; `fan_out`, the other free READY issues declared parallel safe that share no path with it, at most four; `report`, what to announce on the board, as `peers.announce` takes it; and `next_due_at`, the next five-minute boundary after `at`, when to step again. Given an `intent`, only the issues serving its unmet criteria (`intent_realization.work`) are ordered, and an intent whose every criterion has current evidence, or that is satisfied, cancelled or superseded, answers `done` with nothing to run and nothing scheduled; without one, the whole plan is stepped and is done when no unfinished issue is left. Each item names the model `models.route` selects for a worker and the effort of the issue's profile. A provisional decision orders nothing. The same state answers the same order: `key` is the digest of everything but the moment, so a step repeated over unchanged state carries the same key at any later time. Writes nothing.",
                input: step::ProceedStepInput,
                output: step::WorkOrder,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_proceed_step"),
                    http: get("/api/v1/proceed/step"),
                    cli: Some(CliExposure { path: vec!["proceed".into(), "step".into()] }),
                },
                tags: ["proceed", "plan", "coordination", "session", "loop"],
                // the board, the mesh and the clock are live
                cache: CachePolicy::Disabled,
                handler: step::proceed_step,
            }
            .skill("proceed", "intent")
            .follow("Then carry the order out, once: when `done` is true, report the reason and stop. Otherwise announce `report` on the board with `peers.announce`; run `do` with its `input` exactly as given (an order repeated over the same state names the same idempotency key, so it is one execution); start each `fan_out` item in a worker of its own, on the model and effort the item names; and step again at `next_due_at`, with the same intent, until the order is done."),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository whose plan declares one milestone and the issues given as
    /// `(id, depends_on, scope)`.
    fn planned(issues: &[(&str, &[&str], &[&str])]) -> crate::synthetic::SyntheticRepository {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).unwrap();
        for (class, pathspec) in [
            ("project", ".ai/repo/project/project.yaml"),
            ("milestone", ".ai/repo/project/milestones/*.yaml"),
            ("issue", ".ai/repo/project/issues/*.yaml"),
            ("mesh-declaration", ".ai/repo/mesh/*.yaml"),
        ] {
            text.push_str(&format!(
                "  - id: {class}\n    kind: {class}\n    discovery: vcs\n    \
                 pathspec: ':(glob){pathspec}'\n    required: false\n"
            ));
        }
        std::fs::write(&sources, text).unwrap();
        let project = repo.root().join(".ai/repo/project");
        for dir in ["milestones", "issues"] {
            std::fs::create_dir_all(project.join(dir)).unwrap();
        }
        std::fs::write(
            project.join("project.yaml"),
            "schema_version: 1\nname: P\nrepository: o/r\ndefault_branch: master\n",
        )
        .unwrap();
        std::fs::write(
            project.join("milestones/M000.yaml"),
            "id: M000\ntitle: m\nslug: m\norder: 0\npriority: p1\nproblem: p\noutcome: o\n\
             acceptance_criteria:\n  - it works\nvalidation:\n  - \"true\"\n\
             evidence_required:\n  - proof\n",
        )
        .unwrap();
        for (id, deps, scope) in issues {
            let mut y = format!(
                "id: {id}\nmilestone: M000\ntitle: t {id}\nslug: t\npriority: p1\n\
                 profile: implementation\nobjective: o\nscope:\n"
            );
            for s in *scope {
                y.push_str(&format!("  - {s}\n"));
            }
            if !deps.is_empty() {
                y.push_str("depends_on:\n");
                for d in *deps {
                    y.push_str(&format!("  - {d}\n"));
                }
            }
            y.push_str(
                "acceptance_criteria:\n  - it works\nvalidation:\n  - \"true\"\n\
                 evidence_required:\n  - proof\n",
            );
            std::fs::write(project.join(format!("issues/{id}.yaml")), y).unwrap();
        }
        repo
    }

    fn hold(key: &str, exclusive: bool, issue: Option<&str>, words: &str, scope: &[&str]) -> Hold {
        Hold {
            key: key.into(),
            holder: format!("holder of {key}"),
            exclusive,
            issue: issue.map(Into::into),
            words: words.into(),
            scope: scope.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn read(source: &str) -> ProceedSource {
        ProceedSource {
            source: source.into(),
            state: ProceedSourceState::Read,
            holds: 0,
            summary: String::new(),
        }
    }

    fn plan_of(repo: &crate::synthetic::SyntheticRepository) -> Plan {
        Plan::build(&repo.context().unwrap().index)
    }

    #[test]
    fn with_nothing_held_the_selection_is_plan_next_and_says_what_it_unblocks() {
        let repo = planned(&[
            ("I0001", &[], &["src/a"]),
            ("I0002", &["I0001"], &["src/b"]),
            ("I0003", &["I0002"], &["src/c"]),
            ("I0004", &[], &["src/d"]),
        ]);
        let plan = plan_of(&repo);
        let d = decide(
            &plan,
            None,
            Some("s1"),
            &[],
            vec![read("board"), read("mesh")],
        );
        let picked = d.selected.as_ref().expect("an issue is selected");
        assert_eq!(
            Some(picked.issue.as_str()),
            plan.next_ready(None).map(|i| i.id.as_str())
        );
        assert_eq!(picked.issue, "I0001");
        assert_eq!(
            picked.terms.unblocks,
            ["I0002", "I0003"],
            "transitively, each once"
        );
        assert!(picked.terms.in_milestone);
        let order: Vec<&str> = d.candidates.iter().map(|c| c.issue.as_str()).collect();
        assert_eq!(
            order,
            ["I0001", "I0004"],
            "only READY issues are candidates"
        );
        assert_eq!(d.waiting.get("BLOCKED"), Some(&2));
        assert!(
            d.reason
                .starts_with("I0001 is the first READY issue that nothing refuses in M000"),
            "{}",
            d.reason
        );
        assert!(!d.provisional);
        assert_eq!(d.next.len(), 3);
        assert!(
            d.next[0].starts_with("majordomus mesh claim src/a --session s1 --issue I0001"),
            "{:?}",
            d.next
        );
        assert!(
            d.next[2].contains("--issue I0001 --scope src/a"),
            "{:?}",
            d.next
        );
    }

    #[test]
    fn a_held_issue_is_refused_with_the_hold_and_the_next_one_is_taken() {
        let repo = planned(&[
            ("I0001", &[], &["src/a"]),
            ("I0002", &[], &["src/b"]),
            ("I0003", &[], &["src/c"]),
            ("I0004", &[], &["src/d"]),
            ("I0005", &[], &["src/e"]),
        ]);
        let plan = plan_of(&repo);
        let holds = [
            hold("m/c1", false, Some("I0001"), "", &[]),
            hold("board:x:p2", false, None, "taking I0002, then I00030", &[]),
            hold(
                "m/c3",
                false,
                None,
                "",
                &[".ai/repo/project/issues/I0003.yaml"],
            ),
            hold("m/c4", true, None, "", &["src"]),
            hold("m/c5", false, None, "", &["src/e"]),
        ];
        let d = decide(&plan, None, None, &holds, vec![read("board"), read("mesh")]);
        let by: Vec<(&str, Vec<(ProceedConstraint, &str)>)> = d
            .candidates
            .iter()
            .map(|c| {
                (
                    c.issue.as_str(),
                    c.refused
                        .iter()
                        .map(|r| (r.constraint, r.hold.as_str()))
                        .collect(),
                )
            })
            .collect();
        use ProceedConstraint::{Claimed, Named, ScopeHeld};
        assert_eq!(
            by,
            [
                ("I0001", vec![(Claimed, "m/c1"), (ScopeHeld, "m/c4")]),
                ("I0002", vec![(Named, "board:x:p2"), (ScopeHeld, "m/c4")]),
                ("I0003", vec![(Claimed, "m/c3"), (ScopeHeld, "m/c4")]),
                ("I0004", vec![(ScopeHeld, "m/c4")]),
                ("I0005", vec![(ScopeHeld, "m/c4")]),
            ]
        );
        assert!(d.selected.is_none());
        assert_eq!(
            d.reason,
            "every one of the 5 READY issue(s) is held by another worker"
        );
        assert!(d.next.is_empty());

        // without the exclusive claim over src, the first free issue is taken, and an
        // advisory hold over its scope is shared, not refused
        let d = decide(
            &plan,
            None,
            None,
            &holds[..3],
            vec![read("board"), read("mesh")],
        );
        let picked = d.selected.expect("one is free");
        assert_eq!(picked.issue, "I0004");
        assert!(d.reason.contains("after 3 held by others"), "{}", d.reason);
        let d = decide(
            &plan,
            None,
            None,
            &holds[4..],
            vec![read("board"), read("mesh")],
        );
        let e = d.candidates.iter().find(|c| c.issue == "I0005").unwrap();
        assert!(e.refused.is_empty());
        assert_eq!(e.shared_with, ["m/c5"]);
    }

    #[test]
    fn an_unread_source_is_said_in_the_reason_and_an_empty_plan_is_not_a_waiting_one() {
        let repo = planned(&[("I0001", &[], &["src/a"])]);
        let unread = ProceedSource {
            source: "mesh".into(),
            state: ProceedSourceState::Unread,
            holds: 0,
            summary: "not active here".into(),
        };
        let d = decide(
            &plan_of(&repo),
            None,
            None,
            &[],
            vec![read("board"), unread],
        );
        assert!(
            d.reason
                .ends_with("; provisional, decided without all of: mesh (see sources)"),
            "{}",
            d.reason
        );
        assert!(
            d.provisional && d.next.is_empty(),
            "a partial read takes nothing"
        );
        assert_eq!(
            d.selected.map(|c| c.issue).as_deref(),
            Some("I0001"),
            "but names its choice"
        );
        let undeclared = ProceedSource {
            source: "mesh".into(),
            state: ProceedSourceState::Undeclared,
            holds: 0,
            summary: String::new(),
        };
        let d = decide(
            &plan_of(&repo),
            None,
            None,
            &[],
            vec![read("board"), undeclared],
        );
        assert!(
            !d.provisional && d.next.len() == 3,
            "no mesh declared is no mesh missed"
        );

        let empty = crate::synthetic::SyntheticRepository::small().unwrap();
        let d = decide(&plan_of(&empty), None, None, &[], vec![]);
        assert_eq!(
            d.reason,
            "the plan declares no unfinished issue to hand out"
        );

        // the only issue is ACTIVE: unfinished, and not READY
        let waiting = planned(&[("I0001", &[], &["src/a"])]);
        let at = "2026-09-01T00:00:00Z";
        let record = waiting.root().join(".ai/repo/project/issues/I0001.yaml");
        let mut y = std::fs::read_to_string(&record).unwrap();
        y.push_str(&format!(
            "started_at: {at}\nunsealed_stamps:\n  - started_at {at}\n"
        ));
        std::fs::write(&record, y).unwrap();
        let d = decide(&plan_of(&waiting), None, None, &[], vec![]);
        assert_eq!(d.waiting.get("ACTIVE"), Some(&1));
        assert!(d.selected.is_none());
        assert!(d.reason.starts_with("no issue is READY"), "{}", d.reason);
    }

    #[test]
    fn an_id_is_named_only_as_a_word_of_its_own() {
        assert!(names("I2972, then I2973", "I2972"));
        assert!(names("(I2972)", "I2972"));
        assert!(names("I2972", "I2972"));
        assert!(!names("I29720", "I2972"));
        assert!(!names("XI2972", "I2972"));
        assert!(!names("", "I2972"));
        let two = ProceedCandidate {
            issue: "I1".into(),
            title: "t".into(),
            milestone: "M".into(),
            terms: ProceedTerms {
                in_milestone: true,
                wave: 0,
                priority: "p1".into(),
                unblocks: Vec::new(),
            },
            refused: Vec::new(),
            shared_with: Vec::new(),
            scope: vec!["src/a".into(), "docs/b.md".into()],
            finishes_with: ProceedEvidence {
                objective: String::new(),
                have: 0,
                need: 1,
            },
        };
        let s = steps(&two, None);
        assert!(
            s[0].starts_with("majordomus mesh claim src/a docs/b.md --session <your mesh session>"),
            "{}",
            s[0]
        );
        assert!(
            s[2].ends_with("--issue I1 --scope src/a,docs/b.md"),
            "start takes one comma list: {}",
            s[2]
        );
        // a title and a path are data: quoted where a shell would read them as code
        let odd = ProceedCandidate {
            title: "Keep $(printf X) and `id` literally, it's said".into(),
            scope: vec!["src/file name".into(), "src/*.rs".into()],
            ..two.clone()
        };
        let s = steps(&odd, Some("board-p1"));
        assert_eq!(
            s[0],
            "majordomus mesh claim 'src/file name' 'src/*.rs' --session board-p1 --issue I1 \
             --intent 'I1: Keep $(printf X) and `id` literally, it'\\''s said'"
        );
        assert!(
            s[2].ends_with("--scope 'src/file name,src/*.rs'"),
            "{}",
            s[2]
        );
        assert_eq!(word(""), "''");
        assert_eq!(clip("abcdef", 3), "abc…");
        assert_eq!(clip("abc", 3), "abc");
    }

    #[test]
    fn the_handler_reads_the_board_and_tells_an_undeclared_mesh_from_an_unread_one() {
        let repo = planned(&[("I0001", &[], &["src/a"])]);
        let local = || ProceedPlanInput {
            local: true,
            ..Default::default()
        };
        let d = proceed_plan(&repo.context().unwrap(), local()).unwrap();
        assert_eq!(d.selected.map(|c| c.issue).as_deref(), Some("I0001"));
        let mesh = d.sources.iter().find(|s| s.source == "mesh").unwrap();
        assert_eq!(mesh.state, ProceedSourceState::Undeclared);
        assert!(!d.provisional, "{}", d.reason);

        // a declared mesh this process does not run is a source it could not read
        let dir = repo.root().join(".ai/repo/mesh");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("majordomus.yaml"),
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n",
        )
        .unwrap();
        let d = proceed_plan(&repo.context().unwrap(), local()).unwrap();
        let mesh = d.sources.iter().find(|s| s.source == "mesh").unwrap();
        assert_eq!(mesh.state, ProceedSourceState::Unread, "{}", mesh.summary);
        assert!(d.provisional && d.next.is_empty(), "{}", d.reason);

        // and a declaration the index refuses is not the absence of one
        std::fs::write(
            dir.join("majordomus.yaml"),
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: bananas\n",
        )
        .unwrap();
        let d = proceed_plan(&repo.context().unwrap(), local()).unwrap();
        let mesh = d.sources.iter().find(|s| s.source == "mesh").unwrap();
        assert_eq!(mesh.state, ProceedSourceState::Unread, "{}", mesh.summary);
        assert!(
            mesh.summary.contains("could not be read"),
            "{}",
            mesh.summary
        );
        assert!(d.provisional && d.next.is_empty(), "{}", d.reason);
    }

    #[test]
    fn a_server_that_cannot_be_asked_leaves_its_board_unread() {
        let mut board = read("board");
        unasked_board(&mut board, "the leaseholder at http://x answered 404");
        assert_eq!(board.state, ProceedSourceState::Partial);
        let plan = plan_of(&planned(&[("I0001", &[], &["src/a"])]));
        let d = decide(&plan, None, None, &[], vec![board, read("mesh")]);
        assert!(d.provisional && d.next.is_empty(), "{}", d.reason);
        assert!(d.reason.contains("board"), "{}", d.reason);
    }

    #[test]
    fn a_session_of_the_same_name_on_another_runtime_holds() {
        use crate::mesh::journal::{ClaimMode, SessionInfo};
        let repo = planned(&[("I0002", &[], &["src/b"]), ("I0003", &[], &["src/c"])]);
        let dir = repo.root().join(".ai/repo/mesh");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("majordomus.yaml"),
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n",
        )
        .unwrap();
        let ctx = repo.context().unwrap();
        let net = std::sync::Arc::new(crate::mesh::testing::Net::default());
        let here = crate::mesh::testing::runtime(&net, "here:1");
        let there = crate::mesh::testing::runtime(&net, "there:1");
        let info = SessionInfo {
            session: "board-p1".into(),
            client: "test".into(),
            ..SessionInfo::default()
        };
        let claim = |c: &crate::mesh::cooperation::Cooperation, scope: &str, issue: &str| {
            c.claim(
                &info,
                vec![scope.into()],
                None,
                ClaimMode::Exclusive,
                Some(issue.into()),
            )
            .expect("admitted")
        };
        claim(&there, "src/b", "I0002");
        claim(&here, "src/c", "I0003");
        // a runtime is alive to another only while its beat keeps arriving
        there.journal().beat_own();
        here.dial(&["there:1".into()]).expect("linked");
        here.sync_with(there.runtime_key()).expect("synced");
        crate::mesh::testing::attach(&ctx, here.clone());
        let d = proceed_plan(
            &ctx,
            ProceedPlanInput {
                session: Some("board-p1".into()),
                local: true,
                ..Default::default()
            },
        )
        .unwrap();
        let why = |id: &str| -> Vec<ProceedConstraint> {
            let c = d.candidates.iter().find(|c| c.issue == id).unwrap();
            c.refused.iter().map(|r| r.constraint).collect()
        };
        assert_eq!(
            why("I0002"),
            [ProceedConstraint::Claimed],
            "board-p1 there is another session"
        );
        assert!(why("I0003").is_empty(), "board-p1 here is the caller");
        assert_eq!(d.selected.map(|c| c.issue).as_deref(), Some("I0003"));
    }

    #[test]
    fn the_handler_reads_live_mesh_claims_and_leaves_out_the_callers_own() {
        use crate::mesh::journal::{ClaimMode, SessionInfo};
        let repo = planned(&[
            ("I0001", &[], &["src/a"]),
            ("I0002", &[], &["src/b"]),
            ("I0003", &[], &["src/c"]),
        ]);
        let dir = repo.root().join(".ai/repo/mesh");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("majordomus.yaml"),
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n",
        )
        .unwrap();
        let ctx = repo.context().unwrap();
        let c = crate::mesh::testing::cooperation();
        let claim = |session: &str, scope: &str, issue: Option<&str>| {
            let info = SessionInfo {
                session: session.into(),
                client: "test".into(),
                ..SessionInfo::default()
            };
            c.claim(
                &info,
                vec![scope.into()],
                Some(format!("{session} works")),
                ClaimMode::Exclusive,
                issue.map(Into::into),
            )
            .expect("admitted")
        };
        claim("w1", "src/a", Some("I0001"));
        claim("w2", "src/b/parser", None);
        claim("me", "src/c", Some("I0003"));
        crate::mesh::testing::attach(&ctx, c.clone());
        let d = proceed_plan(
            &ctx,
            ProceedPlanInput {
                session: Some("me".into()),
                local: true,
                ..Default::default()
            },
        )
        .unwrap();
        let mesh = d.sources.iter().find(|s| s.source == "mesh").unwrap();
        assert_eq!(
            (mesh.state, mesh.holds),
            (ProceedSourceState::Read, 2),
            "{}",
            mesh.summary
        );
        let why = |id: &str| -> Vec<ProceedConstraint> {
            let c = d.candidates.iter().find(|c| c.issue == id).unwrap();
            c.refused.iter().map(|r| r.constraint).collect()
        };
        assert_eq!(why("I0001"), [ProceedConstraint::Claimed]);
        assert_eq!(why("I0002"), [ProceedConstraint::ScopeHeld]);
        assert!(
            why("I0003").is_empty(),
            "the caller's own claim is not held against it"
        );
        assert_eq!(d.selected.map(|c| c.issue).as_deref(), Some("I0003"));
        assert!(
            !d.provisional && d.next[0].contains("--session me"),
            "{:?}",
            d.next
        );
    }

    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        let e = &m.capabilities[0];
        assert_eq!(e.capability.id.as_str(), "proceed.plan");
        assert!(e.capability.kind.is_read_only());
        let http = e.capability.exposure.http.as_ref().unwrap();
        assert_eq!(
            (http.method.as_str(), http.path.as_str()),
            ("GET", "/api/v1/proceed/plan")
        );
        assert_eq!(
            e.capability.exposure.cli.as_ref().unwrap().path,
            ["proceed", "plan"]
        );
    }
}
