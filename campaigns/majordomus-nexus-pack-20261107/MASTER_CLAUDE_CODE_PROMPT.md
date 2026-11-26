# MAJORDOMUS NEXUS — MASTER CLAUDE CODE MISSION

> Execute inside the real, current `prismatic-majordomus` checkout. This is an **implementation mandate**, not a request for a proposal. Read the current repository before writing code. It is November 2026; historical source snapshots are not a substitute for the live branch.

## 0. Mission and north star

Build **Majordomus Nexus**, a provider-neutral, evidence-driven **universal intent ingress and coordination capability** that turns a prompt from Claude Code or another supported surface into an accountable, context-aware, persistent, governed execution. Integrate and consolidate the **existing** Majordomus ecosystem: `.ai/**`, context resolution, rules, doctrines, KB, ADRs, Git/history, sessions, intent/issue/milestone graph, `proceed`, IDD, AI mesh, peers, skills, capabilities, event bus, typed executions, CI/release, telemetry, CLI, API/OpenAPI, MCP, Cockpit/LiveView where present, docs, sites and GitHub Pages.

Do **not** implement a new orchestration silo. Nexus should be the **unified decision boundary and adapter** onto the canonical systems that already exist. Its output should make every existing surface, and every future client, more useful with no duplicated catalogues or duplicate truth.

The system must work unattended for reversible, permissioned development tasks. Consult existing knowledge, source, tests and available AI peers before asking a human. Ask the human **only** when an actual hard boundary requires it: missing authorization/credentials, irreconcilable high-impact conflict, unclear product choice with materially different irreversible outcomes, budget limits, legal/security implications, protected branch restrictions, or an externally controlled action. Human silence does not imply consent.

**Deliver completed code, migrations, tests, docs, governance, derived surfaces, and deployment evidence**, not speculative designs or a forest of PRs. Prefer small production-safe slices, integrated continuously. No fake green, no force pushing protected branches, no bypass of branch protection, no fabricated claims about deployments or peer consensus.

## 1. Mandatory preflight: discover, then act

First determine `git remote -v`, default branch and HEAD, worktree topology, clean/dirty state, current PRs/issues/milestones and active peers. Treat unknown GitHub data as unknown rather than pretending it was checked. Read `AGENTS.md`, `CLAUDE.md`, `.ai/README.md`, `.ai/manifest.yaml`, canonical policies and effective rules; follow `majordomus context` and `majordomus context resolve <path>` where available, plus the relevant README chain. Scope all operations by existing worktree/claim machinery. Do not load `.ai/local/` as general model context: it is operational data and may include confidential prompts.

Inspect existing implementation and its **current** interfaces for: Claude prompt capture and lifecycle hooks; provider adapters and mod/plugin configuration; sessions, handover and compact; intent/IDD/proceed; peer announcements and mesh; Rust `capability!` registry; task/milestone issue graph; execution engine and persistence; schemas and generators; CI/release; API/MCP/Cockpit. In the supplied 2026-09-11 snapshot there were already `.claude/hooks/majordomus-capture`, session start/compact/end hooks, `lib/capture.sh`, an explicit `.ai/manifest.yaml` registry, canonical Rust capabilities, a shared MCP server, worktree isolation and `majordomus check`/`finish`. **Verify their current successors; extend them, do not recreate them.** The older `docs/EXECUTIONS.md` explicitly described in-process executions as non-durable; check whether that has since been solved before implementing persistence.

Before any new module, command, workflow, schema or generated file: search for equivalents and duplicates, inspect canonical ownership, run `scripts/collision-check --new ...` when available, inspect `majordomus_peers` and announce the intended change/scope through `majordomus_announce` or the current equivalent. Peer claims do not substitute for real locks. Git and runtime evidence override chat recollections. Capture factual discovery in a compact persistent audit artifact using current repository conventions.

**Do not edit generated `AGENTS.md`/`CLAUDE.md` directly** if they are generated from `.ai/repo/policy.yaml`; change the canonical source and regenerate through the real command. Do not edit duplicated CLI/OpenAPI/MCP lists: change canonical `capability!`/registry declarations and derive projections. Do not invent new `.ai` section directories without reconciling the manifest and validating schemas.

## 2. Core invariants

