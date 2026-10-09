# knowledge observe records one observation.recorded line of a fixed shape, and refuses any other

## What it means

A worker records friction it met — a workaround, a defect, a drift, a repeated manual step — with one command, `majordomus knowledge observe`, and the ledger gains one `observation.recorded` line carrying the kind, the subject, the statement and the evidence references. The command works inside a task and outside one. Anything not of that shape is refused with exit 2 and nothing is written.

## How it works

`--kind` is one of `friction`, `workaround`, `defect`, `drift`, `repetition`. `--subject` is a repository-relative path that does not climb out of the repository, or `capability:`, `rule:`, `command:` or `gate:` followed by an identifier, so that a route can later be derived from the subject's structure rather than from words. Each `--evidence` is `file:`, `test:`, `commit:`, `issue:` or `claim:` followed by what it names. The statement is one quoted argument on one line, and a statement that opens like a turn of a conversation is refused for the reason a knowledge record may not carry one. The line names the active task, or `none`.

## How to see it

```bash
majordomus knowledge observe --kind friction --subject scripts/site-build \
  --evidence commit:bf9b27caeb "a moved head composes a stale rustdoc surface"
majordomus history --event observation.recorded
majordomus knowledge observe --kind annoyance --subject lib "x"; echo "exit $?"   # 2, nothing written
```

## What it does not cover

It judges nothing: whether the observation is right, new or worth acting on is decided at the episode boundary and by the person who reviews the candidate. It does not record observations from a transcript, a hook or a model; the worker states them.

## Why it exists

On 2026-10-08 four concurrent sessions met about thirty pieces of friction with the tool, and every one of them ended up in an agent's private memory or in a message between sessions, where the repository could not read it. `.ai/repo/adrs/0118-a-session-leaves-what-it-observed-and-one-candidate-per-defect-is-routed.md` gives the repository one typed line per observation, the same kind of act as recording a decision.
