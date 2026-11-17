# Execution runbook

## Core acceptance rule

`worker reported success != verified acceptance`. Require code- and policy-SHA-bound evidence, realistic negative tests and a clean independent CI execution. Treat branch protection / required-check wiring as a separate, externally observed state from an ordinary green workflow.

## Recommended waves

| Wave | Milestones | Go/no-go |
|---|---|---|
| 0: Anchor | M00 | Audit facts reproduced or explicitly invalidated; bounded recovery intent and truthful claims |
| 1: Acceptance trust | M01 → M02 → M03 | Local scope edits, trivial verification and `--no-verify` cannot bypass **independent** acceptance |
| 2: Effect & correctness | M04, M05, M10 (respect dependencies) | Mutation survivor killed; YAML/scope correctness restored; unsupported claims retracted |
| 3: Adoption | M06, M07, M08, M12 (respect dependencies) | Provider-native hooks and minimal experience work; lifecycle write owner converged; users actually tested |
| 4: Optimization & outcome | M09, M11, M13 | MCP measured; real intent outcome evidenced; subsystem retention based on actual utility |

M10 depends on M02/M03/M04, M06 on M01/M03/M05, M07 on M03/M04/M05, M08 on M01/M02/M05, M09 on M08, M11 on M03/M07/M10, M12 on M07/M10, and M13 on M00/M07/M12. `MANIFEST.json` is the authoritative machine-readable DAG.

## Session protocol

1. Pin HEAD; check branch ownership and clean/dirty state.
2. Load one prompt. Establish intent, issue and baseline status in native Majordomus structures, not fake GitHub issues.
3. Run reproduce/negative first; implement each issue and attach artifact evidence.
4. Run focused regression, broad relevant gates, generator/drift checks and independent CI where applicable.
5. Record explicit **verified / locally-tested / blocked / rejected / not-applicable** states; never invent results.
6. Advance only when all blocking criteria are validated. Keep bounded handovers with commit/evidence IDs.

## Stop conditions

- A patch weakens validation or creates a ratchet baseline instead of fixing a newly introduced failure.
- The worker modifies its own accepted scope or its allowed verifier without an independent amendment.
- CI executes untrusted PR code with privileged tokens.
- Public docs claim verified when evidence is missing.
- A prompt starts spawning unrelated product features or new autonomous runners.
- An agent modifies private-network mesh defaults or triggers model spend/merges/deploys without explicit permission.

## Scorecard after each milestone

Measured acceptance bypasses; mutation kills; CI authority state; claim-evidence coverage; onboarding files/time; warm/cold hook P50/P95; MCP `tools/list` bytes; first-run successful/failed; optional subsystem cost. Report `not measured` instead of attractive fiction.
