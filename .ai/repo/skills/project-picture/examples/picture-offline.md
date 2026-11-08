# Picture with no network

Intent: "Give me the picture of acme-ledger, offline." MODE offline, or MODE auto on a machine
whose remotes do not answer.

The worker runs stages 1–5 and 10 only. `git fetch` fails, so trunk is read as the last
fetched `origin/main`, and the picture says how old that fetch is. Stage 10 still reconciles
the tree against the `.ai/` records and derives plan drift from local data. Nothing from the
forge, the tracker or the conversations appears as a fact.

The coverage ledger is what makes the result honest:

```markdown
| Tier | Adapter | State | Elapsed | Locators | Newest | Reason |
|---|---|---|---|---|---|---|
| T0 | git | partial | 5 s | 28 | 2031-03-11 | fetch failed; origin/main as of last fetch 2031-03-11T09:02Z |
| T1 | majordomus | read | 19 s | 15 | 2031-03-10 | context, plan, knowledge, decisions |
| T2 | forge CLI | skipped | 0 s | 0 | – | offline |
| T3 | tracker | skipped | 0 s | 0 | – | offline |
| T4 | chat workspace | skipped | 0 s | 0 | – | offline |
| T5 | mesh, other checkouts | partial | 3 s | 4 | 2031-03-11 | local worktrees only |
```

Next actions that would need a remote source say so, for example: "confirm PR #31's state
when online".
