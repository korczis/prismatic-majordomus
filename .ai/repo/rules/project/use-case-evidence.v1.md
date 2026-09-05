---
id: project.use-case-evidence
version: 1
kind: rule
title: A use case is proved by execution, and a capability is covered by a use case
description: Every externally meaningful capability of this repository is demonstrated by an active use case whose scenario executes against the real tool; every example a page shows comes from that execution; a use case's maturity is observed, never authored; and a change to a command, rule, claim, provider, MCP tool, schema, application or workflow runs use-case impact analysis before the work is called complete.
statement: A guaranteed capability has an executable use case, a documented example originates from an executed scenario, a maturity is computed and never written, and a change to a covered entity is followed by usecase impact and the runs it names.
status: active
class: blocking
depends_on: [majordomus.use-case-coverage@1, majordomus.catalogue-integrity@1]
tags: [use-cases, evidence, documentation]
---

# Rationale

A catalogue of prose drifts on the first edit that forgets it; a demonstration nobody
ran is fiction with a prompt sign. Anything derivable is derived, anything not derivable
has one canonical home, and anything claimed as guaranteed is backed by executable
evidence: the use case is where those three meet for the reader who asks "what does a
person do with this, and does it work".

# Required behaviour

Every public command is named and run by an active use case under
`.ai/repo/use-cases/`; a guaranteed claim or an MCP tool without one is a reported gap.
A page shows a command's output only from the evidence of the scenario the generator
executed. `status` and `target` are authored; `maturity` is computed by the generator
and appears nowhere in a file. After a change to `lib/`, `bin/majordomus`,
`share/commands.yaml`, a rule, `docs/CLAIMS.yaml`, the crate or a use case, `majordomus
usecase impact` is run and the scenarios and cases it names are run before `finish`.

# Failure behaviour

`majordomus.use-case-coverage` fails `doctor`, `check` and `finish` on a required gap
(the policy's `use_cases.coverage`); `scripts/generate-site-data` refuses a scenario
that fails and `--check` refuses evidence that does not regenerate; `scripts/site-check`
refuses a page whose evidence did not pass; a reviewer refuses a use case that carries a
copied output or a written maturity.

# Verification

`test/cases/94_use_cases.sh` (the chain, every reference mutated), `test/cases/28_catalogue.sh`,
`test/cases/10_site_data.sh` and `11_site_derivation.sh` (the generated data and its
freshness), `scripts/site-check`, and `majordomus doctor`.
