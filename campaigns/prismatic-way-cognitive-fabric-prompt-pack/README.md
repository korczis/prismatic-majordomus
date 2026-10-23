# Prismatic Way | Cognitive Fabric integration prompt pack

**Target:** `korczis/prismatic-majordomus` (Claude Code, high/max effort)

## Terminology
- **Prismatic Way**: design and delivery discipline: evidence-first; reuse-before-create; derive-before-generate; delta-before-rescan; deterministic-before-LLM; local-before-remote; publish-after-verification; measure-before-optimize.
- **Prismatic Cognitive Fabric**: cross-cutting behavior tying existing event hooks, `.ai` entities, task lifecycle, knowledge/evolve, reasoning/advisors, mesh, telemetry, and UI together. **Not** a new store/service/agent framework.

## Usage
1. Start Claude Code in the repository's canonical worktree. Read `AGENTS.md`, `.ai/README.md`, and the effective scoped rules first.
2. Paste [`prompts/00_MASTER.md`](prompts/00_MASTER.md). It delegates staged implementation to the other prompts.
3. If running in separate sessions, use `01` through `07` in order. For each one: claim scope, implement, test, deliver a PR, handover before advancing.
4. Use `contracts/` and `verification/` as acceptance contracts, not as documents to copy blindly into the repo. Discover actual repository paths, commands and schema versions.
5. Report observed state separately from planned, inferred, or proposed state. No success claim without exact CI/command evidence.

## Contents
| File | Purpose |
|---|---|
| prompts/00_MASTER.md | End-to-end autonomous implementation mandate |
| prompts/01_DISCOVERY.md | Baseline, collision, coverage, repo graph and feature reuse |
| prompts/02_EVENTS_INTENTS.md | Event-driven incremental intent reconciliation |
| prompts/03_KNOWLEDGE.md | Evidence-bearing knowledge candidate/evolve automation |
| prompts/04_REASONING_MESH.md | Budgeted reasoning, peer critique, cooperation |
| prompts/05_SURFACES.md | CLI/API/MCP/Cockpit/Pages live projections |
| prompts/06_ENFORCEMENT.md | Deterministic behavior, safety, security, tests and gates |
| prompts/07_DELIVERY.md | Dogfood, metrics, release, completion, handover |
| contracts/ARCHITECTURE.md | Non-negotiable invariants and boundaries |
| contracts/EVENT_CONTRACT.md | Logical schema and reconciliation algorithm |
| contracts/ECONOMICS.md | Routing, budgets, observability and baselines |
| verification/ACCEPTANCE.md | Test matrix and DoD |
| verification/REPORT.md | End-state evidence report template |

**Important:** This pack proposes work; it does not claim the implementation has been completed. GitHub source review was limited to selected files on the repository default branch, 2026-10-19.
