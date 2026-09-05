+++
title = "A cycle, a self-dependency and a dependency on an issue that does not exist are each refused by name"
description = "The dependency graph is validated, not trusted. Each malformed shape produces its own finding with the issue that caused it: cycle, self_dependency, unknown_dependency, duplicate_dependency, unknown_milestone, premature_execution. A failing finding makes majordomus plan validate and majordomus doctor exit non-zero; majordomus watch reports the same violation as drift."
weight = 91
[extra]
claim_id = "dag-validation"
status = "guaranteed"
source = "docs/claims/dag-validation.md"
+++
{% raw %}

## What it means

The dependency graph is validated, not trusted. Each malformed shape produces its own finding with the issue that caused it: `cycle`, `self_dependency`, `unknown_dependency`, `duplicate_dependency`, `unknown_milestone`, `premature_execution`. A failing finding makes `majordomus plan validate` and `majordomus doctor` exit non-zero; `majordomus watch` reports the same violation as drift.

## How it works

`lib/project.awk` removes issues whose dependencies have all been removed, one layer at a time. Issues that never leave are in a cycle or downstream of one, and those are the issues the finding names. Edge errors are detected while the edge list is built, so an unknown dependency is reported as an unknown dependency rather than becoming an invisible cycle.

## How to see it

```bash
majordomus plan validate
# FAIL cycle       graph — a dependency cycle prevents these issues from ever becoming ready: I0006 I0007
```

## What it does not cover

The graph is validated for shape, not for judgement. An edge that nobody needed is a valid edge; the tool will happily serialise work that could have run in parallel.

## Why it exists

A dependency list without validation is a suggestion. A cycle nobody detects is a set of issues that can never become ready, with no message explaining why.
{% endraw %}
