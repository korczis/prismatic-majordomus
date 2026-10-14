# PHASE 02 — EDITOR-NEUTRAL CORE

Execute 00-MASTER-PROMPT.md + 01-AUDIT-AND-CONTRACT.md context.

Implement or reuse a typed canonical developer environment (no duplication of RepositoryEnvironment). Entity types should cover language capability, formatter/linter/check policy, workflow command reference, schema association, ignored/generated path, available tools, diagnostic, provenance and confidence. Separate editor-neutral semantic intent from Zed serialization details. Deterministic total order must use stable identities and canonical order helpers already present.

Implement resolver for facts vs policy vs preference, unknown/unsupported states, strict capability negotiations, versioned projections, stable serialization and canonical diagnostics. Add `EditorAdapter` interface based on actual extension boundaries, avoiding pseudo-generalization. One mock adapter conformance fixture should prove core extensibility. Implement source-traceable explain data and stable cache invalidation; test concurrency and worktree isolation.

Test: cross-order permutation, idempotent normalization, precedence conflicts, source explanation, absent metadata, renamed/missing schema, escaped paths, varied environments, no network/build side-effects. Commit coherent stage under existing project conventions, then proceed autonomously.
