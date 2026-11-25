---
schema: knowledge/v1
id: owner-decision-baseline-relative-gates
kind: knowledge
class: fact
title: "Owner decision 2026-11-25: an adopter's session lessons become generic Majordomus, each designed, critiqued by other models in several passes, then built; the first is a gate judged against its base"
description: "When a session asks why gates.baseline exists, what the owner asked for when a session's lessons are carried into Majordomus, or which advisors weighed the design of I4730 and what each said"
status: verified
epistemics: decided
date: 2026-11-25
tags:
  - decision
  - owner-answer
  - gates
  - reasoning
provenance:
  origin: extracted
  derived_from:
    - file:.ai/repo/project/issues/I4730.yaml
    - file:.ai/repo/project/issues/I4413.yaml
    - file:docs/CI.md
relations:
  - type: relates_to
    target: file:.ai/repo/project/issues/I4730.yaml
---

# Owner decision 2026-11-25: an adopter's session lessons become generic Majordomus, designed and critiqued in several passes

A session working in an adopting repository (a game-simulation research fork whose gates
include whole-tree scans and nondeterministic fitness checks) distilled four generic lessons
of its day and asked the owner which of them to implement in Majordomus. The owner chose all
four and added an instruction on how. This record holds the question and the answer, and the
design of the lesson this repository's I4730 carries: a gate judged against its base.

# Evidence

- Session: 7709ab47-7786-494d-bbcc-679b293569a7 (Claude Code, openttd-36), 2026-11-25, about
  06:50 CET. Recorded by the worker that built I4730, from the coordinator's brief, in the same
  session.
- Question (AskUserQuestion, translated from Czech): "Yes. From this session I've distilled
  these generic lessons for prismatic-majordomus (nothing OpenTTD-specific). Which should I
  implement there? (Each one first as a design for review, then implementation with tests in
  that repo's own workflow.)" Options: "Build slots and locks" | "Owner answers in the repo" |
  "Plan ↔ merged PRs" | "Baseline-relative gates".
- Answer: all four selected, and the free text, verbatim: "porad se s ostatnimi ai/llm, na
  vice pruchodu a vymslete nejlepsi reseni" ("consult the other AIs/LLMs, in several passes,
  and work out the best solution").
- The owner's rule for nondeterministic checks, as the session stated it: "a PR passes when it
  shows no failure that master doesn't also show, judged over repeated runs".

# The design of I4730, and who weighed it

**Pass 1, the problem.** On one day the trunk of the adopting repository broke two whole-tree
gates — a reference check citing a file that existed only on an open pull request, and a
labelling rule flagging test lines a merged pull request had added — and every pull request's
gate then failed for a reason not its own. Existing mechanisms: I4413 (finish re-runs a failed
verification at the base; planned), I3313 (failure origin from the ledger; planned), I4415
(load and retries; planned), the gates module (judges recorded runs, runs nothing), and this
repository's own committed ratchet files for whole-tree gates, which do not help once the trunk
itself adds findings. Options: (A) everything in shell `finish`, as I4413 writes it; (B) a Rust
judgement over runs the caller supplies, which finish, an adopter's gate script and CI all
call; (C) a Rust capability that also runs the check over an exported base.

**Advisors asked, independently, with the problem and the code and without a conclusion:**
Codex (`codex exec --sandbox read-only`), and an independent Claude session (read-only).
Gemini's client refused to run ("no longer supported"), and no local model was installed, so
both were reported absent rather than waited for. The repository's own shared server timed out
under the machine's load, so no peer of this repository was reachable on its board.

- Codex, round 1: one classifier in the Rust gates layer, finish as the lifecycle caller;
  per id counts c and b over N runs; introduced, fixed, pre-existing, a "regressed" class for
  c > b; flaky as a property that never erases introduced; ids that cannot be extracted are
  never presumed pre-existing; N=20 as a starting point, with the rule of three showing that
  even 20 clean runs leave a 15% upper bound; first slice small.
- Claude, round 1: split by who runs what — finish runs, Rust judges; classes as above with a
  rate comparison and a declared slack; the neutral-perturbation control as the noise floor;
  interleave runs; `in_diff` per finding as the cheapest deterministic slice; never accept a
  red verification; first slice inside I4413.
- Codex, round 2 (on the revised design): a control finding must not erase an introduced id by
  its presence; equal synthetic exit ids prove nothing; inputs need provenance; the rate rule
  needs exactly N runs per side; a run that did not finish must not count like one that did;
  paths need normalising; the last commit to touch a file is weak attribution.
- Claude, round 2: the same objection to the control and to attribution, plus: exit codes
  must be part of a synthetic id (exit 137 is not exit 1); one side is one tree; rate below
  three runs is a presence; say that no multiple-comparison correction is made; a clean
  verdict with a red base must not read as green; ship a first consumer.

**Taken.** Option B. No `noise` class: under `subset` the control never changes what blocks,
and under `rate` it raises the floor only by the rate it measured; a finding in the change's
own files always blocks. Synthetic ids carry the exit code, and a run that did not finish
shows `not-finished` whatever it left. A side is compared only over exactly the declared runs
of one tree. Rate needs three runs. The verdict `clean_with_trunk_failures` and the trunk alarm
keep a red base from reading green. The report carries its denominator, the commits and the
command, and states that no correction for many ids is made. Paths are normalised.

**Rejected, and why.** Attribution by the last commit to touch a file (both advisors: it names
the latest editor, not the cause); a bounded bisect is a follow-up. A `regressed` class (Codex):
under `subset` the owner's rule does not block a worse inherited failure, and under `rate` the
same case is a blocking pre-existing id, so a fourth class adds a word and no decision. Shipping
the runner in this slice (Claude): the runner of `finish` is I4413's, which now depends on
I4730 and hands it its runs; an adopter's gate reaches it with `majordomus-cli run
gates.baseline`. A fixed N (Codex's 20): the caller declares N, and the report states it.
