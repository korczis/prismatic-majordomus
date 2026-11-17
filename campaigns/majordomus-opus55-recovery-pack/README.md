# Prismatic Majordomus: Opus 5.5 Recovery & Evidence-First Prompt Pack

**Purpose:** turn the independent 2026-10-07 assessment (HEAD `3ede593c0e`) into a bounded, auditable implementation campaign. The report is a starting hypothesis, **not** a verified description of whatever HEAD is when these prompts execute.

**Why this is different from another “implement everything ASAP” prompt:** milestones have dependencies, non-goals, a trust model, tests that must fail under attack, and truthful blocked states. A worker never gets to certify its own acceptance merely by asserting that tests ran. No release, merge, deployment or ruleset modification is presumed authorized.

## Contents

- `prompts/`: **14 detailed standalone prompts** for Claude Opus 5.5, one milestone per file, five issues per milestone.
- `MANIFEST.json`: machine-readable milestone DAG with issue counts and priorities.
- `RUNBOOK.md`: execution and review protocol, waves, stop/go decisions.
- `ASSESSMENT-TO-WORK.md`: which reported audit risk each milestone addresses.
- `OPUS-ORCHESTRATOR.md`: bounded campaign dispatcher prompt for use *after* choosing and reviewing a milestone.

## Quick start

1. Read `RUNBOOK.md` and `MANIFEST.json`.
2. In the **current** `prismatic-majordomus` checkout, give Opus 5.5 `prompts/00-control-...md` first. Use the actual path listed in `MANIFEST.json`; filenames are intentionally descriptive.
3. Require it to discover current repository policy, reproduce baseline defects, reconcile `.ai/` intent/milestone/issues, and show exact proofs. Do not issue all 14 prompts concurrently.
4. Continue according to the dependency DAG only when the prior acceptance gates are verified, **not** when a Claude session reports “done.”

## Campaign boundaries

No new generic orchestration, new providers, hosted platform, model catalog, website redesign or speculative mesh extensions as part of recovery. Existing functionality is not automatically disposable. First make it optional, truthful, independently verifiable and cheap to adopt.

## Caveat

This archive is an **implementation prompt specification**, not proof that the targeted code has been written, tested or deployed. Real adoption requires real external humans. Forge ruleset enforcement requires independently observable configuration. Provider hook/version details must be revalidated when implementing.
