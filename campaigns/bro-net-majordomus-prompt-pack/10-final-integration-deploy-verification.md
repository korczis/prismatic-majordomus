# Prompt 10 — Final Integration, Deployment and Verification


You are working inside the `prismatic-majordomus` repository.

Non-negotiable execution discipline:
- Inspect the real repository before deciding architecture.
- Reuse existing Majordomus abstractions, registries, rules, doctrines, schemas, adapters, diagnostics, tests, and deployment paths.
- Do not create parallel systems if an existing canonical mechanism can be extended.
- Everything must be typed, deterministic, data-driven, discoverable, testable, documented, and enforced where appropriate.
- No hardcoded duplicate inventories across CLI/API/MCP/Cockpit/docs.
- No claims without repository evidence.
- No "implemented" status unless tests pass and the relevant runtime/deployment surface is verified.
- Preserve existing public interfaces unless a migration is justified and completed.
- Prefer deleting obsolete duplicated machinery over adding another adapter layer.
- Use the repository's real canonical commands for format/build/test/check/deploy. Discover them instead of inventing them.
- At the end, report root cause, architecture, files changed by purpose, tests executed, validation evidence, and remaining debt.


Mission:
Treat everything not merged, tested, documented, deployed where applicable, and verified as incomplete.

Perform a repository-wide closure pass for the Bro-Net/browser-peer milestone.

Audit:
- branches/worktrees/PRs related to the feature
- stale duplicate implementations
- protocol/schema drift
- missing docs
- missing generated artifacts
- untested paths
- dead code
- orphaned rules/ADRs/issues
- inconsistent versions/links
- Cockpit routes
- API/OpenAPI
- MCP
- CLI
- coverage
- GitHub Pages/public site if applicable

Run the full canonical validation path.

Verify:
- fresh checkout/bootstrap if practical
- extension build/install dev flow
- pairing
- peer registration
- capability discovery
- safe task execution
- provenance
- multi-peer workflow
- policy rejection
- CLI/API/MCP/Cockpit consistency
- docs links
- coverage links
- deployed public documentation/site where project convention expects it

Do not declare success based on code inspection alone.

If the repository uses milestone/issue-driven development:
- ensure work is tied to the correct milestone/issues
- update/close only with evidence
- ensure commits/PRs reference the canonical work items

Final report must contain:
1. root cause / original gap
2. final architecture
3. implemented capabilities
4. security model
5. test matrix and exact commands/results
6. coverage evidence
7. cross-surface proof
8. deployment verification
9. files changed grouped by purpose
10. remaining limitations
11. explicit list of 2025 Bro-Net claims that are NOT implemented

Definition of done:
> A browser is a real Majordomus mesh peer, can advertise safe capabilities, receive authorized tasks, return provenance-backed results, participate in multi-peer workflows, and be inspected consistently through canonical Majordomus surfaces with tests, policies, documentation, and enforcement preventing regression.
