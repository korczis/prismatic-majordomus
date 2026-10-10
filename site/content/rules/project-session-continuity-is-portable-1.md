+++
title = "Session continuity is portable, and what is portable is checked"
description = "Session continuity is portable, and what is portable is checked"
weight = 145
[extra]
kind = "rule"
slug = "project-session-continuity-is-portable-1"
identity = "project.session-continuity-is-portable@1"
status = "active"
source = ".ai/repo/rules/project/session-continuity-is-portable.v1.md"
+++
{% raw %}

## Rationale

The machine is where work runs, not what owns it. Before ADR 0105 a handover lived in one
checkout's `.ai/local/state/`, named that machine, and reached another only through a live
mesh link; a person who stopped on a laptop and opened the repository on a desktop started
from memory. Making it portable is only safe if what is portable is decided by a check rather
than by care: a handover is prose, prose carries whatever its author pasted, and a record
read on another machine is input from somewhere else.

## Required behaviour

1. `continuity publish` refuses a record carrying a credential of a known shape, the value
   of a secret environment variable of the publishing process, a path of a home directory,
   or a repository path that is absolute or escapes the repository; the refusal names the
   field and never repeats the value, and the store is left untouched.
2. Every read of a store admits each record — bounds, schema, id against content and file
   name, signature against the device key, repository identity, portability — and reports
   every refusal as a diagnostic. A sync never merges a forged, foreign or leaking record
   into the local tree.
3. A plan decides trust, lineage and source compatibility before a resume writes anything; a
   resume proceeds only on `ready` or `ready_with_warnings`. Divergence is decided by parent
   links; separate lines are never reported as a conflict.
4. A handover from a dirty tree is resumed with `source_incomplete`, never as fully ready, and
   no uncommitted content reaches the other machine.
5. Commands in a record or a plan are recommendations; nothing is executed.

## Failure behaviour

A refused publication exits 10 and writes nothing. A plan with blockers and a resume that did
not proceed exit 10 with the blockers and the commands that resolve them. An unreachable
remote exits 10 and leaves pending records pending.

## Verification

The cases named in `x-majordomus.tests`: the two-machine run (821), dirty source and source
compatibility (822), divergence (823), separate work (824), offline (825), secrets (826),
admission of tampered, newer, foreign and untrusted records (827).
{% endraw %}
