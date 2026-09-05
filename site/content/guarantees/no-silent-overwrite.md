+++
title = "A hand-edited instruction file is never silently overwritten"
description = "If a projection target on disk matches neither the stamp it carries nor the freshly generated output, update refuses to touch it (exit code 15), names the file, and points at update --diff <target>. Overwriting requires --force, given knowingly. Nothing you typed into a generated file disappears without a decision."
weight = 31
[extra]
claim_id = "no-silent-overwrite"
status = "guaranteed"
source = "docs/claims/no-silent-overwrite.md"
+++
{% raw %}

## What it means

If a projection target on disk matches neither the stamp it carries nor the freshly generated output, `update` refuses to touch it (exit code 15), names the file, and points at `update --diff <target>`. Overwriting requires `--force`, given knowingly. Nothing you typed into a generated file disappears without a decision.

## How it works

Before writing anything, `lib/update.sh` renders every target into a temporary directory and classifies each as unchanged, updatable (the content on disk matches the hash its own stamp names), creatable (absent), or hand-edited (the content differs from its stamp, or there is no stamp at all). One hand-edited target aborts the whole run before any write, so a partial update cannot occur. `--dry-run` prints the plan without writing.

## How to see it

```bash
echo "my own rule" >> CLAUDE.md
majordomus update
# REFUSE projection CLAUDE.md — current content matches neither its own stamp nor the new output (hand-edited?); use --diff CLAUDE.md, then --force
majordomus update --diff CLAUDE.md
majordomus update --force
```

## What it does not cover

`--force` is a deliberate override; Majordomus records nothing about the discarded edit beyond the diff you were shown.

## Why it exists

The general rule for every command: no silent destructive behaviour. The same protection covers `init` (refuses an existing installation) and `start` (refuses while a task is active).
{% endraw %}
