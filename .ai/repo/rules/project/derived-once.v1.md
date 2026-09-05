---
id: project.derived-once
version: 1
kind: rule
title: Derived state is computed once per state version
description: Anything derivable solely from unchanged canonical repository state is computed once per version of that state and read many times; a command's hot path does not rediscover, reparse or rebuild it per item, per request or per call.
statement: Compute canonical truth once per state version, project it once where possible, execute only what is request-specific, and prove it with the work counters rather than with a clock.
status: active
class: advisory
depends_on: [project.derived-files-regenerated@1]
tags: [performance, doctrine]
---

# Rationale

The shell tool was slow enough to be routed around: doctor took ten seconds on every commit
because two validators ran one awk process per key inside loops over files that had been
flattened once already, the knowledge compiler hashed every source with its own process,
and the site generator built each record with one jq per record and one process per list.
None of it was an algorithm problem. Each was the same shape: work that depends only on
unchanged canonical files, redone per item. A process costs milliseconds; a thousand of
them cost the tool its users.

# Required behaviour

A loader reads its inputs once and hands the result to readers: many files by one process
(`mj_yaml_flatten_many`, `mj_sha256_many`), a flat file into variables by one process
(`mj_yload`, `mj_yload_dir`), a row set by one awk. A reader expands what was loaded; it
does not run a process per key, per row or per file. A command that repeats the same
lookup in a loop hoists it out of the loop. When a phase must read many files, it says so
once, in one process, and the work counters show it: `MJ_TIMING=1` prints, for any
command, the phases ranked by time and the `yaml_flatten`, `yaml_flatten_many`,
`sha256_many` and `git` counts, and a loop that regressed to one process per item is
visible there before it is visible on a clock.

# Failure behaviour

No command decides this rule; a reviewer does, with the counters in front of them. Case
`test/cases/78_flatten_once.sh`, when it exists, holds the counts of the commands the
pre-commit hook runs to the number of distinct canonical files they read; a change that
re-flattens inside a loop fails it.

# Verification

Review, with `MJ_TIMING=1 bin/majordomus <command>` before and after the change, and the
commit message carrying both numbers (see `project.performance-evidence`).
