+++
title = "Runtime continuity: root causes and the reference differential"
description = "why a worker's context, its prompts and the rules applied to its work were disconnected, measured with file evidence; the differential against the reference repository, what was adopted, adapted and superseded, and the invariants imported"
weight = 17
[extra]
source = "docs/RUNTIME_CONTINUITY.md"
+++

{% raw %}

Audit of 2026-09-15. Majordomus at master `c3f20da3f`; reference `~/dev/prismatic-platform` at
`5625d160f1` (the mission named `prismatic-platform3`, which does not exist; the person chose
`prismatic-platform`), plus its worktree `claude-majordomus-integration-20260913-102303`
(`6ef3edbdfc`, unmerged). Every row below is backed by a file:line in the four audit reports;
nothing here is a documentation claim unless marked. A location in the *Reference* column or
after "Reference:" is a path in that repository, not in this one; where such a path could be
mistaken for a file here, its directory and file name are given separately.

## 0. The premise, corrected

The reference is **not** a stronger implementation of session context or prompt provenance.
It has no session object, no compiled context, no context injection, placeholder-null prompt
provenance fields in every record, ~20 provider call sites bypassing its own metadata wrapper
and an enforcement check that is not registered. Its Majordomus worktree retires its own
protocol scripts and adopts Majordomus hooks. What it does prove is a set of narrow
record-integrity and enforcement-plumbing invariants, listed in §3.

Majordomus also **never invokes a model** (verified: no provider client, HTTP call or CLI spawn
anywhere). "Prompt persistence at the provider invocation boundary" therefore cannot mean a
wrapper around an SDK call; the boundary Majordomus owns is the provider's hook and the MCP
session. That is the central adaptation.

## 1. Root causes (not symptoms)

<div class="overflow-x-auto" tabindex="0">

