# Majordomus Convergence Prompt Pack

Purpose: finish the architectural convergence of `prismatic-majordomus` instead of adding another layer of partially integrated features.

This pack was prepared from the 2026-09-11 repository snapshot whose audit identified a strong canonical/derived architecture plus several remaining migration gaps. Treat every audit fact below as a starting hypothesis to re-verify against the checkout you actually run against. The repository is the authority, not this pack.

## What this pack is for

Use it when you want an implementation agent to take Majordomus from “many of the right mechanisms exist” to “the core invariants are actually true end-to-end”. It is deliberately biased toward closure, deletion of accepted debt, and proof.

The desired end state is:

```text
facts / intent
    ↓
typed canonical model
    ↓
capabilities + rules + evidence
    ↓
one implementation owner
    ↓
CLI / HTTP / OpenAPI / MCP / Cockpit / docs / generated site
    ↓
end-to-end tests + CI gates + deployed evidence
```

No consumer-specific shadow reality. No human-only “blocking” invariant. No permanent baseline that merely memorializes known breakage. No claim of completion without execution evidence.

## Recommended execution

There are two modes.

### Mode A — autonomous convergence

Give `prompts/00_MASTER_AUTONOMOUS_CONVERGENCE.md` to a high-context coding agent at the repository root. This is the preferred mode when the agent can operate for a long session, inspect the whole tree, edit, test, commit, push, and verify CI/deployment.

### Mode B — staged

Run prompts `01` through `15` in order. Each prompt is independently restartable. Before a stage begins, it must read the handover/evidence from previous stages and re-verify the repository state.

Never run later stages merely because an earlier agent wrote “done”. Run the gates.

## Hard completion rule

A stage is not complete because code exists. It is complete only if all applicable closure dimensions are proven:

- root cause understood,
- canonical implementation exists,
- legacy/duplicate implementation migrated or deleted,
- tests cover success, failure, edge and regression paths,
- affected public surfaces are integrated,
- rule/doctrine/policy enforcement is executable,
- generated artifacts and docs are synchronized,
- CI/local gates pass,
- commits are landed,
- remote state is verified when access permits,
- deployment/site/runtime state is verified when the project requires it,
- evidence is recorded and linked.

See `DEFINITION_OF_DONE.md`.

## Important anti-patterns

Do not “finish” the project by:

- raising a baseline,
- weakening a gate,
- deleting a failing test without replacing its obligation,
- adding `.sort()` calls independently across consumers,
- duplicating command/capability/rule lists,
- generating docs from a different source than runtime,
- creating a Cockpit-only implementation of domain semantics,
- preserving shell business logic simply because it already works,
- adding a second parser for canonical data,
- marking a claim `guaranteed` because a source file exists,
- calling a reviewer-held convention “machine enforced”,
- creating an “adapter” that only forwards to the old split-brain implementation forever,
- expanding the roadmap before foundation closure.

## Snapshot facts to re-verify

The audit used a snapshot identified as `master` at commit:

```text
1a651e644dfde8b11a53cc343551e3215df0d86d
```

Observed at that snapshot:

- `.ai/repo/development-semantics-baseline.txt` contained 15 `backing:*` debts plus one Cockpit surface debt.
- command capabilities were materially narrower than the shell development lifecycle; the obvious mutating capabilities included `executions.start`, `executions.cancel`, and `peers.announce`.
- `scripts/rust-coverage-threshold` contained `90`.
- `lib/capture.sh` registered automatic capture/lifecycle adapters for `claude-code`, while provider metadata/MCP configuration supported additional providers.
- canonical-order debt baseline was `crate_sorts=106`, `shell_sorts=0`; audit scanning observed a rise to approximately `122` and `13` respectively.
- `scripts/ci/lease-reader-check` appeared capable of flagging its own lease/schema references.
- `cockpit/nav.rs` described/declared `Executions` and `Quality` areas while the runtime area inventory required verification for consistency.
- the repository already had substantial rules, doctrines, claims, ADRs, issue/milestone data, cross-surface tests, generated docs and CI machinery.

Do not blindly preserve these numbers. They are coordinates for investigation, not desired baselines.

## Files in the pack

- `INSTRUCTIONS.md` — operator protocol and handoff rules.
- `AUDIT_FINDINGS.md` — concise technical starting evidence.
- `DEFINITION_OF_DONE.md` — final invariant checklist.
- `PROMPT_INDEX.md` — stage purpose/order.
- `prompts/00_MASTER_AUTONOMOUS_CONVERGENCE.md` — one long-running end-to-end prompt.
- `prompts/01_...` through `15_...` — focused stages.
- `templates/EVIDENCE_REPORT.md` — evidence format.
- `templates/HANDOVER.md` — restart-safe stage handoff.
- `templates/DEBT_LEDGER.md` — temporary migration debt tracking; final accepted debt should be zero for core convergence.
- `templates/RULE_COVERAGE_MATRIX.md` — rule → validator → tests → surfaces → evidence.

## Final objective

The pack is successful only when Majordomus can prove, from its own canonical data and executable checks, that the core development/governance lifecycle is single-source, capability-driven, fully enforced, tested, documented, surfaced, landed and verified.
