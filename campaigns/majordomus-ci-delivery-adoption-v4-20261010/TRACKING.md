# Evidence and progress tracking

**This is a template.** The *live* project ledger under the current `.ai/` canonical project model is authoritative. Do not keep independent status in this Markdown file after import.

## Per-issue minimal record

`issue_id`, `canonical_intent_id`, `milestone_id`, `owner/worker`, `scope`, `dependencies`, `status` (`pending|active|blocked|validated|merged|released|deployed`), `PR`, `commit SHA`, `selected gates`, `executed gate evidence`, `reused evidence digest`, `before/after time`, `CI run URL`, `release URL`, `deploy verification URL`, `blocker`, `checkpoint`, `next ready`.

## Summary table

| Milestone | Issues | Status | CI evidence | Release/deploy evidence |
|---|---:|---|---|---|
| M01 | 5 | pending | pending | pending |
| M02 | 4 | pending | pending | pending |
| M03 | 4 | pending | pending | pending |
| M04 | 4 | pending | pending | pending |
| M05 | 4 | pending | pending | pending |
| M06 | 4 | pending | pending | pending |
| M07 | 4 | pending | pending | pending |
| M08 | 4 | pending | pending | pending |
| M09 | 4 | pending | pending | pending |


## Release cadence

At each validated integration increment: inspect `release analyze`/equivalent, perform version bump if canonical policy requires, build/verify attestations, deploy if within granted permissions and smoke test before declaring complete. Each failed/blocked step must leave evidence and not destroy unrelated progress.

## SLO statistics

Report `PR created → verdict`, `runner allocated → verdict`, queue, cache warm/cold, full-impact exceptions separately. Include raw sample and script; never publish only a flattering mean.
