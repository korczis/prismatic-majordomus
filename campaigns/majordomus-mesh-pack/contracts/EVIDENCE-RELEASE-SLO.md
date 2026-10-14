# Evidence, acceptance and release contract

States are not synonyms: ingested, interpreted, reconciled, started, implemented, tested, verified, integrated, released, deployed. Determine status from explicit canonical events and validators. Do not mark an intent satisfied merely because all issues closed.

For every requirement: source revision/segment, criterion, implementation PR/commit, required checks, verification command/result, accepted evidence, build artifact, deployed artifact (if applicable). Preserve raw references to proof and access-control boundaries.

Indicative stretch SLOs, measured at baseline and evaluated by p50/p95, not presumed achievements: warm local read <100ms; scheduling p95 <5s; controller resume p95 <15s; small-change incremental CI p95 <60s when feasible; merge of already-eligible PR p95 <2m. Safety invariants: unauthorized protected-branch bypass 0, unsupported satisfaction 0, duplicate irreversible external side effects 0. Report real bottlenecks and tradeoffs.

Release only under existing versioning policy, with reproducible artifact identity, smoke checks, health checks, rollback and verified live deployment. Landing page may only claim empirically proven behaviors.
