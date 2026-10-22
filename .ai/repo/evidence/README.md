---
schema: context/v1
id: ai.repo.evidence
kind: context
title: Evidence
description: The recorded executions a claim's proof is read from — what ran, against which commit, and with what result.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Evidence

`docs/CLAIMS.yaml` binds a claim to a test by path. A path says a proof exists somewhere;
it does not say the test ever ran, against which revision, or with what result. This
directory holds the other half: `ledger.json`, the latest recorded execution of every test
this repository runs, with the commit it ran against, the state of the tree at the time,
the digest of the test's own source, when it was recorded, where the run happened, and the
command that produces it again.

A claim's proof is the join of the two, and it is derived on every read — never written
down. `majordomus evidence show` is the whole matrix; `docs/EVIDENCE.md` is the reference.

The ledger is written by `majordomus evidence record`, which reads what a run already
wrote (`MJ_TEST_REPORT=<file> bash test/run.sh`, and `cargo test`'s output) and stamps it
with the provenance the run itself did not carry. It is never edited by hand to make a
gate pass: a result nobody measured is exactly the failure this directory exists to
prevent, and unlike a number in prose it is one line in a diff.

It holds the latest execution per test and no history, because the question it answers —
is this claim proven now — only ever reads the newest. It holds no captured output: a log
of every case is megabytes per run and belongs where the run happened. What is kept is the
durable semantic part, which is what a reader needs to decide whether to believe it.
