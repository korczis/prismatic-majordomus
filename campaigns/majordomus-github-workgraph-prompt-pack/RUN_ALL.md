---
id: github-workgraph-run-all
kind: orchestration-prompt
target: claude-code-fable-1m
---

# RUN ALL — Majordomus GitHub Work Graph Campaign

You have a large context window. Use it deliberately.

Read every file in this prompt pack, beginning with `00_SHARED_CONTRACT.md`.

Then execute phases `01` through `13` in order against `~/dev/prismatic-majordomus`.

## Operating rules

- Do not merely summarize the prompts.
- Do not stop after the audit.
- Do not ask me to choose obvious implementation details.
- Use the repository's own task/worktree/session lifecycle.
- Respect effective `.ai/` context and path-specific rules.
- Announce work to the Majordomus coordination layer if available.
- Continue phase-to-phase while gates pass.
- If a phase uncovers an architectural blocker, fix it before continuing.
- If a later phase falsifies an earlier assumption, repair the implementation and rerun affected gates.
- Keep one coherent campaign branch/worktree unless repository policy requires issue-per-branch decomposition; if it does, follow that policy and preserve handovers.
- Commit logical completed phases according to existing repository commit/versioning policy.
- Never push secrets.
- Never silently force destructive GitHub mutations.
- Prefer read-only observation and dry-run plans before remote changes.
- Remote GitHub application must be verified after mutation.
- Keep default tests offline/deterministic.
- Use real GitHub state only when credentials and repository lifecycle safely permit it.
- Use current local repository state as truth about the implementation; public GitHub may lag.

## Context discipline

At the start:
1. load AGENTS and `.ai/` discovery protocol;
2. resolve context for paths you expect to touch;
3. inspect current branch/worktree/session/handover;
4. inspect recent changes related to GitHub/work graph;
5. build an internal architectural map.

Before each phase:
- reread that phase prompt;
- reread shared invariants relevant to it;
- inspect phase handover;
- resolve any newly touched path context.

Do not waste the 1M context by reading the entire repository indiscriminately. Build a dependency-shaped context:
- canonical governance/docs;
- domain models;
- capability framework;
- Git/worktree;
- provider/GitHub;
- API/MCP;
- Cockpit;
- tests/generation/CI.

## Progress protocol

At each phase boundary, record:
- what invariant is now true;
- tests proving it;
- any migration performed;
- generated outputs;
- unresolved risks;
- next phase.

If the repository has a canonical session checkpoint/handover mechanism, use it. Do not create a competing `STATUS.md`.

## Final requirement

You are finished only when Phase 13's acceptance checklist is evidenced by the repository.

The desired outcome is not "GitHub integration works".

The desired outcome is:

> Majordomus models work once, derives readiness and completion from evidence, projects/reconciles GitHub explicitly, traces outcomes to code and code back to outcomes, exposes the same semantics through every supported surface, and mechanically refuses drift/bypass.
