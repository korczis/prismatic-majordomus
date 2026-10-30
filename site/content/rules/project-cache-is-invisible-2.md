+++
title = "A cache never changes what a command observes"
description = "A cache never changes what a command observes"
weight = 89
[extra]
kind = "rule"
slug = "project-cache-is-invisible-2"
identity = "project.cache-is-invisible@2"
status = "active"
source = ".ai/repo/rules/project/cache-is-invisible.v2.md"
+++
{% raw %}

## What changed in version 2, and why

Version 1 was written before any cache existed and placed every cache under `.ai/local/`,
which is one checkout's ignored state. On 2026-10-22 the first cache that is not a reading of
the repository arrived: `majordomus mesh build-cache ensure` keeps the result of a dependency
install (`npm ci`, `mix deps.get`) and of a cargo build directory as archives, keyed by the
lockfile, the toolchain and the platform rather than by any checkout (ADR 0157, decisions 8 and
13; I3952). Its whole value is that every checkout of a machine — and, through the store, every
machine — reuses one entry. Kept under one checkout's `.ai/local/` it would be rebuilt once per
worktree, which is the cost it exists to remove.

So version 2 admits a second place, on the conditions that make a shared cache as safe as a
private one: it lives under the user's cache directory (`~/.cache/majordomus/`), it is bounded
by a number the policy declares, and a committed command measures it and reduces it
(`project.accumulation-is-measured`). Everything else version 1 required stands, and it now
names the files a cache leaves as part of what must not change.

## Rationale

A cache is a second source of truth, which is exactly the thing this repository refuses.
It is admitted only under conditions that make it unobservable: keyed by what it derives
from, so that no edit to its inputs can be answered with a stale entry; bypassable, so that a
doubt can be settled by running without it; proved equivalent, so that the claim of being
invisible is a test rather than a belief. An entry that cannot be proved to be the one that was
saved — its digest does not match, an entry would land outside its target — is a miss, never a
partial answer: the uncached path runs instead.

## Required behaviour

A cache key is a content hash of the canonical inputs it derives from, or the commit and tree
state that determine them, never a timestamp alone. Every command that can read the cache
accepts a way to ignore it (an environment switch or an option, documented in `docs/CLI.md`):
`MAJORDOMUS_CAPABILITY_CACHE=off` for the executor's cache, `MAJORDOMUS_BUILD_CACHE=off` for the
build cache. A case runs each cached command once with the cache cold, once warm and once
bypassed, and compares what it observes — stdout, findings and exit code for a reading, the file
list and the content of every file for an install — byte for byte; a marker the cache writes to
name the key in place is the one file such a comparison leaves out, and the case says so. A
mutating command never reads a cache for the state it mutates.

A cache of one checkout lives under `.ai/local/` and is never tracked. A cache shared by every
checkout of a machine lives under `${XDG_CACHE_HOME:-~/.cache}/majordomus/`, never inside a
checkout; its bound is declared in the policy, enforced when an entry is added (oldest entries
go first), and measured and reduced by `scripts/reap-orphans`. An entry fetched from another
machine is restored only after the digest recorded when it was saved matches.

## Failure behaviour

No command decides this rule; each cache's equivalence case is the decision for that cache,
and a cache without one is not merged. A cache whose entry fails its digest or its safety check
costs time and never correctness: the command it stood in for runs.

## Verification

The equivalence case of each cache: `test/cases/3952_a_dependency_install_is_restored_and_saved.sh`
for the build cache (cold, warm and bypassed installs leave the same tree; a corrupt archive and
an escaping entry are misses).
{% endraw %}
