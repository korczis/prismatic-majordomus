# Release evidence template (populate from real tools, no placeholders in final)

Produce this as a **generated report from actual observed data**. Never publish a static report containing placeholder text.

## Scope and provenance

Repository remote/default branch, baseline SHA, release SHA, active .ai policy revision, task/correlation ID, provider/hook coverage matrix.

## Architectural ownership

Canonical owners reused; modules updated; removed duplicated mechanisms; typed schema migrations; source of authority for ingress/intent/execution/ledger.

## Functional proof

- Initial and follow-up prompts, one canonical intent, distinct immutable receipts.
- Context decisions linked to source/ADR/rule, with evidence and exclusions.
- Proceed/IDD/Mesh integration route selected and why.
- Durable replay/recovery and permission-denial evidence.
- CLI/API/MCP/Cockpit/docs agreement and preview/materialization demonstration.

## Verification matrix

Exact command, start SHA, result/status, diagnostics, evidence file/CI link, run timestamp. Mark unexecuted/unsupported as such.

## Delivery truth

Branch/PR links, current CI runs and tested head SHAs, merge SHAs and target branch HEAD; release version and live site revision/probe if applicable; blocked approvals with precise required action.

## Impact

Measured baseline/after for latency, context size, duplicate resolution and cost where collected, with methodology. Never fabricate percentage savings or provider reconciliation.

## Remaining debt

Only real unresolved limitations with evidence and owner; no generic wish list disguised as completion.
