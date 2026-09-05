---
id: project.cache-is-invisible
version: 1
kind: rule
title: A cache never changes what a command observes
description: A cache of derived state, if one exists, is keyed by the version of the canonical state it derives from, is bypassable, and produces byte-identical output to the uncached path; a cache that can be observed through a command's output, exit code or findings is a defect, and a cache that no case proves equivalent does not exist.
statement: Cache only what is semantically safe, key it by the state version, prove it byte-identical over every read-only command, and keep a switch that turns it off.
status: active
class: advisory
depends_on: [project.derived-once@1, project.no-claim-without-test@1]
tags: [performance, doctrine]
---

# Rationale

A cache is a second source of truth, which is exactly the thing this repository refuses.
It is admitted only under conditions that make it unobservable: keyed by what it derives
from, so that no edit to canonical state can be answered with a stale reading; bypassable,
so that a doubt can be settled by running without it; proved equivalent, so that the claim
of being invisible is a test rather than a belief. The batch reads of `project.derived-once`
removed the need for a cross-process cache in the shell tool, and no such cache exists in
it at the time of writing; the rule binds the day one is added.

# Required behaviour

A cache key is a content hash of the canonical inputs it derives from, or the commit and
tree state that determine them, never a timestamp alone. Every command that can read the
cache accepts a way to ignore it (an environment switch or an option, documented in
`docs/CLI.md`). A case runs every read-only command of `share/commands.yaml` once with the
cache cold, once warm and once bypassed, and compares stdout, stderr findings and exit code
byte for byte. A mutating command never reads a cache for the state it mutates. The cache
lives under `.ai/local/` and is never tracked.

# Failure behaviour

No command decides this rule until a cache exists; the equivalence case is the decision
then, and a cache without it is not merged.

# Verification

Review, and the equivalence case once a cache exists.
