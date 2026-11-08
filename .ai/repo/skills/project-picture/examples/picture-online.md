# Picture of one supervised project, every source reachable

Intent: "Load everything about acme-ledger and give me the picture." MODE auto resolves to
local and supervised, DEPTH standard, SINCE 30 days.

The worker runs stages 1–5 before any network call. It then gives the forge, the tracker and
the chat workspace one read-only worker each, and reconciles their results in stage 10.

Excerpt of the Markdown output:

```markdown
## TL;DR (as_of 2031-03-14)
- acme-ledger reconciles supplier invoices read from scanned documents.
- Trunk has the matching engine; operation still runs the version before it.
- Blocker: deploys from trunk fail since 2031-03-03 (approval guard, run 812).
- Next: a second reviewer approves PR #23; owner: the infrastructure lead.
- Scope agreed in chat on 2031-03-12 is not in the plan yet.

## Contradictions
| One side | Other side | Locator |
|---|---|---|
| plan: T7.2 todo (DERIVED from plan.json) | PR #31 merged, closes it (OBSERVED) | plan.json@b3e19c0, PR #31 |
| chat: scope cut to 10 days (OBSERVED, internal) | plan: 21 days remain (OBSERVED) | #acme-internal 1962…, docs/effort |

## Coverage
| Tier | Adapter | State | Elapsed | Locators | Newest | Covered |
|---|---|---|---|---|---|---|
| T0 | git | read | 4 s | 31 | 2031-03-14 | origin/main at b3e19c0, 30 days |
| T1 | majordomus | read | 22 s | 17 | 2031-03-09 | context, plan, 0 candidates; handover stale (5 d) |
| T2 | forge CLI | read | 41 s | 63 | 2031-03-13 | 42 open, 18 closed issues, 3 PRs, deploy runs |
| T3 | tracker API | failed | 2 s | 0 | – | 401 unauthenticated; offline export used |
| T4 | chat workspace | partial | 60 s | 24 | 2031-03-14 | 2 channels (1 shared-with-external), 2 DMs; cut at its budget share |
```

The JSON rendering carries the same sections, and every finding carries its `tag` and
`locator`.
