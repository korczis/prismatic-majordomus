---
schema: feature/v1
id: evidence
kind: feature
title: A claim is proven by a run that happened, not by a file that exists
short_title: Evidence
headline: Every claim is joined to the execution recorded for its test — the outcome, the commit, the tree, the digest and the command that produces it again — and a run against this commit is never reported as the same thing as a run whose inputs merely have not changed.
summary: The claims matrix binds a claim to a test by path, and a path resolving says a file exists rather than that anything ran. The ledger records the latest execution of every test with its provenance; the join is derived on every read and answers, per claim, whether the repository can honestly call it proven, only-not-invalidated, stale, failing or never run.
status: stable
weight: 175
domain: evidence
featured: true
areas: [verification, governance]
modules: [evidence]
commands: [evidence]
docs: [docs/EVIDENCE.md]
adrs: [adr-0041]
claims: [evidence-proof-is-an-execution, evidence-currency-is-not-collapsed, evidence-partial-run-preserves-the-rest, evidence-navigates-both-ways, evidence-unsupported-guarantee-is-reported, evidence-judged-at-the-presented-revision, evidence-a-skip-is-not-run, evidence-a-failure-outranks-an-absence, evidence-a-crate-binary-that-ran-nothing-is-not-a-pass, evidence-tree-is-measured-by-the-run, evidence-a-recording-is-a-typed-run, evidence-local-recording-stays-local]
related: [benchmarks, ci, declare-once]
tags: [evidence, claims, tests, verification, provenance]
---

## What it does

`majordomus evidence show` renders every claim of `docs/CLAIMS.yaml` against the run
recorded for its test: the proof state, the sentence that explains how that state was
derived, the execution behind it, the files that have changed since, and the command that
produces the proof again. `evidence claim` is one claim in full and `evidence test` is the
reverse — one test with every claim it proves — read from the same derivation, so the two
directions cannot disagree. `evidence subject <key>` answers the same question for any
subject of the index — a claim, a rule, a feature, a command, a capability, an MCP tool or a
use case — over the command line, HTTP and MCP alike, and `evidence coverage` lists the
coverage the run records carry, current only for its own commit on a clean tree.
`evidence publish` (the capability `evidence.publish`) writes the site's evidence publication
of the checked-out commit: its verdicts are the committed ledger's, weakened and never
strengthened by the run records read, its one state is the first matching row of the
publication table, and it carries only allow-listed fields, every text through the one
redactor.
`evidence record` writes what a run already reported into the ledger, stamped with the
commit, the tree state, the digest of the test's own source, the time and the origin. It
reads each runner through one adapter, which the web report's test page reads too: every
line the runner printed is read into a per-test result or listed in the run record with
why. `evidence record --run suite` runs the suite itself, stamps its own run and keeps it as
a local run, in the local ledger and the local run index under `.ai/local/evidence/runs/`,
which holds the newest fifty and never reaches the tracked ledger.

The seven states are ranked, and two of them are deliberately not one. `proven` is a
passing run recorded against this very commit with a clean tree. `inputs unchanged` is a
passing run where nothing the claim itself names has moved since — the absence of a known
invalidation, which is weaker, and which is labelled as weaker everywhere it is shown.

## What it does not do

It does not prove a claim at HEAD from a run made elsewhere: the dependency model is the
files the claim names and nothing transitive, so `inputs unchanged` is honest about the
change it cannot see. It does not resist forgery — the ledger is a tracked file and a
hand-edited outcome is a line in a diff, not a cryptographic failure — and the ledger keeps
no history and no captured output, because the question it answers only ever reads the
newest result and a log of every case belongs where the run happened: in the run record, kept
by a CI run's artifact or by the bounded local run index.
