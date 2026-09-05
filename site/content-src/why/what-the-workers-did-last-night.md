+++
title = "Asking what the workers did last night, and getting a transcript"
description = "Why a conversation log cannot answer an operational question, and how an append-only ledger with a closed vocabulary can."
weight = 8
[extra]
hook = "asked what the workers did last night and had only transcripts to grep"
responsibilities = ["watch", "state"]
commands = ["history", "search"]
claims = ["history-ledger-read", "ledger-integrity", "event-vocabulary", "record-search", "record-retention", "retention-caps", "semantic-retrieval"]
+++
## The moment

Three sessions ran overnight. One finished, one was handed over, one stopped somewhere. The
question is simple — what was started, what was accepted, at which commit — and the only
material is a directory of chat logs and a git log that says "wip".

## Why it happens

A transcript is a record of what was said, not of what happened. It is long, it is
narrative, it is in whatever words the worker chose, and the facts that matter — the task,
the head, the outcome — are diluted in it or absent from it. When the notes directory in
the source environment reached ten gigabytes and fifteen hundred files, recovering working
state meant a written runbook and up to half an hour of a person's time. A store that only
grows, in a format only a person can read, is not evidence; it is an archive nobody opens.

## What Majordomus does

Every event the tool records — a task started, a checkpoint, a decision, a question opened
or resolved, a handover, a finish with its outcome and the verification exit code — is one
line in an append-only ledger, and the vocabulary of events is closed on the way in and on
the way out. `majordomus history` reads it back as operational history: filtered by task,
event and time, oldest first so that one task's lines read as a narrative, with `--json`
for the raw lines. A line the tool cannot parse is a failure in `history --validate`,
`doctor`, `check` and `watch`, because a ledger that cannot be parsed cannot be evidence.

`majordomus search` is the other half: a literal, case-insensitive scan across handovers,
checkpoints, decisions, questions, prompts and the ledger, in that order, because that is
the order in which to trust them. It is not an index and not an embedding, on purpose. Both
stores have retention caps that `doctor` checks, and `history --rotate` archives the oldest
lines without ever deleting one.

## What it does not do

It records nothing about what anyone said; there is no transcript in the ledger and no way
to put one there. It does not summarise, rank or interpret. Ranked or semantic retrieval
over the records was considered and rejected, and the claims matrix carries that as a
rejected row rather than leaving it to a roadmap.

## Try it

```bash
majordomus history --since 12h
majordomus history --task t-20260903193012-a4f1
majordomus search "callback" --kind decision --kind checkpoint
```
