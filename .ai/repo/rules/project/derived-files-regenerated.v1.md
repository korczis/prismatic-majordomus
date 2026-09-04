---
id: project.derived-files-regenerated
version: 1
kind: rule
title: Derived files are regenerated, never edited
description: A generated file is changed by changing its canonical source and rerunning the generator, and both are committed together.
statement: A generated file is changed by changing its canonical source and rerunning the generator, and both are committed together.
status: active
class: blocking
depends_on: []
tags: [derived]
---

# Rationale

A hand edit to a generated file is the two-rulebooks failure in miniature; docs/GITHUB_PAGES_ARCHITECTURE.md lists what is generated and CI refuses a tree whose derived files are stale.

# Required behaviour

A generated file is changed by changing its canonical source and rerunning the generator, and both are committed together.

# Failure behaviour

No command decides this rule; a reviewer does, and a change that violates it is not merged. Where a behavioural case covers part of it, that case is named below.

# Verification

Review. scripts/generate-site-data --check runs in CI; test/cases/51_derived_artifacts_committed.sh proves the committed derived files match their sources; the projection_integrity rule covers the generated instruction files.
