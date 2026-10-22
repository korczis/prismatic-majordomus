---
id: project.provider-support-is-declared
version: 1
kind: rule
title: What a provider's adapter can do is declared, never observed from the filesystem
description: The lifecycle events a provider's adapter drives, and whether it can archive prompts, are declared in share/providers.yaml and read there by every surface; no reader answers the question by looking for an installed hook, and no document carries its own list.
statement: A surface reports a provider's lifecycle and prompt-capture support from the declaration in share/providers.yaml; answering from the presence of a hook file, or from a list written into prose or code, is a bug.
status: active
class: blocking
depends_on: [project.providers-are-data@1, project.interfaces-are-projections@1]
tags: [providers, sessions, continuity, capabilities]

x-majordomus:
  tests: [test/cases/170_provider_lifecycle_declared.sh, test/cases/171_session_observability.sh]
---

# Rationale

"Does Claude Code fire `PreCompact`?" has one correct answer and two tempting wrong ones.

Reading the provider's hook directory answers a different question — *did somebody run
`majordomus capture install` in this checkout?* — and presents the answer as though it were a
statement about the provider. A clean clone would be told the provider has no lifecycle at
all; a checkout where somebody hand-wrote a shim would be told it has one the tool cannot
drive. Both are confidently wrong, which is the failure mode this repository is most often
bitten by.

Writing the list into a page, a table or a Rust constant is the second source of truth that
goes stale the day an adapter changes. `project.providers-are-data` already settled this for
what a provider *is*; this rule extends it to what the tool's adapter for it *can do*, which
is the half a surface outside the shell has no other way to learn.

# Required behaviour

`share/providers.yaml` declares, per provider, `lifecycle:` — the events the adapter drives,
in the provider's own vocabulary — and `prompt_capture:`. A provider with no `lifecycle` is
one the tool ships no lifecycle adapter for; that is a fact, not an omission, and it is
reported as itself.

Every surface reads it there. `lifecycle.providers` in `apps/majordomus-cli` is the
capability, and the Cockpit's Continuity page, `GET /api/v1/lifecycle/providers` and the MCP
tool are its projections. No page, document or generator carries its own list, and no reader
stats a hook path to decide the answer.

The adapter table the shell drives — `MJ_CAPTURE_LIFECYCLE` and `MJ_CAPTURE_ADAPTERS` in
`lib/capture.sh` — keeps the columns only the shell needs: the shim paths, the configuration
file, the payload keys, the environment variable naming the provider session. It is not a
second answer to this question; it is the wiring behind the declared one, and the two must
agree.

# Failure behaviour

`test/cases/170_provider_lifecycle_declared.sh` reads both halves and fails when they
disagree: a provider whose shell adapter drives an event the YAML does not declare, a
provider that declares a lifecycle no shell adapter drives, or prompt capture declared where
it is not adapted. It runs in the `shell-suite` gate with every other case, and it has been
shown to fail against a deliberately removed event rather than passing regardless.

# Verification

`bash test/run.sh 170_provider_lifecycle_declared`, and
`curl -s $URL/api/v1/lifecycle/providers | jq '[.providers[] | {id, lifecycle, prompt_capture}]'`
against a checkout with no provider hooks installed, which must still report the declared
events. `test/cases/171_session_observability.sh` asserts exactly that, in a fixture that
carries no provider hook directory at all.

The duplication is deliberate and temporary. ADR 0052 settles the direction — one canonical
session domain service in the executable, with the shell an adapter over it — and when
`lib/capture.sh` reads `share/providers.yaml` instead of carrying its own table, this rule's
gate becomes a tautology and can be retired. Until then it is what makes the duplication
safe rather than silent.
