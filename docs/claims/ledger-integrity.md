# Every line of the ledger is a well-formed event

## What it means

`.ai/local/state/ledger.jsonl` is append-only and is the only record of what happened that nothing else can reconstruct: which tasks were started, checkpointed, handed over and finished, and on what contract. A line that is not a well-formed event is reported by name and line number, by `check`, by `doctor` and by `watch`.

## How it works

`mj_ledger_bad_lines` in `lib/common.sh` reads the file and returns the numbers of lines that are not JSON objects carrying a `ts` and an `event`. `mj_validate_ledger` turns that into a finding; the doctrine `ledger_integrity` is declared blocking and enforced by `check`, `doctor` and `watch`, so the class is what makes it stop a command rather than a decision inside the validator. The readers — `history`, `search` — skip a malformed line and keep going rather than crashing, so a damaged ledger degrades into a shorter one instead of an unusable one.

## How to see it

```bash
echo 'this is not json' >> .ai/local/state/ledger.jsonl
majordomus history --validate     # exit 10, naming the line
majordomus doctor                 # FAIL records  ledger.jsonl — line(s) 8 are not well-formed events
majordomus watch                  # DRIFT records ledger.jsonl
```

## What it does not cover

Well-formed is not the same as true. The check proves each line is an event with a timestamp and a name; it does not verify that the event describes something that happened, and it cannot detect a line that was removed. Append-only is a convention the commands honour, not a property the filesystem enforces.

## Why it exists

Every other durable record here can be rebuilt: projections regenerate from the policy and carry their own stamps, the task record is written fresh by `start`. The ledger cannot. A single corrupt line quietly truncating the history a reader will act on is the failure worth spending a check on.
