# watch reports policy, projection, state, scope, handover, verification, staleness and retention drift

## What it means

`majordomus watch` is the read-only inspection of everything that can silently disagree: the policy changed after the last `update` (policy drift); a generated file no longer matches its own stamp (projection drift); the task record no longer describes the checkout (state drift); touched files outside scope (scope drift); a task marked handed over with no handover file, or one missing a required section (handover drift); a task marked complete with no `task.finished` in the ledger (verification drift); a checkpoint older than the profile interval (staleness); a store over its cap (retention). Each finding names the command that reproduces it; exit 11 when anything drifted.

## How it works

`lib/watch.sh` recomputes the policy hash and compares it with the one each projection's stamp names, hashes each projection's content against its stamp, loads the task record and profile, reuses the scope and divergence logic from `check`, greps the ledger and the handover store, and counts against the retention caps. It writes nothing.

## How to see it

```bash
sed -i.bak 's/checkpoint_interval_default: 15m/checkpoint_interval_default: 25m/' .majordomus/policy.yaml
majordomus watch; echo $?
# DRIFT policy      .majordomus/policy.yaml — policy or profiles changed after the last update  [reproduce: majordomus update --dry-run]
# 11
```

## What it does not cover

`watch` is never used to block; drift in work in progress is information for a person. It is not a daemon; it runs when invoked.

## Why it exists

Drift is how every abandoned tool in the source environment died: the policy said one thing, the generated file another, the hook a third, and nothing compared them. Only deterministic comparisons are implemented; nothing here guesses.
