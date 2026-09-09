---
schema: feature/v1
id: use-cases
kind: feature
title: Every use case is executed against the tool, not described
short_title: Use cases
headline: What a person does with the tool is one file with a scenario the tool runs against itself; the page shows that execution, and a public command no use case runs is a gap the tool reports.
summary: A use case names the commands, rules, claims and applications it relies on and carries a scenario as data; usecase run executes it in a disposable repository and records normalised evidence; maturity is observed from the evidence; coverage of commands, guaranteed claims and MCP tools is tallied and the policy says which gaps fail.
status: stable
weight: 150
featured: false
areas: [verification, documentation]
commands: [usecase]
kinds: [use-case, application, taxonomy]
rules: [project.use-case-evidence, majordomus.use-case-coverage, majordomus.catalogue-integrity]
docs: [docs/USE_CASES.md, docs/CATALOGUE.md]
adrs: [adr-0008]
claims: [use-case-coverage, use-case-evidence, use-case-impact, catalogue-resolves]
use_cases: [add-a-use-case-and-prove-it]
related: [doctrine, why]
tags: [use-cases, evidence]
---

## What it does

`majordomus usecase run` executes every scenario; `usecase coverage` tallies every public
command, guaranteed claim and MCP tool against the use cases that name and run it;
`usecase impact` says what a change reaches; `usecase scaffold --missing` writes a draft for
each gap from what the tool already knows. An application says when the tool fits and,
because a catalogue that only lists fits is marketing, when it does not.

## What it does not do

A draft never counts until it is made active, and a status written by hand is refused: the
maturity of a use case is what its evidence supports.
