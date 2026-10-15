# PAGE 05 — Persistent CAS, output restoration, integrity and GC

**Claude Code Opus/Fable execution prompt.**

Implement a safe local content-addressed cache using existing cache subsystem when possible. Store typed manifest: key, node, version, input fingerprint, output relative paths, content hashes, sizes, file modes where needed, provenance, validation result and creation metadata. Use temporary directories, atomic publish, read-after-write verification, concurrency-safe locks and stale-lock recovery. Never store secrets, `.ai/local`, unrelated user files or paths outside repository root.

On cache hit, verify every expected output, including deleted/stale output ownership and digest, before skip. If outputs differ from desired bytes, restore only managed outputs atomically; if missing/corrupt cache entry, rebuild safely. Reject traversal paths and symlink escapes. Cache does not prove semantic correctness independent of its declared inputs; support explicit cache disable/full mode. Eviction must be quota- and age-bounded, avoid destroying in-use entries, and make `gc` a separate authorized action.

Support cross-worktree reuse by content key while keeping mutable worktree output isolated. Test read/write corruption, truncated manifest, mismatched hash, concurrent writers, interrupted write, old schema, permission errors, symlink attacks, restore-over-unmanaged-file refusal, and GC races. Instrument hit rate, bytes, save/restore time and avoided execution. Exit gate: cache loss never produces a false success; clean full outputs match restored outputs exactly.


## Mandatory Majordomus discipline
Read `AGENTS.md`, `CLAUDE.md`, applicable `.ai/**` rules, current source, tests and relevant ADRs before editing. Use the current default branch and compare with unfinished PRs/worktrees first. Do not presume paths or APIs remain unchanged. Reuse the existing Rust CLI/capability registry, generator framework, `scripts/derive`, `scripts/derive-check`, CI planner, evidence types, and projection architecture. Never create a parallel source of truth, manual duplicate inventory, or cosmetic documentation-only enforcement. Stay inside this phase's scope. No privileged branch-protection bypass, force push, unsafe cleanup, release tag, or production deploy without existing explicit authorization. Always distinguish implemented, tested, integrated, published, and live-verified. If access is unavailable, produce an exact blocker and verify everything else. Do not claim time savings without measurements.

## Standard completion handoff
At the end write a machine-readable and human-readable checkpoint in the repository's existing approved context/evidence mechanism: source HEAD and worktree state; files changed; exact test commands/results; relevant measured durations; pending blockers; invariants proven; next phase's prerequisite status. Commit and propose/integrate according to repository policy and checks; do not create a new PR if an existing one should be updated. After each change rerun the narrow relevant tests and generated-artifact drift checks. A green test for an old SHA is not proof for a new one.
