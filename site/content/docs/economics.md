+++
title = "Economics"
description = "the claim it refuses to make, what v0.1 controls without measuring, where the cost actually is, what the ledger alone can measure, and what honest measurement would take"
weight = 33
[extra]
source = "docs/ECONOMICS.md"
+++

{% raw %}

Majordomus does not state a token-saving percentage it has not measured, and this document
contains none. The numbers are in [`generated/economics.md`](https://github.com/korczis/prismatic-majordomus/blob/master/docs/generated/economics.md), written
by `majordomus generate economics` from recorded runs, and on the site's economics page, which
renders the same generated data. Typed into prose, a number about tokens, context or cost is
refused by `majordomus economics check`.

## The question

For the same task, from the same repository state, with the same model, the same tools and
the same acceptance tests: how many tokens does a coding session consume with Majordomus
installed and without it?

The unit is tokens per **successfully completed** task. A run that saves tokens by failing is
not a saving, so a pair of runs counts only when both passed every success gate.

## What is measured, and how each number is labelled

Every number the subsystem produces carries one of five classes, and a number computed from
several inputs is never stronger than the weakest of them:

<div class="overflow-x-auto" tabindex="0">

| class | meaning |
|---|---|
| observed | returned by the provider or its harness at run time and recorded unmodified |
| counted | computed deterministically from bytes on disk with a named, pinned tokenizer |
| derived | arithmetic over observed or counted values |
| estimated | a number standing in for one nobody measured; never compared, never published |
| counterfactual | what a mechanism would have avoided, modelled rather than observed; never a saving |

</div>


Two measurements exist, and they answer different questions:

- **Total-token reduction** (`effective_token_reduction`, derived from observed usage).
  Matched live runs of a task corpus, control against treatment. This is the only kind of
  number that could support a claim about what Majordomus saves.
- **Context selection** (`context_reduction_ratio`, derived from counted tokens). For every
  issue of this repository's plan, what the context compiler selected out of what it judged
  relevant. No model is involved. It is *not* total savings, and every surface says so.

## Control and treatment

The canonical definitions are in
[`.ai/repo/benchmarks/economics/methodology.yaml`](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/benchmarks/economics/methodology.yaml).

- **Control, without Majordomus.** The fixture repository as a competent team keeps it: a
  README, a short `CLAUDE.md` pointing at the conventions, conventions and decision records in
  `docs/`, a green test suite. Claude Code runs headless with the model the suite names, its
  built-in tools and permissions, and no MCP server, user settings or skills.
- **Treatment, with Majordomus as a user installs it.** The same fixture after
  `majordomus init`, `majordomus update` (the bootstrap added to the fixture's own `CLAUDE.md`
  as a generated region) and `majordomus capture install --provider claude-code`, committed.
  Everything else is identical. The MCP server and the git hooks are deliberately excluded
  from this first treatment.

Both arms are isolated from the operator's own configuration in the same way, so neither
inherits instructions, connectors or memory the other lacks.

## The task corpus

Seven tasks in [`.ai/repo/benchmarks/economics/tasks/`](https://github.com/korczis/prismatic-majordomus/tree/master/.ai/repo/benchmarks/economics/tasks),
each standing for a development pattern this repository sees: a defect whose fix is written
down in a decision record, a small additive feature, a change across four modules and a
persisted format, a suite broken by a rename where a convention decides the right fix, a
versioned storage change, a user-reported regression, and a feature built across two
sessions where the second starts from what the first left.

Each task names its fixture, an optional setup patch, a verify command, **hidden** acceptance
tests copied in only after the last session, and a reference solution.
`majordomus economics references` proves, with no model involved, that every hidden test fails
on the task's starting state and passes on its reference — so a failed run says something
about the agent, not about the test.

## From runs to a statement

1. `majordomus economics run --suite pilot` runs every task in both arms and records one file
   per run: the provider's usage per request and per model, as reported (message by message,
   deduplicated), the harness's own cost projection, tool calls, and the verdict of each
   success gate. Nothing a session said or read is recorded. A recorded run is evidence and
   is never written over: a run already recorded is skipped, and `--force` is refused. A run
   that should not count is excluded in the methodology with a reason, and another
   repetition is recorded in its place.
2. The calculator pairs control and treatment runs of the same task and repetition, checks
   that they are comparable (suite version, the model requested and the models that
   answered, fixture, harness version, configuration, revision, session count) and valid
   (both succeeded, both report usage), and names the reason for every pair that is not.
3. Per valid pair: `reduction = 1 - treatment / control`. Every per-pair and aggregate token
   figure on every surface is this reduction: positive means Majordomus used fewer tokens,
   and a negative reduction means Majordomus used more tokens. It is reported as it is, and
   no surface flips its sign.
4. Across pairs: the distribution (median, quartiles, 95th percentile, mean, standard
   deviation) and a percentile bootstrap interval of the median with a recorded seed, and the
   same by task category, session count and complexity. The repetitions of one task are not
   independent, so the interval resamples whole tasks rather than single pairs; each
   interval names the method it was computed with.
5. The verdict: a quantitative total-token claim is allowed only when every threshold of the
   methodology's publication rule holds — enough valid pairs, enough task categories with
   enough pairs each, enough repetitions of every task in each arm, a high enough valid-pair
   rate, a narrow enough interval, current evidence, and no run from a dirty tree. The rule
   is evaluated for the primary metric over all the evidence, so only that metric, unfiltered,
   can be verified. Below that, every surface states that no verified total-token-savings
   claim is available, beside the preliminary observation labelled as preliminary.

No run is excluded unless the methodology names it with a reason, and the primary metric is
then shown with and without it (`effective_token_reduction.including_excluded`).

## When evidence goes stale

Every record carries a digest of the files its suite depends on: the methodology, the corpus,
the fixture, and the parts of Majordomus that shape what a treatment session is given (the
skeleton `init` writes, the provider templates, the briefing and handover code). When any of
them changes, the evidence is reported as **stale**, and a claim bound to it loses its
guarantee until the suite is run again. Evidence recorded under another methodology version is
**incompatible** and never pooled.

## Reproducing it

```bash
majordomus economics references         # every task can be failed and passed; no model
majordomus economics measure            # the context suite; deterministic, no model
majordomus economics run --suite pilot  # live sessions; needs a Claude Code login, spends usage
majordomus economics summary            # every figure, recomputed from the recorded runs
majordomus economics explain effective_token_reduction
majordomus economics check              # refuse an unsupported claim
```

The same answers are served at `GET /api/v1/economics`, `/api/v1/economics/explain`,
`/api/v1/economics/runs` and `/api/v1/economics/check`, by the MCP tools
`majordomus_economics*`, and on the Cockpit's Economics page. All of them are projections of
one calculator, `apps/majordomus-cli/src/economics`.

## What this does not measure

- A large repository. The fixture is small by design, so that verifying a run takes seconds;
  context selection matters most where there is much to select from, and that is not measured.
- Other harnesses and models than the ones a suite names. Provider counts are never compared
  across providers.
- The MCP server, mesh coordination between parallel workers, and the finish contract's effect
  on rework. Each would be its own treatment.
- Money. Cost is the harness's own projection at run time, not a bill, and historical runs are
  never re-priced.
- A corrected treatment. The treatment is Majordomus as a user who installs only the shell
  tool gets it, and its generated bootstrap advises commands that installation cannot run
  (`majordomus worktree`). What a session spends on them is part of what that user gets, and
  it is measured as such, not corrected for.
- The corpus as it is now. The context suite counts the corpus at the revision it recorded,
  not at the current one; a later change to the plan or to the files it selects from moves
  nothing until the suite is measured again.

## The hypotheses

The benchmark was built to test hypotheses recorded in the methodology before any evidence
existed. They are hypotheses, and the generated report lists them as such beside whatever the
evidence shows.
{% endraw %}
