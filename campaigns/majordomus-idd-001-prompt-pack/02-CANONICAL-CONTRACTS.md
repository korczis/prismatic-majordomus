# MISSION 02 — CONTRACT FIRST, END-TO-END INTENT LIFECYCLE

Inspect existing domain and reuse its identifiers/state machines. Define a canonical typed projection (not necessarily new storage types) linking intent → plan/revision → milestone → issue → execution/attempt → evidence → PR/release/deployment. Relationships must include stable identifiers, references, causal provenance, scope/repository, ordering, version/conflict markers, and policy metadata. Preserve many-to-many links if existing model supports them.

Required operations, derived from registry where relevant: submit/draft intent, critique, propose plan, edit/revise, approve, materialize milestones/issues, schedule eligible work, pause/resume scheduling, cancel, retry, verify, close/reopen if permitted, inspect history. Don't synthesize action IDs without discovering conventions. Do not count an LLM statement as completion evidence.

Plan realization: validate against canonical schema and policy, deduplicate equivalent work, reject cyclic dependencies, detect orphan nodes and impossible criteria, estimate constraints/budgets with explicit uncertainty, map each acceptance criterion to verifiable postconditions; produce provenance for every newly materialized entity. Idempotency must survive duplicate UI submissions and coordinator restart where durability is promised. Concurrency: use existing version/transaction conventions; conflict is explicit rather than last-write-wins.

All new entities must appear without per-consumer registration in capabilities, CLI, API/OpenAPI, MCP, `/ui` navigational projections and generated docs where applicable. Distinguish product curated navigation from discoverable operation catalogs. Generate contracts from authoritative Rust types and existing descriptors; never manually synchronize JSON schemas.

Tests: lifecycle transition table (allowed/forbidden), invariant/property tests for shuffled dependency registration, repeat materialization, duplicate IDs, stale revision, parallel writes, failed policy gate, missing acceptance evidence, reopened issue, status drift, legacy migration and backward compatible serialized contracts. E2E: given UI-created intent produces plan; approval creates work; eligible work can launch via the same executor used by CLI/MCP.

Success gate: one real intent can be entered in `/ui`, inspected, amended, proposed, approved and materialized with no sidecar issue/planning model. Every state transition produces an audit record and is verifiable via existing API/CLI/MCP as applicable.
