# README, AGENTS, provider adapters, and projections

## Human-to-agent discovery

Target chain:

```text
README.md
   ↓ mentions
AGENTS.md
   ↓ backlink to README.md
   ↓ MUST read
.ai/README.md
   ↓ manifest/protocol
.ai/repo/...
```

`README.md` remains primarily human-facing.

`AGENTS.md` becomes a tiny provider-neutral bootstrap, not a duplicate policy
document.

## AGENTS.md target responsibility

It should approximately communicate:

```text
- read README.md for human project context
- read .ai/README.md before substantive work
- follow .ai discovery and rule-loading protocol
- load mandatory effective rules/dependencies
- load task-relevant skills/knowledge
- do not implicitly load .ai/local/**
- if Majordomus tooling is vendored under .majordomus/, treat it as external/read-only unless explicitly working on Majordomus itself
```

Do not embed profile tables, finish-contract prose, lifecycle manuals, or the
entire rule corpus in AGENTS.

## Provider-specific files

`CLAUDE.md`, `GEMINI.md`, and other provider-specific entrypoints are thin
bootstrap adapters only.

Shared normative content belongs under `.ai/`.

Provider files may contain only:

- native include/import mechanics,
- unavoidable provider-specific loading instructions,
- pointer to `.ai/README.md`,
- optionally a pointer to `AGENTS.md`.

No provider file may have a unique repository rule that does not exist in the
portable layer.

## Projection engine

Do not discard deterministic projection infrastructure merely because the
generated output becomes smaller.

`majordomus update` may continue to manage provider bootstrap files and regions.

However its inputs must move from old project `.majordomus/` paths to `.ai/`
plus distribution adapters.

## Projection provenance / fingerprints

This is a special case.

The current tracked fingerprint file helps distinguish:

```text
legitimate regeneration after canonical input changed
vs
manual edit of generated projection
```

Do NOT casually move this to an ignored local cache because a fresh clone would
lose previous-generation provenance.

Acceptable solutions:

### Option A — retain minimal tracked projection provenance

Move/reshape it into a clearly derived tracked `.ai/repo` metadata location and
document why it is tracked.

### Option B — self-describing generated targets

Embed sufficient generated-body hash/provenance in managed headers/regions to
prove whether the managed content was hand-edited, eliminating the external
fingerprint file.

Prefer B if it can be done without destabilizing the transformation. Otherwise
use A first and leave a clean follow-up issue.

Whatever solution is chosen MUST retain current hand-edit refusal behavior and
be behaviorally tested across a fresh clone.

## `init`

New `majordomus init`:

- finds Git repository root,
- creates/extends `.ai/`,
- seeds tracked repo context,
- seeds vendored baseline rules,
- creates no meaningful local state until needed,
- ensures `.ai/local/` is ignored,
- installs/seeds thin provider bootstraps or tells `update` to do so,
- never creates repository project-data under `.majordomus/`,
- never edits `.envrc` silently,
- never installs the Majordomus executable.

## Installation independence

Do not make generated files depend on the absolute installation path.

Current hook guidance that embeds a specific binary path should be reviewed.
Prefer repository-agnostic invocation (`majordomus ...`) where appropriate, or
an explicit configured executable if the project intentionally pins a local
installation.

`doctor` should validate actual wiring rather than assume `.majordomus/bin`.

## Old-layout migration collision

Legacy repositories may already have:

```text
.majordomus/policy.yaml
```

while the new world may use:

```text
.majordomus/bin/majordomus
```

as an optional tool installation.

Migration logic MUST distinguish old project-data layout from new optional tool
distribution.

Suggested detection:

```text
old project layout:
  .majordomus/policy.yaml exists

new vendored tool layout:
  .majordomus/bin/majordomus and distribution markers exist
```

If both old project data and a new tool distribution appear in the same path,
fail closed with a clear migration diagnostic rather than overwrite either.
