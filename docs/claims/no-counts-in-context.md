# A hardcoded count in the always-loaded file is a failure

## What it means

The always-loaded instruction file must not contain sentences like "14 agents", "97 apps" or "63 skills". Counts written into prose are stale within days and a worker reads them as fact. The rule is: write the command that computes the number, never the number. `doctor` fails on a digit sequence next to the words that usually carry such counts.

## How it works

`lib/doctor.sh` greps the always-loaded projection for a number followed by `agents`, `files`, `apps`, `commands`, `skills` or `rules`. Any match is a failing finding with a reproduce command. The rule is deliberately narrow and lexical so it cannot be argued with.

## How to see it

```bash
echo "This repository has 12 agents." >> .majordomus/providers/body.md && majordomus update --force
majordomus doctor
# FAIL counts      CLAUDE.md — hardcoded count in always-loaded context
```

## What it does not cover

Counts phrased without those nouns pass. The check catches the common lie, not every lie.

## Why it exists

In the source environment five authority files gave five different application counts, three agent counts and five pillar counts, some contradicting each other fifteen lines apart in the same file. A validator existed to catch it and still found fifteen discrepancies after a cleanup.
