# OPERATOR RUNBOOK | MJ-IDD-002

## Unpack and verify
Place this pack in a scratch path OUTSIDE tracked source, or under a dedicated existing prompt-pack registry if the repository already implements one. Do not silently overwrite `.ai` rules or replace AGENTS.md. Run `python3 tools/validate_pack.py` from the unpacked pack root. It checks schema structure, mission graph and referenced files; it is not an implementation test.

## Single Claude Code session
1. Open the repository root in Claude Code; inspect its native allowed permissions and status.
2. Give Claude the absolute path to this extracted pack and paste the `prompts/MASTER.md` content, or include the prompt file using a supported local file reference.
3. Tell it: 'Read manifest and each referenced mission only when its dependencies are ready; repository governance overrides assumptions. Perform preflight, execute one verified vertical slice, continue through safe independent work.'
4. Follow implementation evidence in the repo and actual deployed system, not synthetic Markdown status.

## Four concurrent sessions (only if mesh coordination works)
Open separate sessions/worktrees. Each session receives the SAME master prompt plus `runbooks/COOPERATION.md`. One coordinator performs preflight and leases tasks; the remaining workers ask the verified mesh for non-overlapping assignments. If mesh is not functional, use explicit nonconflicting worktrees and a single human-/repo-authorized integration coordinator rather than pretending the agents cooperate.

## Suggested slice order
A. UI invokes existing executor and live stream. B. Intent editing/materialization. C. Cross-peer scheduling/recovery. D. Interactive evidence-linked execution graph. E. PR/test/deploy closure. Each iteration also updates rules, docs, generated artifacts and tests in the same PR.

## How to judge output
Require actual source locations, executed checks with exit results, current SHA, screenshots/browser tests if available, traceable canonical IDs, and verified deployment signals. Failed check is FAILED, missing test is UNVERIFIED. No invented speedups. Use `schemas/evidence.schema.json` for machine-readable run reports.

## Safe reruns
Re-open manifest and persisted canonical mission state; inspect git and live sessions. Re-evaluate task leases and stale evidence. Idempotent tasks should safely no-op; mutations should carry keys/version constraints. Never blindly replay pushes, merges or deployment actions.