1. **One canonical reality.** Facts and operations have one owner; everything else is a deterministic, typed, versioned projection.
2. **Preserve input.** The original human prompt is immutable evidence. Never replace, rewrite, redact silently or reinterpret it as an authorized mutation. Treat hook-added context as separate provenance-bearing material. Don't turn `UserPromptSubmit` into a destructive prompt rewriter; the official command hook adds context, it does not replace prompt text.
3. **Resolve, don't duplicate.** Every prompt binds to an existing intent/task/execution when semantically appropriate, or takes a read-only/ephemeral path; no issue or PR spam.
4. **Evidence over inference.** Keep `observed`, `derived`, `inferred`, `proposed`, `verified`, `unknown` semantically distinguishable. Infer from trusted sources but never auto-promote speculation to architecture or an accepted ADR.
5. **Persistent workflow.** Durable state transitions, resumability and replay-safe/idempotent effects; distinguish at-least-once event delivery from effect idempotence. Reconcile state with Git and external systems after crashes.
6. **Respect authorization.** Model deliberation is not user consent. Never bypass Claude Code permissions, GitHub controls, secrets policy, quota/budget or pre-existing risk gates. Auto-merge/deploy only if existing permissions and policies authorize it and evidence proves gates passed.
7. **Cheap fast path.** Every prompt may enter a lightweight ingress; only complex work triggers expensive context, planning, peer debate or mesh execution. No blocking network/compilation from synchronous prompt hooks.
8. **Provider neutral.** Claude is the first adapter, not the domain model. Support other providers only through verified event/transport seams; do not scrape unstable internals by default.
9. **Derived across surfaces.** CLI, JSON, API, OpenAPI, MCP, Cockpit, docs, site, telemetry and hook/statusline adapters expose the same truth with semantic versions and cross-surface contract tests.
10. **No invisible gaps.** Unsupported ingress, missing coverage, unavailable peers, partial failure, stale caches and unmerged branches have explicit diagnostic states. No success-by-default.

## 3. Architecture to discover/refine, not blindly recreate

```
Claude Code / Codex / Gemini / CLI / Cockpit / API / MCP / GitHub
                      |
             provider ingress adapters
                      v
      canonical Nexus Prompt/Intent Envelope
       identity · provenance · dedupe · policy
                      |
      Context compiler + Intent resolution
    .ai · knowledge · ADR · Git · graph · history
                      |
         Decision & Execution Coordinator
        existing IDD / Proceed / Mesh / skills
                      |
      canonical execution + event/state owner
      persistent journal · recovery · evidence
                      |
      verification / delivery / reconciliation
                      |
 CLI / HTTP / OpenAPI / MCP / Cockpit / Pages / reports
```

This is a **logical** architecture. Do not automatically create six new crates, services or tables. Build an ownership map and reuse the implementation with the strongest existing invariants. If an existing session/capture ledger and an execution store are separate, preserve their distinct semantics: immutable raw prompt evidence vs mutable runtime state and append-only transition evidence. Don't turn an archive of private raw prompts into a searchable KB or publish it.

### Model contracts (conceptual)

`PromptEnvelope` / `IngressEvent`: schema version, provider/transport, session, original prompt reference, prompt identity, repository/Git context, causation/correlation IDs, initiator (human/system/agent), intent candidate, event timestamp, data classification; immutable once accepted.

`Resolution`: route (`QUERY`, `ACTION`, `INTENT`, `CAMPAIGN`, `CONTROL`), reuse/merge/create decision, considered candidates, conflict references, confidence/calibration, policy version and explanation. A read-only question should not allocate a milestone.

`ContextBundle`: reference to precise source versions and scoped documents, provenance, relevance/exclusion reasons, token budget, expiry/cache fingerprint, disclosure eligibility, facts vs hypotheses. Zero arbitrary whole-repository dumping.

`Decision`: policy input hashes, evidence refs, capabilities considered, route selected, safety approval status, dissent/peer review result and why; reproducible as far as practical.

`Execution`: canonical current state, related intent/work item, actor/owner, resource claims, deadline, retry/idempotency key, deps, progress and evidence links. State machine transitions are typed and validated. Projection at a given sequence/version must be stable.

`Outcome`: actual tests, commands and exit diagnostics, Git SHAs/PR/CI/deploy evidence, generated artifact drift verdict, `.ai` reconciliation, status (`completed`, `partial`, `blocked`, `failed`, `cancelled`), unresolved debt.

Adapt names/shapes to actual repository conventions. No parallel identity model; source Git identity from Git, not hand-authored values.

## 4. Route & governance semantics

- **QUERY**: read-only answer; cheap targeted context, no issue/PR; only operational receipt if configured. Do not force all questions through `proceed`.
- **ACTION**: bounded modification; existing minimal task/proceed lifecycle, focused gates, no new intent hierarchy unless necessary.
- **INTENT**: meaningful feature/change; resolve against existing graph; use IDD, decisions, acceptance, dependency graph.
- **CAMPAIGN**: multi-subsystem mission; bounded DAG, capability-aware mesh, independent scoped workers, shared integration owner; avoid unbounded fan-out.
- **CONTROL**: inspect, resume, pause, cancel, replan or close existing execution; refuse unauthorized or ambiguous references.

