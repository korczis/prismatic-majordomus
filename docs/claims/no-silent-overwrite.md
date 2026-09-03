# A hand-edited instruction file is never silently overwritten

## What it means

If a projection target on disk matches neither its recorded fingerprint nor the freshly generated output, `update` refuses to touch it (exit code 15), names the file, and points at `update --diff <target>`. Overwriting requires `--force`, given knowingly. Nothing you typed into a generated file disappears without a decision.

## How it works

Before writing anything, `lib/update.sh` renders every target into a temporary directory and classifies each as unchanged, updatable (current file matches its fingerprint), creatable (absent), or hand-edited. One hand-edited target aborts the whole run before any write, so a partial update cannot occur. `--dry-run` prints the plan without writing.

## How to see it

```bash
echo "my own rule" >> CLAUDE.md
majordomus update
# REFUSE projection CLAUDE.md — current content matches neither its fingerprint nor the new output (hand-edited?); use --diff CLAUDE.md, then --force
majordomus update --diff CLAUDE.md
majordomus update --force
```

## What it does not cover

`--force` is a deliberate override; Majordomus records nothing about the discarded edit beyond the diff you were shown.

## Why it exists

The general rule for every command: no silent destructive behaviour. The same protection covers `init` (refuses an existing installation) and `start` (refuses while a task is active).
