+++
title = "A use case is run, not described"
description = "A list of what a tool can do is the easiest page to write and the first to go stale. How Majordomus keeps each use case as one file whose scenario runs against the real tool in a disposable repository, shows that run's normalised output on the site, and lets nobody type the maturity a use case claims."
template = "page.html"
[extra]
lang = "en"
related = ["proof-carrying-documentation/", "a-rule-is-proven-by-what-it-refuses/", "written-once-derived-everywhere/"]
[extra.translations]
cs = "pripad-uziti-se-spousti-ne-popisuje/"
+++

A use case, in [`docs/USE_CASES.md`](@/docs/use-cases.md), is a task somebody performs with Majordomus, written once as data and proved against the real tool. It is not a tutorial, a feature list or a page. The manual states its principle once: anything derivable is derived, anything not derivable has one canonical home, and anything claimed as guaranteed is backed by executable evidence. The site's list of use cases is built on that principle, and this article follows how.

## One file, everything else derived

A use case is one file under `.ai/repo/use-cases/`. Its front matter names the commands, MCP tools, rules, claims, responsibilities and applications it touches, and its body has three sections: the situation, the scenario and the outcome. The page, the category it sits in, the links to and from it, the tally of which capabilities have a use case and the evidence a page shows are all derived. There is no registry to edit, no template to touch and no count to bump.

The schema refuses a key it does not declare. `majordomus usecase validate` refuses a reference that does not resolve, a scenario step that runs a command the use case does not list, a body without its three sections, and an active use case that aims at `guaranteed` without a scenario.

## The scenario is the proof

The scenario is one fenced YAML block between the situation and the outcome, so a reader meets the situation, the proof and the result in the order they happen. `setup` names a prepared state, a fixture script shared with the command demonstrations, so what a use case starts from is what a command page shows. Each step is one real invocation of `bin/majordomus` in a disposable repository, given as an argument vector and never as a shell string, and it expects an exit code and, where it says so, patterns in the output or in a file.

A step may also be a worker's: one shell line between two invocations, recorded as done by the worker rather than the tool. One scenario can then tell a whole story. A worker strays, the tool refuses, the worker takes it back, and the tool accepts. [The challenge](@/challenge.md) on this site is such a scenario, and its page replays the recorded run.

## Fixture or live

A scenario says which repository it is about. A fixture scenario, the default, runs in a disposable repository that CI can recreate. A live scenario asks its questions of the repository the command was invoked in. It prepares nothing and may run only the argument vectors that `share/commands.yaml` declares read-only, so it cannot change what it asks about. It may also carry an obligation step, which runs nothing and asserts that something a class of work owes was discharged, through the same judgement `majordomus check` reads.

A live scenario's verdict is `unmet`, not `fail`. A failing fixture scenario is a defect in the tool, while an outstanding obligation in a live one is work not done, and the two are not the same news. Live evidence describes one tree on one machine at one minute, so it is never committed: a derived file whose content depends on who derived it is what [ADR 0005](@/adrs/adr-0005.md), accepted, forbids. `usecase coverage` likewise counts a live scenario as naming a command and never as covering it: a guarantee CI cannot run again is not a guarantee. [ADR 0038](@/adrs/adr-0038.md) (proposed) is the decision.

## Evidence a page can show

`majordomus usecase run` writes, for every step, the command, its arguments, the exit code against the expected one, the normalised output, every assertion with its result, and the timing. Normalised means that the paths, the home directory, timestamps, task and session ids, record hashes and durations are replaced by placeholders, and nothing behavioural is. Two runs of one scenario on one tool are byte-identical.

The site generator does not read that local evidence. It runs every scenario itself, through the same command, and embeds the result in `site/data/generated/catalogue.json`, and CI refuses site data that differs from a clean regeneration. So a page that shows what a command printed shows exactly that, and a pasted transcript has nowhere to go.

## Maturity is observed

Two fields are authored: the status (active, draft or deprecated) and the target (guaranteed or advisory). What a use case is gets computed when the site data is generated. It is `described` when it has no scenario, `executable` when it has one with no passing evidence in this generation, `verified` when the scenario passed, and `guaranteed` only when it is verified, every claim it names is guaranteed and every rule it names is enforced. Nobody writes `guaranteed` into a use case.

## Coverage the policy can require

`majordomus usecase coverage` treats every public command, every guaranteed claim with a responsibility and every MCP tool as a target. It counts the active use cases that name each target, those whose scenario runs it and those with passing evidence, and calls the target covered, partial or a gap. The policy says what a gap means for each class. This repository requires every public command and keeps claims and MCP tools advisory, and the blocking rule [majordomus.use-case-coverage](@/rules/majordomus-use-case-coverage-1.md) runs the tally under `doctor`, `check` and `finish`. A required gap refuses completion and names the capability and the command that scaffolds a draft for it.

A draft written by `majordomus usecase scaffold --missing` validates and runs, and covers nothing until a person or an agent completes the narrative, tightens the assertions and makes it active. Nothing in the scaffold needs a model, and the manual is explicit that nothing a model writes is evidence.

## How to check it

`majordomus usecase run` runs every fixture scenario, `majordomus usecase run <id> --keep` keeps the repository for a look, and `majordomus usecase show <id>` prints the file and its last evidence. `majordomus usecase coverage --check` applies the policy, and `majordomus usecase impact` names, from the files a change touched, the commands, rules, use cases and behavioural cases to run next. The guarantees [use-case-coverage](@/guarantees/use-case-coverage.md), [use-case-evidence](@/guarantees/use-case-evidence.md) and [use-case-impact](@/guarantees/use-case-impact.md) each name `test/cases/94_use_cases.sh` as their proof.

## What this does not claim

It does not claim that every behaviour of the tool has a use case. The policy requires every public command and only reports the gaps for claims and MCP tools. Nor is a live scenario's result a guarantee: it describes the tree it ran in. ADR 0038 is proposed and ADR 0005 is accepted; the commands, the generator and the rule described here are on master and run. How a guarantee is held to a test that names it back is in [Proof-carrying documentation](@/proof-carrying-documentation.md), how a rule is held to a case that can refuse its violation in [A rule is proven by what it refuses](@/a-rule-is-proven-by-what-it-refuses.md), and why a derived page gives the same bytes every time in [Written once, derived everywhere](@/written-once-derived-everywhere.md).
