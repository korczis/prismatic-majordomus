+++
title = "No pull request can be merged into the default branch while the verdict status is red, because a branch rule requires it"
description = "The ci status computed by ci-verdict would be a *required* status check on"
weight = 131
[extra]
claim_id = "ci-verdict-is-required"
status = "planned"
source = "docs/claims/ci-verdict-is-required.md"
+++
{% raw %}

## What it means

The `ci` status computed by [ci-verdict](ci-verdict.md) would be a *required* status check on
`master`, so a pull request whose gates did not pass could not be merged. That is what makes a
gate a gate rather than a report.

**It is not true today**, and this page exists so that the gap is visible rather than implied
by the claim next to it. Measured on 2026-09-11:

```
$ gh api repos/korczis/prismatic-majordomus/branches/master/protection
404 Branch not protected
$ gh api repos/korczis/prismatic-majordomus/rulesets
[]
```

No branch protection, no ruleset. Every gate in this repository is **advisory at the merge**.

## How it works

It does not, yet. Closing this is a repository-settings act an operator takes on GitHub — a
branch rule or a ruleset on `master` that requires the `ci` status — and not a change to any
file in this tree. There is nothing to implement here: `scripts/ci/verdict` already computes
the status, the `ci` job already runs with `if: always()` so the status is never left pending,
and `test/cases/26_ci_wiring.sh` already proves the job needs every job of the model. What is
missing is the setting that consumes it.

## How to see it

```bash
gh api repos/korczis/prismatic-majordomus/branches/master/protection   # 404 Branch not protected
gh api repos/korczis/prismatic-majordomus/rulesets                     # []
```

## What it does not cover

Requiring the status would not make the trunk green; it would stop the trunk going red by a
merge, which is a different and narrower thing. It also would not close the derived-data hole
`docs/CI.md` describes — a server-side merge runs no `pre-commit` hook, and the gate that
checks currency reports after the merge rather than before it — though a required status is
the mechanism that would let such a check refuse.

## Why it exists

Three consequences this repository has actually paid for trace back to this one absence. A red
trunk persists, because nothing refuses the merge that reddens it. Derived data goes stale
across a server-side merge, because the only guard is a client-side hook that the server-side
merge never runs. And a green run reads as "nothing could object" when what it means is
"nothing objected" — a verdict about a subject nothing was obliged to consult.

The claim beside this one asserted the branch rule as a fact for as long as it has existed. It
was not softened to make it defensible and it was not deleted to improve a count: the true part
stayed `guaranteed`, and the part that is not true is recorded here as `planned` — "specified
and not implemented, named so the omission is visible", which is what this matrix defines that
status to mean. A promise nobody keeps is worse when nobody can see it is unkept.
{% endraw %}
