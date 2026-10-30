# Architecture contract: Prismatic Way

## Normative definitions
- **Observed**: reproducible direct reading from an authoritative source with locator + timestamp/version.
- **Derived**: deterministic transformation with reproducible inputs and algorithm version.
- **Inferred**: fallible interpretation with source, uncertainty and provenance; never a factual promotion.
- **Proposed**: candidate state mutation awaiting applicable policy and evidence checks.
- **Verified**: claim has specified acceptance evidence that actually proves it, at a named revision/time.
- **Superseded**: invalidated by an identified newer assertion; preserve lineage.

## Authority boundaries
1. `.ai/repo/**` holds tracked, normative repository context. `.ai/local/**` is private checkout state: never committed, publicly served, or read as normative policy. Shared operational facts from mesh come from authenticated signed journal/fold, not copy/pasted local files.
2. Git/source/tests/CI hold their own authorities. Existing entity registry identifies canonical records; no competing storage or ID namespace for same concepts.
3. Advisor outputs are non-authoritative evidence; reviewer agreement alone never changes verification class.
4. Mesh discovery is awareness; handshake/trust is required for cooperation; a scope announcement is not a lock; exclusive claims use the existing mesh protocol. Network partitions can create conflicts, so don't assert linearizability.
5. Merge, release, deployment and versioning obey their existing owners/seats. Events do not grant execution rights.
6. Public projection is an intentional whitelist derived from publication contracts; private runtime data never becomes public by default.

## Algorithmic requirements
- Stable entity and repository identity; content digests; typed relations; causal event refs; deterministic total ordering.
- Idempotent event application; dedup; bounded fanout, queue, recursion, retries and time; recoverable dead-letter diagnostics.
- Incremental graph impact using reverse dependencies; deterministic cache invalidation; no hot-path full repo rescans.
- Bounded synchronous hook work; asynchronous inference/peer work only when necessary, no blocking on remote APIs in shell entry/session start.
- No LLM invocation when parsing, AST, diff, schema, policy or graph reasoning suffices.
- Every mutable state transition uses existing writers, scopes and audit records; conflicts explicit and recoverable.
- Reuse provider-neutral reasoning/skills/capability generation. Zero consumer-specific entity registration.
- Performance improvements judged against measured baseline and correctness; no benchmark gaming.

## Anti-goals
No newly invented 'memory service', second event bus, shadow intents table, speculative ADR auto-writer, unlimited watcher daemon, direct autonomous remote code execution, hardcoded UI inventories, publishing full traces/prompts, subscription credential automation bypassing provider contracts, fake live status, or silent retry loops.

## Definition of Prismatic Way
**Reuse before create. Derive before infer. Infer before ask. Verify before promote. Increment before rescan. Push before poll. Measure before optimize. Complete before celebrate.**
