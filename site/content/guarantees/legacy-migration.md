+++
title = "A repository on the pre-.ai layout is migrated once, explicitly, with a previewed plan and a verified backup of its local state, and the command is idempotent afterwards"
description = "A repository whose project data still lives under .majordomus/ is not silently upgraded. Every ordinary command refuses that layout with exit 12 and names majordomus migrate. The migration itself prints its whole plan under --dry-run, one line per file with its action and destination, and writes nothing; the real run moves the canonical files into .ai/repo/ with history following, moves the state into .ai/local/state/ and takes it out of the index, backs the state up byte for byte first and verifies the copy, drops only the derived files it can regenerate, and reports a file it does not recognise rather than deleting it. Run again on a migrated repository it says so and exits 0."
weight = 73
[extra]
claim_id = "legacy-migration"
status = "guaranteed"
source = "docs/claims/legacy-migration.md"
+++
{% raw %}

## What it means

A repository whose project data still lives under `.majordomus/` is not silently upgraded. Every ordinary command refuses that layout with exit `12` and names `majordomus migrate`. The migration itself prints its whole plan under `--dry-run`, one line per file with its action and destination, and writes nothing; the real run moves the canonical files into `.ai/repo/` with history following, moves the state into `.ai/local/state/` and takes it out of the index, backs the state up byte for byte first and verifies the copy, drops only the derived files it can regenerate, and reports a file it does not recognise rather than deleting it. Run again on a migrated repository it says so and exits `0`.

## How it works

`lib/migrate.sh` builds one plan table from the skeleton manifest — the destinations come from the same manifest `init` uses — and the same table drives the dry run and the real run. Tracked canonical files move with `git mv`; templates identical to the tool's own are dropped and customised ones kept; `generated/` is dropped because every projection now carries its own stamp. The state is copied under `tmp/majordomus-migrate-backup/<utc>/state/` and compared before it moves. A `.majordomus/` that also holds a tool installation, a destination under `.ai/` that already exists, or a legacy policy that does not parse each refuse before anything is written. The run ends by re-stamping the projections and running `doctor`, reporting each exit on its own line.

## How to see it

```bash
majordomus check                       # majordomus: project data lives under .majordomus/ (the pre-.ai layout); run: majordomus migrate   exit 12
majordomus migrate --dry-run           # move .majordomus/policy.yaml -> .ai/repo/policy.yaml … dry run: nothing written
majordomus migrate                     # backup .majordomus/state/ -> tmp/majordomus-migrate-backup/…/state/ (byte for byte; verified) … migrated: …
majordomus migrate                     # already on the .ai layout; nothing to do   exit 0
```

## What it does not cover

The backup is on the same disk as the repository, which protects against a bad move and not against a lost machine. The state stops being tracked, so its durability after migration is the checkout plus the backup, not the branch; the tracked half is left for a person to review and commit.

## Why it exists

A layout change that a tool performs on its own, on first run, is the moment users lose data and cannot say what they lost. Making the move a command with a plan, a backup and a refusal for everything it does not understand turns it into a reviewable step. `test/cases/66_migrate_legacy.sh` builds a legacy tree by hand and proves the dry run, the backup, the refusals and the idempotence.
{% endraw %}
