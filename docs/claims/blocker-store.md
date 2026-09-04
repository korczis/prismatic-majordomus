# An unparseable question blocks acceptance exactly as an unresolved one does

## What it means

`.ai/local/state/open-questions.md` is read by the gate that refuses completion while something is unresolved. If an entry in it does not parse, the gate cannot see it — so a malformed entry is treated as a blocking failure, not as a formatting problem.

## How it works

`mj_question_malformed` in `lib/question.sh` returns the line numbers of entries that do not match the record shape. `mj_validate_blockers` checks the store *before* it looks for unresolved entries for the current task, and reports a parse failure under the same `blockers` category, because it is the same rule: this gate can be relied on. `mj_validate_questions_store` runs the same check from `doctor` and `watch`, which have no active task to check against but still need to know the store is sound. Both doctrines are declared blocking.

## How to see it

```bash
printf -- '- [unresolved] no separator and no date\n' >> .ai/local/state/open-questions.md
majordomus check                  # FAIL blockers open-questions.md — line(s) 4 do not parse
majordomus doctor                 # FAIL records  open-questions.md
majordomus watch                  # DRIFT records open-questions.md
```

## What it does not cover

The check proves the store is readable, not that the questions in it are the right ones. Nothing detects a blocker that was never written down, which is the more common failure and not one a validator can reach.

## Why it exists

A gate whose input can be made invisible by a typo is worse than no gate, because it reports success. Treating an unreadable entry as equivalent to an open one removes the incentive to route around the gate by malforming it, deliberately or otherwise.
