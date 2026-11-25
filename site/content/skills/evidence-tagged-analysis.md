+++
title = "Evidence-tagged multi-pass analysis"
description = "Answer a hard causal question in passes (the question and its rules first, independent lenses, lenses refuting each other, a synthesis from confirmed findings only) with every claim tagged by its evidence and the bundle checked by reasoning analysis."
weight = 3
[extra]
id = "evidence-tagged-analysis"
status = "active"
version = 1
source = ".ai/repo/skills/evidence-tagged-analysis/SKILL.md"
+++
{% raw %}

## Purpose

Find out why something happened when one confident reading would be wrong. A single pass tends
to produce plausible claims that are wrong: a list called empty that had held rows, a default
blamed as an outside change, a ratio computed over mismatched periods. Each of these reads as
well as a right claim. This procedure makes each claim carry its evidence, puts every finding in
front of a reader whose job is to refute it, and builds the conclusion only from what survived.

It is not a review of a change (`repo-review`), not a status picture (`project-picture`), and
not a decision under uncertainty (`majordomus reasoning`, ADR 0098). Those can feed it, and a
decision can follow from it.

## When to use

- The question is causal and the evidence is spread over many sources: logs, stores, code,
  history.
- Several workers, or several models, will read in parallel and their claims need reconciling.
- The result will be published and acted on, so a wrong claim would be costly to reverse.

## Procedure

### 0. The question and the rules, first

Write the `question` document before anything is read:

- the questions, Q1..Qn;
- the hard rules:
  - what is read-only;
  - what may run, and how long;
  - the evidence grammar below;
  - an unrun check is reported as not run;
  - a missing source goes into the gaps ledger;
- the data sources, with exact paths;
- the output format;
- the pass plan.

Every worker of every pass gets this document whole.

### 1. Pass 1: independent lenses

Give each lens to its own worker, and do not let a lens see another lens's output. Each lens
writes a `lens` document with these parts:

1. a **Summary** in three sentences;
2. a **findings table**, with columns `ID | Claim | Tag | Evidence`. The id is `<lens>-F<n>`.
   The tag is one of these:
   - **OBSERVED**: seen in the data. The evidence holds a locator: `path:line`, a URL, a commit
     hash, or a backtick span with the command, query or log line that shows it.
   - **DERIVED**: computed from what was observed. The evidence holds the formula, or the
     findings it is computed from.
   - **INFERRED**: a hypothesis. The evidence says what would confirm it.

   When a claim rests on mixed evidence, tag it with the weakest kind;
3. at most three root causes, each a chain of finding ids;
4. remedies: what, where, how to verify it, the expected effect and the owner;
5. a **Gaps ledger**: what could not be verified, and why. `- none` is an empty ledger.

Prose around the tables is free, but it is not a claim. A claim that matters goes in a row.

### 2. Pass 2: refutation

Give each worker the findings of lenses other than its own, and make the job refutation, not
restatement:

- recompute the numbers;
- re-read every locator;
- check that compared quantities share a basis (the same period, the same population);
- look for counter-evidence.

Each finding gets exactly one row in a `cross-check` document, with columns
`ID | Verdict | Tag | Evidence`:

- **CONFIRMED**, with what was recomputed or re-read;
- **REFUTED**, with the reason and the corrected value;
- **UNVERIFIABLE**, with what is missing, which also goes into the gaps ledger.

A qualifier such as `CONFIRMED (scoped)` is allowed. The verdict's evidence is tagged like a
finding's.

When two cross-checks disagree on a finding, settle it the way a disagreement is settled in
`majordomus reasoning`:

- record the disagreement;
- run the experiment that decides it, and record the resolution;
- keep one verdict row, whose evidence names the experiment.

A count of opinions decides nothing.

Ask one outside view of the whole picture through `scripts/advisor-consult`, which records the
consultation. No advisor is required. When none is available, the bundle says so in its gaps
ledger and the analysis continues.

### 3. Pass 3: synthesis

The `synthesis` document uses CONFIRMED findings only. It holds:

- a **root-cause table**, with columns `RC | Cause | Findings`. Root causes are ranked by
  impact, and each is a chain of CONFIRMED ids. Say whether the ranking is measured or
  descriptive;
- a **remedy table**, with columns `ID | Remedy | Addresses | Owner` and optionally
  `Verified by`. Each remedy addresses a root cause or a finding. A remedy with no owner is
  written `unowned`, which marks it as a hole for someone to take;
- a **refuted register**, with columns `ID | Verdict | Correction`. It lists every REFUTED
  finding and what replaces it, so that nobody reuses it;
- a **Gaps ledger**, which names every UNVERIFIABLE finding and every check that was not run.

Then act on it. Send each remedy to its owner, and file a plan issue for each hole. Correct
every wrong claim you made or relayed earlier, in the open and where it was made.

### 4. Check the bundle

```sh
majordomus reasoning analysis --path <bundle>
```

The bundle is ready when the answer reads `conforms`. If a check fails, fix the document and
run the check again. If the answer is `incomplete`, the missing pass is named, and the bundle
is not ready until that pass is done.

What the check proves is structure, and its answer says so:

- every claim row is typed and sourced;
- every finding has one verdict;
- the synthesis cites only CONFIRMED findings;
- the gaps are ledgered.

It does not prove that a locator supports its claim. Only the refutation pass establishes that.

### 5. Publish

Publish the whole bundle where it is tracked. The synthesis alone reads `incomplete`, because
what survived refutation is then unknown.

## Output

A bundle directory, every Markdown file declaring its role in front matter:

```yaml
---
analysis:
  id: <one id for the whole bundle>
  pass: question | lens | cross-check | synthesis
  lens: <the lens, for a lens or a cross-check>
---
```

A bundle holds:

- one `question`;
- one `lens` per lens;
- one or more `cross-check` documents;
- one `synthesis`.

Files that are not Markdown, and subdirectories, are reported as ignored. A Markdown file
without the front matter is refused, never skipped. The conforming example is
`test/fixtures/analysis/conforming/`.

The report of the work follows `report-verification-state`. Two curated lessons apply
throughout:

- "an unrun check is reported as unrun";
- "a wrong claim is corrected in the open".
{% endraw %}
