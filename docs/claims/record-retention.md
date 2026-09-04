# Rotating the ledger archives the oldest lines and never deletes them

## What it means

`ledger.retention_max_lines` has always been reported by `doctor` and `watch` when exceeded. `majordomus history --rotate` is the action that resolves it: the oldest lines move into a dated archive file beside the ledger, and the newest stay.

Nothing is deleted, ever.

## How it works

The archive is `state/ledger.<utc-compact>.jsonl.archived`. Rotation refuses if that path already exists rather than appending to or overwriting it, writes the archive before truncating the ledger, and appends a `ledger.rotated` event recording how many lines moved, how many were kept, and the archive's path — so the history of the history is itself in the history.

Under the cap it does nothing and says so, rather than producing an empty archive.

## How to see it

```bash
before=$(wc -l < .majordomus/state/ledger.jsonl)
majordomus history --rotate
# rotated: 4812 line(s) to .majordomus/state/ledger.20260903T204411Z.jsonl.archived, 5000 kept

archived=$(wc -l < .majordomus/state/ledger.*.jsonl.archived)
kept=$(wc -l < .majordomus/state/ledger.jsonl)
# archived + kept accounts for every line that existed, plus the rotation event itself

majordomus history --event ledger.rotated --all
```

## What it does not cover

Rotation is manual. Nothing rotates on a schedule or on write; `doctor` and `watch` report the cap and a person or a hook decides. A tool that silently truncated a record store while you were not looking would be the opposite of the point.

`history` reads the live ledger only. An archived file is a plain JSONL you can read yourself; nothing joins it back automatically.

Handover and checkpoint stores have caps that are reported and never enforced. There is no rotation for record files, because deleting a continuation record is a decision a person should make deliberately.

## Why it exists

Append-only stores that nobody prunes become the thing that made an append-only store seem like a bad idea. One repository in the material this design came from accumulated gigabytes of snapshots that nothing ever read. A cap with no action is a nag; an action that deletes is a hazard. Archiving is the version that is safe to run.
