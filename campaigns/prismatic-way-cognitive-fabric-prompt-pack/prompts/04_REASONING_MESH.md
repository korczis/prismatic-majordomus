# WAVE 04 — Local-first budgeted reasoning and mesh collaboration

Do not invent a new oracle, advisor registry, mesh federation, or authority. Integrate with `majordomus reasoning` (ADR 0098), its capability-based optional advisor plan, evidence-based conclusions and disagreement experiments. Use current reasoning modes; CI must not need models. Cache identical, valid reasoning only within safe scopes and explicit validity windows.

Routing policy: deterministic extraction first; if classification is genuinely ambiguous use smallest capable locally available model; material/high/critical engineering uncertainty follows existing reasoning assessment/plan; request independent advisors only according to existing budget/materiality policy. Never silently substitute absent model with fabricated output. Track model/source diversity, not vote count.

Reuse `majordomus_mesh_claim`, review request, handover, signed event journal/fold and peer board as designed. Discovery is awareness, not trust; the mesh is not a remotely authenticated arbitrary command executor. Preserve repo identity, signed events, handshake, replay protection, trust policy and exclusive-scope conventions. Partitioned mesh may conflict; surface it, reconcile explicitly on healing. No promise of strong consistency.

Define budgets (time, token, cost, retries, peer fanout), cancellation, timeout, circuit breaker, queue capacity, slow peer fallback and fairness. Use current telemetry wherever it exists. Assign work only when a meaningful independent scope and capability are demonstrated. Never allow advisor advice to bypass verification, protected branches, code review, or merge/release seats.

Tests: 0 advisors, 1 advisor, disagreeing advisors, duplicate models via one adapter, timeout/rate limit, offline mode, peer reconnect, expired claim, partition conflict, poisoned payload, claimed scope violation, unexpected model cost.
