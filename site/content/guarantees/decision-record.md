+++
title = "A decision is recorded with its reason and its task, and is superseded by a later entry rather than edited"
description = "state/decisions.md was a file the finish contract read and nothing wrote. majordomus decision add writes it: what was decided, why, what was rejected, what the evidence is, which task decided it, and at which commit."
weight = 61
[extra]
claim_id = "decision-record"
status = "guaranteed"
source = "docs/claims/decision-record.md"
+++
{% raw %}

## What it means

`state/decisions.md` was a file the finish contract read and nothing wrote. `majordomus decision add` writes it: what was decided, why, what was rejected, what the evidence is, which task decided it, and at which commit.

`--why` is required. A decision with no recorded reason cannot be reviewed later, only re-argued, and re-arguing a decision whose rationale was lost is how a team makes the same change twice in opposite directions.

## How it works

`decision add` appends one entry. `Task` and `Head` are computed. Absent optional fields are written as `-` rather than omitted, so every entry has the same shape and a reader never has to distinguish "not applicable" from "the writer forgot".

An entry is never edited or deleted. `--supersedes "<text>"` records that a later decision replaced an earlier one, and refuses text matching no recorded decision — a supersession always points at something real.

`decision list` prints newest first, with `--task` and `--limit`; `decision show "<text>"` prints the first entry whose title matches, and exits `12` when nothing does rather than succeeding with empty output. Text inside an HTML comment is the file's own template and is not an entry.

`check`, `doctor` and `watch` report an entry missing `Task`, `Head` or `Why` as a **warning**: the file is hand-editable by design, and an entry nothing can attribute is a decision that no gate will ever find. The `deep-work` profile's `decision_record_required` then refuses a completed finish that has no entry naming the task — that refusal is what makes the warning matter.

## How to see it

```bash
majordomus decision add "Normalise the callback URI before comparing state" \
  --why "the mismatch is a trailing slash, not a forged state parameter" \
  --rejected "relaxing the comparison, which would accept genuinely forged states" \
  --evidence "test/auth/callback_test.exs:41"

majordomus decision list --limit 1
majordomus decision show "callback URI"
majordomus decision add "x" --why w --supersedes "a decision nobody made"; echo $?   # 2
```

## What it does not cover

There is one level of decision, not two. A task-local choice and an architectural one go in the same file, distinguished by their text. Nothing here is an ADR process, and a repository that wants one should keep it where it keeps its other documents.

Nothing checks that a decision is honoured. The record says what was decided; the diff says what was done; comparing them is a review, not a check.

Supersession is a text reference, not a graph. Nothing renders a chain or detects a cycle.

## Why it exists

The decisions in an AI-assisted repository are made inside conversations that end. Six weeks later the code shows what was chosen and nothing shows what was rejected or why, so the next worker re-opens the question with less information than the one who closed it.
{% endraw %}
