# The context a worker is given is assembled from durable state in authority order

## What it means

`majordomus context` prints one briefing: the git identity, the task and the profile constraining it, the questions blocking acceptance, recent decisions, the newest checkpoint, the most relevant handover, the files already touched in scope, and recent history — in that order. A worker that has lost its conversation runs one command and has what it needs, without reading a transcript or reconstructing state from someone's memory.

The order is authority order, and it is deliberate. Git first, because it is the only thing that cannot be stale. Then the task and its profile, because they say what may be done. Then blockers, because they change what may be accepted. Authored records next, and event history last, because it is the weakest evidence about the present.

## How it works

`mj_context_sections` in `lib/context.sh` writes each section to a numbered file, then `mj_ctx_render` concatenates them in that order with a header and a trailer. Which sections exist is decided by the active profile's `context` block: `context.decisions`, `context.relevant_files`, `context.recent_history_depth`, `context.architecture_notes`, `context.current_state`, `context.task`. A `routine` task gets the task and the state; `deep-work` gets the repository's decisions rather than only its own.

This is the only code that reads those fields. Before it existed they were written by `init` and read by nothing, which made them documentation wearing the costume of configuration.

Every omission is printed under `EXCLUDED` with the field that caused it — `decisions — profile routine sets context.decisions: false` — so an under-filled context is debugged by reading the list rather than by guessing.

Nothing is persisted. The output is recomputed from the records and git on each run, so it cannot become a stale artifact of its own.

## How to see it

```bash
majordomus start "fix the callback" --scope lib/auth --profile debugging
majordomus context
# ## GIT / ## TASK / ## PROFILE debugging / ## OPEN QUESTIONS / ## DECISIONS ...

majordomus context --json | jq '[.sections[].id]'
# ["profile","questions","decisions","checkpoint","handover","files","history"]

# the same task under a profile that asks for less
majordomus context | sed -n '/## EXCLUDED/,$p'
```

## What it does not cover

It does not measure what the worker then reads. The briefing is offered; nothing observes whether it was used, and the `minimum-context` claim stays advisory for exactly that reason.

It does not capture command output. A profile with `context.failing_output: true` is told so honestly in the exclusion list: Majordomus does not run your tests and cannot paste their output for you.

It selects records, it does not judge them. A handover whose claims are wrong is included, labelled with how far git has moved since it was written.

## Why it exists

The alternative that every AI-assisted repository reaches for first is saving the conversation, and it fails three ways: a transcript is not state, it grows with the session rather than the work, and it goes stale without saying so. Selecting from typed records, in a fixed order, with the exclusions named, is the version of the same idea that survives contact with a repository that keeps moving.
