# Portable rule-object format

## Goal

Replace the semantic split between:

```text
provider/body prose
share/doctrines.yaml
```

with portable first-class rule objects while preserving machine enforcement.

## Storage

Majordomus distribution source:

```text
share/standard/rules/
├── manifest.yaml
└── *.md
```

Repository vendored baseline:

```text
.ai/repo/rules/vendor/majordomus/
├── manifest.yaml
└── rules/
    └── *.md
```

Repository-specific rules:

```text
.ai/repo/rules/project/
└── *.md
```

## v1 rule metadata

Use YAML frontmatter compatible with the project's intentionally restricted YAML
subset.

Example:

```markdown
---
id: majordomus.scope-integrity
version: 1
kind: rule
title: Scope integrity
description: A task may only be accepted when touched files remain inside its declared scope.
statement: Work outside the claimed scope is not accepted as completed work.
status: active
class: blocking
depends_on: []
tags: [scope, verification]

x-majordomus:
  validator: scope
  category: scope
  enforced_by: [check, finish, watch]
  policy_key: scope_respected
  exit_code: 10
  claims: [scope-enforcement, scoped-task]
  tests: [test/cases/04_start_check.sh]
---

# Rationale

...

# Required behavior

...

# Failure behavior

...

# Verification

...
```

## Required generic fields

For v1 require at least:

```text
id
version
kind
title
description
statement
status
class
depends_on
```

Where:

```text
kind = rule
status = active | deprecated
class = blocking | advisory
```

Preserve current blocking/advisory semantics. Do not create a severity ladder.

## IDs and versions

Identity comes from frontmatter, not filename.

Recommended IDs:

```text
majordomus.scope-integrity
project.english-only
project.data-driven
```

Version is an exact integer in v1.

Dependencies use exact versioned references, e.g.:

```yaml
depends_on:
  - majordomus.state-consistency@1
```

Do not implement semver ranges during this transformation.

## Dependency graph

The rule resolver MUST:

- reject missing dependencies,
- reject duplicate IDs at the same effective scope,
- reject cycles,
- produce deterministic topological order,
- report source path and vendor/project provenance,
- never silently select an arbitrary version.

## Baseline + project semantics

For transformation v1:

```text
effective set = vendor baseline + project rules
```

No project rule may silently disable or weaken a vendored baseline rule.

If contradictions are detected and cannot be ordered safely, fail closed with a
clear diagnostic.

Do not build override/federation policy yet.

## Majordomus extension

Generic readers may ignore unknown `x-*` fields and still understand the rule.

Majordomus reads `x-majordomus` to bind rules to validators, commands, claims,
tests, and exit behavior.

The current doctrine-wiring `doctor` guarantee MUST be recreated against these
rule objects.

Expected chain remains:

```text
rule declared
→ validator exists
→ named commands dispatch it
→ blocking failure propagates non-zero
→ named behavioral test exists
→ CI actually runs the test
```

Also preserve reverse validation:

```text
enforcement validator
→ declared by an effective rule
```

## Vendor manifest

Example:

```yaml
vendor: majordomus
package: majordomus-standard-rules
version: 1
format: ai-rules/v1
source_revision: <git revision or release identifier>
rules:
  - id: majordomus.scope-integrity
    version: 1
    file: rules/scope-integrity.v1.md
```

The repository's vendored package is authoritative for that repository.

An installed newer executable may report a newer baseline but MUST NOT silently
activate it.

## Vendor update

Implement or prepare a clear explicit operation, preferably along lines of:

```text
majordomus rules vendor status
majordomus rules vendor diff
majordomus rules vendor update
```

Exact CLI naming may follow existing command conventions, but:

- update is explicit,
- diff is reviewable,
- write is atomic,
- hand edits in vendor are detected/refused,
- project rules are never overwritten.

If adding a new public command would create excessive migration scope, a
minimal explicit `majordomus update --vendor-rules` path is acceptable, but
document the compromise. Do not silently update on ordinary `update`.
