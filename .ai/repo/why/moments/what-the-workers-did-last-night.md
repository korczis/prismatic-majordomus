---
schema: moment/v1
id: what-the-workers-did-last-night
kind: moment
title: 'Asking what the workers did last night, and getting a transcript'
short_title: 'Only transcripts'
hook: 'asked what the workers did last night and had only transcripts to grep'
summary: 'A conversation log records what was said, not what happened, so the operational question has to be answered by a person reading prose.'
status: stable
severity: high
frequency: common
weight: 80
featured: true
audiences: [engineering-lead, ai-native-team, enterprise, research-team]
areas: [observability]
lifecycle: [operations, review]
tags: [observability, ledger, events, audit]
signals:
  - id: grep-the-logs
    text: 'Answering "what happened overnight" meant reading or grepping conversation logs.'
  - id: no-structured-events
    text: 'There is no structured record of what was started, decided or accepted — only prose.'
  - id: git-log-says-wip
    text: 'The git history is the only machine-readable trace, and it says "wip".'
examples:
  - id: three-overnight-sessions
    audience: ai-native-team
    title: 'Three sessions, one morning'
    before: 'One finished, one was handed over, one stopped; the material is a directory of chat logs and a git log that says "wip".'
    after: 'Every event is one line in an append-only ledger with a closed vocabulary; `history --since 12h` answers the question directly.'
  - id: weekly-report
    audience: engineering-lead
    title: 'The Monday question'
    before: 'What shipped, what was decided and what is blocked are reconstructed by asking three people.'
    after: '`history` filtered by event and time reads as operational history, oldest first, per task.'
  - id: evidence-request
    audience: enterprise
    title: 'Show what happened, not what was said'
    before: 'The available artefact is a transcript: voluminous, unstructured and not evidence of anything.'
    after: 'The ledger carries the task, the head, the outcome and the verification exit code, and refuses to hold a conversation.'
commands: [history, search, watch]
capabilities: [objects.search, perf.counters]
claims: [history-ledger-read, ledger-integrity, event-vocabulary, record-search, record-retention, retention-caps, semantic-retrieval]
doctrines: [majordomus.ledger-integrity, project.never-store-transcripts, majordomus.retention-caps, majordomus.sessions-are-workers]
use_cases: [read-back-what-happened, open-and-close-a-session, find-an-object-without-reading-everything]
related: [failure-disappears-between-sessions, who-did-this-and-under-what-policy, spend-not-tied-to-outcomes]
aliases: ['no telemetry for agents', 'overnight run visibility', 'transcripts are not evidence']
---

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

## Why a better model does not fix it

Asking a model to summarise the transcripts produces a fluent paragraph with no provenance,
which is the same problem one level up. The missing thing is a structured event at the
moment the event happened; nothing recovered afterwards from prose has the same standing.

## What it costs

Half an hour of a senior person, every morning, to produce an answer that is a
reconstruction rather than a reading. And the answer cannot be trusted enough to act on
without checking, so it is often produced twice.

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
the order in which to trust them. It is not an index and not an embedding, on purpose.

## Before and after

```text
before   $ ls .sessions/ | wc -l ; grep -ri "auth" .sessions/ | less
         1500

after    $ majordomus history --since 12h
         task_started    t-…a4f1  profile=implementation  head=8c31f0e
         decision        t-…a4f1  "normalise the callback URI before comparing"
         handover        t-…a4f1  advanced
         task_finished   t-…b207  outcome=partial  verify=make test  exit=0  1m12s
```

## How to verify it

Run a task through start, checkpoint and finish, then read `history --task <id>`. Every line
is an event from the closed vocabulary with facts computed from git; corrupt a line and
`history --validate`, `doctor`, `check` and `watch` all fail.

## What it does not do

It records nothing about what anyone said; there is no transcript in the ledger and no way
to put one there. It does not summarise, rank or interpret. Ranked or semantic retrieval
over the records was considered and rejected, and the claims matrix carries that as a
rejected row rather than leaving it to a roadmap.
