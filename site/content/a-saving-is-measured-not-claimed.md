+++
title = "A saving is measured, not claimed"
description = "Fewer tokens per task is the value Majordomus is most often assumed to have, and a token-saving percentage is the easiest thing a project like it could invent. How the repository answers the question with matched runs judged by hidden tests, labels every number by how it was obtained, and publishes no saving until a rule declared before the evidence allows it."
template = "page.html"
[extra]
lang = "en"
related = ["silence-is-not-a-verdict/", "proof-carrying-documentation/", "a-doubt-is-settled-by-an-experiment/"]
[extra.translations]
cs = "uspora-se-meri-netvrdi/"
+++
{% raw %}

Fewer tokens per task is the value Majordomus is most often assumed to have. [ADR 0082](@/adrs/adr-0082.md) (proposed), dated 24 September 2026, starts from the fact that nothing had measured it: the economics document refused from its first version to state any token saving. A belief, the record says, is exactly what this repository exists to turn into a checked fact or retire.

A token-saving percentage is, its own claim page says, the most quotable thing the project could publish and the easiest to invent. Every routing document studied while designing the tool carried improvement figures with no measurement behind them. The ADR answers with one calculator that derives every number from recorded runs, labels each number by how it was obtained, and publishes no saving until a rule declared before the evidence allows it.

## Same task, two arms

The question is the one [`docs/ECONOMICS.md`](@/docs/economics.md) states: for the same task, from the same repository state, with the same model, tools and acceptance tests, how many tokens does a coding session consume with Majordomus installed and without it? The unit is tokens per successfully completed task. A run that saves tokens by failing is not a saving, so a pair counts only when both runs passed every success gate.

The control is the fixture repository as a competent team keeps it: a README, a short `CLAUDE.md`, conventions and decision records in `docs/`, a green test suite, and headless Claude Code with no MCP server, user settings or skills. The treatment is the same fixture after `majordomus init`, `majordomus update` and `majordomus capture install --provider claude-code`, committed, with everything else identical. The MCP server and the git hooks are left out of this first treatment, and measuring them would be a new treatment.

## A corpus that can be failed

The tasks in `.ai/repo/benchmarks/economics/tasks/` each stand for a development pattern this repository sees, from a defect whose fix is written down in a decision record to a feature built across two sessions. Each names its fixture, a verify command, a reference solution and hidden acceptance tests copied in only after the last session. A run succeeds when the fixture's own suite passes, the hidden tests pass and no test file the fixture started with was deleted.

`majordomus economics references` proves, with no model involved, that every hidden test fails on the task's starting state and passes on its reference, so a failed run says something about the agent, not about the test. The first live suite, `pilot`, runs every task in both arms and is small on purpose: its declaration calls its results preliminary by the publication rule.

## Two measurements, never merged

Every number carries one of five classes. Observed is what the provider or its harness reported at run time, unmodified; counted is computed from bytes on disk with a named, pinned tokenizer; derived is arithmetic over the two and never stronger than its weakest input. Estimated stands in for a number nobody measured and is never published, and counterfactual, what a mechanism would have avoided, is modelled and never a saving.

Two measurements answer different questions. The total-token reduction, `effective_token_reduction`, is derived from observed usage in matched live runs and is the only kind of number that could support a claim about what Majordomus saves. Context selection, `context_reduction_ratio`, is derived from counted tokens: what the context compiler selected for each issue of this repository's plan out of what it judged relevant, with no model involved. It is not a saving, and the two are never merged; the count names its tokenizer, `o200k_base`, which is not every model's, and is never compared with tokens a provider reported.

Per valid pair, `reduction = 1 - treatment / control` on total tokens: positive means Majordomus used fewer tokens, negative that it used more, and no surface flips the sign, because the ADR requires a task where Majordomus costs more to be shown, not averaged away. Across pairs the calculator reports the distribution and a percentile bootstrap interval of the median with a recorded seed, resampling whole tasks, since the repetitions of one task are not independent.

## The rule comes before the evidence

The methodology declares, before any evidence, the minimum number of valid pairs, of task categories with enough pairs each and of repetitions of every task in each arm, the lowest valid-pair rate and the widest interval it accepts, and it requires current evidence from a clean tree. A total-token claim is allowed only when every threshold holds, and only for the primary metric over all the evidence, unfiltered. Below the rule, every surface states that no verified total-token-savings claim is available, beside the preliminary observation labelled as such.

No run is excluded unless the methodology names it with a reason, and the primary metric is then shown with and without it. A recorded run is never written over, `--force` is refused, and a pair that is not valid is reported with its reason, never dropped silently.

What reaches a reader is held by `majordomus economics check`, which CI runs in every plan as the structure gate `economics-claims`. It refuses a percentage or a multiplier typed into hand-written prose in the same sentence as a word of the economics vocabulary, and a bound claim that reads `guaranteed` while its metric's evidence does not stand. It reads text, not meaning, so a claim phrased without a quantity passes.

## Evidence goes stale

Every record carries a digest of the files its suite depends on, for a live suite the methodology, the corpus, the fixture and the parts of Majordomus that shape what a treatment session is given. The digest is recomputed on every read, and when any of those files has changed, the evidence is reported as stale and a claim bound to it loses its guarantee until the suite is run again. Evidence recorded under another methodology version is incompatible and never pooled.

## What stands, and what is not measured

The deterministic context suite was first recorded on 26 September 2026; its figure is on the [Token economics](@/economics.md) page, marked measured and derived from counted. When this article was written, in October 2026, no live run had been recorded, so the generated verdict was that no verified total-token-savings claim is available; the same page shows the verdict as it stands. The hypotheses the benchmark was built to test were written down before any evidence and are listed as untested; one of them is that short isolated tasks gain less and may cost more.

A reader can repeat it. `majordomus economics references` and `majordomus economics measure` need no model, `majordomus economics run --suite pilot` needs a Claude Code login and spends usage, `majordomus economics summary` recomputes every figure from the recorded runs, and `majordomus economics explain effective_token_reduction` shows what one metric rests on. The same answers are served at `GET /api/v1/economics`, by the MCP tools `majordomus_economics*` and on the Cockpit's Economics page, all projections of one calculator.

The fixture is small by design, so a large repository, where context selection matters most, is not measured; nor are other harnesses and models, the MCP server, coordination between parallel workers or the finish contract's effect on rework. Cost is the harness's own projection at run time, not a bill. The treatment is Majordomus as a user who installs only the shell tool gets it, so what a session spends on commands its bootstrap advises but that installation cannot run is measured, not corrected for.

ADR 0082 is proposed; the claims that context selection is counted and that an unsupported savings figure is refused are guaranteed, and the claim of a measured total-token change is planned. Why a check that never reported is not a pass is in [Silence is not a verdict](@/silence-is-not-a-verdict.md), how a repository refuses its own documentation in [Proof-carrying documentation](@/proof-carrying-documentation.md), how a disagreement is settled by an experiment rather than a count in [A doubt is settled by an experiment](@/a-doubt-is-settled-by-an-experiment.md), and the method in [`docs/ECONOMICS.md`](@/docs/economics.md).
{% endraw %}
