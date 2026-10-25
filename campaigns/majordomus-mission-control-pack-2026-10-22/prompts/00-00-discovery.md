# 00 — Current-state audit and reusable ownership map

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Fetch latest master and all relevant active PRs: #821 completion policy, #1840 integration train, #2365 guarded fast-forward, provider.yml, deployment model, MCP bridge and IDD test 3220 (numbers are historical). Identify canonical owners of capability invocation, execution start/transition, worker lifecycle, session, mesh nodes, journal, peer board, lease, event transport, repository discovery, policy/approval, issue/intent model, UI router, snapshot generator, release and provider. Classify present/partial/missing at runtime AND production. Build executable dependency DAG, current SHA and high-impact blockers. Determine current gigalixir secrets as present/absent by safe workflow status only. Document capability/HTTP/MCP route reuse table and risks. Avoid implementation claims from PR body alone. Deliver one verified baseline document, failure-first acceptance list, no code yet.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
