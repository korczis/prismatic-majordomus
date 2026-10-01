+++
title = "What the context compiler selects, out of what it judged relevant, is counted in tokens of a named tokenizer and recorded, never estimated"
description = "For every issue of the repository's plan, majordomus economics measure asks the context compiler (majordomus devcontext) for the context of that work and counts, in tokens of a named and pinned tokenizer, what it judged relevant and what it selected. The result is recorded under .ai/repo/benchmarks/economics/runs/context/ with the revision, the tokenizer and a digest of the inputs it depends on."
weight = 95
[extra]
claim_id = "context-selection-counted"
status = "guaranteed"
source = "docs/claims/context-selection-counted.md"
+++
{% raw %}

## What it means

For every issue of the repository's plan, `majordomus economics measure` asks the context compiler (`majordomus devcontext`) for the context of that work and counts, in tokens of a named and pinned tokenizer, what it judged relevant and what it selected. The result is recorded under `.ai/repo/benchmarks/economics/runs/context/` with the revision, the tokenizer and a digest of the inputs it depends on.

## How it works

Each seed is compiled under the compiler's default budget. Every distinct file the compiler reached is read from disk once and counted with `o200k_base` (tiktoken-rs, pinned to one version). A *candidate* is a file the compiler judged relevant: selected, or left out only because the budget ran out. Files it reached and judged irrelevant are counted separately and never divided by. The metric `context_reduction_ratio` is the median over seeds of `1 - selected / candidate`, derived from counted values. The compiler's own bytes-over-four estimate is recorded beside the count, so how far the budget's unit is from real tokens is itself a metric (`context_cost_model_error`).

Evidence is *current* while the compiler's code, the knowledge sources and the methodology are unchanged since it was recorded, and *stale* the moment any of them changes.

## How to see it

```bash
majordomus economics measure --dry-run          # count, write nothing
majordomus economics explain context_reduction_ratio
majordomus economics summary --format json | jq '.context'
```

## What it does not cover

It is context *selection*, not total token savings. It says what the compiler put in front of a worker out of what it found relevant; it does not say what a session consumed, and a session remains free to read anything. The tokenizer is not the tokenizer of every model. The metric states both in its own `not` field and warnings, on every surface.

## Why it exists

A budget measured in bytes over four is a guess about tokens. Counting with a named tokenizer, recording the count with its revision, and marking it stale when the mechanism changes turns the compiler's selection into a measurement that can be checked and reproduced.
{% endraw %}
