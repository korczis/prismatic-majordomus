+++
title = "Completed work advances the version by at least the cadence over the trunk, a larger contract requirement wins, a refused completion advances nothing and a retry advances nothing again"
description = "A change set that carries work and is completed, or merged into the trunk, leaves the version"
weight = 40
[extra]
claim_id = "accepted-work-advances-the-version"
status = "guaranteed"
source = "docs/claims/accepted-work-advances-the-version.md"
+++
{% raw %}

## What it means

A change set that carries work and is completed, or merged into the trunk, leaves the version
at least one cadence step above the version the trunk declared, and at least as high as the
public contract requires. On a repository whose cadence is a minor, internal work on `1.4.0`
ends at `1.5.0`, a compatible addition also at `1.5.0`, and a breaking change on `1.x` at
`2.0.0`, because the contract's major is larger than the cadence's minor. A completion that is
refused advances nothing, and finishing or advancing again after the version was advanced
writes nothing.

## How it works

`majordomus release obligation` measures the change set against the trunk it is integrated
into: every changed path is classified as a projection (`merge=derived`), publication evidence
(a release record), the writer's own version advance, or work. Only work owes the cadence. The
minimum is the greatest of the trunk's version, the contract's floor (ADR 0051) and the trunk
raised by the cadence (ADR 0106). `finish --outcome completed` judges its whole contract
first; only when nothing else is unmet does it run `release advance`, which writes the minimum
through the one writer `release bump` uses, re-reads the obligation, and records
`release.advanced` before `task.finished`. An obligation that already holds is not written
again, so idempotence is a property of the predicate rather than of a counter.

## How to see it

```bash
majordomus release obligation            # state, minimum, what the change set carries, and why
majordomus release advance --dry-run     # what the advance would write
majordomus finish --outcome completed --verify-command "..."   # judges, then advances once
```

## What it does not cover

Advancing the declared version is not a release: it writes no release record, creates no tag
and publishes no binary; those remain the release pipeline's, after publication. The end of a
provider's episode is not a completion and advances nothing. History before the merge that
introduced this decision is not judged.

## Why it exists

The contract measurement is silent about most work, so a trunk could take a hundred fixes and
stay on one version while the version stopped naming what was served. A bump in a hook or a
workflow would have counted boundaries instead of work, and advanced several times for one
change.
{% endraw %}
