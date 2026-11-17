# 08 — Documentation, landing, pages and design consistency

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Derive docs, CLI/MCP/OpenAPI references, UI & Cockpit pages, GH Pages public snapshots, English+Czech landing, release notes, demonstration scenarios and breadcrumbs from canonical registry, execution schema and measured receipts. Ensure no source duplication, generated fingerprints and checks, public-safe redaction. Landing sells actual browser-tested story and explicitly distinguishes local live UI versus Pages snapshot, configured versus reachable Gigalixir, installed-release status. Playwright across languages and widths (320/390/768/desktop) verifies every button/link either works or shows grounded unavailable state, screenshots and accessibility. Don't embed localhost links into public pages for remote visitor as though navigable; provide safe setup deep links and QR only when identity/route secure. All data tied to observed SHA.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
