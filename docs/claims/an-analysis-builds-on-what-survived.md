# An evidence-tagged multi-pass analysis is checked, not trusted; reasoning analysis says whether every finding of its bundle is tagged and sourced, judged by exactly one cross-check verdict, cited by the synthesis only when CONFIRMED and ledgered when UNVERIFIABLE, and a bundle missing a pass is incomplete, never ok

## What it means

An analysis of a hard question runs in passes: lenses that each find something, then
cross-checks that try to refute what the lenses found, then a synthesis. A conclusion built
from that analysis rests only on what survived the cross-checks, and a reader can check
that it does. Each claim states its evidence:

- OBSERVED, with where it was seen;
- DERIVED, with the formula;
- INFERRED, with what would confirm it.

A refuted claim is listed so that nobody reuses it. What could not be verified is ledgered.
An analysis that skipped a pass cannot pass as complete.

## How it works

The skill `evidence-tagged-analysis` (`.ai/repo/skills/evidence-tagged-analysis/SKILL.md`)
holds the protocol and the document shapes. The capability `reasoning.analysis`, implemented
in `apps/majordomus-cli/src/reasoning/analysis.rs`, reads one bundle: a directory whose
Markdown files declare their pass in front matter. It runs seven checks:

- `bundle`
- `question`
- `findings`
- `verdicts`
- `synthesis`
- `citations`
- `gaps`

Every claim is a table row with named columns. The answer is `conforms`, `fails` or
`incomplete`, and `ok` is true only for `conforms`.

## How to see it

```
majordomus reasoning analysis --path test/fixtures/analysis/conforming   # conforms, exit 0
```

`test/cases/4777_an_analysis_builds_on_what_survived.sh` runs that command on the conforming
fixture bundle. It then plants each defect in a copy and expects it named with exit 10:

- a root cause built on a REFUTED finding;
- an UNVERIFIABLE finding missing from the gaps ledger;
- an OBSERVED finding without a locator;
- a finding no cross-check judged;
- a Markdown file that declares no pass.

A synthesis without its cross-checks reads `incomplete`.

## What it does not cover

The check proves structure, and every answer says so in its `promise` field. It does not
prove that a locator supports its claim, or that a ratio compares like with like. Only the
refutation pass, done by a worker, can establish that. Prose outside the tables is not
checked, so a claim that matters has to be a table row.
