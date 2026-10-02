# Source snapshot: observed current repository

The uploaded forensic snapshot was inspected before this pack was written.

## Git state observed

Verified short HEAD:

```text
29a5879
```

Recent history includes active M003 session/knowledge work, including:

```text
0db0501 feat(knowledge): discover sources from the repository index, never by walking the tree
34cd249 feat(session): open an execution episode, and refuse to open a second
ef65f95 feat(session): close an episode into an envelope, attributed by a ledger stamp
29a5879 chore(state): record why the enforcement gap was taken outside the plan
```

The snapshot was not clean. Observed changes:

```text
M .majordomus/state/current.yaml
M .majordomus/state/ledger.jsonl
M site/data/generated/source.json
?? .majordomus/state/checkpoints/20260904T175335Z--master--29a5879--de9a34e2f0b40ee0.md
```

Treat current operational records as transient state. Never reset or discard
unknown user changes blindly in the live repository.

## Current scale

Observed in the snapshot:

```text
25 doctrine declarations
63 issue contracts
10 milestone records
4 execution profiles
4 reusable prompt assets
48 behavioral test case scripts
```

The repository has extensive path coupling to `.majordomus/`; the migration is
therefore a behavioral refactor, not a directory rename.

## Current ownership

### Repository-specific `.majordomus/`

Currently contains all of these mixed concerns:

```text
.majordomus/
├── policy.yaml
├── profiles/
├── project/
├── prompts/
├── providers/
├── templates/
├── generated/
└── state/
```

### Tool distribution

Currently primarily lives under:

```text
bin/
lib/
share/
test/
```

`share/` includes:

```text
share/doctrines.yaml
share/knowledge-sources.yaml
share/allow/
share/skeleton/
share/applications.yaml
share/use-cases.yaml
```

The current `bin/majordomus` already resolves its libraries relative to its own
binary directory, which is compatible with a read-only/global/package install.

## Current generated-provider model

Current `update` roughly does:

```text
.majordomus/policy.yaml
+ .majordomus/profiles/
+ .majordomus/providers/body.md
+ .majordomus/providers/<provider>.tmpl
        ↓
AGENTS.md / CLAUDE.md / ...
        ↓
.majordomus/generated/fingerprints.yaml
```

The target model keeps deterministic provider adapters/projection protection
where useful but removes normative rule duplication from generated provider
files.

## Current doctrine model

`share/doctrines.yaml` is currently the machine enforcement registry. Each rule
declares fields including:

```text
id
title
class
principle
summary
validator
category
enforced_by
policy_key
exit_code
claims
test
```

`doctor` verifies the wiring chain, including validator existence, dispatcher
reachability, failure propagation, test evidence, and CI wiring.

DO NOT lose this guarantee during rule migration.

## Current knowledge direction

The repository has already deliberately adopted:

```text
discover canonical knowledge sources from Git
NOT recursive filesystem walking
```

This invariant must carry into `.ai/` discovery. An LLM may traverse a logical
manifest/dependency graph. Majordomus MUST NOT define repository context as an
unbounded recursive walk of `.ai/**`.

## Current project model

`.majordomus/project/` contains canonical milestones/issues. Status and
execution order are derived from declared dependencies. Do not introduce stored
status fields during migration.

## Current continuity model

The repository already has sessions, checkpoints, handovers, decisions,
questions, ledger, current task, and context assembly.

The target changes the ownership of these operational records to
`.ai/local/state/`, which is machine/checkout-local and Git-ignored. This is an
explicit contract change approved by the operator.
