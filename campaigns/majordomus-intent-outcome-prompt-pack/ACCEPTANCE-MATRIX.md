# Executable acceptance matrix

Create behavioral test(s) consistent with the repo's existing test infrastructure. Each row must map to an actual test run, source revision and documented verdict, not a ticked box by assertion.

| ID | Scenario | Required observed behavior |
|---|---|---|
| A01 | Intent with no work | Declared/planned semantics follow canonical derivation, not false completion |
| A02 | Criteria link missing | Block and report unresolved reference |
| A03 | Issue missing serves | Refuse unexplained work where policy requires intent coverage |
| A04 | Plan start before critique | Atomic refusal before state transition |
| A05 | Critique with open blocker | Start refused, reason traceable |
| A06 | Gap missing criterion | Validation failure with criterion ID |
| A07 | Synthesized plan rerun | Stable identities, no duplicate tasks/issues |
| A08 | Only local worker available | Progress or explicit blocked; no fake peers |
| A09 | Parallel workers race | Lease/ownership prevents duplicate execution |
| A10 | Interrupted execution | Resume deterministically without duplicate side effects |
| A11 | Tests pass but criterion not evidenced | Intent remains unsatisfied |
| A12 | Command exits 0 but checks wrong thing | No semantic proof accepted |
| A13 | Deployment cannot be independently verified | Unknown/not_derivable, not satisfied |
| A14 | Stale test/command/deploy evidence | Satisfaction revoked |
| A15 | Failed run after completed milestone | Contradiction visible, proper validation response |
| A16 | Updated source after green run | Re-verification and targeted replan |
| A17 | Superseded/cancelled intent | No unintended autonomous execution |
| A18 | Invalid/stale peer result | Not accepted as proof |
| A19 | Platform repo / injected repo | Same policy, types, gates and behavior |
| A20 | CLI / REST / MCP / Cockpit | Agreement on derived stage and proof state |
| A21 | Mobile viewport / keyboard | All primary navigation/actions usable, no broken drawers |
| A22 | New criterion or entity | Appears without new hardcoded consumer registration |
| A23 | CI/local gate | Missing critique/evidence fails where blocking |
| A24 | External side effect unauthorized | Refused or approval-gated, not silently executed |
| A25 | Missing provider/network | Truthful degraded mode, no fake green |
| A26 | Repeated run on same data | Deterministic output, no task churn |

**Pass standard:** all in-scope rows have executed evidence; missing capabilities remain clearly not implemented. Local benchmark for critical paths, schema/contract tests, security redaction tests, cross-platform behavior where CI runners exist, canonical full check. Do not claim platform-independent testing if only one OS was exercised.
