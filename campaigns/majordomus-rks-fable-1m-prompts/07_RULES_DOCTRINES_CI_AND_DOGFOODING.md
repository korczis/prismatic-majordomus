# Phase 7 — Rules, Doctrines, CI Enforcement, Baselines, and Dogfooding

Use the master contract and completed RKS vertical slice.

Make RKS self-enforcing inside Majordomus and safe for adoption in other repositories.

## Doctrine

Add or extend the repository’s canonical doctrine system with a **Repository Knowledge Integrity Doctrine** equivalent to:

1. Knowledge must identify provenance.
2. Machine-derived claims must reference evidence.
3. Generated knowledge must not be manually maintained.
4. Repository changes must invalidate affected knowledge deterministically.
5. Existing documentation must be reused rather than duplicated.
6. Conflicting evidence must remain explicit until resolved.
7. Brownfield adoption must support baselines.
8. New knowledge debt must not silently increase under protect/strict policy.
9. Machine-readable representation is canonical for Majordomus-managed state.
10. Markdown is a projection or registered source, not an untracked parallel truth.
11. RKS must be worktree-aware.
12. RKS schemas and migrations are versioned.

Use the repo’s existing typed/schema/frontmatter conventions. Do not add a free-floating unvalidated Markdown doctrine.

## Rule integration

Add the narrowest necessary rules for agent behavior, especially:

- agents must inspect RKS impact before completing changes that modify tracked evidence
- generated/derived RKS files must not be manually edited if ownership says Majordomus
- external documentation must not be auto-rewritten without explicit safe ownership/policy
- any new extractor/kind/schema must integrate through canonical discovery and tests

If current AGENTS/CLAUDE files are generated from rules, update the canonical source only.

## CI modes

Integrate RKS check into existing repository gates.

Support policy modes:

```text
observe
warn
protect
strict
```

Ensure behavior is centralized.

### protect semantics

At minimum, compare current state to baseline for categories such as:

- new stale nodes
- new conflicts
- new uncovered deterministic entities
- invalid/missing evidence
- schema violations

Existing baseline debt may remain; new debt above baseline fails.

Do not collapse everything into one arbitrary scalar score.

## Baseline lifecycle

Implement explicit commands/API actions for:

- show baseline
- compare baseline
- deliberately update/accept baseline

Baseline updates must be visible in git/audit history if stored in repo and must never happen implicitly as a side effect of `check`.

## Majordomus dogfooding

Bootstrap RKS on the Majordomus repository itself.

Use actual repository evidence to create its initial knowledge state.

Then enable at least `protect` mode for the parts mature enough to enforce, or document a staged mode if existing debt requires `warn` first.

The repo must prove that:

- its own RKS knowledge is generated/discovered according to doctrine
- CI checks it
- docs/Cockpit can render it
- changes can trigger impact/staleness

## Self-reference test

Add an end-to-end test or scripted fixture demonstrating:

1. a known Majordomus subsystem has a current knowledge node,
2. a relevant source changes,
3. RKS marks the node affected/stale,
4. reconciliation restores current state,
5. `knowledge check` passes again.

Use a test fixture or temporary worktree; do not mutate developer state destructively.

## Integration with issue/milestone/worktree workflow

Where current Majordomus process metadata allows, expose knowledge impact on issue/worktree state.

A feature branch should be able to report something equivalent to:

```text
Knowledge impact: 4 nodes
Reconciled: 3
Outstanding: 1
```

Avoid hard dependency on GitHub network availability for local checks.

## CI performance

Keep fast deterministic checks in normal PR gates. Heavy semantic/LLM reconciliation must not become an always-on network-dependent CI blocker.

Separate:

```text
fast deterministic gate
optional/heavier semantic enrichment
```

## Documentation

Document:

- doctrine
- maturity modes
- baseline lifecycle
- dogfooding policy
- how a repository graduates from observe → strict

## Acceptance criteria

- RKS is enforced by existing rule/doctrine machinery.
- CI can protect against new knowledge debt.
- Baseline updates are explicit.
- Majordomus dogfoods RKS.
- Worktree/issue integration is visible where appropriate.
- Fast CI path remains deterministic and network-independent.
