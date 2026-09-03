# Economics

Short, because v0.1 measures nothing and says so.

## The claim we refuse to make

Majordomus does not reduce token spend by any percentage. No benchmark exists, so no
number appears anywhere in this repository. Every routing document studied while
designing this tool carried figures like "35 % better" with no measurement behind
them, and every one of those figures was later found to be invented.

## What v0.1 controls without measuring

The policy and profiles fix four things that, in the environments studied, were the
main sources of waste:

| Lever | Waste it addresses | How v0.1 handles it |
|---|---|---|
| always-loaded budget | every session re-reading a 1,000-line contract | hard line cap, `doctor` fails over it |
| profile effort | maximum reasoning as the default | effort is a profile field; escalation is a recorded event, not a habit |
| profile verbosity | narrating every intermediate step | verbosity and presentation are profile fields, separate from effort |
| scope and finish contract | duplicated and abandoned work | one task per checkout, overlap reported, `finish` refuses unverified completion |

These are declared controls. Their effect is not measured by v0.1.

## What would have to be true to measure

The unit that matters is **cost per accepted outcome**, not tokens. To compute it,
each `task.finished` ledger record would need, from the provider:

- input, output, and cached tokens for the session
- model and effort actually used
- elapsed time and tool-call count

and, from the person: whether the outcome was accepted, and whether rework followed.

The ledger already records the task, profile, outcome, contract result, and
verification command with its exit code and duration. The provider-side fields are the
missing half. v0.3 in the roadmap adds them only for providers that expose them
honestly; any `estimated_` field stays out of enforcement and out of comparisons.

## The comparison this would enable

Once measured, the question is not "which model uses fewer tokens" but which profile
produces the best accepted-outcome rate per unit cost for a task class. That
comparison, across a team's real work, is the durable value. Nothing in v0.1 pretends
to have it.
