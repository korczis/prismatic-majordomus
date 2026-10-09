---
id: project.session-continuity-is-portable
version: 1
kind: rule
title: Session continuity is portable, and what is portable is checked
description: A handover reaches another machine as a signed, admitted record; secrets and machine-local values never travel; a resume validates trust, lineage and source before it restores anything.
statement: "Durable continuity does not depend on the machine that wrote it: a handover is published as a record carrying only portable state — no credential, no secret environment value, no path of a developer's disk, no process or socket — and is refused, not redacted, when it would carry one. Every record is admitted (schema, id, signature, repository identity, portability) before anything reads it. A resume restores state only on a plan that validated the signer's trust, the record's lineage and the local source's compatibility, decides order by lineage and never by a clock, and executes nothing a record says."
status: active
class: blocking
depends_on: [project.diagnostics-decide-the-exit@1]
tags: [continuity, handover, devices, security]

x-majordomus:
  tests:
    - test/cases/821_a_session_moves_between_machines.sh
    - test/cases/822_a_dirty_handover_restores_context_not_source.sh
    - test/cases/823_the_same_work_continued_twice_is_a_conflict.sh
    - test/cases/824_separate_work_on_two_machines_is_not_a_conflict.sh
    - test/cases/825_a_handover_published_offline_waits_for_its_remote.sh
    - test/cases/826_a_handover_carrying_a_secret_is_not_published.sh
    - test/cases/827_a_record_is_admitted_before_it_is_read.sh
---

# Rationale

The machine is where work runs, not what owns it. Before ADR 0105 a handover lived in one
checkout's `.ai/local/state/`, named that machine, and reached another only through a live
mesh link; a person who stopped on a laptop and opened the repository on a desktop started
from memory. Making it portable is only safe if what is portable is decided by a check rather
than by care: a handover is prose, prose carries whatever its author pasted, and a record
read on another machine is input from somewhere else.

# Required behaviour

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

# Failure behaviour

A refused publication exits 10 and writes nothing. A plan with blockers and a resume that did
not proceed exit 10 with the blockers and the commands that resolve them. An unreachable
remote exits 10 and leaves pending records pending.

# Verification

The cases named in `x-majordomus.tests`: the two-machine run (821), dirty source and source
compatibility (822), divergence (823), separate work (824), offline (825), secrets (826),
admission of tampered, newer, foreign and untrusted records (827).
