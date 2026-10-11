# Majordomus Mesh + Prompt-Pack Lifecycle — Opus/Fable Prompt Pack

**Type:** implementation input, not finished software. **Target:** `korczis/prismatic-majordomus`. **Date:** 2026-10-10. **Pack schema:** self-described version 1.0.0 (not a claim of supported Majordomus parser schema).

## Purpose
Turn uploaded prompt packs and existing plans into a single durable, cooperative artifact-to-outcome pipeline: upload → provenance → reconciliation → intent/milestones/issues → mesh → PRs → release → verifiable evidence → `majordomus ui`.

## How to run
1. Open the latest checkout in Claude Code Opus/Fable.
2. Paste the content of `MASTER.md` and provide this extracted directory as readable context.
3. Require the agent to inspect current HEAD, not treat this prompt as current implementation facts.
4. The agent should ingest/reconcile M00–M14 through actual Majordomus mechanisms where available, otherwise use existing supported planning facilities until import is built.
5. Execute M00 first, then dependency-ready milestones, checkpointing actual work. You should not need to repeat `proceed` once a durable controller exists.
6. Do not automatically execute files just because they were uploaded. This archive contains instructions for an authorized operator/agent.

## File layout
- `MASTER.md` overall operational prompt
- `milestones/Mxx-*.md` one milestone per file, seven issue-level work packages each (105 total)
- `contracts/` explicit execution, source mapping, UX and acceptance contracts
- `manifest.json` structured file inventory with content hashes and dependency hints
- `INDEX.md` deterministic milestone routing

## Important scope distinctions
- This pack is an external *desired outcome* specification, not proof that any feature exists.
- Earlier project snapshots may differ from current checkout; re-inspect source and tests.
- Proposed CLI commands and schema records are target UX, not guaranteed current commands.
- Site update claims must be verified against live behavior.
- CI <60s is a stretch SLO for incremental changes, not a permission to skip tests.

## Tracking rule
A source segment may link to multiple criteria and an issue may serve several criteria. All associations need immutable source-revision provenance. Never compute progress as an unverified count of closed issues.
