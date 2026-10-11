# 04 — Completion, version bump, release and deployment

Build on actual existing typed completion/variobump/version allocator/release/deploy model; inspect ADR 0134 and merged status of relevant PRs. No duplicate allocator. Independent semantic completion unit = canonical completion event with stable identity and evidence. A merge and the issue closure it triggers share one causality/transaction ID. A milestone gets a distinct event only if independently verified policy criteria define an additional real unit. Deduplicate duplicate GitHub notifications and replayed state transitions.

Mandatory durable state-machine transitions (adapt to existing canonical names):

`DISCOVERED -> ELIGIBLE -> MERGED_OR_COMPLETED -> VERSION_RESERVED -> TAGGED -> RELEASED -> DEPLOYED -> VERIFIED -> SETTLED`

Use explicit persisted error/retry states. Reserve version under a fenced allocator lease with optimistic expected base SHA; canonical Elm-like compatibility classifier determines major/minor/patch with current policy floor. Allocation keyed by completion ID. Never bump on retries or mutable UI read. Never publish a version that cannot be tied to exact integrated code and verdict. New commits/deployments cannot silently invalidate pending earlier obligations. If high-frequency release processing is slow, optimize build caches and artifacts; do not silently coalesce required individually released events.

Prepare generated changelog/docs/site metadata from canonical registry, ensure version files/manifests and lockfiles obey existing bump mechanism, check drift, run required release gates, publish signed/provenanced artifacts when policy requires, deploy using existing pipeline, verify live version/health/routes (including docs/site/API as applicable). Record hashes, tag, release URL, deployment target, observed deployed SHA/version, timings and proofs. Network timeout means UNKNOWN until reconciled. If release publication is blocked, queue durable unsatisfied obligation; keep only independent work advancing safely and honor ordering constraints. Do not invent empty bump commits for admin closures.
