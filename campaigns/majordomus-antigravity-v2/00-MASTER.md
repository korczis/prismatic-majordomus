# Majordomus Editor/Agent Intelligence V2: Incremental Derivation Compiler

You are Claude Code Opus/Fable working in the real `prismatic-majordomus` checkout. Read and obey `AGENTS.md`, `.ai/**`, architecture, ADRs, quality gates and all existing source-of-truth contracts. Read the previous Antigravity pack before editing. This V2 pack supersedes only the architecture of derivation/cache/build-time integration. Do not assume a subsystem exists just because it was described in an earlier prompt. First discover the real implementation and tests. No speculative claims about Antigravity file formats or support.

## Mission and outcome

Turn Editor/Agent Intelligence into an incremental, typed, explainable, content-addressed *projection compiler* that reuses Majordomus' existing derive/infer capabilities. Support Antigravity and Zed through the same canonical core. Implement `build.rs` only where compile-time embedding of stable canonical artifacts is demonstrably valuable. `build.rs` must never write source-tree/editor/agent configuration. Deliver an end-to-end usable slice and enforce it through existing gates. Prefer the narrowest correct change, delete redundant generators. Build does not depend on the Majordomus executable being built.

## Mandatory audit, before design

1. Determine actual language, workspace/crate boundaries, build script topology, existing `derive` / `infer` machinery, snapshots, cache keys, schema registry, versioning, emitted artifacts, `editor` features, `AGENTS.md`, `.agents`, `.zed`, MCP, UI and docs.
2. Inspect existing build scripts, rerun directives, `OUT_DIR`, workspace build-dependencies and cyclic dependency risks. Inventory each generated artifact's producer, consumers, owner, mutability, side effects and frequency of change.
3. Measure current cold/warm derive, CLI checks, editor sync and incremental Cargo rebuild. Record before-state data, avoid assuming improvement from architectural complexity.
4. Compare source-of-truth overlaps: schema registry vs `.ai`, workflows vs tasks, agent instructions vs rules vs skills. Produce a migration/deduplication plan.
5. Verify actual Antigravity configuration syntax and precedence against installed version/official documentation; never invent keys, legacy workflows or implicit MCP write permissions.
6. Decide whether a generic graph layer already exists. If so extend it, do not introduce another.

## Core contract: typed content-addressed DAG

Conceptual nodes (rename to align with real code): `SourceNode`, `DerivedNode`, `ProjectionNode`, `DiagnosticNode`. Each node has a stable identity and schema/algorithm version; normalized typed inputs; explicit ordered dependency IDs; SHA-256/BLAKE3 or already-standard project hash; canonical encoding; deterministic output or a structured diagnostic; provenance and explanation. Distinguish inputs by mutability: `build-stable`, `repository`, `worktree`, `runtime`, `user-private`. Never leak private inputs or hash raw secrets into user-visible output. Fingerprints are not authorization, provenance is not proof of truth, and confidence must not silently override policy.

Cache key formula (conceptual): hash(node-kind, schema-version, generator-version, canonical-params, sorted dependency fingerprints, platform context only if material). Canonicalize maps and ordered sequences deliberately; exclude timestamps, working directory and irrelevant environment from stable build keys. Avoid cycles, detect duplicate IDs, bound graph traversal and cardinality. Persist only pure immutable results in a versioned cache; use atomic writes, integrity checks and eviction policy. Cached and uncached results must be byte-identical and diagnostics identical for the same canonical inputs. Cache corruption must fall back to recomputation; no stale success by default.

Change propagation: `source changed -> invalidate descendants -> recompute affected nodes -> publish changed outputs only`. Use subtree hashing where justified and measured; a Merkle DAG is not a license for an unnecessary framework. No hashing all of `.git`, `target`, editor caches, dependency trees or ignored artifacts. Support full clean rebuild for verification, explicit explain trace, diagnostics on absent/ambiguous sources, idempotent operation, and robust behavior during concurrent writers.

## Build-time integration boundary

Build scripts must remain extremely small and not import the runtime daemon, CLI, MCP server or any dependency that introduces a compilation cycle. Prefer a small pure build-safe crate/library/shared schema crate *only if required*; otherwise a simple `build.rs` module is sufficient. Never run `cargo`, `majordomus`, `git status`, `curl`, shell pipelines, networks, providers or editor processes within `build.rs`. Never write outside `OUT_DIR`, modify checked-in outputs, lock the whole workspace, read user secrets, or make builds depend on worktree live state.

