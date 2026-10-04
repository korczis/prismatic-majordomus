# A milestone will be closed on GitHub only once the intents it realises are satisfied

## What it means

**This is not implemented.** It is published so that a known gap is visible rather than assumed to be covered.

The intended shape is that the GitHub projection reads the intents a milestone realises, and keeps the milestone open on GitHub while any of them is not satisfied.

## How it works today

`scripts/github-sync` projects each milestone from its derived plan status alone and reads no intent. Closed work the evidence contradicts is refused by the `intent-realization` gate in this repository (`intent-realization-held-to-evidence`), not by the tracker.

## How to see it

```bash
grep -c intent scripts/github-sync   # 0
```

## What it does not cover

It would still be a projection: GitHub would not decide satisfaction, and closing a milestone there would change nothing here.

## Why it exists

A tracker that shows a milestone closed while its intent is false repeats the mistake the intent model exists to correct, one projection further out.