| Failure | Root cause | Evidence |
|---|---|---|
| Context not generated / reloaded | Three independent compilers — the SessionStart briefing (`lib/derive.sh:323`), the `context` builder (`lib/context.sh:127`), Rust `devcontext::compile` (`devcontext/mod.rs:762`). None persists a revision that is consumed. | runtime audit §2 |
| Context used ≠ context recorded | The frozen "working context" stores the builder's output; the agent receives the briefing, which is never stored or fingerprinted. | `session_context.sh:164` vs `derive.sh:336` |
| Staleness undetected | Freshness compares git head and branch only; policy, rules, ADRs, issue, handover, dirty tree never invalidate. Nothing re-injects after SessionStart. | `session_context.sh:92-114` |
| Context not reaching Codex/Gemini | Lifecycle and capture adapter tables hold a claude-code row only; other clients get MCP `instructions()` with no task or continuity. | `capture.sh:249,286`; `mcp/protocol.rs:308` |
| Failures invisible | Every hook path exits 0; failures go to logs the agent never reads (two live now: stale executable, 0.6.1 server vs 0.7.0 tree). | `capture.sh:1475-1500` |
| Prompts lack provenance | Only writer runs at UserPromptSubmit with a closed field set; no Stop/response hook; no field for task, issue, context revision, model, result, lineage. | `capture.sh:83,110` |
| Attribution wrong | 21 episodes stranded open, `session.recovered` never written; resolution falls back to a last-opened pointer; Rust ledger reads a layout that no longer exists; repository_id spelled two ways. | `common.sh:860-890`; `ledger.rs:388` |
| Governance not enforced | Rules run only when `check/finish/doctor/watch` is invoked by name; the single Rust execution path (`CapabilityExecutor::execute`) has no policy stage, so HTTP/MCP/Cockpit writes (`plan.transition`, `recover.orphans`) bypass it. | `doctrine.sh:172`; `executor.rs`; `router.rs:858` |
| Enforcement idle | Task validators skip with no task; pre-push `finish --check` exits 0 with no task or a handed-over one (the primary checkout's state since 09-05); the briefing tells agents task-less work is fine. | `check.sh:111`; `finish.sh:39,48-50`; `derive.sh:349` |
| Proof declared, not observed | `rules report`: 106 NotRun, recorded 0, passing 0, `satisfied: true` — `unsupported()` has no NotRun arm; 92 of 137 rules have no validator. | `rules/mod.rs:1000` |
| No applicability | "Applies" = command name in `enforced_by`. The only reasoned selection of rules/ADRs is devcontext's, feeding a context nothing consumes. ADRs are never enforced. | `doctrine.sh:145`; `devcontext/select.rs` |
| Two rule engines | Validators must be shell functions; Rust owns proof states, context selection and a plan mirror but cannot evaluate a rule. | `doctrine.sh:143` |
| Issue ↔ session disconnected | Task record has no issue field; `plan start` stamps a time with no actor/task/session; prompts carry neither; joins are heuristics on an untracked rotating ledger and branch names. | `start.sh:88-94`; `plan.sh:449` |
| Claims don't refuse | Peer board (memory), task scope (`current.yaml`), plan issue scope — three representations, all report-only. | `peers.rs:345`; `start.sh:110`; `plan.rs:233` |
| Canonical model undecided | ADRs 0047/0052 (typed session domain in Rust, episode lifecycle independent of task) are *proposed*; the shell writes, Rust reads and holds writers nothing calls. Seven session branches (ADRs 0043, 0046) are unmerged and ~500 commits behind. | `session/mod.rs:5-30`; `origin/feature/the-session-is-evidence` |

</div>


## 2. Differential matrix

Decision key: ADOPT (take the reference mechanism), ADAPT (take the invariant, change the
mechanism), SUPERSEDE (Majordomus is already ahead; the fix is Majordomus's own), N/A.

<div class="overflow-x-auto" tabindex="0">

| Invariant | Reference implementation / owner / enforcement / tests | Majordomus current / owner / bypass / defect | Decision |
|---|---|---|---|
| Session identity & lifecycle | None; client-supplied id copied into records; resume behavioural (`CLAUDE.md:45-48`) | Episodes via provider hooks (ADR 0015); shell-written; 21 stranded; pointer fallback | SUPERSEDE — accept ADR 0052/0047, one owner, automatic recovery |
| Context compilation | `scripts/ai-context` prints git facts; a hand-written `current.md` under its `.ai/context/` | Three compilers; devcontext has fingerprint, no consumer | SUPERSEDE — one compiler (devcontext), persisted revisions |
| Context revision identity | None | None persisted | SUPERSEDE — new immutable revision store |
| Invalidation | None | head/branch only | SUPERSEDE — dependency fingerprints (git, index, issue, milestone, rules, ADRs, handover, knowledge) |
| Context consumption | None (agent told to run scripts) | SessionStart stdout only, Claude Code only | ADAPT — hook injects current revision, re-injects on staleness at UserPromptSubmit; MCP resource for other clients |
| Prompt record integrity | Byte-exact body, 0600, atomic no-clobber `ln` publish, tested (`prompt-log:137-184`) | Redacted, episode-linked; grep idempotence; sed redaction | ADOPT byte-exact + no-clobber publish; keep redaction |
| Prompt provenance fields | 8 fields hard-coded null | No task/issue/context/model/result fields | SUPERSEDE — typed record linking context revision, task, issue, milestone |
| Response / terminal state | None | None (no Stop hook) | ADAPT — Stop hook closes the invocation record; missing terminal state is a diagnostic |
| Lineage / retries | None | None | ADAPT — root/parent ids from provider session + prompt order; subagent/tool-derived prompts where the provider exposes them |
| Model invocation wrapper | `RuntimeAdapter.send_request` with metadata; bypassed ~20×; check unregistered | No model calls at all | N/A for SDK wrapping; ADOPT the lesson: a boundary check must be registered and blocking — add a gate that fails on any provider client/HTTP call |
| Handover resolution | Refuses other repo/branch; divergence classified exact/advanced/diverged/different_context (`handover:333-503`, tested) | `--resolve` scoped to worktree+branch, labels divergence | ADOPT the refusal and four-state classification tests where Majordomus lacks them |
| Handover structure | Prose + computed front matter; `session_id` null in practice | Prose + computed front matter; task_id only | SUPERSEDE — structured references (episode, issue, context revision, commits, open gates) |
| Governance typing | Pillars registry with `@wired` vs `@documented_unwired` (`pillars.ex:23,28`); rules untyped | Rules typed (class, validator, enforced_by, proof); 92 without validator | ADOPT declared-vs-wired split in reports; `NotRun` ≠ satisfied |
| Applicability | `check.doctrines --changed` over staged ACMR paths, tested | None beyond command name | ADAPT — typed evaluator (task, paths, capability) → APPLICABLE/NOT_APPLICABLE/UNKNOWN with reason; reuse devcontext selection |
| Preflight at execution | Git hooks only; CI advisory; subagents ungoverned | Shell commands only; Rust executor has none | SUPERSEDE — policy stage inside `CapabilityExecutor::execute` for writing capabilities |
| Missing gate fails closed | Distinct exit when a gate artifact is missing (`03-ai-review-gate:25-30`) | Hook shims exit 0 when the tool is missing | ADOPT |
| Overrides & bypass audit | Four typed trailers; bypass env recorded before verdict (`aiad-override-policy.sh`, `pre-push:210-271`) | `--no-verify` and unset hooksPath leave no trace | ADAPT — overrides as ledger events; doctor/CI detect commits made without the hook |
| Claims | `wt claim` refuses overlap at claim time; commit-time check undispatched | Three report-only claim systems | ADAPT — one claim model; refuse at claim time and at commit |
| Issues/milestones | GitLab only; commit-msg requires `#NNN` syntax, not existence | Local canonical records + GitHub projection with drift gate | SUPERSEDE — add task↔issue link at `start`, commit trailer resolved against the plan |
| ADR relevance | DB fields only; no selection | devcontext selection; no enforcement | ADAPT — ADRs selected by affected paths into context with reason; conflicts surfaced in preflight when an ADR declares a checkable constraint |
| Bootstrap | Hand-written CLAUDE/AGENTS with budget + link-existence gate at commit | Generated projections; briefing; told, not forced | ADOPT the budget+reference-existence gate if Majordomus lacks equivalent; keep generation |
| Hook test fidelity | Test executes the block extracted from the dispatcher (the hook test in its worktree's scripts directory) | Cases run shims directly | ADOPT |
| Aggregator skips | Missing test silently skipped; skipped hook test counts as pass | Known locally (memory: skipped case counts as pass) | ADOPT as a requirement: missing = fail, skip ≠ pass |
| Knowledge | Session notes, history and knowledge share one store | Shell source list; ADR 0058 unmerged | SUPERSEDE — land ADR 0058 separation |
| Events | Three audit formats | 26 ledger events; many never written; two Rust writers, one unlocked | ADAPT — typed events for context/prompt/governance through one locked writer |
| Surfaces parity | Hand-written mirrors, stale counts | Registry-derived CLI/HTTP/OpenAPI/MCP/Cockpit with drift gates; shell tool outside registry | SUPERSEDE — new state exposed only as capabilities |

</div>


## 3. Imported invariants (to be carried into docs/ as the mapping record)

1. A handover from another repository or branch is never resolved as current; a detached HEAD
   matches only its own worktree. Reference: `scripts/handover:333-349`, `test-handover.sh:196-207`.
2. Divergence is classified from ancestry, and missing objects are "ancestry unknown".
   Reference: `scripts/handover:476-503`.
3. Records publish atomically without clobbering. Reference: `scripts/prompt-log:172-184`.
4. A prompt body is stored byte-exact. Reference: `scripts/prompt-log:137-141`, `test-prompt-log.sh:29-47`.
5. A malformed record is skipped with a named warning and can never win selection.
   Reference: `scripts/handover:432-440`.
6. Inherited `GIT_*` variables are scrubbed in hooks. Reference: `scripts/handover:20`.
7. A declared control with no checker is reported as unwired, never counted enforced.
   Reference: `pillars.ex:23,28`.
8. A missing gate artifact blocks with a distinct exit code. Reference: `03-ai-review-gate:25-30`.
9. Overrides are typed and audited before the verdict. Reference: `aiad-override-policy.sh:87-90`.
10. Claims refuse overlap at claim time. Reference: `operations.sh:878-932`, in its `scripts/wt/`
    directory.
11. Hook tests execute the dispatcher's own block. Reference: `test-majordomus-hooks.sh`, in the
    `scripts/` directory of its Majordomus worktree.
12. A boundary rule needs a registered, blocking check — the reference's unregistered credo
    check is the counter-example. Reference: `llm_adapter_enforcement.ex` absent from `.credo.exs`.
{% endraw %}
