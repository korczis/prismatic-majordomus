+++
title = "Every public command is named and run by an active use case, a guaranteed claim or an MCP tool without one is a named gap, and the policy says which gaps fail doctor, check and finish"
description = "A capability that nobody has written a use case for is a named gap, not a silence. For"
weight = 124
[extra]
claim_id = "use-case-coverage"
status = "guaranteed"
source = "docs/claims/use-case-coverage.md"
+++
{% raw %}

## What it means

A capability that nobody has written a use case for is a named gap, not a silence. For
every public command of the command registry, every guaranteed claim that belongs to a
responsibility, and every MCP tool the executable projects, the tool counts the active
use cases that name it, the ones whose scenario runs it, and the ones with passing
evidence. The policy's `use_cases.coverage` says, per class, whether a gap is `required`
(a failure of `doctor`, `check`, `finish` and `usecase coverage --check`), `advisory`
(reported once, with the list) or `off`. A draft use case never counts.

## How it works

`lib/usecase.sh`: `mj_uc_coverage_rows` derives the targets from `share/commands.yaml`
(visibility `public`), `docs/CLAIMS.yaml` (status `guaranteed`, responsibility not
`none`) and `docs/generated/registry.json` (every `tool`), and tallies them against the
use cases loaded once into variables; `mj_validate_use_case_coverage` is the validator of
the doctrine `majordomus.use-case-coverage`, dispatched from `doctor`, `check` and, through
the finish key `use_cases_covered`, `finish`; `majordomus usecase coverage` prints the
same rows, `--json` for machines, `--check` to exit 10 on a required gap.

## How to see it

```bash
majordomus usecase coverage                 # every target: use cases, executable, evidence, status, policy
majordomus usecase coverage --check         # exit 10 on a required gap
majordomus usecase scaffold --missing --dry-run
```

## What it does not cover

Coverage is by name and by execution, not by meaning: a use case that runs a command
trivially covers it. The narrative is reviewed by a person; the tally says the command has
one to review.

## Why it exists

Documentation that nobody is forced to write is documentation that is not written. With
the denominator computed from the registries, adding a command creates its gap the moment
the registry names it, and `finish` refuses to call the work complete until a use case
runs it.
{% endraw %}
