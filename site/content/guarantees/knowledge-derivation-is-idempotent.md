+++
title = "A second derivation over the same evidence writes nothing, says so, and leaves the working tree unchanged; the record id is derived from the episode and a digest of the evidence so the same fact always lands in the same file"
description = "Running the deriver twice over the same ledger and the same git produces the same bytes the second time, reports every record as unchanged, and leaves nothing for git status to show. The end event, the compaction event and majordomus knowledge derive by hand all go through the same writer, so a compaction followed by a close, or a person re-running the command after a hook, never produces a second copy of a record."
weight = 185
[extra]
claim_id = "knowledge-derivation-is-idempotent"
status = "guaranteed"
source = "docs/claims/knowledge-derivation-is-idempotent.md"
+++
{% raw %}

## What it means

Running the deriver twice over the same ledger and the same git produces the same bytes the second time, reports every record as unchanged, and leaves nothing for `git status` to show. The end event, the compaction event and `majordomus knowledge derive` by hand all go through the same writer, so a compaction followed by a close, or a person re-running the command after a hook, never produces a second copy of a record.

## How it works

A record's id is the episode id followed by a digest of the canonical evidence string — the event name, the task and the text of the decision, question or outcome — so the same fact always lands in the same file, and two worktrees never write one name for different content. The record's `date` is the day of the evidence line, never the clock, and no random value enters the file. The deriver composes the record into a temporary file in the same directory, compares it byte for byte with the file the id names, removes the temporary file when they are identical and renames it over the old one when they differ. The ledger line `knowledge.derived` carries both counts, written and unchanged. `--dry-run` reports what would be written and touches neither the tree nor the ledger.

## How to see it

```bash
majordomus knowledge derive                             # written .ai/repo/knowledge/candidates/<id>.md
git add .ai/repo/knowledge && git commit -q -m "candidates"
majordomus knowledge derive                             # unchanged .ai/repo/knowledge/candidates/<id>.md
git status --porcelain                                  # empty
majordomus knowledge derive --dry-run                   # says what it would do; the ledger gains no line
majordomus history --event knowledge.derived            # written 0, unchanged 1 on the second line
```

## What it does not cover

Idempotence is over the evidence, not over time: a new decision in the same episode is a new record. A candidate that was promoted or rejected is skipped, not rewritten, so the acts survive a re-run; a candidate a person edited by hand is rewritten by the next derivation, because the evidence and not the file is canonical while it is still a candidate.

## Why it exists

The atomic writer every local record uses appends a random suffix and never overwrites, which is right for a checkpoint and wrong for a record that must be the same file on every run. A deriver that wrote a fresh file at every boundary would fill the review queue with copies and make the tree dirty after every compaction, and the ledger would say it wrote something when nothing changed. `.ai/repo/adrs/0058-knowledge-is-derived-at-the-episode-boundary-and-a-person-promotes-it.md` fixes the id-named file and the temporary-and-rename write.
{% endraw %}
