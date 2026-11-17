# 02 — `majordomus serve` local-first interactive UI

**Read MASTER_PROMPT.md first; its invariants are binding.**

## Implementation directive
Make serve discover repo exactly once, restore durable local state and expose current live /ui/idd through the existing HTTP server. Add responsive entity navigation intent→milestone→issue→task, readiness/policy/evidence, context-sensitive primary action (Implement/Resume/Verify/Deliver/View blockers), confirmation/details if privilege needed, progress drawer and keyboard/accessibility. No fake statuses, no bespoke UI DB; state comes from canonical capabilities. Browser Playwright: visit /ui/idd, open real intent and issue, start with click, display a stable request id, forbid invalid action and show reason, refresh 320px and desktop, no JS errors. Reuse existing case 3220 and enhance, never replace with a toy screenshot test.

## Mandatory handoff
Write: canonical files changed, existing capabilities reused, tests run/results, CI SHA, decisions, blockers, safe rollback, risk of change and next step. Keep incomplete as incomplete.
