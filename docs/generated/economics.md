<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the token-economics methodology and every benchmark run committed under .ai/repo/benchmarks/economics; regenerate with `majordomus generate`
     Generator: majordomus-cli 0.9.0 -->
# Token economics: benchmark report

Generated from recorded evidence by `majordomus generate economics`. Every figure below is computed by one calculator (`apps/majordomus-cli/src/economics`) from the raw runs under `.ai/repo/benchmarks/economics/runs/`; nothing here is typed by hand.

## Verdict

No verified total-token-savings claim is available: no valid matched pair of live runs is recorded under methodology 1.

The publication rule is not met:

- 0 valid pair(s); the rule asks for 30
- 0 categor(ies) with at least 5 valid pairs; the rule asks for 4
- 0 repetition(s) of csv-credit-notes (baseline) in suite pilot, the least repeated; the rule asks for 3 of every task and variant
- no interval: too few valid pairs, or too few tasks among them

## Question and methodology

For the same task, from the same repository state, with the same model, the same tools and the same acceptance tests: how many tokens does a coding session consume with Majordomus installed and without it?

Methodology version 1; primary metric `effective_token_reduction`. The methodology is `.ai/repo/benchmarks/economics/methodology.yaml`.

### Control and treatment

- **Without Majordomus** (control, `baseline`): The fixture repository as a competent team keeps it: a README, a short CLAUDE.md pointing at the conventions, the conventions and decisions in docs/, a green test suite. The session runs Claude Code headless with the same model, the same built-in tools and permissions, no MCP server, no user settings, no skills.
  Excludes: the .ai layer, generated provider bootstraps, Claude Code hooks, session briefing, handovers, peer board, MCP server.
- **With Majordomus as a user installs it** (treatment, `majordomus`): The same fixture after `majordomus init`, `majordomus update` and `majordomus capture install --provider claude-code`, committed: the .ai layer, the Majordomus bootstrap added to the fixture's own CLAUDE.md as a generated region (the policy's region mode, which is how a repository with instruction files of its own adopts it) and generated AGENTS.md and GEMINI.md, and the SessionStart, UserPromptSubmit, SessionEnd and PreCompact hooks that brief each session and record its boundaries. Everything else is identical to the control.
  Excludes: the MCP server, git pre-commit and pre-push hooks.

### When a total-token claim may be published

At least 30 valid matched pairs; at least 4 task categories with 5 valid pairs each; at least 3 repetitions of every task and variant; at least 8000 in 10000 attempted pairs valid; a bootstrap interval no wider than 2000 in 10000; evidence current; no run recorded from a working tree with uncommitted changes.

## Evidence

| suite | kind | freshness | runs | valid pairs | attempted | control failed | treatment failed | both failed | other | revisions | harness | models |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| context | context | no evidence | 0 | 0 | 0 | 0 | 0 | 0 | 0 |  |  |  |
| pilot | live | no evidence | 0 | 0 | 0 | 0 | 0 | 0 | 0 |  |  |  |


## Metrics

| metric | value | status | class | n | interval | what it is not |
|---|---|---|---|---|---|---|
| `effective_token_reduction` | not measured | not measured | derived from observed | 0 | — | not context reduction: this is everything the provider reported for the whole task, not what one mechanism selected |
| `cost_reduction` | not measured | not measured | derived from observed | 0 | — | not a bill: the harness prices usage from its own table at run time, and historical runs are never re-priced |
| `tool_call_reduction` | not measured | not measured | derived from observed | 0 | — | not a token metric |
| `continuation_reduction` | not measured | not measured | derived from observed | 0 | — | only the session that continued the work, not the whole task |
| `first_request_overhead` | not measured | not measured | derived from observed | 0 | — | not the whole overhead: what the treatment reads later because its instructions say to is in the total, not here |
| `completion_rate.baseline` | not measured | not measured | derived from observed | 0 | — | not a quality score: a run passes the gates or it does not |
| `tokens_per_completed_task.baseline` | not measured | not measured | observed | 0 | — |  |
| `transcript_resume_avoided.baseline` | not measured | not measured | counterfactual from observed | 0 | — | not a saving of Majordomus: both arms start the next session fresh; this is the modelled cost of resuming a transcript instead |
| `completion_rate.majordomus` | not measured | not measured | derived from observed | 0 | — | not a quality score: a run passes the gates or it does not |
| `tokens_per_completed_task.majordomus` | not measured | not measured | observed | 0 | — |  |
| `transcript_resume_avoided.majordomus` | not measured | not measured | counterfactual from observed | 0 | — | not a saving of Majordomus: both arms start the next session fresh; this is the modelled cost of resuming a transcript instead |
| `context_reduction_ratio` | not measured | not measured | derived from counted | 0 | — | not total token savings: it says what the compiler selected from what it found relevant, not what a session consumed, and a session remains free to read anything |

Formulas and warnings:

