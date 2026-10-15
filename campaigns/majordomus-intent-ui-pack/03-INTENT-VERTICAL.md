# 03 — COMPLETE INTENT → GAP → PLAN → EXECUTION → EVIDENCE VERTICAL

Build the real workflow that starts with natural-language desired outcome and ends in proof, not a synthetic demo. Reuse current typed intent model, criteria, evidence ledger, realization graph, issue/milestone relationships, preflight, gap and critique records, sessions and task scheduler. Verify which behaviors exist; fill only missing links. Distinguish inference proposals from committed canonical records.

Sequence:

**Capture:** create/select an intent and attach project/repo identity, clear outcome, measurable criteria, invariants, constraints, autonomy, budget, hazards and provenance. Natural language may propose data; deterministic typed validation decides acceptance. Support draft, validation diagnostics and editing. No silent LLM hallucination becomes canonical.

**Understand:** derive current gap from actual source, runtime, tests, docs and linked issues. Show observations, confidence, timestamps and evidence. Separate observed, inferred and unknown facts. Run an adversarial/opposition pass using existing provider/workflow mechanics where available; record critique, risks, alternative approaches and unresolved objections. Block work if mandatory critique/preflight is missing.

**Plan:** produce a typed plan whose milestones/issues explicitly serve acceptance criteria; dependency DAG must be acyclic, scoped, explainable, reversible as policy permits and capable of reporting parallel ready waves. Preview changes and proposed GitHub reconciliation, show diffs before authorizing writes. Derive readiness from dependencies, blockers and evidence, never a second mutable status field.

**Execute:** select ready work, apply task/scope/worktree ownership and invoke existing worker/session/mesh capabilities with correct actor, budget and supervision. Provide pause/request-cancel/retry/reassign only where semantics are defined. Checkpoints survive reconnect and restarts. Workers must update real canonical facts, not UI-local tasks.

**Verify:** link commits, tests, checks, generated docs, issues, PRs and deployment evidence back to individual criteria. An intent's status is derived; stale or failed evidence cannot satisfy it. Add explain and drill-through links. Reevaluate on HEAD change or evidence invalidation. Show blocked/unknown instead of pretending everything green.

Test edge cases: orphan issue; criterion without work; conflicting criterion evidence; dependency cycle; missing critique; stale test after commit; duplicate intent submit; malicious prompt text; partially failed wave; interrupted session; mismatched GitHub issue ID; scope violation; concurrent edits; a feature complete locally but not deployed. Tests must exercise UI/API/CLI derived parity and confirm no implicit external mutation during preview.

Deliver one realistic repo-local dogfood intent that runs in a disposable fixture. It must prove capture, gap, critique, plan, one work item, one execution and evidence-based satisfaction or an explicit truthful blocked result.
