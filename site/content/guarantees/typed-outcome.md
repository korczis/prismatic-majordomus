+++
title = "An outcome is a value from a closed vocabulary, not free text"
description = "A task ends with exactly one of completed, partial, blocked, no_match, failed (plus handed_over when it continues elsewhere and active while it runs). no_match means the work was done and the thing sought does not exist; failed means the work could not be done. They look alike in a chat — \"we found nothing\" versus \"we could not search\" — and are different facts a supervisor must tell apart to decide whether to retry, escalate or accept."
weight = 49
[extra]
claim_id = "typed-outcome"
status = "guaranteed"
source = "docs/claims/typed-outcome.md"
+++
{% raw %}

## What it means

A task ends with exactly one of `completed`, `partial`, `blocked`, `no_match`, `failed` (plus `handed_over` when it continues elsewhere and `active` while it runs). `no_match` means the work was done and the thing sought does not exist; `failed` means the work could not be done. They look alike in a chat — "we found nothing" versus "we could not search" — and are different facts a supervisor must tell apart to decide whether to retry, escalate or accept.

## How it works

`lib/finish.sh` rejects any other `--outcome` value with a usage error. Each outcome has its own contract: `completed` requires verification and the full note; `partial` and `blocked` require a note with `# Next Action`; `no_match` and `failed` require `# Reason`; `blocked` skips the open-questions line because open questions are expected. The value is written into the task record and the ledger, where `watch` checks it against the ledger's `task.finished` events.

## How to see it

```bash
majordomus finish --outcome done
# majordomus: finish: unknown outcome 'done'
majordomus finish --outcome no_match --note n.md
# … lacks section(s): Reason   (until the note explains what was not there)
```

## What it does not cover

The vocabulary is fixed in v0.1; adding an outcome is a contract change to `finish`, not a configuration option.

## Why it exists

One hundred and fifty-seven of two hundred and ninety-eight session notes in the source environment carried a `Status:` line; the values were free text, in two languages, including one that read "very low probability, critical impact". Prose never substitutes for the typed field.
{% endraw %}
