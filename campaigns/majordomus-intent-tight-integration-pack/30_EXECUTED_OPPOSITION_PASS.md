# Milestone 3 — Turn critique records into an executed opposition pass

Goal: Majordomus should orchestrate the opposition step, not merely refuse work if somebody forgot to write a critique.

## Current weakness to close

A typed critique record plus a "must exist before execution" guard proves paperwork happened, not that the system actually performed the opposition step.

## Target lifecycle

```text
gap
  → draft plan
  → opposition requested
  → opposition executed
  → findings
  → plan revised or accepted
  → execution allowed
```

## Provider independence

Core correctness MUST NOT require a specific LLM vendor.

Use the repository's existing reasoning/advisor/provider abstraction if present.

Possible execution modes, in priority order:

1. available configured advisor/agent performs critique,
2. local deterministic structural opposition checks run,
3. if external model is optional/unavailable, record that fact and execute every deterministic opposition check available,
4. policy decides whether absence of model-based critique is blocking for this plan class.

Do not hardcode Claude/Gemini/Codex.

## Opposition input

Must be derived from canonical state:
- intent,
- invariants,
- criteria,
- gap,
- proposed milestones/issues,
- dependencies,
- evidence assumptions,
- relevant ADRs/rules.

## Opposition output

Typed, append-only evidence:

```text
OppositionResult
  plan_revision
  concerns[]
  challenged_assumptions[]
  uncovered_criteria[]
  dependency_risks[]
  scope_risks[]
  verdict
  provenance
```

Adapt to current schema conventions.

## State transition

Execution may begin only when:
- required opposition has run,
- result applies to current plan revision,
- blocking findings are resolved or accepted through canonical decision policy.

A critique against an old plan revision must not authorize a changed plan.

## Tests

Prove:
- missing opposition refuses,
- stale opposition refuses,
- changed plan invalidates prior approval,
- blocking finding prevents execution,
- resolved/revised plan passes,
- no-provider mode behaves according to policy,
- deterministic opposition checks always run,
- opposition provenance is visible.

## Surfaces

Expose status through:
- plan/intent CLI,
- structured JSON,
- API/MCP if plan state is public,
- Cockpit intent view later.

## Acceptance criteria

There is no path where execution is considered opposition-complete merely because a manually authored critique file exists.
