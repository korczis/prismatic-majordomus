# Workbench acceptance matrix and launch check

| Test scenario | First expected | Owning prompt |
|---|---|---|
| Open `New Intent` from `/ui/idd` | Real editor; no tracked writes | P00 |
| Edit long multilingual free text, refresh, resume | Exact draft and revision restored | P01 |
| Two tabs edit same revision | Conflict, never silent overwrite | P01 |
| Draft cites ADR + existing intent with valid links | Reasons + source IDs + freshness | P02 |
| Changed text while old request is in flight | Old result discarded | P02, P03 |
| Actual model analysis with evidence | Real source-backed suggestion, never fake | P04 |
| Provider missing | Explicit unavailable/partial; editor still works | P04 |
| Ambiguity requires decision | One relevant question, answer updates draft | P05 |
| Prompt and pack from one revision | Stable hashes, copy/export, source anchors | P03 |
| Requirement restates an intent or issue | Match existing, no new issue | P06 |
| Draft has uncovered criterion / cyclic dependency | Shared validator refuses without writing | P07 |
| Explicit approval of current exact digest | Valid canonical records, one write | P08 |
| Same approval key repeated | No duplicate records | P08 |
| Master/source revision changes before approval | Refused stale, recheck required | P08 |
| Ready issue dispatched twice | One existing execution, canonical status | P09 |
| Evidence fails | Cannot mark intent satisfied | P09 |
| Malicious HTML/prompt injection/private canary | No execution, no XSS, no public leak | P00–P10 |
| End-to-end on real server | Browser reaches canonical plan | P10 |
| Master/release/deploy evidence | Observed, not inferred from PR green | P11 |

## Minimum launch checkpoints

**Checkpoint A: LOCAL FIRST USABLE**
- A real server route and New Intent entry exists.
- Draft survives reload.
- Context cards have real source anchors and state.
- Prompt and atomic pack are copyable, revision-bound, and contain truthful citations.
- Assistant answer actually executes when available; fallback is explicitly unavailable.
- No raw transcript, credentials or private draft in committed/public artifacts.

**Checkpoint B: MATERIALIZED + EXECUTABLE**
- Dedupe + coverage classification grounded in canonical records.
- In-memory validator identical to actual plan/intent validation.
- Materialization is human-authorized, CAS-safe, idempotent and atomic.
- Issues are canonical and reach existing proceed/execution lifecycle.
- Derived satisfaction remains evidence-gated; published/static UI is not the local editor.

## Delivery gates per prompt
1. Current master SHA and repo context resolved.
2. Existing issues/PR collisions and dependencies considered; scope/peer claim obeyed.
3. Rust/Cargo tests for touched area; browser test if UI changes.
4. Relevant `majordomus intent validate`, `majordomus plan validate` and `scripts/ui check`, using actual supported invocation.
5. `majordomus generate` and `majordomus generate --check` for generated surfaces when relevant; use-case impact/coverage where relevant.
6. Security/privacy tests for private state and public projection if touched.
7. Canonical docs, version/release policy, diff review, no placeholder artifacts.
8. Pushed PR + observed CI + guarded merge to master if authorized, otherwise explicit evidence-backed blocked state.
9. Real runtime reproduction, and deployed surface verification when genuinely applicable.
