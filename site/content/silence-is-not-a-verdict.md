+++
title = "Silence is not a verdict"
description = "In the interface a person looks at, a check that never reported and a check that reported green are the same absence. How Majordomus keeps them apart: a verdict arranged so that it arrives, a status of its own for a gate no run has reported, proof as a recorded run, and a plain account of what a gate enforces and what only review holds."
template = "page.html"
[extra]
lang = "en"
related = ["proof-carrying-documentation/", "everything-followed/", "a-doubt-is-settled-by-an-experiment/", "a-saving-is-measured-not-claimed/"]
[extra.translations]
cs = "mlceni-neni-verdikt/"
+++
{% raw %}

On 10 September 2026, four validation runs of master sat in the queue, the oldest for seven hours. Every Linux job in them had finished, and in one run five of those jobs had failed, but three jobs on macOS runners had never started, and the verdict job, `ci`, needs all three. So nothing reported at all.

The trunk carried branch-breaking defects through the night while every surface a person looks at showed nothing wrong. The rule [project.never-reported-is-not-green](@/rules/project-never-reported-is-not-green-1.md) opens with that night: nobody was deceived by a green tick, because there was no green tick, but by an empty space where a verdict goes, which reads exactly like one. Its statement is short: a check that has not reported is unknown, never passing, and a change is merged against the trunk's measured failing set, not against silence.

## The same shape, in other clothes

The rule records the shape twice more the next day. A publication that had succeeded reported `cancelled`, because the job's own wait for the content delivery network expired before the network caught up, and the site was serving the new commit a minute later. The answer was to ask the site what it was serving, not to re-run a deploy that had already happened. And every pull request open that day showed failing checks, each of them the trunk's own, so a worker comparing against zero would have merged nothing and left the trunk red.

The cheap reading, a colour, a status word or a count of failures, is not the reading that decides; what decides is what a run actually said, and about which tree. So a change is judged against the trunk's own failing set, and identical or smaller is the bar, because strictly zero while the trunk is red would freeze every repair behind every other repair. A bounded wait that expired is a measurement that did not finish, not one that came back negative.

## A verdict is arranged to arrive

The rule calls a gate whose runner is so scarce that its answer never comes documentation, not enforcement. [ADR 0037](@/adrs/adr-0037.md) (accepted) measured why waiting would not help: master took a push about every seven minutes on an integration day, the macOS suite ran for about 106 minutes, and over that whole day not one of those gates completed. A gate marked `on-demand: true` now runs only in a plan that asks for it, such as the nightly one, so a routine push skips it rather than queueing it, and `ci` can report. A nightly that goes red and is not read, the record warns, is the same defect, one cadence slower.

## Asked, and not answered

Inside a task the principle becomes a status a machine holds, under the blocking rule `majordomus.completion-gates`. `majordomus check` and `majordomus finish` judge each gate the task's change set selects, and `queued` means the plan selects the gate and no run has ever reported: not `unknown`, because the question was asked and no answer came, and not `pass`, the mistake the word exists to make impossible. A gate that failed, went stale or is blocked refuses the outcome `completed`, and so does a verdict that could not be read at all, because `completed` is a claim that the verdict is known.

A `queued` gate is reported by name, never accepted as a pass and never refused. `check` lists the required gates that have never reported, followed by `never-reported is not green` and the rule's id, and the executable's finding adds that a task completed over them is completed unverified. It refuses nothing because a repository where recording is new would otherwise refuse every finish.

## Proof is a run that was recorded

The claims matrix had the same defect, in front of the public. Before [ADR 0041](@/adrs/adr-0041.md) (proposed), the only thing checked about a claim's test was that its path resolved, so the public site displayed a guarantee because a file existed. The decision makes a claim's proof a recorded execution, which names the commit it ran against and the command that runs it again. The record expected every claim to read `not_run` on the day it landed, and called that the improvement: the previous state was unknown, displayed as guaranteed.

There is deliberately no `not_run` outcome to record, and the weaker passing state, `inputs_unchanged`, is the absence of a known invalidation rather than proof at HEAD. [`docs/EVIDENCE.md`](@/docs/evidence.md) says that a surface rendering it with the same tick as `proven` has reintroduced the defect, and the evidence gate, `evidence-check`, is planned on every push with `always: true`, which the document calls the other half of the rule.

When [ADR 0080](@/adrs/adr-0080.md) (proposed) measured master at `8d37d011ef`, the ledger held six local executions and no claim was `proven`, yet the gate was green, because its ratchet then counted `stale` as support. That green, the record says, was honest about what it measured and said much less than a reader takes it for. Its decision is that a run records and never commits: a separate pull request whose diff is the ledger alone lands the recording, under the same required checks as any other. That pull request is not wired yet, so a validating run's rows still decide no verdict, and an absent report is named, never counted as a pass.

## A skipped gate owes its proof

[ADR 0186](@/adrs/adr-0186.md) (proposed) serves the intent that a pull request's required verdict arrive within five minutes, never by weakening a gate. Each gate declares a lane, and the required verdict needs every feedback-lane gate green and every other selected gate either run or reused as `inputs_unchanged`: a recorded passing result whose inputs, toolchain and meaning are byte-identical. The proof lane, with the full suite, the full browser sweep, macOS, coverage and the installer, is required on the combined top before it lands and on a release, and unknown impact selects everything. A red there blocks the next landing, and the record applies the rule to every skipped gate's proof. The owner has not accepted it.

## What a gate holds, and what review holds

The rule itself is active and advisory, decided, in its own words, by review and by the record, not by a gate, because no check can tell that a person believed silence. Its nearest mechanical support is that the planner refuses a model whose verdict cannot be reached and that the published-site gate, `pages-live`, asks the site rather than the workflow; the planner's case, its one verification, holds that every declared gate is reachable in some plan. Everything else, the comparison against the trunk's failing set included, is held by review, and a report saying something passes when no run finished is, in the rule's words, a correctness failure rather than a matter of tone.

[ADR 0039](@/adrs/adr-0039.md) (proposed) turns the argument on rules themselves: the liveness doctrine landed as four advisory rules, because a rule whose class asserts an enforcement that does not exist is the shape this rule refuses in the CI direction. Two became blocking on 30 September, once scans could decide them, and one of them, `project.every-wait-is-bounded`, depends on this rule and carries it into a live wait, where silence is inspected and never counted as progress. A milestone recorded on 9 October 2026 found the full scheduled CI plan, the only one that selects every gate, without a green verdict in its last ten runs while push runs on master stayed green, with nothing alarming and the rule review-only.

ADR 0037 is accepted, while ADR 0039, ADR 0041, ADR 0080 and ADR 0186, which cite the rule, are still proposed. How a stream of signals names a source it could not read is in [Everything, followed](@/everything-followed.md), how a repository refuses its own documentation in [Proof-carrying documentation](@/proof-carrying-documentation.md), how a disagreement is settled by an experiment rather than a count in [A doubt is settled by an experiment](@/a-doubt-is-settled-by-an-experiment.md), and the evidence model in [`docs/EVIDENCE.md`](@/docs/evidence.md).
{% endraw %}
