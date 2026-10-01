# An unsupported savings claim is refused

## What it means

No number about tokens, context or cost reaches a reader unless recorded evidence produced it. Hand-written prose that states one is refused, and a claim bound to a metric cannot be `guaranteed` while that metric's evidence does not stand.

## How it works

`majordomus economics check` runs two checks. The first scans every hand-written document, page, site data file and claim sentence for a quantity (a percentage, a multiplier such as "3x fewer") in the same sentence as a word of the economics vocabulary. Generated files are exempt, because their numbers come from the calculator. The second reads the claims the methodology binds to metrics: a bound claim may be `guaranteed` only while its metric is `verified` (sampled evidence meeting the publication rule) or `measured` (deterministic evidence), and current. Evidence that goes stale takes the guarantee with it. Every finding is named; the exit code is 10.

## How to see it

```bash
majordomus economics check
majordomus economics check --format json | jq '.findings'
```

## What it does not cover

It reads text, not meaning: a claim phrased without a quantity, or a quantity in a sentence that names none of the vocabulary, passes. It covers the files the repository publishes, not what anyone says elsewhere.

## Why it exists

Every routing document studied while designing this tool carried improvement figures with no measurement behind them. A rule against doing the same is only as strong as the check that enforces it.
