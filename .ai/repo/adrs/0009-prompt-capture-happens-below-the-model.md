---
schema: adr/v1
id: adr-0009
kind: adr
title: Prompt capture happens below the model, or is reported as unavailable
status: accepted
date: 2026-09-05
tags:
  - capture
  - providers
  - privacy
provenance:
  origin: authored
---

# 9. Prompt capture happens below the model, or is reported as unavailable

## Context

`.ai/local/prompts/` has been part of the layout since the `.ai` transformation, described
as raw local user-prompt history, and it has been empty. The `05_AI_PROTOCOL.md` note that
defined it was careful — prompts are stored "when a capable integration can observe them",
and Majordomus "MUST NOT claim it captures prompts from providers it cannot observe" — so
the emptiness was honest rather than broken. But no integration was ever built, and the
gap between a declared directory and a capability is exactly the kind of thing that gets
read as a feature.

The obvious fix is a line in `AGENTS.md` telling the worker to write its prompts down. It
cannot work, for four separate reasons:

- **The model never sees the bytes.** What reaches it is what the provider assembled;
  anything it wrote back would be a reconstruction, and a reconstruction is not evidence.
- **It is not deterministic.** The write depends on the model choosing to make a tool call.
  It will happen for the first prompt of a session and not the twentieth, producing an
  append-only archive with holes nobody can see.
- **It is not verifiable.** There is no behavioural test that can prove "the worker wrote
  the prompt down after reading a file", so under `no-claim-without-test` it could never be
  a guarantee — only advisory prose.
- **It is circular.** The instruction is loaded when the worker reads `.ai/`, which happens
  after the session's first prompt has already been submitted. The one prompt that usually
  carries the whole intent is the one such a rule structurally cannot capture.

## Decision

- **Capture is a provider hook, not an instruction.** `majordomus capture prompt` reads one
  payload on stdin and writes one record. For Claude Code the event is `UserPromptSubmit`,
  which runs before the model is invoked. Providers with no such event are reported
  `unsupported`; none is assumed.
- **One prompt is one file**, `YYYYMMDDHHMMSS-<slug>.json`, the slug being the opening of
  the prompt. A shared append-only file would make every record depend on every earlier
  one: a truncation loses the tail, two hooks can interleave, and finding a prompt means
  reading the file. Separate files are immutable once written, sort chronologically by
  name, and make the directory listing a readable history on its own. Identity for
  idempotence stays a field inside the record — a hook delivered twice within the day
  writes once — and the slug stays a hint, since escapes and non-ASCII collapse into
  hyphens while the prompt inside the file is untouched.
- **A shim is the wiring, and it is tracked.** `capture install` writes
  `.claude/hooks/majordomus-capture` and the entry in `.claude/settings.json`, and refuses
  to rewrite either — it prints what to add and exits 15. The shim derives the repository
  from its own path: the provider substitutes its project directory into the command string
  textually, exports nothing that names the repository, and does not contract the working
  directory a hook runs in.
- **`wired_by: provider-hook:<provider>` is an enforcement kind, verified by execution.**
  `doctor` drives a synthetic payload through the shim into a throwaway archive and reads
  the record back. The states are `unsupported`, `unconfigured`, `named`, `wired` and
  `verified`, and only `verified` satisfies a declared enforcement. Because `doctor` runs on
  `pre-commit`, a hook that stops capturing stops the commit.
- **The archive is evidence, never knowledge.** It stays under the ignored half of the
  layer; nothing loads it into a context and no command retrieves from it. The writer emits
  a closed field set, so the model's half of the exchange has no field to arrive in, and
  the `prompt-capture` doctrine fails if anything under the archive is tracked by Git or if
  a record carries a response, a completion or a transcript.
- **The text is copied, not re-encoded.** `lib/json_scan.awk` returns the raw JSON span of
  each top-level member, so the captured prompt is byte-identical to what the provider sent
  and no decode-and-re-encode step can lose an escape. It refuses a payload it cannot parse
  rather than reading part of one.
