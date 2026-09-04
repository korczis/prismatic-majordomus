# Installing into a repository that already has an installation is refused

## What it means

`majordomus init` creates `.ai/`. If it already exists, `init` refuses with exit code 15 and says so. There is no flag that rewrites anything: `--extend` adds what is missing from the skeleton and overwrites nothing, so a policy, a profile or a rule the repository has edited stays exactly as it is, and `.ai/local/state/`, where the task record, decisions, open questions, handovers and ledger live, is never written over by any run of `init`.

A repository whose project data still lives under `.majordomus/`, the pre-`.ai` layout, is refused too, with `majordomus migrate` named as the way forward; `init` does not create a second layout beside the first.

## How it works

`lib/init.sh` resolves the layout before copying anything from `share/skeleton/`. On the `.ai` layout without `--extend` it refuses; with `--extend` every file is copied through one helper that returns without writing when the destination exists, and it lists what it created. The `.ai/local/` ignore line is appended to `.gitignore` once, however often `init` runs.

## How to see it

```bash
majordomus init
majordomus init          # majordomus: .ai/ already exists in … (use --extend to add what is missing; nothing is overwritten)
echo $?                  # 15
rm .ai/repo/workflows/plan.md
majordomus init --extend # created .ai/repo/workflows/plan.md, and nothing else changed
```

## What it does not cover

`--extend` restores a file the skeleton ships and the repository deleted, which is the right answer for a missing file and the wrong one for a file deleted on purpose; a deletion that should stick is a project decision to record, not something `init` can know.

## Why it exists

Four supervisory tools in the source environment were installed, ran for days, and were abandoned; one reason was that reinstalling anything silently reset what little state they had.
