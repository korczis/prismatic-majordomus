---
id: project.hot-path-reads-once
version: 1
kind: rule
title: A command reads each canonical file at most once
description: Within one invocation a command flattens, hashes or parses a canonical file at most once, however many validators, accessors or projections need it; the second reader gets the first reading.
statement: One invocation, one reading per canonical file; the work counters are the proof, and a count that exceeds the number of distinct files read is a defect.
status: active
class: advisory
depends_on: [project.derived-once@1]
tags: [performance, doctrine]
---

# Rationale

`project.derived-once` says what to compute once per state version. This rule is the
per-invocation form of it, which is where the regressions actually appear: a validator
that flattens a policy the loader already flattened, an accessor that re-reads a record per
field, a projection that hashes a source the discovery pass hashed. Each is correct and
each is invisible until the counters are read. The tool counts its readings precisely so
that this rule has a number to be checked against.

# Required behaviour

A command loads what it needs in a loader and hands the reading on: the policy, the
profiles, the registries, the plan model, the context documents, the knowledge sources are
each read once and expanded afterwards. A validator reads through the loader, never
through the file. `MJ_TIMING=1` prints the `yaml_flatten`, `yaml_flatten_many`,
`sha256_many` and `git` counts of the invocation; the sum of files those readings cover is
at most the number of distinct canonical files the command needs.

# Failure behaviour

No command decides this rule; a reviewer does, with the counters. Case
`test/cases/78_flatten_once.sh`, when it exists, holds the counts of `doctor` and `watch`
on the tool's own repository to the number of distinct canonical files they read, and a
change that reads a file twice fails it.

# Verification

`MJ_TIMING=1 bin/majordomus doctor 2>&1 >/dev/null | grep count`, before and after a
change to a loader, a validator or an accessor.
