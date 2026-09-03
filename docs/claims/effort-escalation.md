# Escalating reasoning effort after repeated blocked attempts is recorded rather than assumed

## What it means

A profile may declare `effort_escalation`: after a stated number of blocked attempts, effort rises to a stated level. Escalation is a declared, visible rule in the profile — not a habit of running everything at maximum — and it appears in the profile table every worker reads.

## How it works

`effort_escalation.after_blocked_attempts` and `effort_escalation.to` are validated profile fields; `debugging` escalates to `xhigh` after two blocked attempts, `deep-work` after one. The projection renders them next to the base effort.

## How to see it

```bash
grep -A2 '^effort_escalation' .majordomus/profiles/debugging.yaml
```

## What it does not cover

**Advisory.** v0.1 records the rule; it does not count attempts or observe the worker's effort. Making escalation an observed event needs telemetry that does not exist yet.

## Why it exists

Maximum effort as the default was one of the named cost sources; the source environment's routing guidance said effort should be "set only where it differs from the session default", which is the sparse, deliberate form this field keeps.
