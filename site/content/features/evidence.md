+++
title = "A claim is proven by a run that happened, not by a file that exists"
description = "The claims matrix binds a claim to a test by path, and a path resolving says a file exists rather than that anything ran. The ledger records the latest execution of every test with its provenance; the join is derived on every read and answers, per claim, whether the repository can honestly call it proven, only-not-invalidated, stale, failing or never run."
weight = 175
[extra]
id = "evidence"
status = "stable"
source = ".ai/repo/features/evidence.md"
+++
{% raw %}

## What it does

`majordomus evidence show` renders every claim of `docs/CLAIMS.yaml` against the run
recorded for its test: the proof state, the sentence that explains how that state was
derived, the execution behind it, the files that have changed since, and the command that
produces the proof again. `evidence claim` is one claim in full and `evidence test` is the
reverse — one test with every claim it proves — read from the same derivation, so the two
directions cannot disagree. `evidence record` writes what a run already reported into the
ledger, stamped with the commit, the tree state, the digest of the test's own source, the
time and the origin.

The seven states are ranked, and two of them are deliberately not one. `proven` is a
passing run recorded against this very commit with a clean tree. `inputs unchanged` is a
passing run where nothing the claim itself names has moved since — the absence of a known
invalidation, which is weaker, and which is labelled as weaker everywhere it is shown.

## What it does not do

It does not prove a claim at HEAD from a run made elsewhere: the dependency model is the
files the claim names and nothing transitive, so `inputs unchanged` is honest about the
change it cannot see. It does not resist forgery — the ledger is a tracked file and a
hand-edited outcome is a line in a diff, not a cryptographic failure — and it keeps no
history and no captured output, because the question it answers only ever reads the newest
result and a log of every case belongs where the run happened.
{% endraw %}
