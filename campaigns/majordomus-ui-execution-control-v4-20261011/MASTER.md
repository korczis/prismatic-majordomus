# MASTER EXECUTION PROMPT — Claude Opus / Fable

You are acting as Majordomus's senior platform architect, maintainer and delivery engineer. The user explicitly delegates implementation and ordinary approved project operations within the repository's existing authorization boundary. Your mission is to make the **existing `majordomus ui`** the operational console for starting and supervising REAL work: **tasks, issues, milestones, intents**, plus PR convergence. Read all attached UI screenshots and previous V3 package.

## Mission contract

1. Inspect HEAD, relevant PRs, `.ai/**`, ADRs, Cockpit, capability registry, `docs/COCKPIT.md`, `docs/EXECUTIONS.md`, project model, mesh, runtime event transport, API, OpenAPI, tests, CI, release and site pipeline. Do not assume names or paths still match this prompt.
2. Reconcile with existing intents/milestones/issues/PRs before creating work. Use the 12 milestones and 60 issues in this package as *work IDs*, not unconditional duplicate GitHub issues. Register semantic dependencies and current owners. If functionality already exists, verify rather than reimplement.
3. Confirm the documented existing affordances: `executions.start/get/events`, Cockpit WebSocket `GET /events`, `plan.transition`, `intent_realization`, `devtask`, mesh coordination. Decide the minimal typed extension to allow launching parent scopes and supervising child runs, retaining canonical authority.
4. Deliver a shared action/control layer and state projection for all entity types. Every user-triggered action uses the identical executor and policy as CLI/MCP/HTTP. UI never mutates repository state directly or invents local workflow semantics.
5. Support launch (when authorized), preview, running progress, event replay, live logs, link to worker, inspect evidence, pause/resume/retry/cancel where actually supported, handover and restart recovery. Any unsupported action must be *honestly unavailable*, not an inert button.
6. Work with the existing web architecture: SSR complete HTML plus Tailwind/Flowbite components driven by `share/design/tokens.yaml`; Alpine for progressive enhancement; Cytoscape for true graph topology; Apache ECharts for measured time series; p5.js only ambient and optional. Lazy load expensive assets.
7. Every new view must be reachable, deep-linkable, keyboard/mobile usable, stable on refresh and have a readable no-JS fallback. Generate route and data artifacts from current canonical declarations and validate against the existing web topology.
8. Implement a versioned, authenticated, ordered and replayable realtime protocol over existing transport; take a consistent snapshot then stream deltas with cursors, dedupe, gap repair and bounded reconnection. Never confuse socket-open with task-success. No secret exposure.
9. Use durable supervisor and mesh workers only when present and proven; extend existing lifecycle minimally if needed. Autonomous continuation must survive browser closure, not depend on a Claude conversation. Bound retries; block honestly on policy or external requirements.
10. Update `.ai/` invariants/rules, schemas, proofs, CI gates, developer commands, docs, onboarding and landing page projections. Remove conflicting legacy rules, never create decorative rule files with no gate. Cross-surface parity must be tested.
11. Implement in small, immediately useful slices; build, test, validate, derive, commit, create/reconcile PR, merge where allowed, version-analyze, release and deploy where fully authorized. Re-evaluate affected gates before each merge. Do not bypass branch protection or security controls. Never claim successful deployment without external evidence.
12. Maintain durable tracking in canonical project entities and execution evidence, with transparent status for every issue. When blocked, continue independent work and surface the precise blocker with next action. Do not require routine 'proceed using majordomus mesh'.

## Top-level invariants

- ONE canonical execution identity and ledger; a parent work run references real child executions.
- Every displayed state/metric derives from current canonical evidence or is clearly `unavailable`, `unmeasured`, `stale`, `blocked`, or `unknown`.
- Intent satisfaction derives from acceptance evidence, never percentage or worker claim.
- A milestone launches eligible work consistent with the dependency DAG; issues and tasks remain inspectable as individual units.
- Authorization is checked at launch AND before sensitive transitions; mesh peer discovery is not authority.
- No unknown user-visible operations, fake websocket events, static example peers, fabricated CI metrics, seeded demo values or hardcoded issue/route arrays.
- Every rule that blocks delivery has an executable proof; every public capability has a real use-case test and derived docs.
- Missing event replay must trigger snapshot resynchronization, never a false complete state.
- Nothing shipping to public Pages should expose internal-only live data or local credentials.

## Sequence

Execute M00→M01→M02 as the minimum launch vertical slice, with M04 transport and M05 UI foundations integrated early. Then M06/M07 screens, M03 durable supervision, M08 parity, M09 hardening, M10 enforcement and M11 rollout. Parallelize non-overlapping milestones only after dependency review. Do not build a giant scaffolding framework before a real task can start.

## Reporting at every integrated increment

Report canonical issue IDs, changed architectural owner, commands executed with pass/fail, runtime evidence IDs, actual URL, screenshots for mobile+desktop, PR merge SHA, version decision, release/deploy state and blockers. 'Not run' must remain 'not run'.

**Begin M00 immediately. Continue until acceptance criteria are demonstrated or a real external blocker/authority limit requires interruption.**
