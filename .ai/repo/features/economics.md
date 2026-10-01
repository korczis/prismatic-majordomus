---
schema: feature/v1
id: economics
kind: feature
title: Token economics, measured from matched runs and never typed
short_title: Economics
headline: What a coding session consumes with Majordomus and without it, from matched runs judged by the same hidden tests, with every number labelled by how it was obtained and published only past a declared rule.
summary: A task corpus runs in a real harness twice — without Majordomus and with it — and the provider's usage, the gate verdicts and the context compiler's counted selection are recorded as raw facts; one calculator derives pairs, distributions, seeded bootstrap intervals and the one statement the methodology's publication rule allows, projected to the CLI, HTTP, OpenAPI, MCP, the Cockpit, a generated report and the site; economics check refuses any savings number typed by hand.
status: stable
weight: 47
featured: false
areas: [observability, cost]
modules: [economics]
rules: []
docs: [docs/ECONOMICS.md]
adrs: [adr-0082]
claims: [context-selection-counted, economics-claims-gated, token-savings-measured]
use_cases: []
cockpit: [economics]
related: [models, evidence]
tags: [economics, tokens, benchmark, evidence]
---

## What it does

`majordomus economics run` executes each task of `.ai/repo/benchmarks/economics/` in Claude
Code, once in the fixture as a team keeps it and once with Majordomus installed, and records
what the provider reported and whether the hidden acceptance tests passed.
`majordomus economics measure` counts, with no model, what the context compiler selects for
every issue of the plan. `economics summary`, `explain` and `runs` — and the same capabilities
over HTTP, MCP and the Cockpit — show every metric with its class, sample size and interval,
every pair including the invalid ones, and the verdict. `economics check` refuses a savings
number that no evidence produced.

## What it does not do

It does not claim a saving the evidence does not support: below the publication rule the
verdict says there is no verified claim. It does not compare counts across providers or
tokenizers, re-price old runs, or record anything a session said.
