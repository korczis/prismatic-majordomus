# Milestone 06: Self-hosted outcome proof and release

Use the converged architecture from 00–05. This milestone is not more infrastructure. It is the reproducible evidence that the product delivers meaningful work with lower latency and no hidden manual steps.

## Issue 06.1: Own-repository vertical slice

Create a scoped, realistic but disposable change spanning intent → plan → issue → worker/task execution → validation → review/PR → merge → version/release → deployment verification where configured and authorized → linked evidence → close. Use real Majordomus commands, only actual integrations; redact credentials, no forced public deployment. Fixture/sandbox path may substitute for unsafe irreversible steps, but must be labeled simulated.

## Issue 06.2: External consumer repository

Spin up a fresh disposable project, install using the actual documented one-line path (or version-pinned release), initialize `.ai/` and optional direnv/CLI/UI according to real capabilities. Repeat a task with tests, missing evidence rejection, re-run, completion and status projection. Prove there are no repo-specific hardcoded paths or manually registered consumers. Re-run without internet if the documented installed feature is supposed to be local-first.

## Issue 06.3: Failure and recovery rehearsal

Inject interrupted derive, failed test, stale PR, repository HEAD advance, peer disappearance if peer runtime is available, incorrect release digest and aborted deploy (mock if unavailable). Each must fail closed with coherent diagnostics and recover without corrupting canonical state. Test on Linux and macOS where accessible; mark unavailable test environments explicitly.

## Issue 06.4: Compare against measured baseline

Publish evidence-backed comparison: median and p95 feedback latency, no-op work eliminated, derive critical path, CI latency, eligible PR dwell time, manual interventions, accepted outcomes, regressions. State environment, sample size, confidence and exceptions. Do not invent 60s CI or 5-minute release achievement. Gate on bounded non-regression with justified tolerance.

## Issue 06.5: Truthful public claims and shipping

Update `docs/CLAIMS.yaml`, docs, readmes, use-case demos, release notes and site through canonical derive, with tests proving they match reality. Only bump/publish/deploy using existing release semantics and permissions; verify installed artifact and UI, tag immutable SHA, link provenance. Report remaining debt as explicit tracked issues. Avoid premature feature expansion.

Acceptance: full verified pathway works in self-hosting and installed consumer environment; live measured gain is reported; rule→validator→test→CI enforcement is demonstrated; nothing depends on hand-edited generated docs or unverified success claim.
