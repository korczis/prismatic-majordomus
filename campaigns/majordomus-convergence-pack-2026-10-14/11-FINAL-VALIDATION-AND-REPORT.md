# Mission 11 — adversarial acceptance and authoritative release decision

The terminal action is NOT 'merge complete'. It is a fresh evidence-based review at the final integrated SHA.

## Independent reviewer checklist

1. Confirm all listed P0, P1 and P2 findings from the ledger have delivered, superseded, invalidated, or BLOCKED disposition with evidence. A closed GitHub issue without test and landed change is not delivered.
2. Run full repository's prescribed commands for format, clippy/static, unit, end-to-end, docs, schema, derives, governance doctrines, negative/fault tests, security, UI, installer and performance. Execute with clean checkout; capture SHA/platform/results/timestamps.
3. Verify mutation tests: deliberately break one policy wire, one derive projection, an authentication guard, a journal durability expectation and a release record, and confirm the actual gates refuse each. Restore each mutation cleanly.
4. Compare real API/CLI/MCP/UI/Pages entities and status semantics, versioned schema and missing-data behavior; no separate shadow inventories.
5. Inspect `git diff --check`, generated drift and file ownership; secret scan test output and all artifacts; audit dependencies and licensing where supported.
6. Fresh PR reconciliation, no lost unique patches; unmerged/blocked items enumerate proof and next owner. Trunk branch protection and CI green on exact head, not historic green.
7. Release only if policy allows. Verify artifact checksums, installer on target, published Pages commit, correct version and live smoke. Else report 'not released', never partial green.
8. Re-audit public docs, README, landing page, claims against CURRENT evidence and publish changes through canonical generator.
9. Run controlled live or emulated mesh failure cases; distinguish outcomes precisely.
10. Independent reviewer challenges all 'PASS' claims by opening artifacts and attempting minimal reproduction. Reject claims with missing provenance.

## Mandatory final Czech report

- Verified repo, HEAD, date and environment;
- What was fixed, by issue and commit/PR;
- What failed or is blocked and why;
- Exact test commands, passed/failed counts and logs/artifacts;
- Security boundary and remaining threats;
- Before/after benchmarks with sample definitions;
- PR graph and backlog delta without inflated throughput metrics;
- Installed/public deployment state, verified SHA;
- External adoption experiment and reproducible transcript;
- Claims changed, guarantees downgraded, newly provable truths;
- Residual P0/P1 gaps, owner and next step;
- Final rating with explicit uncertainty and no celebratory claims unsupported by evidence.

NO unconditional 'done'. Report `COMPLETE`, `PARTIAL`, `BLOCKED`, `UNVERIFIED` per dimension and overall. Machine report and human summary derived from same canonical ledger.
