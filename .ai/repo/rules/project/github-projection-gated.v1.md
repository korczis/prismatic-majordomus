---
id: project.github-projection-gated
version: 1
kind: rule
title: The GitHub projection is checked by a gate, not by somebody remembering to run it
description: A projection this repository claims is generated from the canonical model is proved by a gate that reads the remote and fails on drift; a detector that exists but is never called is not enforcement, and a gate that cannot reach the remote reports that it cannot rather than passing.
statement: Every claim that GitHub agrees with the canonical project model is backed by a gate that reads the remote and fails; the gate refuses new drift outright and ratchets the known backlog; and a gate that cannot reach the remote exits unusable rather than clean.
status: active
class: blocking
depends_on: [project.no-claim-without-test@1, project.derived-once@1]
tags: [projection, github, enforcement, drift]

x-majordomus:
  tests: [test/cases/97_github_gate.sh, test/cases/45_github_projection.sh, test/cases/1038_a_head_is_judged_against_what_it_changes.sh]
---

# Rationale

`scripts/github-sync --check` has exited 11 on drift since the day it was written. Nothing
ever called it.

`scripts/ci/core-check` ran the adapter's `--plan` and `--render`, which prove that a
projection can be *produced* from the canonical model. Whether the remote had *received*
one was never asked. So the projection was applied once, on 2026-09-04, and then decayed —
measured at `867f3a9`, 200 drift findings against 15 milestones and 184 issues, of which
181 were canonical records that had never existed on GitHub. Every build was green
throughout, and `.ai/repo/project/README.md` went on stating that the GitHub issues and
milestones come from `majordomus plan`.

The instructive part is which surface stayed honest. All eight projected milestones were in
sync, because milestones are few and were touched by hand. The surface a person looks at
was the surface that still agreed, which is precisely why nobody noticed for five days.

This is the repository's own stated failure mode — a claim whose evidence is a script that
exists rather than a gate that runs — applied to the one subsystem whose entire purpose is
to stop two records of the same work from disagreeing.

# Amended in place, 2026-10-10

Until this amendment the rule refused `behind`, `state`, `milestone` and `closed` on every
run, whatever the run was of. The adapter projects only the trunk and nothing applies it
automatically, so a pull request that edits a projected record was refused for the one
thing it could not do before merging: #855 changed one line of I1900 and #820 closed eight
records, and neither could go green on its own head. Had either landed, the trunk would
have been refused until somebody applied by hand. The amendment adds one class, pending,
and bounds it on both sides; nothing else the rule refused is refused less.

It amends version 1 rather than issuing a version 2 because a rule in an open pull request
(#831, `project.a-failed-read-is-not-an-empty-answer`) depends on this one `@1`, and a
dependency that does not resolve stops the whole set from applying. A version 2 carrying
this text follows once that pull request is on the trunk.

# Required behaviour

- A gate reads the remote and fails on drift. It runs on every change that can move the
  canonical model or the adapter, which the CI model selects by path class, not by
  somebody adding it to a list of steps.
- The states that mean new drift, or that a person's text is at stake, are refused from the
  first run and have no tolerance: `behind`, `edited`, `conflict`, `unmanaged`, `state`,
  `milestone`, `closed`.
- A head is judged against what it changes, not against GitHub's live state. Of those
  states, the four an apply writes (`behind`, `state`, `milestone`, `closed`) are pending,
  reported and not refused, for the records a head changed since its merge base with the
  branch it merges into, and on the trunk for the records that changed in a landing
  younger than the window declared in [`.ai/repo/ci/github.yaml`](../../ci/github.yaml). A
  record has changed when its file or its rendering differs, because a status is derived.
  Past the window the trunk is refused. The other states are never pending, and neither
  is any state on a record the head did not change.
- A change that cannot be measured is not assumed either way: with no base that resolves,
  no merge base, a shallow clone or no declared window, the gate says which and refuses
  what it refused before the amendment.
- The backlog of a projection that stopped being applied is ratcheted against a committed
  baseline rather than tolerated silently. The number may fall and may never rise, and
  writing it is a deliberate act, as it already is for
  [`.ai/repo/claim-proof-baseline.txt`](../../claim-proof-baseline.txt).
- A gate that cannot reach the remote — no `gh`, no token, no permission — exits unusable
  and says which, and never exits clean. A projection gate that skips quietly reintroduces
  the exact defect it was written to remove.
- Applying the projection stays a deliberate human act. CI proves agreement; it does not
  create issues.

# Failure behaviour

`scripts/ci/github-check` exits 10 and prints one line per refused state, each naming the
records and the command that reproduces it, and one line per ratcheted state naming the
count and the baseline. It exits 12 when it cannot read the remote, with the reason. A
pending record is one `PENDING` line naming the records and the commit they are measured
from, counted apart from the findings in the last line and never an exit status; on the
trunk past the window the same records are a refused line that names
`scripts/github-sync --apply`.

# Verification

`test/cases/97_github_gate.sh` proves each half against a fixture remote, with no network:
a record whose canonical text has moved fails the gate, a record added to the model and
never projected fails the ratchet, a backlog at its baseline passes, and the gate is
declared in `.ai/repo/ci/gates.yaml` for the path classes that can move either side.

`test/cases/45_github_projection.sh` proves the six states the gate reads.

`test/cases/1038_a_head_is_judged_against_what_it_changes.sh` proves the pending class
against a scripted `gh`: a record a head changed is pending and the run exits 0, a record
it did not change is refused beside it, a record whose rendering moved with an untouched
file is pending, a landing inside the window is pending on the trunk and the same landing
past it is refused with the remedy, and a base or a window that cannot be read refuses.
