# Cost per accepted outcome will be reported from measured data only

## What it means

**Planned.** The unit that matters is tokens, cost and time per accepted outcome — not per prompt — computed from measured telemetry and the ledger's outcomes. A task run under the right profile may spend more on one prompt and less on the task, because it needed two sessions instead of three and no retry.

## How it works

Nothing is implemented; it depends on telemetry landing first. The ledger already carries outcome and verification per task, so once tokens and cost are recorded the comparison is a query over one file.

## How to see it

There is nothing to run.

## What it does not cover

No percentage of any kind is claimed anywhere on this site or in the repository. None has been measured, so none is stated — that rule is checked by the site build, which fails on a number in the marketing copy.

## Why it exists

"Saves tokens" is the wrong metric and the easiest to fake. Cost per accepted engineering outcome is the number a team can act on, and it can only be stated after twenty to fifty real tasks have been measured.
