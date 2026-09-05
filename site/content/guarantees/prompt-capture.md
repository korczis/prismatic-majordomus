+++
title = "A declared provider hook captures the person's raw prompts below the model, and doctor proves it by running it"
description = ".ai/local/prompts/ holds the prompts a person actually submitted, written by the provider's own hook before the model was invoked. A repository that declares the enforcement is held to it: if the hook stops capturing, doctor fails, and because doctor runs on pre-commit, the commit stops with it. A provider whose prompts cannot be observed is reported as unsupported, not treated as one that had nothing to say."
weight = 28
[extra]
claim_id = "prompt-capture"
status = "guaranteed"
source = "docs/claims/prompt-capture.md"
+++
{% raw %}

## What it means

`.ai/local/prompts/` holds the prompts a person actually submitted, written by the provider's own hook before the model was invoked. A repository that declares the enforcement is held to it: if the hook stops capturing, `doctor` fails, and because `doctor` runs on `pre-commit`, the commit stops with it. A provider whose prompts cannot be observed is reported as `unsupported`, not treated as one that had nothing to say.

## How it works

`capture install` writes a shim at `.claude/hooks/majordomus-capture` and the `UserPromptSubmit` entry in `.claude/settings.json`, refusing to rewrite either — it prints the entry to add and exits 15. Claude Code runs the shim before the model sees the prompt and hands it a JSON payload on stdin; `mj_capture_prompt` in `lib/capture.sh` reads it with `lib/json_scan.awk` and writes one record as one file, `.ai/local/prompts/YYYYMMDDHHMMSS-<slug>.json`, where the slug is the opening of the prompt. A record is immutable once written, two hooks racing cannot interleave inside one file, the names sort chronologically, and a person can find a prompt by listing the directory.

The scanner returns the raw JSON span of each top-level member, so the captured text is byte-identical to what the provider sent: there is no decode-and-re-encode step that could lose an escape, and a payload the scanner cannot parse is refused whole rather than read in part. Records stay idempotent on the provider's own prompt identity, which is a field rather than the name: a hook delivered twice within the day writes once. A redelivery days later is not a case this claims to cover, and the code says so where the window is chosen.

`wired_by: provider-hook:<provider>` is an enforcement kind, and `doctor` decides it by driving a synthetic payload through the shim into a throwaway archive and reading the record back. That is why the states are five words and not two: `unsupported`, `unconfigured`, `named`, `wired`, `verified`. Only `verified` satisfies a declared enforcement, and only `verified` means a payload actually came out the other side.

A capture cannot fail loudly, so it fails into `.capture.log` beside the records, and a non-empty log is a blocking failure rather than a note. It is the only check that catches the important failure: if the provider renames the field the prompt arrives in, the hook still runs and the self test still passes, because the self test sends a payload of the tool's own making. Without the log, every prompt would be lost in silence.

Nothing prunes the archive. A prompt is not a restatement of state held elsewhere, so no cap governs it and no command removes one as a side effect; `doctor` reports the count and the size instead, and deleting is a person's deliberate act.

The doctrine `majordomus.prompt-capture` separately holds the archive: nothing under it may be tracked by Git, the ignore boundary must cover it, and every record must carry the closed field set. A record naming a response, a completion or a transcript is a failure, because the archive holds the person's half of the exchange and no more.

## How to see it

```bash
majordomus capture status         # claude-code  unconfigured
majordomus capture install
majordomus capture status         # claude-code  verified
chmod -x .claude/hooks/majordomus-capture
majordomus capture status         # claude-code  named — not executable
majordomus doctor                 # exit 10, FAIL wiring prompt-capture
```

## What it does not cover

The scope is prompts submitted through a wired provider adapter in this checkout, and it cannot be widened by this mechanism. Prompts typed on the web, in the desktop application, on another machine, or into a provider with no adapter are not observable from a hook that was never run, and none of them appear here. Only Claude Code has an adapter; Codex and Gemini report `unsupported` until someone establishes whether either has an equivalent event.

Nor does it cover everything the event delivers. Claude Code fires `UserPromptSubmit` for messages it injects into the turn as well, and those are filtered out by declared origin and by the markers they open with — deliberately, since they are not the person's prompts, but it does mean the archive is what the filter let through and not a transcript of the event.

Nor is the archive knowledge. Nothing loads it into a context, no command retrieves from it, and no command summarises it. It is evidence a person can read, and the tool's only claims about it are that it is real, complete for what it covers, and never committed.

## Why it exists

The directory was declared by the `.ai` protocol and stayed empty, which reads as a feature that does not work. The obvious repair — a line in `AGENTS.md` telling the worker to record its prompts — cannot be one: the model never sees the bytes the person typed, the write depends on it choosing to make a tool call, no behavioural test can prove it happened, and the instruction is loaded only after the session's first prompt has already been submitted. `.ai/repo/adrs/0007-prompt-capture-happens-below-the-model.md` records the decision that followed.
{% endraw %}
