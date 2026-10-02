# Acceptance criteria

The transformation is complete only when all relevant criteria have executable
evidence.

## A. Installation / ownership

- [ ] Majordomus can execute from outside the repository.
- [ ] Managed repository behavior does not require `.majordomus/`.
- [ ] Optional `.majordomus/` tool checkout/submodule can be read-only.
- [ ] No mutable repository project state is written into the tool distribution.
- [ ] `majordomus init` creates/extends `.ai/`, not project-data `.majordomus/`.
- [ ] `init` does not silently modify `.envrc`.

## B. `.ai` portability

- [ ] `.ai/README.md` explains the format without requiring Majordomus.
- [ ] `.ai/manifest.yaml` declares format version and registered sections.
- [ ] `.ai/repo/**` is tracked canonical repo context.
- [ ] `.ai/local/**` is ignored.
- [ ] `.ai/local/**` is not implicitly loaded/published/projected.
- [ ] Fresh clone can understand the shared AI contract without Majordomus installed.

## C. Canonical data migration

- [ ] Policy lives at `.ai/repo/policy.yaml`.
- [ ] Profiles live at `.ai/repo/profiles/`.
- [ ] Reusable repo prompts live at `.ai/repo/prompts/`.
- [ ] Project milestones/issues live at `.ai/repo/project/`.
- [ ] All commands use centralized new path resolution.
- [ ] No stored project status field is introduced.

## D. Local state

- [ ] Operational state lives at `.ai/local/state/`.
- [ ] Current task, ledger, checkpoints, handovers, sessions, questions and local decisions still function.
- [ ] `.ai/local/prompts/` is defined as raw local user-prompt history but Majordomus does not falsely claim automatic capture.
- [ ] `.ai/local/session-contexts/` is bounded/local and non-authoritative.
- [ ] Worktree scope-overlap behavior still works.
- [ ] Fresh clone starts with no local operational state by design.

## E. Rules

- [ ] Standard Majordomus rules exist as portable Markdown rule objects.
- [ ] Each rule has deterministic ID/version/metadata.
- [ ] Repository baseline is vendored under `.ai/repo/rules/vendor/majordomus/`.
- [ ] Vendor package has provenance/version manifest.
- [ ] Project rules have a separate namespace/location.
- [ ] No implicit override system exists in v1.
- [ ] Missing rule dependency fails.
- [ ] Rule dependency cycle fails.
- [ ] Duplicate/ambiguous effective rule identity fails.
- [ ] `doctor` proves validator/dispatcher/failure/test/CI wiring from rule objects.
- [ ] Reverse validator-without-rule detection still exists.
- [ ] Baseline executable upgrade does not silently change repo effective rules.

## F. Bootstrap/providers

- [ ] README points humans/tools to AGENTS.
- [ ] AGENTS backlinks to README and points to `.ai/README.md`.
- [ ] AGENTS contains no duplicated normative rule corpus.
- [ ] CLAUDE/GEMINI/provider files contain only thin bootstrap/provider mechanics.
- [ ] A provider-specific repo rule cannot exist only in one adapter.
- [ ] `update` remains deterministic.
- [ ] Region projection mode remains safe.
- [ ] Hand-edit protection remains behaviorally proven.

## G. Knowledge

- [ ] Repository knowledge discovery remains declared/Git-index driven.
- [ ] No recursive filesystem walk defines semantic knowledge.
- [ ] Canonical docs/code are referenced, not copied into `.ai` solely for retrieval.
- [ ] Rebuildable knowledge caches are local unless explicitly justified otherwise.
- [ ] No embeddings/vector DB/semantic inferred edges are added by this transformation.

## H. Migration

- [ ] Legacy repository layout is detected.
- [ ] Migration does not confuse old project `.majordomus/` with optional new tool installation.
- [ ] Ambiguous mixed layout fails closed.
- [ ] Migration has preview/dry-run or equivalent review surface.
- [ ] Migration preserves unknown files.
- [ ] Migration preserves existing state in the same checkout.
- [ ] Migration is idempotent or clearly reports already migrated.
- [ ] Post-migration `doctor` passes.

## I. Evidence/documentation

- [ ] Existing claims are updated to the new ownership/durability semantics.
- [ ] Every new public capability claim has behavioral evidence.
- [ ] Site/data projections derive from canonical sources.
- [ ] No generated site file is manually patched as the canonical fix.
- [ ] `bash test/run.sh` passes.
- [ ] `doctor` passes.
- [ ] `plan validate` passes.
- [ ] `git diff --check` passes.

## J. Stale path audit

There must be no live production dependency on:

```text
.majordomus/policy.yaml
.majordomus/profiles/
.majordomus/project/
.majordomus/prompts/
.majordomus/providers/
.majordomus/state/
.majordomus/generated/
```

Remaining strings may exist only for:

- migration code/tests,
- legacy documentation,
- historical evidence,
- optional `.majordomus/` tool installation references.

Every remaining occurrence should be reviewed manually.