- `effective_token_reduction`: median over valid pairs of 1 - treatment_total_tokens / control_total_tokens, where total tokens are all input (uncached, cache write, cache read) and all output the provider reported for every session of the run.
- `cost_reduction`: median over valid pairs of 1 - treatment_cost / control_cost, where cost is the harness's own total_cost_usd at run time.
- `tool_call_reduction`: median over valid pairs of 1 - treatment_tool_calls / control_tool_calls.
- `continuation_reduction`: median over valid multi-session pairs of 1 - the treatment's last-session total tokens / the control's.
- `first_request_overhead`: median over valid pairs of the treatment's first-request input tokens minus the control's: the instruction files and hook output a session is given before it has done anything.
- `completion_rate.baseline`: runs that passed every success gate / runs recorded, over the runs of pairs whose two sides were both recorded, are comparable and are not excluded; a failed run counts, which is the point.
- `tokens_per_completed_task.baseline`: median over the runs of valid pairs of the total tokens the provider reported for the run, so that both arms describe the same tasks.
- `transcript_resume_avoided.baseline`: over the runs of valid pairs, the previous session's last-request input minus the next session's first-request input: what resuming the previous transcript would have re-sent at the start, against what a fresh session was given.
  - counterfactual: modelled from observed values, never a measured saving
- `completion_rate.majordomus`: runs that passed every success gate / runs recorded, over the runs of pairs whose two sides were both recorded, are comparable and are not excluded; a failed run counts, which is the point.
- `tokens_per_completed_task.majordomus`: median over the runs of valid pairs of the total tokens the provider reported for the run, so that both arms describe the same tasks.
- `transcript_resume_avoided.majordomus`: over the runs of valid pairs, the previous session's last-request input minus the next session's first-request input: what resuming the previous transcript would have re-sent at the start, against what a fresh session was given.
  - counterfactual: modelled from observed values, never a measured saving
- `context_reduction_ratio`: median over seeds of 1 - selected_tokens / candidate_tokens, where candidates are the distinct files the compiler judged relevant to the work (selected, or left out only for budget) and selected are those it put in the budget, counted with the named tokenizer; files it judged irrelevant are reported, never divided by.

## Pairs

Every declared pair, valid or not. Tokens are the provider-reported totals of every session of the run.

| task | category | rep | status | control tokens | treatment tokens | token reduction | cost reduction | first-request overhead | reasons |
|---|---|---|---|---|---|---|---|---|---|
| csv-credit-notes | regression | 1 | missing | — | — | — | — | — | the control run is not recorded |
| csv-credit-notes | regression | 2 | missing | — | — | — | — | — | the control run is not recorded |
| currency-czk | small-feature | 1 | missing | — | — | — | — | — | the control run is not recorded |
| currency-czk | small-feature | 2 | missing | — | — | — | — | — | the control run is not recorded |
| invoice-discounts | cross-module | 1 | missing | — | — | — | — | — | the control run is not recorded |
| invoice-discounts | cross-module | 2 | missing | — | — | — | — | — | the control run is not recorded |
| recurring-invoices | multi-session | 1 | missing | — | — | — | — | — | the control run is not recorded |
| recurring-invoices | multi-session | 2 | missing | — | — | — | — | — | the control run is not recorded |
| storage-v2 | schema-change | 1 | missing | — | — | — | — | — | the control run is not recorded |
| storage-v2 | schema-change | 2 | missing | — | — | — | — | — | the control run is not recorded |
| test-repair | test-repair | 1 | missing | — | — | — | — | — | the control run is not recorded |
| test-repair | test-repair | 2 | missing | — | — | — | — | — | the control run is not recorded |
| vat-rounding | bug-fix | 1 | missing | — | — | — | — | — | the control run is not recorded |
| vat-rounding | bug-fix | 2 | missing | — | — | — | — | — | the control run is not recorded |

"Token reduction" is `1 - treatment / control` on total tokens, and "cost reduction" the same on the harness's cost projection: positive means Majordomus used fewer tokens, and a negative reduction means Majordomus used more tokens. "First-request overhead" is not a reduction: it is the treatment's first-request input minus the control's, so positive means Majordomus added tokens before the model acted. An excluded pair shows its reductions so that the outlier rule's effect is visible; only valid pairs enter the metrics.

0 of 14 declared pair(s) are valid.

## Hypotheses

Stated before any evidence existed, so that the evidence can refute them. They are not results.

- `long-running-work` (untested): Long-running agent development work consumes 30 to 70 percent fewer tokens with Majordomus, centred near half.
- `short-tasks` (untested): Short isolated tasks gain less and may cost more, because the bootstrap and briefing are paid on every session.
- `continuity` (untested): Work that spans sessions gains the most, because a handover replaces rediscovery.

## Reproduce

```sh
majordomus economics references        # every task fails on its start and passes on its reference; no model
majordomus economics measure            # the context suite; deterministic, no model
majordomus economics run --suite pilot  # live sessions; needs a Claude Code login and spends usage
majordomus economics summary            # recompute every figure from the recorded runs
majordomus economics explain effective_token_reduction
```

## Limitations

- One fixture repository, small by design; a large repository is where context selection matters most, and it is not measured here.
- One harness (Claude Code) and the model each suite names. Tokens are the provider's; counts from different providers are never compared directly.
- Sessions are stochastic. A pair is one sample of each arm; only the distribution over many pairs says anything.
- Cost is the harness's own projection at run time, not a bill.
- Context reduction is measured against what the compiler itself judged relevant.
- The context suite counts the corpus at the revision it recorded, not at the current one: a later change to the plan or to the files it selects from moves nothing until the suite is measured again.
- The treatment is Majordomus as a user who installs only the shell tool gets it, and its generated bootstrap advises commands that installation cannot run (`majordomus worktree`). What a session spends on them is part of what that user gets, and it is measured as such, not corrected for.
