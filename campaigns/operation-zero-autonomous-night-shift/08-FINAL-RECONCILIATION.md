# 08 — Exit criteria and global reconciliation

Re-fetch GitHub PR state with full pagination and independently check `is:open is:pr`. Verify no open PRs, accounting for delayed indexing via direct API state when needed. Query all unresolved version/release/deploy obligations, linked independently completed issues and milestones, unexpired leases, dirty orphan worktrees, pending CI verdicts, failure queue and governance/claims drift. Cross-check git tags, GitHub releases, actual deployed build/revision and rendered API/UI/MCP projections.

Success is `open_pr == 0 && pending_required_version == 0 && pending_release == 0 && pending_deploy_verification == 0 && canonical_gates_green && reconciled_related_entities && no_invalid_cleanup`. A zero PR count reached by merely closing all PRs is FAILURE. If external blockers remain, show the exact nonzero fixed point and proof that all independent runnable work was processed. Do not conceal blockers or claim 100% complete.

At the end, produce chronological ledger: per PR number, disposition, merged SHA/proof, linked issue/milestone, completion ID, allocated version or justified non-release, release URL, deployed revision, gates, blockers and elapsed stage timings. Produce concise operator report; no vague 'done'.
