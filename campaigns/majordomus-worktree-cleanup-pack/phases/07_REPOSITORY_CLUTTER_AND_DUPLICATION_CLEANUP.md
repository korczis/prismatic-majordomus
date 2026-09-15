# Phase 07: Repository Clutter, Legacy Duplication, and Structural Hygiene

## Goal

Remove non-worktree sediment exposed by the campaign, but only where canonical ownership is proven.

This is not a general rewrite of the repository.

## 1. Revisit clutter inventory

Use phase 01 findings and post-integration reality.

Classify each candidate as:

- canonical and keep,
- legacy duplicate and remove,
- generated and should be regenerated/relocated,
- temporary and remove,
- local-only and add/fix ignore policy,
- migration residue and remove,
- unknown and preserve,
- should move under canonical `.ai/**` or `.majordomus/**` structure,
- should be replaced by typed/generated tooling.

## 2. Root hygiene

Audit root-level files/scripts for accidental permanence.

For every non-obvious root file ask:

- is it part of canonical public repository interface?
- is it duplicated elsewhere?
- can it be represented by existing Rust CLI/tooling?
- is it generated?
- does a rule/doctrine require a different home?

Do not mechanically move everything into `.majordomus`; follow existing architecture and avoid breaking public entrypoints.

## 3. Session/context/handover duplication

If multiple legacy session/context directories exist:

- identify canonical current mechanism from repository rules,
- map legacy content to canonical form,
- preserve valuable historical state,
- migrate only what is meaningful,
- remove duplicate active mechanisms,
- ensure future agents have one discoverable entry path.

Do not invent a third session system.

## 4. Generated artifacts

For generated files:

- identify canonical generator,
- regenerate from source,
- compare output,
- delete stale duplicate generated trees,
- add drift validation if missing and justified.

Never hand-edit generated output to "make the diff smaller".

## 5. Temporary reports and patches

Campaign recovery artifacts are temporary evidence until phase 11.

Do not delete them yet unless copied to the canonical handover/recovery location and proven unnecessary.

Old unrelated temp files can be removed once provenance is clear.

## 6. Ignore policy

Update `.gitignore` or equivalent only for genuine recurring generated/local artifacts.

Do not hide legitimate source files just because they are inconveniently untracked.

## 7. Dead compatibility shims

Remove shims only if all consumers have migrated and tests prove no required interface remains.

Prefer deletion over keeping two ways to do the same thing forever.

## Acceptance criteria

- root/repository clutter is materially reduced,
- canonical ownership is clearer than before,
- no valuable historical/session state is silently lost,
- duplicate active mechanisms are removed,
- generated artifacts have one generator/source,
- ignore rules match reality,
- repository gates remain green.
