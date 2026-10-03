# Prompt 09 — Governance Preflight, Rules/Doctrines/Policies, and Done Invariants

## Mission

Make governance executable and visible.

`AGENTS.md` / `CLAUDE.md` must remain generated bootstraps. The canonical normative state stays under `.ai/` according to repository policy.

## Effective governance view

Cockpit must let a developer inspect:
- effective rules for repository/path/task;
- dependencies among rules;
- doctrines;
- policies;
- applicable workflows;
- ADRs;
- resolved README/context chain;
- use cases impacted;
- violations;
- remediation;
- provenance.

## Preflight

Implement/reuse one canonical governance preflight run before substantive mutation.

It should evaluate, as applicable:
- context loaded/resolved;
- active task/mandate;
- scope declared;
- peer announcement;
- overlap/collision;
- worktree topology;
- branch state;
- relevant rules/doctrines;
- impacted use cases;
- generated artifact ownership;
- required tests/docs obligations.

The result is typed and exposed through every relevant transport.

Do not rely on a paragraph in `AGENTS.md` as enforcement.

## "Done" invariant

Create/reuse a canonical finish/check report that answers explicitly:

- is all work from this task/session implemented?
- tested?
- regression-tested?
- documented?
- generated artifacts updated?
- OpenAPI/Swagger updated automatically?
- CLI/API/MCP/Cockpit parity satisfied?
- GH Pages updated/generated?
- version bump required/performed?
- changelog required/performed?
- committed?
- pushed?
- integrated/merged?
- CI green?
- deployed where applicable?
- deployment verified?
- stale branch/worktree/PR left behind?
- handover/session context updated?
- issue/milestone status updated?

Each answer needs evidence, not a boolean conjured by UI.

## Enforcement

Wire preflight/check/finish into:
- local canonical quality command;
- relevant git hooks;
- CI;
- agent task lifecycle;
- Cockpit status.

Avoid making routine read-only inspection unusably slow.

## Bootstrap regeneration

If governance changes require `AGENTS.md` or `CLAUDE.md` text:
1. edit canonical policy/rule source;
2. regenerate;
3. verify both generated files;
4. add drift tests;
5. update docs.

Never edit generated bootstrap files as canonical fixes.

## Acceptance

Introduce tests that deliberately violate:
- scope;
- worktree;
- missing tests;
- missing docs;
- generated drift;
- missing transport parity;
- stale peer claim;
- incomplete finish obligations.

Verify actionable, deterministic failures.
