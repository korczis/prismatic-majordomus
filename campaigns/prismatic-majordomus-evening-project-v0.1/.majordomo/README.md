# Majordomus

Majordomus is the supervisory operating layer for AI-assisted work in this repository.

## Read in this order

1. `policy.yaml`
2. `state/current.md`
3. the smallest sufficient profile from `profiles/`
4. `workflows/task-lifecycle.md`
5. `workflows/verification.md` before declaring completion
6. `workflows/handover.md` before transferring work

## Core rules

- Sessions are workers, not memory.
- Durable state lives outside chat history.
- Load minimum sufficient context.
- One worker, one explicit scope.
- Escalate model capability and reasoning only when justified.
- Separate reasoning depth from output verbosity.
- Verify outcomes, not activity.
- Define done before execution.
- Hand over durable state, not transcripts.
- Stop when the execution contract is satisfied.

## State

`state/current.md` holds current scope/status. `state/decisions.md` holds durable decisions. `state/open-questions.md` holds unresolved issues material to execution.

## Profiles

- `routine`: small bounded work
- `implementation`: normal engineering work
- `debugging`: root-cause work with evidence and regression verification
- `deep-work`: architecture, investigation, unusually difficult work
