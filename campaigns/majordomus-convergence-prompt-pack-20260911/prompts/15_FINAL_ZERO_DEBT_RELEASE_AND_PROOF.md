# Stage 15 — Final Zero-Debt Release and Proof

Close the convergence. No celebratory prose until the repository proves it.

## 1. Re-run from clean checkout state

Prefer a fresh worktree/clone of the landed candidate SHA so untracked local state cannot make tests accidentally pass.

Run the full canonical quality pipeline, including every new gate introduced in stages 02–14.

## 2. Verify zero core debt

Search canonical debt/baseline/allowlist inventories. Core convergence categories must have no accepted violation:

- development capability backing,
- Cockpit semantic surface ownership,
- blocking rule enforcement,
- changed-code coverage,
- presentation ordering,
- provider lifecycle registration,
- lease/parser single-source,
- generated drift,
- guaranteed claim evidence.

If a generic historical baseline remains for unrelated legacy debt, prove it does not contain any core item covered by this pack.

## 3. Zero-registration demonstration

Using a controlled fixture or temporary test entity:

- add one representative canonical capability/entity/rule,
- verify it is discovered,
- verify canonical order,
- verify CLI structured listing,
- verify HTTP/OpenAPI projection,
- verify MCP projection,
- verify Cockpit projection when metadata permits,
- verify generated docs/index,
- verify required test/enforcement obligations are demanded automatically.

The demonstration itself should become a permanent integration/property test if practical.

## 4. Version/changelog/release

Follow repository semantic/Elm-like versioning policy if it exists. Derive the version bump from actual public API/schema changes. Update changelog/release notes from canonical change/evidence data where tooling supports it.

Do not manually bump versions in multiple locations.

## 5. Land and deploy

- final commits clean,
- push,
- verify remote target branch SHA,
- verify CI green at exact SHA,
- verify package/release jobs if applicable,
- verify GitHub Pages/docs at exact revision/version,
- verify runtime deployment/health if configured,
- verify Cockpit/API/MCP smoke tests against deployed/local release target as appropriate.

## 6. Final evidence report

Produce a concise but complete report:

### Root causes removed

List systemic causes, not symptoms.

### Canonical architecture now

Show owner → projections data flow.

### Debt retired

Show before/after counts and removed baselines.

### Rules/enforcement

Show blocking rule coverage totals and proof.

### Testing

Show changed line/branch coverage and major E2E suites.

### Cross-surface parity

Show representative capability/rule/session/provider traversals.

### Adversarial proof

Summarize stage 14 mutation results.

### Landing/deployment

List landed SHA, CI run, docs/deploy evidence.

### Remaining limitations

Only genuine product limitations or external blockers. Do not list unfinished core convergence as “future improvement”.

## Final acceptance statement

Only state completion if the evidence supports:

> Majordomus discovers/models once, executes/enforces once, and derives every relevant surface from canonical typed state; core rules and changed code are fully tested, core migration debt is zero, and the landed/deployed state has been verified.
