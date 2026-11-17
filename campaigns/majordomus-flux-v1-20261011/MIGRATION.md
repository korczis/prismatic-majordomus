# Adoption and rollout

Audit existing PRs against Flux before adding issues. Keep public routes stable. Preserve the current Rust `El` escaping and existing `cockpit/view.rs` pagination until migrated and behavior parity is proven. Do not replace one cache by a parallel one: unify accounting and remove dead code after rollout.

The `.ai/` model must carry canonical architectural decision, rule-to-proof linkage, CI gate mapping and examples. Use existing schemas and generators; if new metadata is essential, extend a canonical schema and regenerate. Update docs/COCKPIT.md, docs/GITHUB_PAGES_PERFORMANCE.md, docs/GITHUB_PAGES_ARCHITECTURE.md, docs/CI.md, operator guides and generated website routes according to actual source layout. Demonstrate performance results and provenance from measured evidence, not manually invented speedups.

Deploy slices: first safe server-side fragment/query/cache improvements; then incremental derive; then pagination component; then live enhancements; then full route parity. Validate via tests and measured baseline at each stage, version bump using existing release analysis, merge and deploy if authorized. Never claim deployment because a command was requested. Produce reproducible source SHA and HTTP checks after actual deployment. Document rollback mechanism and observability alert thresholds.

Promote only verified features to landing/Pages. Public static Pages must not present live dynamic runtime data as current unless its provenance and freshness semantics explicitly permit it.
