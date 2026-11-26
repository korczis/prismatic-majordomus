# Prompt index / execution DAG

Každý krok je samostatný bounded implementation checkpoint. DAG běží v pořadí dependencies; jednotlivé kroky se **nesmějí automaticky měnit na samostatné PRs**.

| ID | Lane | Prompt | Dependencies |
|---|---|---|---|
| 00 | foundation | [Baseline And Reconciliation](prompts/00_BASELINE_AND_RECONCILIATION.md) | none |
| 01 | foundation | [Canonical Ownership And Contracts](prompts/01_CANONICAL_OWNERSHIP_AND_CONTRACTS.md) | 00 |
| 02 | foundation | [Prompt Capture And Ingress](prompts/02_PROMPT_CAPTURE_AND_INGRESS.md) | 01 |
| 03 | foundation | [Claude Code Lifecycle Bridge](prompts/03_CLAUDE_CODE_LIFECYCLE_BRIDGE.md) | 02 |
| 04 | foundation | [Provider Adapters And Ingress Conformance](prompts/04_PROVIDER_ADAPTERS_AND_INGRESS_CONFORMANCE.md) | 02, 03 |
| 05 | intelligence | [Evidence Aware Context Compiler](prompts/05_EVIDENCE_AWARE_CONTEXT_COMPILER.md) | 01, 02 |
| 06 | intelligence | [Intent Resolution And Semantic Dedup](prompts/06_INTENT_RESOLUTION_AND_SEMANTIC_DEDUP.md) | 02, 05 |
| 07 | intelligence | [Proceed Idd Execution Unification](prompts/07_PROCEED_IDD_EXECUTION_UNIFICATION.md) | 01, 06 |
| 08 | intelligence | [Mesh Council And Collision Avoidance](prompts/08_MESH_COUNCIL_AND_COLLISION_AVOIDANCE.md) | 07 |
| 09 | intelligence | [Durable Execution And Recovery](prompts/09_DURABLE_EXECUTION_AND_RECOVERY.md) | 02, 07 |
| 10 | intelligence | [Ai Governance Reconciliation](prompts/10_AI_GOVERNANCE_RECONCILIATION.md) | 05, 09 |
| 11 | experience | [Persistent Intent Composer And Prompt Preview](prompts/11_PERSISTENT_INTENT_COMPOSER_AND_PROMPT_PREVIEW.md) | 06, 09, 10 |
| 12 | experience | [Cli Api Mcp Schema And Projections](prompts/12_CLI_API_MCP_SCHEMA_AND_PROJECTIONS.md) | 01, 09, 10 |
| 13 | experience | [Cockpit Blackboard And Live State](prompts/13_COCKPIT_BLACKBOARD_AND_LIVE_STATE.md) | 11, 12 |
| 14 | experience | [Ai Telemetry Economics And Adaptation](prompts/14_AI_TELEMETRY_ECONOMICS_AND_ADAPTATION.md) | 02, 09 |
| 15 | hardening | [Security Performance And Fault Injection](prompts/15_SECURITY_PERFORMANCE_AND_FAULT_INJECTION.md) | 03, 04, 08, 09, 10 |
| 16 | hardening | [End To End Dogfooding And Coverage](prompts/16_END_TO_END_DOGFOODING_AND_COVERAGE.md) | 13, 14, 15 |
| 17 | hardening | [Ci Delivery And Release Train](prompts/17_CI_DELIVERY_AND_RELEASE_TRAIN.md) | 16 |
| 18 | hardening | [Cleanup Evolution And Operability](prompts/18_CLEANUP_EVOLUTION_AND_OPERABILITY.md) | 17 |

## Priority and integration gates

1. First useful vertical slice: canonical prompt receipt + `UserPromptSubmit` context enrichment + true provider hook smoke test, built on existing capture (00–03).
2. Next: context + intent continuation + existing Proceed/IDD/Mesh coordination + durable replay (05–09).
3. Governance, real intent composer, derived interfaces and telemetry, without altering canonical state ownership (10–14).
4. Fault injection and full E2E **before merge** (15–16); verify CI/release/merge/deploy afterwards (17); final reconciliation and docs/operations (18).

## Resume protocol

At session start, read the current repo state and existing Majordomus task/decision/ledger checkpoint. For each task compute `not-started | active | implemented | verified | integrated | blocked`, grounded in repo evidence, not a hand-maintained progress checkbox. Resume the first unmet dependency node; do not rerun already successful external side effects. Commit/test only within authorized scope.
