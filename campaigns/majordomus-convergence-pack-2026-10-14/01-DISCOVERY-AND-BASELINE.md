# Mission 01 — Live inventory and evidence baseline

Goal: invalidate stale assumptions before modifications.

1. Confirm repository owner, default branch, HEAD SHA, commit timestamp, working tree state, git remotes, active branches and worktrees. Check whether running inside a dirty tree; do not steal another session's state.
2. Read repository policy, `.ai` knowledge, rules, doctrines, scopes, worktree declarations, ADRs, schemas, CI definitions, release metadata, deployment docs, server boundaries, actual Rust binaries and shell entrypoints.
3. Query GitHub API (via approved tools, `gh` if authenticated) for all open PR and issues, states, merge bases, review status, checks, conflicts, branches, status age, labels, dependencies, release tags, deploy workflow status and published artifacts. Paginate. Snapshot all data and timestamp it. Previous sampling reported 42 open PR and 100 sampled open issues; recalculate without truncation.
4. Compare `master` against audit baseline 97c4521. For each cited issue, determine whether it remains open, changed, delivered elsewhere, superseded, missing, or disproven. Distinguish GitHub issue state from `.ai` derived issue status.
5. Build source map: Rust crate(s); portable shell core; canonical policy/governance; mesh modules; HTTP/server modules; UI static/JS; site generation; agent adapters; tests; CI gates; deployment.
6. Resolve actual CLI/just commands from registries, not copied examples. Determine unit vs end-to-end suites and which side effects need permission.
7. Record test baseline WITHOUT modifying fixtures: formatting, static checks, targeted suites, derive-check, docs checks, credentials scanner, last actions statuses. Capture command, SHA, exit code, duration, known environment limitations. CI red != necessarily code red; isolate.
8. Inventory public claims. Read `docs/SITE_CLAIMS.md` and its source of truth (audit observed `docs/CLAIMS.yaml`), classify guarantees vs advisory/planned/rejected. Collect evidence pointers.
9. Inventory the 42 previously observed PR by exact head/base SHA, patch-id and changed paths, detect stacked chains, duplicates already in trunk, stale branches, open security impacts, merges that occurred since sampling.
10. Produce architecture map grounded in actual source, not hypothetical: boundary, responsibility, owned durable state, data contract, surface projections and test coverage.

Deliverables: `reports/00-baseline.md`, machine-readable `reports/baseline.json` (omit secrets), `reports/issue-coverage.tsv`, `reports/pr-inventory.tsv`, `reports/provenance-map.md`. Use repository's canonical report location if prescribed.

Acceptance: no finding is fixed from an unverified historical assumption; precise HEAD and date recorded; all API pagination complete or bounded/marked incomplete; baseline failures separated from new failures; every finding has an owner and exact evidence status.
