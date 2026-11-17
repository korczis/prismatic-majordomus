# SCORECARD, SLOS AND BACKPRESSURE RULES

Targets are proposals to baseline and adapt, NOT measured current capability. Distinguish ordinary low-risk PRs from heavyweight release/security changes.

| Metric | Suggested target | Definition |
|---|---|---|
| PR ready-to-merge p50 | <= 5 min | ready verdict timestamp -> merged timestamp, eligible ordinary class |
| PR ready-to-merge p95 | <= 20 min | same, representative cohort; no percentile on tiny sample |
| Affected derive warm p50 | <= 30 s | real touched-projection scenario on documented runner |
| Essential pre-merge checks | <= 60 s fast lane where sound | preserve full protected gates as required |
| Autonomous delivery fraction | >= 90% eventually | verified deployed intents without manual interventions / all attempted autonomous intents |
| Post-merge required verification | 100% | all merged change candidates with mandated postmerge checks |
| Provenance/canonical UI coverage | 100% for supported critical paths | views derived from same backend model, no duplicated frontend truth |
| Secret leakage | 0 known incidents | tested classes of credentials, not universal proof |

## Definitions
Track count and time separately for queue wait, CI setup, build, test, derive, review, merge, release, rollout. Include failed and censored attempts. Report median and p95 only if sample size justifies. Attribute manual touches explicitly: permission grant, commands typed, conflict repair, force-rerun, deployment intervention.

## Backpressure
Stop assigning speculative new features when (a) ready PR oldest age exceeds SLO persistently, (b) open queue grows beyond measured throughput, or (c) recurring integration failures dominate capacity. Allow critical security fixes. Use hysteresis to prevent flip-flop, record exact observed inputs. Never turn a heuristic into a blocking rule without well-defined applicability and negative proof.

## Scorecard JSON shape (illustrative, not a new source of truth)
```json
{"schema_version":"derived-from-existing-registry","revision":"<sha>","window":"<observed-window>","counts":{"attempted_intents":null,"verified_deployed_intents":null,"manual_interventions":null},"timings":{"pr_ready_to_merge_p50_s":null,"pr_ready_to_merge_p95_s":null,"derive_warm_p50_s":null},"evidence":[],"unknowns":[]}
```
Populate only from existing canonical events and recorded runs. Null is honest when not observed.
