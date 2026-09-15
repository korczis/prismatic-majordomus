# Phase 11: Final Proof, Remote Verification, Recovery Check, and Handover

## Goal

Prove that the repository is actually clean, integrated, reproducible, and safer than before.

Do not remove campaign recovery artifacts until this proof is complete.

## 1. Fresh remote verification

Fetch remotes again.

Verify:

- local integration branch OID,
- `origin/master` OID,
- expected equality/relationship,
- no accidental unpushed integration commits,
- no remote divergence introduced by concurrent work.

## 2. Final topology proof

Capture final:

- `git worktree list --porcelain`,
- local branch inventory,
- relevant remote branch inventory,
- stash inventory,
- current status in every surviving worktree,
- branch↔path mapping validation.

Every surviving exception must have a reason.

## 3. Full validation

Run repository-canonical equivalents of:

- formatting,
- lint/static checks,
- build/check,
- full test suites relevant to changed workspace,
- CLI integration tests,
- API/OpenAPI tests,
- MCP tests,
- Cockpit/frontend tests,
- schema/front matter validation,
- docs/site build,
- generated drift checks,
- worktree lifecycle integration tests,
- repository doctor/check gates.

If the repo is large, use the canonical aggregate gate rather than inventing incomplete hand-selected commands.

## 4. Recovery drill

Prove that at least the major pre-cleanup state could still be recovered from the recorded safety mechanism.

You do not need to fully restore into the live repo.

Verify:

- bundle/tag/ref exists,
- representative object IDs resolve,
- preserved untracked artifact checksums match,
- recovery instructions are sufficient.

Only after this proof may temporary campaign artifacts be compacted/removed according to policy.

## 5. Inspect diff/history quality

Review:

- recent master history,
- merge/rebase artifacts,
- accidental generated churn,
- duplicate implementations,
- temporary debugging code,
- commented-out legacy blocks,
- local absolute paths,
- leaked credentials,
- campaign-only files accidentally committed.

Fix issues before finalizing.

## 6. Final acceptance matrix

All must be true or explicitly explained:

### Git topology

- [ ] primary integration branch is in canonical primary repo
- [ ] surviving feature worktrees are under inferred sibling `-wt` root
- [ ] branch/path mapping is deterministic
- [ ] no duplicate feature worktree exists
- [ ] no stale worktree metadata remains
- [ ] no unknown detached HEAD remains

### Integration

- [ ] all merge-worthy work from campaign is integrated
- [ ] superseded work has evidence
- [ ] deferred work is explicit and isolated
- [ ] master/origin synchronization is verified
- [ ] no force-push occurred on integration branch

### Preservation

- [ ] dirty/untracked work was preserved before destructive actions
- [ ] stashes were audited
- [ ] recent relevant dangling/reflog-only work was audited
- [ ] recovery checkpoint was verified

### Repository hygiene

- [ ] legacy worktree roots are gone or explicitly retained
- [ ] root clutter in scope is cleaned
- [ ] duplicate session/context mechanisms in scope are consolidated
- [ ] generated artifacts have canonical ownership
- [ ] no campaign temp junk remains committed accidentally

### Enforcement

- [ ] worktree topology is validated automatically
- [ ] unsafe cleanup is blocked
- [ ] lifecycle tooling is tested
- [ ] governance docs/rules/doctrines are updated appropriately
- [ ] no duplicate consumer registry exists

### Surfaces

- [ ] CLI uses canonical model
- [ ] API/OpenAPI reuse canonical model where applicable
- [ ] MCP reuses canonical model where applicable
- [ ] Cockpit reuses canonical model where applicable
- [ ] docs are synchronized

### Quality

- [ ] canonical repository gates pass
- [ ] generated drift checks pass
- [ ] no secrets/local absolute paths introduced
- [ ] final Git status is clean in integration worktree

## 7. Final handover report

Provide a compact but precise report:

### Before

- worktree count and roots,
- branch count/classification,
- dirty/stash/detached risks,
- major clutter categories.

### Integrated

For each integrated feature:

```text
source branch
old head
strategy
resulting master commit/range
validation
```

### Removed

- worktrees,
- local branches,
- remote branches if any,
- stashes,
- stale metadata,
- repository clutter.

### Retained intentionally

List exact branches/worktrees/artifacts and reasons.

### Enforcement added

Explain the single canonical lifecycle architecture and gates.

### Validation evidence

List exact commands and results.

### Recovery evidence

Explain checkpoint location/refs and how recovery was verified.

### Remaining debt

Only genuine unresolved items with exact reasons. Do not invent future work to make the report look sophisticated.
