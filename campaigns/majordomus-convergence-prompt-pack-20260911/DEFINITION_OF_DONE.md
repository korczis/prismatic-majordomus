# Definition of Done — Core Convergence

The convergence is complete only when the repository itself can prove the following.

## Canonical runtime

- [ ] Every public development mutation is represented by a typed canonical capability or an explicitly justified non-capability primitive.
- [ ] Each canonical operation has exactly one implementation owner.
- [ ] Shell, CLI, HTTP, MCP and Cockpit do not own duplicate domain mutation logic.
- [ ] `.ai/repo/development-semantics-baseline.txt` has no accepted backing/surface debt; remove it if obsolete.
- [ ] Adding a representative capability does not require duplicating inventories across consumers.

## Governance

- [ ] Every blocking rule is machine-decidable and executable, or is reclassified so its semantics are truthful.
- [ ] Every rule/doctrine/policy has a canonical ID, owner/source, validator relationship, test obligations and evidence relationship.
- [ ] Validators are reachable from canonical check/doctor/gate commands.
- [ ] Orphan validators and decorative unenforced rules are rejected.
- [ ] Rules can be inspected, invoked/validated and explained from CLI/API/MCP/Cockpit where appropriate.

## Testing / quality

- [ ] 100% changed-line coverage for new/modified production code.
- [ ] 100% changed-branch coverage where measurable.
- [ ] Every changed public function/command/capability/rule has behavioral tests.
- [ ] Every fixed bug has a root-cause regression test.
- [ ] Cross-surface contract tests prove canonical parity.
- [ ] Existing repository-wide coverage floor does not regress.
- [ ] Coverage policy is enforced automatically in CI and locally through canonical quality commands.

## Ordering / determinism

- [ ] No user-facing/default collection depends on filesystem, locale, hash/insertion or concurrent completion order.
- [ ] Canonical presentation order is defined once and reused.
- [ ] `order-check` is green without increasing debt baselines.
- [ ] Algorithmic/internal sorts are distinguished from presentation ordering by explicit architecture/checking, not accidental grep exceptions.
- [ ] Shell collation is deterministic wherever shell sorting remains.

## Session / provider lifecycle

- [ ] Supported providers have an explicit lifecycle/capture capability matrix.
- [ ] Claude/Codex/Gemini integrations are implemented to the maximum semantics each provider exposes, with tested graceful degradation.
- [ ] Unsupported lifecycle features are visible as unsupported, never silently absent.
- [ ] Session context generation/reload/checkpoint/handover is automatic at provider lifecycle boundaries where available.
- [ ] Provider-specific adapters feed one canonical session/context model.

## Cockpit / API / MCP

- [ ] Cockpit is a management/control surface, not only a read-only catalogue.
- [ ] Applicable mutating capabilities can be invoked safely through the canonical execution/permission path.
- [ ] Inputs are schema-driven and validated.
- [ ] HTTP/OpenAPI/MCP projections derive from canonical capability metadata.
- [ ] Navigation/areas/routes are consistent and drift-tested.
- [ ] No frontend array duplicates backend canonical inventories without generation/drift proof.

## Claims / evidence

- [ ] Every guaranteed claim maps to executable obligations.
- [ ] Current test/gate execution evidence is linkable by commit/build/run identity.
- [ ] Claim drift is machine-detectable.
- [ ] Documentation/site shows claim state truthfully.

## Entry / runtime convergence

- [ ] Repository entry can discover/attach/start the required local Majordomus runtime without duplicating domain logic into `.envrc`.
- [ ] Entry has bounded latency and no unexpected network/build work in the hot path.
- [ ] Stale/missing runtime is surfaced and repaired through a deliberate convergence mechanism.
- [ ] Session/peer/context status is observable and testable.

## Repository hygiene / landing

- [ ] Relevant worktrees/branches are reconciled; no needed implementation remains stranded.
- [ ] Generated artifacts are synchronized.
- [ ] Local canonical gates pass.
- [ ] CI passes on the landed commit.
- [ ] GitHub Pages/docs deployment is current and verified when part of the project pipeline.
- [ ] Runtime deployment is verified where configured.
- [ ] No stale claim says an unbuilt feature is complete.
- [ ] No acceptance baseline hides core convergence debt.

## Final zero-registration proof

Perform at least one controlled representative addition (then keep or revert as appropriate) proving that a new typed entity/capability/rule propagates automatically to all relevant projections without editing every consumer manually.
