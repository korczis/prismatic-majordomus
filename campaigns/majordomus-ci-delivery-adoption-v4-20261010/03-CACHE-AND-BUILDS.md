# M03: Cache and builds

## Goal
Secure, high-hit-rate artifact reuse and Rust compile elimination across CI jobs.

## Issues

## Issue V4-03-01: Build key model

**Implementation sequence**
1. Discover the exact Rust toolchain, lockfile, target, profile, feature/env flags and generated-source dependencies.
2. Define reproducible cache key schema and invalidation tests covering all compiler-input dimensions.
3. Disallow cross-trust write restoration and unsafe untrusted code to privileged contexts.
4. Preserve `Swatinem/rust-cache` where it works; improve configuration rather than stacking duplicate caches.

**Acceptance / evidence**
- [ ] Cross-target and cross-toolchain contamination tests
- [ ] Cache hit/miss metrics

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-03-02: Compile once where safe

**Implementation sequence**
1. Identify jobs redundantly building the same binary, extract an artifact-producing build stage where it shortens critical path.
2. Attach SHA-256 and build-identity manifest; verify before every consumer using existing provenance mechanisms.
3. Keep per-platform builds where ABI differs; never move linux binary to macOS.
4. Compare upload/download cost to compilation saved and choose empirically.

**Acceptance / evidence**
- [ ] Reused binary compatible with checked-out commit and generator generation
- [ ] No build drift or spoofable artifact path

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-03-03: Dependency and site cache

**Implementation sequence**
1. Unify tool setup for Zola/site, cargo metadata and static tools where trust-safe.
2. Pin versions, invalidate on manifests/lockfiles and toolchain, and avoid network downloads on routine warm PRs when safe.
3. Add regression fixtures for cache corruption and restore key fallback.

**Acceptance / evidence**
- [ ] Cold cache builds correctly
- [ ] Warm cache saves time

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Issue V4-03-04: Cache observability

**Implementation sequence**
1. Emit cache hit, bytes, elapsed restore/save and build wall time into existing CI timings/evidence streams.
2. Expose measured data through existing CLI/API/MCP/UI projections if present, without a shadow metrics store.
3. Analyze eviction and save-on-success implications.

**Acceptance / evidence**
- [ ] Per-run cache metrics discoverable
- [ ] No secret or personal data leaks

### Required invariant per issue
- Treat existing repo model as canonical; update it rather than adding an independent source of truth.
- Link work to intent/milestone, execute focused positive+negative tests, record evidence and gate results.
- Update relevant README/ADR/rules/claims only when the repo policy requires, and derive generated docs.
- Check scope conflicts with concurrent PRs; publish only verified results. Capture previous/current timing on affected path.

## Milestone exit gate
- [ ] Verified warm and cold replay
- [ ] Performance benefit measured, not guessed
