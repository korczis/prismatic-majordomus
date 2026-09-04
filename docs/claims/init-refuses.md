# Installing into a repository that already has an installation is refused

## What it means

`majordomus init` creates `.majordomus/`. If that directory already exists, `init` refuses with exit code 15 and says so. `--force` rewrites the policy, profiles, templates and provider templates — and still never touches `state/`, where the task record, decisions, open questions, handovers and ledger live.

## How it works

`lib/init.sh` tests for the directory before copying anything from `share/skeleton/`. Under `--force` it copies everything except `state/`, and creates `state/decisions.md` and `state/open-questions.md` only when they are absent. `--gitignore` appends one line to `.gitignore`, once.

## How to see it

```bash
majordomus init
majordomus init          # majordomus: .majordomus/ already exists in … (use --force to rewrite everything except state/)
echo $?                  # 15
```

## What it does not cover

`--force` does overwrite a hand-edited `policy.yaml`; the refusal protects state, and the flag is the explicit decision to reset configuration.

## Why it exists

Four supervisory tools in the source environment were installed, ran for days, and were abandoned; one reason was that reinstalling anything silently reset what little state they had.
