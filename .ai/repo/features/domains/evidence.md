---
schema: domain/v1
id: evidence
kind: domain
title: Evidence
headline: 'A claim names the test that settles it, a recorded run is judged against the commit it ran at, and a claim with no recorded run says so.'
problem: 'Claims outrun proof. The documentation says one thing, the runtime does another, and nobody can show which is true.'
status: stable
weight: 40
tags: [evidence, provenance, claims]
---

# Evidence

What shows that a statement about the product is true: claims joined to the executions
recorded for their tests, use cases the tool runs against itself, measurements of speed
and of token cost that are recorded rather than typed, and the provenance of every record.
A run against this commit is never reported as the same thing as a run whose inputs merely
have not changed.

Not in this domain: the verdict on one task, which is completion; and the rules a claim is
about, which is governance.
