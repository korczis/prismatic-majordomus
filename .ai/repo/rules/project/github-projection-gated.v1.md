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
  tests: [test/cases/97_github_gate.sh, test/cases/45_github_projection.sh]
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

# Required behaviour

- A gate reads the remote and fails on drift. It runs on every change that can move the
  canonical model or the adapter, which the CI model selects by path class, not by
  somebody adding it to a list of steps.
- The states that mean new drift, or that a person's text is at stake, are refused from the
  first run and have no tolerance: `behind`, `edited`, `conflict`, `unmanaged`, `state`,
  `milestone`, `closed`.
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
count and the baseline. It exits 12 when it cannot read the remote, with the reason.

# Verification

`test/cases/97_github_gate.sh` proves each half against a fixture remote, with no network:
a record whose canonical text has moved fails the gate, a record added to the model and
never projected fails the ratchet, a backlog at its baseline passes, and the gate is
declared in `.ai/repo/ci/gates.yaml` for the path classes that can move either side.

`test/cases/45_github_projection.sh` proves the six states the gate reads.
