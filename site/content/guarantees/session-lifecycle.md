+++
title = "A provider hook opens the episode below the model and hands the worker what the last one left, records what a compaction is about to discard, and closes the episode with a continuation record beside its envelope"
description = "Where a provider fires session events, the episode boundary is drawn by that provider's own hook rather than by the model working inside it. SessionStart opens the episode and hands the worker the briefing the policy declares, PreCompact records what the conversation is about to stop holding, SessionEnd writes a continuation record and closes the episode into the shared record under .ai/repo/sessions/, and none of it depends on a worker remembering to run a command. At the open, the context the builder resolved is written to .ai/local/session-contexts/ and kept for the episode, so what the worker was told is evidence rather than recollection."
weight = 150
[extra]
claim_id = "session-lifecycle"
status = "guaranteed"
source = "docs/claims/session-lifecycle.md"
+++
{% raw %}

## What it means

Where a provider fires session events, the episode boundary is drawn by that provider's own hook rather than by the model working inside it. A start event opens the episode and hands the worker the briefing the policy declares, a compaction event records what the conversation is about to stop holding, an end event writes a continuation record and closes the episode into the shared record under `.ai/repo/sessions/`, and none of it depends on a worker remembering to run a command. Every provider with such events gets all three; what each calls them is that vendor's business and is declared rather than assumed. At the open, the context the builder resolved is written to `.ai/local/session-contexts/` and kept for the episode, so what the worker was told is evidence rather than recollection.

## How it works

`capture install` writes three more shims beside the prompt one — `majordomus-session-start`, `majordomus-session-end` and `majordomus-session-compact`, under the hook directory that provider reads — and the matching entries in that provider's own configuration file, refusing to rewrite a configuration it did not write and naming each entry that is missing. Each shim hands its payload to `majordomus capture session --provider <name> --event start|end|compact`, which reads the provider's session identity and the event's source or reason with `lib/json_scan.awk` and calls `session start`, `session close` or `checkpoint --derive`. What an adapter is — the file, the dialect, each event's own name, the payload keys, the end reasons that mean a deliberate ending, and what the provider does with the start hook's standard output — is one `hooks` block per provider in `share/providers.yaml` (ADR 0024). Adding a provider is that block; the shell holds the event kinds this tool recognises, the shim names, and one writer per configuration dialect, because a file format is a syntax and a syntax is not a field.

Every direction is idempotent, because the events are. A start event fires again on a resume, so the start passes `--if-open keep` and the already-open episode is kept rather than replaced. An end event fires whether or not anything was opened, so the close passes `--if-none ignore` and writes nothing when there is nothing to close. A compaction with no active task records nothing, because a checkpoint is a progress note inside a task and one written outside a task would belong to no work. The event's reason decides the outcome: a reason the declaration lists as deliberate closes the episode as `closed`, and anything else — a crash, a name the declaration has not seen, or a provider whose end event says nothing about itself at all — closes it as `interrupted`, because calling a cut-short episode complete is the worse of the two mistakes.

The start hook writes a briefing to standard output and the other two write none. A provider adds a start hook's output to the context it is about to build, which is the one moment at which a continuation record reaches a worker without the worker remembering to ask. How it takes it off standard output is declared with the events: one adds the text verbatim, another parses a single JSON object and reads `hookSpecificOutput.additionalContext` out of it, and text that is not that object is not a briefing to it and is dropped without saying so — and a record nothing loads is a record nobody reads. What the briefing may carry is bounded by `session.briefing_budget_lines` and limited to references, divergence labels, the blockers that refuse acceptance, and the one section of a handover a resuming worker acts on; it never carries a conversation, and `session.briefing_on_start: false` restores silence. The end and compaction events fire inside a turn already under way, where output would alter what somebody is doing rather than furnish it, so they say everything on stderr. No hook exits 2, for the same reason `capture prompt` never does.

An episode that ends with its task still active leaves a continuation record before the envelope closes. The two documents answer different questions — the session record indexes what the episode produced, a handover is what the next worker resumes from — and until the end event wrote one, the next worker inherited the first and not the second. Its body is derived by `handover --derive` from the task record, the ledger, git and the open questions, all of which are already written and already validated; nothing calls a model.

`session start` freezes the working context: front matter carrying `schema: session-context/v1`, the episode's identity, the provider and the provider's own session id — the same string the prompt records carry, which is what ties the two stores together — and then the context builder's output, verbatim, under a heading. A `## Notes` section follows for the worker's own account of the work. `session close` appends a `## Close` section naming the outcome and the shared record; the document is appended to and never rewritten, so what a worker typed into it survives the close.

`wired_by: provider-hook:<provider>:session` is what makes a repository answerable for the wiring. `doctor` decides it by driving a synthetic payload through the end shim with the mutation disabled: the shim resolving the repository, finding the executable, the adapter matching and the payload parsing all run, and only the close is left out — closing somebody's open episode is not a price a diagnostic may charge. The five states are the prompt aspect's five states, because they are the same five facts.

## How to see it

```bash
majordomus capture install
majordomus capture status                    # claude-code:session  verified
printf '{"session_id":"cc-1","source":"startup"}' | .claude/hooks/majordomus-session-start
                                             # stdout is the briefing the next worker is handed
majordomus session status                    # an open episode nobody typed a command for
printf '{"session_id":"cc-1"}' | .claude/hooks/majordomus-session-compact
majordomus checkpoint --show                 # what the compaction recorded
majordomus session context                   # .ai/local/session-contexts/<stamp>--<id>.md
printf '{"session_id":"cc-1","reason":"prompt_input_exit"}' | .claude/hooks/majordomus-session-end
majordomus session latest                    # the closed record, with its commits and references
```

## What it does not cover

A provider with no session-start and session-end event has no lifecycle adapter and is reported `unsupported` with the reason its declaration gives — never silently treated as having no sessions. Which providers have one is `majordomus capture status` and `docs/generated/providers.md`, not a list written here. A session opened on the web, in another application or on another machine is not observable from a hook that was never run.

A hook a provider declines to run is outside what this can see. Codex holds every hook untrusted until a person trusts it, and clamps its end event to three seconds, so a close that writes a continuation record can be killed part-way; `capture status` reports the wiring it can drive and cannot report a refusal that happens inside the provider.

The working context is a snapshot, not a live view: it is what the builder resolved at the open, and the repository moves underneath it. It is also local, and stays local — it names this machine and it cannot be reproduced by any surface, so it is never published, never indexed, and never loaded into a context on its own.

It is not a transcript and cannot become one. The derived half is the builder's output and the authored half is a summary of the work; a front-matter key naming a message list, a completion or a model's reply is a blocking failure, which is how `project.never-store-transcripts` is kept mechanically rather than by memory.

## Why it exists

`.ai/local/session-contexts/` was named by the layer from the beginning and had no producer: `init` created the directory, one sentence of prose described it, and nothing ever wrote a file into it. A directory the skeleton creates and nothing fills is a promise the layer does not keep, and a reader cannot tell an empty store from an unimplemented one. Giving it a producer meant answering where the episode's boundary comes from, and the answer was the one ADR 0009 had already given for prompts: below the model, in the provider's hook, where running it is the proof that it works. `.ai/repo/adrs/0015-the-episode-boundary-is-drawn-by-the-provider-not-by-the-mod.md` records that decision, and `.ai/repo/adrs/0017-an-episode-that-opens-is-handed-what-the-last-one-left.md` records the briefing, the compaction event and the continuation record that follow from it.
{% endraw %}