Eligible outputs: stable compile-time Rust module or const data, capability registries used by binary, prevalidated schema indexes, versioned interface descriptors. Restrict inputs to a proven small set of stable canonical paths and emit accurate `cargo::rerun-if-changed=...` / `cargo::rerun-if-env-changed=...` directives. Avoid broad directory watches if they cause rebuild storms. Emit deterministic files under `OUT_DIR`; only rewrite when bytes differ. `OUT_DIR` is persistent across runs, so handle stale artifacts explicitly. `include!(concat!(env!("OUT_DIR"), "/..."))` or `include_bytes!` may embed generated data in production crates. Ensure path independence, offline and cross-compilation behavior; target-specific generation must read Cargo-provided target context, not host `cfg!`.

A separate explicit CLI `editor sync` / `agent sync` owns checked-in generated files. CI reconstructs expected outputs in temp files and compares them byte-for-byte; standard builds must not mutate tracked files. Explicitly test compiling a dependency crate from a read-only source directory.

Do not add a `build.rs` simply to validate or render `.agents/skills` or `.zed` files: repository checks and explicit sync handle that better. If the measured benefit of build-time embedding is negligible or harmful, skip `build.rs` and document this engineering decision with data.

## Projection architecture

`RepositoryEnvironment / canonical discovery -> typed EditorAgentEnvironment -> capability resolution -> minimal editor-specific projection -> safe reconciliation`.

Model capabilities as a typed contract: task execution, workspace settings, language servers, formatters, schema binding, agent rules, skills, MCP connectivity, local paths, trust/safety boundaries. Adapters declare capabilities and supported scopes, version constraints and normalization rules. Missing capability produces structured nonfatal diagnostic or explicit unsupported state; never fabricate a configuration option.

Derive only necessary *delta* from editor defaults and repo policy. Preserve user-owned theme, fonts, keybindings, secrets, token settings and local overrides. Stable generated ownership manifest per artifact, precise merge/conflict and rollback. Before overwrite: detect whether content is managed, user modified, and whether source-version compatible. `render`, `diff`, `check`, `explain`, `sync` and `status` must share a single engine and differ only in authorization/mutation.

Agent rules/skills MUST derive from existing canonical governance/skill registries without independent hand-maintained catalogs; prefer references/links or thin wrappers where supported rather than duplicating prose. MCP tools remain exposed through Majordomus canonical MCP registry. Agent intent work must track canonical intent -> gap -> plan -> issue/milestone -> execution -> evidence -> critique -> completion; do not permit a generated editor file to become a second canonical state store or silent privilege escalation.

## Performance and privacy

Worktree-only changes must not force rebuild of Rust binaries. `cd` or editor opening must not trigger cargo builds/network, and must remain responsive. Runtime projections must be incremental and debounced; do not emit duplicate notifications and do not introduce a permanent watcher unless existing architecture justifies it. Separate hot and cold performance budgets after measurement; report medians/p95, number of nodes invalidated and cache hits. Never expose local paths, credentials, token counts/values or user-specific settings in committed artifacts. Fail closed for write permissions and unsafe ambiguous ownership.

## Acceptance

- Same canonical input -> same graph IDs, hashes and byte output across processes and cache hit/miss.
- Perturb discovery enumeration order -> same output.
- Modify one source -> only dependent outputs recomputed; unrelated Cargo compilation unchanged.
- Stable `.ai` source schema change -> appropriate build-time artifact invalidates and compiles if one exists.
- Worktree, current branch, agents or peer state change -> no build script rerun unless genuinely build-stable input changed.
- No tracked files modified by `cargo build`/`cargo check`.
- No accidental root/source mutation from `build.rs`; only `OUT_DIR` writes.
- Runtime editor sync never silently overwrites user-owned settings; injected malicious paths and symlink escapes rejected.
- Zed/Antigravity produce matching *semantics* via capability negotiation, not identical file formats.
- Canonical CLI/API/MCP/UI/docs models remain consistent, zero manual inventory per consumer.
- New canonical workflow/schema/skill auto-appears in relevant projections; explain traces canonical source.
- Existing `.ai` governance, tests, compatibility and release policies enforced; no unverified deployment claims.

## Execution order

Run phase files 01-06 in order, each with small integrated vertical slice, test + drift gate + docs; commit as existing repo policy permits. Publish a final proof table with before/after timings, node recomputation counts, exact commands/test results, source-to-output provenance and any rejected design options. Avoid introducing `build.rs` if measurement or dependency architecture argues against it. Do not request routine confirmations.
