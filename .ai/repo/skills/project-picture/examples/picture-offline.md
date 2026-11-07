# Picture with no network

Intent: "Give me the picture of acme-ledger, offline." MODE offline, or MODE auto on a machine
whose remotes do not answer.

The worker runs stages 1–5 and 10 only. `git fetch` fails, so trunk is read as the last
fetched `origin/main`, and the picture says how old that fetch is. Stage 10 still reconciles
the tree against the `.ai/` records and derives plan drift from local data. Nothing from the
forge, the tracker or the conversations appears as a fact.

The coverage ledger is what makes the result honest:

```markdown
| Tier | Adapter | State | Reason |
|---|---|---|---|
| T0 | git | partial | fetch failed; origin/main as of last fetch 2031-03-11T09:02Z |
| T1 | majordomus | read | context, plan, knowledge, decisions |
| T2 | forge CLI | skipped | offline |
| T3 | tracker | skipped | offline |
| T4 | chat workspace | skipped | offline |
| T5 | mesh, other checkouts | partial | local worktrees only |
```

Next actions that would need a remote source say so, for example: "confirm PR #31's state
when online".
