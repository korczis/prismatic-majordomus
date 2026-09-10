+++
title = "A projection that stops being applied turns a build red, and a gate that cannot reach GitHub reports that it cannot rather than passing"
description = "The neighbouring claim — [GitHub milestones and issues are generated from the canonical"
weight = 103
[extra]
claim_id = "github-projection-gated"
status = "guaranteed"
source = "docs/claims/github-projection-gated.md"
+++
{% raw %}

## What it means

The neighbouring claim — [GitHub milestones and issues are generated from the canonical
model](github-projection.md) — is about what `scripts/github-sync` *renders* and what it
*refuses*. It says nothing about whether GitHub has actually received any of it. This claim
is the other half: that the two agree now, and that a build fails when they stop agreeing.

## How it works

`scripts/ci/github-check` reads the remote and classifies every record. It applies two
tolerances rather than one.

Refused outright, from the first run, with no allowance:

<div class="overflow-x-auto" tabindex="0">

| state | what it means |
|---|---|
| `behind` | a canonical record moved and nobody projected it |
| `edited` | a person rewrote a generated region |
| `conflict` | a person rewrote it and the record has moved since |
| `unmanaged` | a remote issue claims a canonical id this repository does not have |
| `state` | the remote's open/closed disagrees with the derived status |
| `milestone` | the remote's milestone assignment disagrees |

</div>


Ratcheted against `.ai/repo/ci/github-drift-baseline.txt`:

<div class="overflow-x-auto" tabindex="0">

| state | what it means |
|---|---|
| `missing` | a canonical record with no counterpart on GitHub |
| `adopt` | a counterpart carrying no identity marker |

</div>


The baseline may fall and may never rise. Writing it is a deliberate act, in the same idiom
[`.ai/repo/projection-baseline.txt`](../../.ai/repo/projection-baseline.txt) already uses
for the commands no capability claims.

A gate that cannot reach the remote — no `gh`, no token, no permission — exits 12 and names
which. It never exits clean.

## How to see it

```bash
scripts/ci/github-check                   # 0 in sync, 10 findings, 12 the gate cannot run
scripts/ci/github-check --write-baseline  # record today's backlog as the ceiling, deliberately
scripts/github-sync --plan                # offline: what an apply would create or change
```

## Why it is a separate claim

Because the first claim was true and green for five days while the thing it describes had
stopped happening.

`--check` has exited 11 on drift since the adapter was written. Nothing called it:
`scripts/ci/core-check` ran `--plan` and `--render`, which prove a projection can be
produced. The projection was applied once, on 2026-09-04, and then the model grew from
19 records to over 200 while GitHub kept the 19. Measured at `867f3a9`: 200 drift findings.

All eight projected milestones were in sync throughout, because milestones are few and were
touched by hand — so the surface a person looks at was the surface that still agreed. That
is what a claim citing a script that exists, rather than a gate that runs, buys you.

## What it does not cover

Applying stays a human act. CI proves agreement; it does not create issues.
`scripts/github-sync --apply` is run by a person, deliberately, and the gate's baseline is
lowered in the same commit.

Nor does the gate judge the model. It says which of the two sides moved, not whether the
canonical records describe the right work — that is the plan's business. And it decides
nothing it cannot measure: with no `gh`, no token or no permission it exits 12 naming
which, rather than a green build that read nothing.

## Evidence

`test/cases/97_github_gate.sh` drives every half over a fixture repository with a fixture
remote, offline: a record whose canonical text moved fails outright, a record added to the
model and never projected breaks the ratchet, a backlog at its baseline passes, an edited
region and an unmanaged remote issue are refused, and a gate with no baseline refuses to
guess one. `test/cases/45_github_projection.sh` proves the six states it reads.

The rule is [`project.github-projection-gated@1`](../../.ai/repo/rules/project/github-projection-gated.v1.md).
{% endraw %}
