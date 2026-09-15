# Autonomous Claude Code execution prompt

You are responsible for completing the Majordomus repository transformation
defined by `tmp/transform/`.

This is an implementation task, not a design discussion.

## 0. Load the transformation contract

Determine the Git repository root and read:

```text
tmp/transform/README.md
tmp/transform/00_EXECUTIVE_BRIEF.md
tmp/transform/01_SOURCE_SNAPSHOT.md
tmp/transform/02_DECISIONS_AND_INVARIANTS.md
tmp/transform/03_TARGET_ARCHITECTURE.md
tmp/transform/04_MIGRATION_MAP.yaml
tmp/transform/05_AI_PROTOCOL.md
tmp/transform/06_RULES_FORMAT.md
tmp/transform/07_STATE_MODEL.md
tmp/transform/08_BOOTSTRAP_AND_PROVIDER_MODEL.md
tmp/transform/09_MIGRATION_DAG.md
tmp/transform/10_ACCEPTANCE_CRITERIA.md
tmp/transform/11_TEST_MATRIX.md
tmp/transform/12_COMPATIBILITY_ROLLBACK.md
tmp/transform/13_OPEN_DECISIONS.md
```

Then inspect the current repository in depth before mutation.

The pack contains operator-approved target architecture. Existing generated
`AGENTS.md`, `CLAUDE.md`, `.majordomus/` path conventions, provider body, and
docs are pre-migration evidence. Where they conflict with this pack's target
layout, migrate them.

Do not weaken existing behavioral guarantees.

## 1. Work autonomously

Do not ask the operator questions that can be answered from:

- this pack,
- repository docs,
- existing tests,
- Git history,
- current implementation.

When a minor unspecified choice exists, choose the smallest reversible option
consistent with the invariants and record it under:

```text
tmp/transform/_run/DECISIONS.md
```

Only stop if there is a genuinely irreconcilable contradiction that risks data
loss or violates a non-negotiable invariant.

## 2. Preserve evidence

Before changing files:

```bash
git status --short --branch
git log --oneline -20
bin/majordomus doctor
bin/majordomus plan validate
bash test/run.sh
```

Capture outputs in:

```text
tmp/transform/_run/BASELINE.md
```

`tmp/` is ignored; do not commit the run log.

Do not reset/stash/delete unrelated work blindly. Inspect any dirty files and
preserve them.

## 3. Use the migration DAG

Execute `09_MIGRATION_DAG.md` in order.

Use focused commits per phase.

After each phase run the narrowest relevant behavioral tests plus `doctor`
where possible.

Do not continue past a red invariant merely because later phases might
accidentally make it green.

## 4. Centralize path semantics first

A primary technical goal is to eliminate the current overloaded concept of
`MJ_DIR = <repo>/.majordomus`.

Introduce separate concepts for:

```text
tool distribution root
.ai root
.ai/repo
.ai/local
policy
profiles
prompts
project
state
tool share/schemas/skeleton
```

Do not spread fallback path conditionals across every command.

## 5. New repository contract

By the end:

```text
.ai/
    portable project AI layer

.ai/repo/
    tracked canonical repository context

.ai/local/
    ignored local state

.majordomus/
    optional read-only tool installation only, never required
```

`majordomus init` creates/extends `.ai/`.

It does not install Majordomus and does not silently edit `.envrc`.

## 6. Migrate the current Majordomus repository itself

This repository dogfoods Majordomus.

Move its current repository customization from legacy `.majordomus/` into
`.ai/` according to the migration map.

Do NOT move root product implementation (`bin/lib/share/test`) under
`.majordomus/`.

The Majordomus source repository should not need a nested `.majordomus/`
installation.

## 7. Local state is intentionally untracked

Move operational state to `.ai/local/state/`.

Ensure:

```gitignore
.ai/local/
```

Preserve same-checkout records during migration.

Update behavioral claims to stop implying that local session/task state is
distributed by Git.

Maintain worktree overlap behavior using Git worktree discovery and each
worktree's local state.

## 8. Rule transformation is structural but strict

Create portable rule objects with YAML frontmatter.

Convert all current doctrine entries.

Preserve blocking/advisory behavior and all doctrine wiring guarantees.

Use `x-majordomus` for enforcement-specific fields.

Create a distribution standard rule package and vendor a pinned copy into this
repository's `.ai/repo/rules/vendor/majordomus/`.

Do not silently update vendored baseline when Majordomus executable changes.

Do not implement rule overrides in v1.

## 9. Provider bootstrap transformation

Make `AGENTS.md` small.

It must:

- point back to README,
- point to `.ai/README.md`,
- instruct deterministic rule/context discovery,
- exclude `.ai/local/**` from implicit context.

Provider-specific files remain thin.

The current monolithic provider body ceases to be normative after its content
has been classified into rules/protocol/workflows.

Preserve deterministic generation and region ownership.

## 10. Knowledge

Continue the current M003 direction rather than replacing it.

Knowledge sources are declared and discovered through Git/canonical sources.

No recursive semantic filesystem walking.

No vector DB, embeddings, inferred semantic edges, or unrelated new storage
subsystem.

Use `.ai/repo/knowledge/` for repository source declarations/curated knowledge
and `.ai/local/cache/` for rebuildable local products where appropriate.

## 11. Migration support for consumers

Implement explicit safe legacy migration.

Do not confuse:

```text
legacy .majordomus/policy.yaml
```

with:

```text
new optional .majordomus/bin/majordomus
```

Ambiguous mixed layouts fail closed.

Provide preview/dry-run if possible.

Preserve unknown files and create a recoverable backup for ignored local state.

## 12. Documentation and generated surfaces

Update every affected source of truth.

Regenerate derived site/docs data with existing generators.

Do not edit generated pages as the fix.

Review `docs/CLAIMS.yaml` and `docs/RESPONSIBILITIES.yaml` explicitly and update
claims/evidence links when semantics changed.

## 13. Search aggressively for stale coupling

Before finalizing:

```bash
rg '\.majordomus/(policy|profiles|project|prompts|providers|state|generated)' \
  --hidden \
  -g '!tmp/**' \
  -g '!.git/**' \
  -g '!node_modules/**'
```

Review every result.

Remaining references are allowed only for:

- legacy migration,
- optional tool installation,
- explicit historical docs,
- negative/compatibility tests.

No production reader may depend on old repository project-data paths.

## 14. Final evidence

Run the complete available verification suite:

```bash
bash test/run.sh
bin/majordomus doctor
bin/majordomus plan validate
git diff --check
```

Run site checks/generation required by the repository.

Run shellcheck where the repository expects it.

Record final evidence under:

```text
tmp/transform/_run/FINAL.md
```

## 15. Final report

At completion provide a concise report containing:

1. final architecture implemented,
2. migration behavior,
3. rule/doctrine migration,
4. state semantics change,
5. provider/bootstrap change,
6. tests added/changed,
7. commands run and actual results,
8. any deliberately deferred item and why,
9. commit list,
10. `git status`.

Do not claim a guarantee unless executable evidence proves it.

Keep Majordomus focused. Do not turn this task into a generic agent framework,
remote policy service, semantic memory platform, or provider runtime.
