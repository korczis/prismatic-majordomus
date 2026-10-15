# PHASE 04 — SAFE RECONCILIATION AND COMMAND UX

Execute master and earlier phase requirements.

Implement read-only render/diff/status/explain/check and explicit safe sync. Follow existing CLI grammar. Own fields/tasks narrowly. For legacy unmarked files, never overwrite by default: detect ownership, use backup/three-way merge or refuse with actionable diff. Preserve user-added settings, comments where possible, manual tasks, permissions and line endings; if format-preserving merge cannot be guaranteed, refuse and explain how to migrate. Generated artifacts must be atomic, crash-safe, symlink-safe, lock/concurrency-aware and idempotent.

Use projection manifest/hash or equivalent canonical mechanism for conflict resolution; store metadata in existing project state location rather than introducing untracked clutter. Ensure no remote calls/builds in editor status or directory entry. Provide JSON machine-readable output, clean exit codes for OK/drift/conflict/unsupported/fatal, and meaningful CLI help/completion derived from command registry. Ensure `--check` never mutates and `--dry-run` produces byte-stable proposed changes.

Test migration hazards: unknown keys, comments/JSONC, duplicate task labels, file content modified after last sync, non-writable directories, symlinks, temp failure, concurrent agents, multiple worktrees and unusual paths. Explicitly test deliberate manual modifications are preserved or safely rejected. Proceed to phase 05.
