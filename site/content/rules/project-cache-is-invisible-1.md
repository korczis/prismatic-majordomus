+++
title = "A cache never changes what a command observes"
description = "A cache never changes what a command observes"
weight = 63
[extra]
kind = "rule"
slug = "project-cache-is-invisible-1"
identity = "project.cache-is-invisible@1"
status = "active"
source = ".ai/repo/rules/project/cache-is-invisible.v1.md"
+++
{% raw %}

## Rationale

A cache is a second source of truth, which is exactly the thing this repository refuses.
It is admitted only under conditions that make it unobservable: keyed by what it derives
from, so that no edit to canonical state can be answered with a stale reading; bypassable,
so that a doubt can be settled by running without it; proved equivalent, so that the claim
of being invisible is a test rather than a belief. The batch reads of `project.derived-once`
removed the need for a cross-process cache in the shell tool, and no such cache exists in
it at the time of writing; the rule binds the day one is added.

## Required behaviour

A cache key is a content hash of the canonical inputs it derives from, or the commit and
tree state that determine them, never a timestamp alone. Every command that can read the
cache accepts a way to ignore it (an environment switch or an option, documented in
`docs/CLI.md`). A case runs every read-only command of `share/commands.yaml` once with the
cache cold, once warm and once bypassed, and compares stdout, stderr findings and exit code
byte for byte. A mutating command never reads a cache for the state it mutates. The cache
lives under `.ai/local/` and is never tracked.

## Failure behaviour

No command decides this rule until a cache exists; the equivalence case is the decision
then, and a cache without it is not merged.

## Verification

Review, and the equivalence case once a cache exists.
{% endraw %}