- **Payload fields are candidates, and injected messages are filtered.** The adapter names
  an ordered list of keys the prompt may arrive in rather than one, and a list of origins
  and opening markers that mean the provider is talking to itself rather than a person
  writing. Both were forced by evidence within an hour of the hook going live: the installed
  Claude Code sends the prompt in `prompt`, not the `prompt_text` the documentation
  described, and it fires the same event for `<task-notification>` messages it injects. The
  first was silently losing every prompt and was caught only by the log; the second was
  filling the archive with the provider talking to itself. A skip is not a failure, so it
  writes neither a record nor a log line.
- **A failed capture is a blocking failure, through a log.** The command cannot fail
  loudly, so it writes `.capture.log` beside the records and `doctor` fails while that log
  has entries. This is the only check that catches the failure that actually threatens the
  guarantee: a provider renaming the field the prompt arrives in leaves the hook running and
  the self test passing, because the self test sends a payload of the tool's own making. A
  log nothing reads would have been decoration; this one is the detector.
- **Nothing deletes a record, and there is no cap to configure.** The ledger, the
  checkpoints and the handovers rotate under `retention_max_*` because each restates state
  that survives elsewhere. A prompt does not: it existed once, was derived from nothing,
  and no other file in the repository can reconstruct it. Silently discarding the oldest
  evidence to bound a directory is the failure mode this whole change exists to remove, so
  the archive grows, `doctor` reports its count and size, and removal is a person's
  deliberate act.
- **A repository opts in.** `init` installs no hook into anyone's provider configuration
  and the skeleton declares no enforcement. A repository that wants the guarantee runs
  `capture install` and declares the entry, and is then held to it.

## Alternatives rejected

- **A rule in `AGENTS.md` or a project rule with `class: advisory`.** For the four reasons
  above. It would also have to be described as advisory, which is the opposite of what was
  asked for.
- **Reading the provider's transcript file.** `UserPromptSubmit` hands over a
  `transcript_path`, and parsing it would capture more. It would also capture the model's
  output, which `never-store-transcripts` forbids, and would couple the tool to one
  provider's on-disk format.
- **Rewriting an existing `settings.json` by merging JSON.** Rejected under
  "no silent overwrite": the tool has no JSON writer, a merge would need one, and a hook
  configuration someone else wrote is not this tool's to edit. Refusing with the exact
  entry to add is smaller and honest.
- **Executing the command string found in the provider's configuration to verify it.** That
  is `eval` by another name, which `no-network-no-eval` forbids. Verifying the shim the tool
  wrote, and separately checking that the configuration names it, gives the same evidence
  with a named state (`named`) for the case where the two differ.

## Consequences

- The guarantee is scoped, and the scope has to be said out loud: prompts submitted through
  a wired provider adapter in this checkout. Prompts from the web, from the desktop
  application, from another machine or from an unwired provider are not observable here and
  never will be through this mechanism.
- `doctor` now forks once per declared `provider-hook` entry to run the self test. The cost
  is on the `pre-commit` path and belongs in the measured budget, not in a guess:
  `majordomus bench doctor` is what moves `benchmark.budget.doctor_ms`.
- The archive is the most sensitive thing the tool writes, and now also the only thing it
  writes without a bound. `.ai/local/` was already ignored and the doctrine makes committing
  a record a blocking failure rather than a convention, but growth is reported, not
  governed. If it ever needs to be bounded, that belongs in an explicit command a person
  runs, never in the hook.
- The documentation of a provider's payload is not evidence about that provider's payload.
  This one was wrong about the field name, and the design now assumes it will be wrong
  again: candidates instead of one name, and a log that reports what a payload actually
  carried. The same scepticism should apply to any adapter added later.
- Codex and Gemini gain nothing until someone establishes whether either has an equivalent
  event. Until then `capture status` says `unsupported`, which is a finding and not a gap.
