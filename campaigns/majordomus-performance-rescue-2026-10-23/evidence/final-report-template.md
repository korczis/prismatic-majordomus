# Performance Rescue Report (complete with measured evidence)

- **Review baseline / end SHA:** NOT_MEASURED / NOT_MEASURED
- **Date, machine, build, sample N:** NOT_MEASURED
- **Overall verdict:** NOT_RUN

## Root causes, ranked

| Bottleneck | Source and trace | Cold contribution | Warm contribution | Correction | Status |
|---|---|---|---|---|---|
| NOT_MEASURED | | | | | NOT_RUN |

## Before and after

| User task | Before p50 / p95 / p99 (N) | After p50 / p95 / p99 (N) | Host / SHA / profile | Evidence | Status |
|---|---|---|---|---|---|
| CLI / `cd` | NOT_MEASURED | NOT_MEASURED | | | NOT_RUN |
| `/ui` cold | NOT_MEASURED | NOT_MEASURED | | | NOT_RUN |
| `/ui` warm | NOT_MEASURED | NOT_MEASURED | | | NOT_RUN |
| `/cockpit` cold/warm | NOT_MEASURED | NOT_MEASURED | | | NOT_RUN |
| browser FCP/interaction | NOT_MEASURED | NOT_MEASURED | | | NOT_RUN |
| live event to DOM | NOT_MEASURED | NOT_MEASURED | | | NOT_RUN |

## Semantics and security

- Source freshness/pending/stale correctness: NOT_RUN
- Cache equivalence/worktree separation: NOT_RUN
- No network/build on env enter: NOT_RUN
- No redundant registry or generated surface: NOT_RUN
- Origin/CSP/signed mesh invariant: NOT_RUN

## Changes and issue traceability

Issue -> commit -> affected canonical modules -> test -> policy check -> docs/deploy.

## CI and deployment proof

- Required test commands and exit codes: NOT_RUN
- Browser trace and screenshots: NOT_RUN
- Hosted live served SHA + URL: NOT_RUN
- GH Pages static revision + URL: NOT_RUN

## Remaining blockers / debt

Each item names why, owner, evidence, next action. Never call a task complete merely because it was not observed.