A single prompt can **continue, refine, supersede, split, merge or cancel** an intent; all transitions need evidence and permission semantics. Clarifying questions are a last resort when options materially differ; otherwise choose reversible minimum-scope assumptions, record them and keep moving.

## 5. Claude Code event strategy

Inspect the official docs for the **installed version** before editing settings: https://code.claude.com/docs/en/hooks and https://code.claude.com/docs/en/settings . Use supported project hooks for `SessionStart`, `UserPromptSubmit`, relevant tool-use before/after, subagents, compaction and end/stop lifecycle, but add only events that have an actual use and acceptance test. Reuse the installed `majordomus capture` adapter and its immutable raw prompt policy; extend/compose rather than replace it. Respect the existing capture invariant that a broken archive does not swallow the human's prompt. A policy decision about **tool execution** may deny a dangerous tool at `PreToolUse`; it must not turn incidental telemetry failures into missing prompts.

Synchronous preflight is bounded, local, cached, and not a place for LLM calls or network fan-out. Heavy inference/peer consultation occurs in the asynchronous coordinated execution where permissible; hand a minimal decision/context reference back to Claude. Provide validation for actual hook JSON shapes, stderr/stdout use, exit codes, timeouts, stale process handling, workspace trust and project-local settings precedence. Handle programmatic/system-initiated `UserPromptSubmit` events using source metadata; do not conflate them with newly authorized human instructions. `Stop` is not a verification result: guard reentrant `stop_hook_active`, bounded continuation, and do not block forever. An optional Claude mod is a separately gated optimization, never the sole policy or persistence authority.

Distinguish honest **capture coverage** from universal interception: if Claude, cloud or third-party mode does not load project hooks, report coverage limitations explicitly. Out-of-band `git` edits require normal repo gates, irrespective of hooks.

## 6. Consultation protocol: no unnecessary human bottleneck

When a materially uncertain decision remains after reading the actual source and canonical `.ai`:

1. Frame a compact decision memo: question; constraints; facts with file/commit/test evidence; options; risk; reversibility; what would falsify the preferred choice.
2. Discover permitted peers through the **existing** mesh/peer registry. Announce work and prospective paths first; avoid concurrent workers modifying identical resources. Choose distinct roles: IMPLEMENTER (proposal), CRITIC (adversarial failure/security/counterexamples), VERIFIER (tests/architecture/traceability). A peer's language model family is less important than its independent evidence. Prefer existing connected peers and supported clients. Do not invent a connected Codex/Gemini service or spoof a provider.
3. Constrain each consultation to the minimum relevant excerpt/data classification; don't send secrets, raw prompt archives, personal info or private source to unapproved providers. Honor token/time/cost budgets from current policy; bounded retries and one synthesis round by default, escalating only if new evidence justifies it.
4. Reconcile by **source, code, tests, invariants and reproducible observation**, not majority vote. Capture dissent and confidence, merge findings into the canonical decision/evidence record. If no peer is available, proceed on reversible low-risk work with explicit uncertainty, but do not fabricate peer deliberation.
5. Escalate to human only for actual authorization, irreversible external effect, unresolved high-risk design ambiguity, compliance/security policy, or exhausted authorized budget. State exactly the decision needed and the two best options. Until then, implement reversible verified tasks, not stall the entire train.

Do not invent a second discussion/blackboard/event bus. Integrate existing communication, handover and execution events.

## 7. Persistence, eventing and performance

Use current canonical durable storage (if any), not a new database by default. Persist immutable receipt/evidence before ACK; atomically transition state or use an outbox/inbox/idempotency pattern for external effects. On crash/restart, reconcile Git/CI/deploy before retrying; never blindly re-run pushes, PR creation or deploy. Include concurrency/collision, process restarts, compaction, duplicate hook invocation, out-of-order tool completions and stale repository HEAD in tests.

Keep hook latency and build/deploy costs within **measured** project budgets. Define a new SLO only after baseline measurement. Track receipt rate, policy decision time, cache hit, context bytes/tokens, peer use, deduplication, completion rate, wasted work, recovery and actual delivery evidence. Separate estimated provider economics from reconciled billing. Do not claim savings without control data.

## 8. UX: intent workspace and live prompt pack

