# Acceptance / evidence matrix

Never pre-fill PASS. Agent must update using observed SHA/run evidence, timestamp and source. `UNKNOWN` is not pass.

| Area | Acceptance probe | Expected failure mode | Evidence required |
|---|---|---|---|
| Queue | every open PR classified against fresh base SHA | unknown/conflict blocks integration | PR URL, head/base SHA, checks |
| WIP | bound on active conflicting implementation | no silent spawning | queue snapshot, enforcement test |
| Worktrees | dirty/untracked/active/stale safety fixtures | unknown never deleted | test output + git topology |
| Merge | app-bound required checks on current SHA | stale/foreign check refused | GitHub statuses + merge SHA |
| CI planner | unknown change or missing gate | escalate/refuse | plan JSON + mutation test |
| CI performance | comparable before/after p50/p95 | none; state not measured | workflow/job timing URLs |
| Mesh journal | crash between write and ack | no false durable ack | fault-injection test logs |
| Mesh replication | reorder/duplicate/partition/heal | stable fold and explicit conflict | process lab logs |
| Trust | revocation while linked | reject further authority | revocation integration test |
| MCP | ambiguous side-effect completion | no blind resend | test with injected timeout |
| HTTP | slow clients | bounded resources, other clients served | load test and resource profile |
| Lease | supersede/idle/ownership | old owner cannot remove new lease | multiprocess tests |
| Release | exact CI SHA and verified provenance | no artifact release on failed/missing proof | release run + digest |
| Foreign install | clean unrelated Git repo | no source-tree dependency | smoke logs per platform |
| Intent | real intent→plan→review→proof→satisfaction | stale review/proof blocks | evidence ledger + current SHA |
| Completion | tasks obligations and criteria separate | done does not assert satisfaction | negative tests |
| UI | unknown/failed/slow/unauthorized | no fake green/zero, no ghost actions | browser tests / screenshots |
| Mobile | 320, 360, 390 + desktop widths | no broken drawers/overflow | browser results |
| Cross-surface | new capability in one canonical registry | zero manual registrations | CLI/API/MCP/Cockpit/doc assertions |
| Rollback | failed install/release/upgrade | reversible and logged | test logs |

## Scorecard
- Current default branch SHA: __
- Latest published release/tag: __
- Active/open/draft PR before and after: __ / __
- Targeted merge validation p50/p95 before and after: __ / __
- Full validation p50/p95 before and after: __ / __
- CI minutes per integrated PR before and after: __ / __
- Critical open correctness PR: __
- Release installation matrix result: __
- Demo intent ID, stage, criteria and evidence: __
- Deploy SHA / observation timestamp / verification: __
- Remaining risk-ranked blockers: __

## Report format
For each matrix row use `PASS | FAIL | BLOCKED | UNKNOWN`, *never inferred*. Attach command, log link/file, exact SHA, timestamp and whether the test ran in CI/local/production. A missing app permission does not mean a check passed. A release missing one declared platform fails its release contract. A status stale after a new commit is invalidated.
