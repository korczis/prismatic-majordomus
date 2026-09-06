+++
title = "A provider hook opens and closes the execution episode below the model, and the context resolved at the open is frozen beside it"
description = "Where a provider fires session events, the episode boundary is drawn by that provider's own hook rather than by the model working inside it. SessionStart opens the episode, SessionEnd closes it into the shared record under .ai/repo/sessions/, and neither depends on a worker remembering to run a command. At the open, the context the builder resolved is written to .ai/local/session-contexts/ and kept for the episode, so what the worker was told is evidence rather than recollection."
weight = 137
[extra]
claim_id = "session-lifecycle"
status = "guaranteed"
source = "docs/claims/session-lifecycle.md"
+++
{% raw %}

## What it means

Where a provider fires session events, the episode boundary is drawn by that provider's own hook rather than by the model working inside it. `SessionStart` opens the episode, `SessionEnd` closes it into the shared record under `.ai/repo/sessions/`, and neither depends on a worker remembering to run a command. At the open, the context the builder resolved is written to `.ai/local/session-contexts/` and kept for the episode, so what the worker was told is evidence rather than recollection.

## How it works

`capture install` writes two more shims beside the prompt one — `.claude/hooks/majordomus-session-start` and `.claude/hooks/majordomus-session-end` — and the matching entries in `.claude/settings.json`, refusing to rewrite a configuration it did not write and naming each entry that is missing. Each shim hands its payload to `majordomus capture session --provider <name> --event start|end`, which reads the provider's session identity and the event's source or reason with `lib/json_scan.awk` and calls `session start` or `session close`.

Both directions are idempotent, because the events are. `SessionStart` fires again on a resume and on a compaction, so the start passes `--if-open keep` and the already-open episode is kept rather than replaced. `SessionEnd` fires whether or not anything was opened, so the close passes `--if-none ignore` and writes nothing when there is nothing to close. The event's reason decides the outcome: a reason the adapter lists as deliberate closes the episode as `closed`, and anything else — a crash, a name the table has not seen — closes it as `interrupted`, because calling a cut-short episode complete is the worse of the two mistakes.

Neither hook writes to standard output. Claude Code adds a `SessionStart` hook's output to the model's context, and nothing under the local half of the layer may be loaded into a context implicitly; diagnostics go to stderr, and the command never exits 2, for the same reason `capture prompt` never does.

`session start` freezes the working context: front matter carrying `schema: session-context/v1`, the episode's identity, the provider and the provider's own session id — the same string the prompt records carry, which is what ties the two stores together — and then the context builder's output, verbatim, under a heading. A `## Notes` section follows for the worker's own account of the work. `session close` appends a `## Close` section naming the outcome and the shared record; the document is appended to and never rewritten, so what a worker typed into it survives the close.

`wired_by: provider-hook:<provider>:session` is what makes a repository answerable for the wiring. `doctor` decides it by driving a synthetic payload through the end shim with the mutation disabled: the shim resolving the repository, finding the executable, the adapter matching and the payload parsing all run, and only the close is left out — closing somebody's open episode is not a price a diagnostic may charge. The five states are the prompt aspect's five states, because they are the same five facts.

## How to see it

```bash
majordomus capture install
majordomus capture status                    # claude-code:session  verified
printf '{"session_id":"cc-1","source":"startup"}' | .claude/hooks/majordomus-session-start
majordomus session status                    # an open episode nobody typed a command for
majordomus session context                   # .ai/local/session-contexts/<stamp>--<id>.md
printf '{"session_id":"cc-1","reason":"prompt_input_exit"}' | .claude/hooks/majordomus-session-end
majordomus session latest                    # the closed record, with its commits and references
```

## What it does not cover

Only Claude Code has a lifecycle adapter. A provider without one is reported `unsupported`, and a session opened on the web, in another application or on another machine is not observable from a hook that was never run.

The working context is a snapshot, not a live view: it is what the builder resolved at the open, and the repository moves underneath it. It is also local, and stays local — it names this machine and it cannot be reproduced by any surface, so it is never published, never indexed, and never loaded into a context on its own.

It is not a transcript and cannot become one. The derived half is the builder's output and the authored half is a summary of the work; a front-matter key naming a message list, a completion or a model's reply is a blocking failure, which is how `project.never-store-transcripts` is kept mechanically rather than by memory.

## Why it exists

`.ai/local/session-contexts/` was named by the layer from the beginning and had no producer: `init` created the directory, one sentence of prose described it, and nothing ever wrote a file into it. A directory the skeleton creates and nothing fills is a promise the layer does not keep, and a reader cannot tell an empty store from an unimplemented one. Giving it a producer meant answering where the episode's boundary comes from, and the answer was the one ADR 0009 had already given for prompts: below the model, in the provider's hook, where running it is the proof that it works. `.ai/repo/adrs/0015-the-episode-boundary-is-drawn-by-the-provider.md` records the decision.
{% endraw %}
