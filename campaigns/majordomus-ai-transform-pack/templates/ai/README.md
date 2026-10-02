# Repository AI context

This `.ai/` directory is the provider-neutral AI context and governance layer
for this repository.

It is designed to remain understandable without Majordomus installed.

## Bootstrap

Before substantive planning, implementation, review, or repository mutation:

1. read this file,
2. read `manifest.yaml`,
3. load the registered repository context under `repo/`,
4. resolve mandatory rules and their dependencies,
5. load only task-relevant skills and knowledge,
6. never implicitly load `local/`.

## Ownership

```text
repo/   tracked repository-specific canonical context
local/  machine/checkout-local state; Git ignored
```

`local/` is not normative repository policy and must not be implicitly added to
model context.

## Rules

Read `repo/rules/README.md` before interpreting rule files.

Vendored rules under `repo/rules/vendor/` are pinned repository dependencies and
must not be edited directly.

Repository-specific rules live under `repo/rules/project/`.

## Majordomus

Majordomus may validate, resolve, project, migrate and enforce this layer, but
the `.ai/` format is repository-owned and provider-neutral.

A `.majordomus/` directory, if present, is only an optional installation of the
Majordomus tool and is not repository AI state.