Build on relevant existing Cockpit/LiveView/editor components, not an isolated demo. Support `new intent → type freely → continuously discuss with Nexus → show related `.ai`/Git/ADR/KB/work and conflicts → ask only material clarifications → live preview of executable plan and Claude prompt/prompt pack → materialize through the canonical work graph → execute/progress/replan/verify/merge`. Persist drafts, edit events, decisions and revisions. Both CLI and UI must attach to the same intent, not write rival copies. Streaming updates must show real backend state and reconnect/recovery semantics; zero mock metrics or disconnected buttons. Provide responsive/mobile behavior only where supported by existing UI and tests.

## 9. Delivery, integration and deployment

Read the repository's active task lifecycle, worktree rules, collision checks, issue/PR automation, release/versioning policy and CI constraints **before** creating branches. Use a small number of cohesive integration lanes (suggest 3–5 PRs maximum unless actual branch protection/design demands otherwise), not one PR per prompt. Atomically implement/test/checkpoint each prompt, but batch compatible slices in a lane. Run and honor `majordomus context`, `start`, `checkpoint`, `check`, `finish`, `usecase impact/coverage`, `generate`, drift checks, `just` and collision-check only if verified available, adapting to current interfaces. Do not manually change generated inventories.

Before calling work complete: local quality gates, integration/e2e, security, schemas, cross-surface invariants, docs/examples, version bump per repository SemVer rules, changelog/release record if required, push, PR, CI, review, merge, trunk HEAD proof, deployment/site probes if policy requires. Do not use stale CI results for a newer SHA. If merge/deploy requires an approval, produce a precise blocked handoff but finish all permitted work. Do not report partial work as deployed.

## 10. Implementation program and prompt pack

If this pack is present, read `README.md`, `pack.json`, and `contracts/*.md`. Execute the numbered prompts from `prompts/` in dependency order, grouping them into the release lanes defined by the pack. Each prompt must finish with a concrete code change, a check/evidence artifact, or an explicit `no-op already satisfied` supported by proof. If a step proves a capability already exists, integrate/test it and skip duplicate work. Split oversized steps internally to keep safe bounded units; never split for the purpose of generating more PRs.

If the pack is not present, use the same stages directly:

A. Discover/ownership/architecture; B. Existing capture/ingress and hooks; C. Intent resolver and context compiler; D. Durable coordination with Proceed/IDD/Mesh; E. Governance and evidence; F. Intent workspace and cross-surface projections; G. telemetry, tests, release/CI/proof.

Checkpoint the verified status and next action in **existing** session/handover storage at each major boundary. At the start of a new Claude session, inspect that state and resume, not restart from zero. Avoid asking the user to re-explain any already documented decision.

## 11. Definition of done (hard acceptance)

Prove with real commands/tests that:

- Human and system-initiated prompts are distinguishable and receive immutable, correctly correlated capture if the adapter supports them.
- A first prompt can start an intent; a follow-up can refine the same intent; duplicate prompts cause no duplicate side effects or issues.
- Simple Q&A avoids expensive routing; complex work gets adequate verified context and mesh planning.
- Existing `.ai` rules/ADR/KB and Git/history visibly affect routing; conflicting inputs produce evidence-backed decisions.
- Existing Proceed, IDD, mesh and task graph are **reused**, with no conflicting state owners or hardcoded duplicate registries.
- Prompt/agent/tool/session state survives compaction and restart, with bounded retry and idempotent external mutations.
- Tool permissions, scopes, secrets and protected branches still hold under failure, concurrent work and absent peers.
- New governed capability appears in CLI/API/OpenAPI/MCP/Cockpit/docs/site through canonical declaration, without hand-editing seven lists.
- Intent draft, live preview and materialization are persistent and real, if this UX is in agreed implementation scope.
- The final committed Git SHA, CI run, integration state, and deployed endpoint (if relevant) are verified and cited in the final report.

Don't finish with only tests passing on an unmerged branch. Don't claim all surfaces are working if one is missing. State precisely what is real, what is degraded/unsupported, and any **specific** external gate that cannot be cleared without user authorization.

## 12. Required final report

Return compact but exact: baseline HEAD; architecture reuse/deletions; selected canonical owners; commands/tests run with pass/fail and evidence; per-surface proof; prompt capture/hook coverage matrix; peer consultations actually performed (or `unavailable`); intent continuation demo; durability/crash demo; git SHAs, PR links, merge SHA, current trunk HEAD, deployment URLs/probes; applied version/release; metrics before/after where actually measured; remaining blockers only when objectively external. No placeholders, no marketing claims, no invented data.

**Start now. No architecture-only stopping point. Build and deliver the smallest useful production slice first, then advance the dependency graph until all applicable acceptance criteria are met or a genuine external authorization boundary blocks further mutation.**
