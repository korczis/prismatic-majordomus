# Majordomus Mission Control: prompt pack

Use `MASTER_PROMPT.md` as the **primary single Claude Code / Opus prompt**. Its subsections can be executed concurrently only if canonical scope claims and CI isolation are honored. The separate prompts are handoff-grade atomic slices, not permission to create distinct products.

Start: `cat MASTER_PROMPT.md` in the repo. Feed it to Claude Code with highest available reasoning; ask it to apply the included order to **current** origin/master, not the snapshot noted for audit context. Then run `prompts/00-discovery.md` through `prompts/09-acceptance-and-report.md`, avoiding redundant already-delivered work. The primary goal is a working browser-tested P0 within the first integration wave.

**Order:** 00 discovery → 01 contracts → 02 local server/UI → 03 execution/worker → 04 durable events/recovery → 05 mesh/rendezvous → 06 Gigalixir → 07 delivery → 08 derived documentation/pages → 09 acceptance/report.

Guardrails: DO NOT automatically leak code, prompts, secrets or repo state to public Gigalixir; do not allow unauthenticated mutation; no user-approved credential installs by impersonation; never claim green based on skip, author assertion or stale source revision.

Contents: detailed prompts; contracts/state-machine.md; acceptance/matrix.md; execution-manifest.json. All files are instructions/specifications, not executed changes to GitHub.
