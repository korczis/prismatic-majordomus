# Evening Master Prompt

Build Prismatic Majordomus v0.1 as a self-contained `.majordomo/` protocol directory that can be copied into any repository and activated by a short pointer from `AGENTS.md`, `CLAUDE.md`, or equivalent.

## Definition of done

public-quality repository + self-contained `.majordomo/` + 2–4 line integration + deterministic doctor + behavioral tests + minimal example.

## Hard non-goals

No server, database, dashboard, telemetry backend, model invocation, dynamic routing, autonomous runtime, daemon, scheduler, or MCP server.

## Required UX

`cp -R .majordomo /path/to/project/` then add:

```markdown
## Majordomus
Follow `.majordomo/README.md` for all non-trivial AI-assisted work.
```

## Work protocol

All tasks <= approximately five minutes. After every task: SUMMARY, EVIDENCE, TEST, PLAN REASSESSMENT, CLEANUP. Continuously reduce scope.

Before claiming done: run tests, run doctor against a fresh copy, validate shell syntax/YAML where tooling exists, scan for secrets/private identifiers, prove `.majordomo/` has no private external dependency, and reconcile README claims with reality.
