+++
title = "Every rule the tool enforces is declared once, as a rule object in the repository's effective set, and doctor proves each one is reached by the command that claims to run it"
description = "The rule objects under the repository's rules section — the vendored baseline (.ai/repo/rules/vendor/majordomus/, a copy of the package the tool ships in share/standard/majordomus/) plus the repository's own rules — are the only place a rule is declared. Every command that enforces rules dispatches from it rather than naming checks by hand, and majordomus doctor walks the chain from the declaration to the CI job for each one, reading the source rather than the registry's description of itself."
weight = 23
[extra]
claim_id = "doctrine-registry"
status = "guaranteed"
source = "docs/claims/doctrine-registry.md"
+++
{% raw %}

## What it means

The rule objects under the repository's rules section — the vendored baseline (`.ai/repo/rules/vendor/majordomus/`, a copy of the package the tool ships in `share/standard/majordomus/`) plus the repository's own rules — are the only place a rule is declared. Every command that enforces rules dispatches from it rather than naming checks by hand, and `majordomus doctor` walks the chain from the declaration to the CI job for each one, reading the source rather than the registry's description of itself.

## How it works

`lib/doctrine.sh` loads the registry and, for the running command, executes every doctrine whose `enforced_by` names it. The validator is found by name — a doctrine declaring `validator: scope` runs `mj_validate_scope` — so a doctrine that names a validator nobody wrote is a reported failure and not a silent skip. `mj_validate_doctrine_wiring` in `lib/doctor.sh` then checks six things per doctrine and three about the pipeline:

- the validator function is defined somewhere in `lib/`;
- every command in `enforced_by` exists and calls `mj_doctrine_dispatch`;
- a blocking doctrine's commands can turn a failing finding into a non-zero exit;
- the test it names exists;
- every claim it names is in `docs/CLAIMS.yaml`;
- in the other direction, every `mj_validate_*` function in `lib/` is declared by some doctrine;
- CI runs `test/run.sh`, does not swallow its exit code, and the runner globs `test/cases/` rather than listing cases by name.

## How to see it

```bash
majordomus doctrine status
majordomus doctrine show majordomus.scope-integrity
majordomus doctor        # OK doctrine  <n> doctrines — validator, dispatch, propagation, test and CI resolve for every one
```

## What it does not cover

The dispatch check is textual: it proves the command's source calls the dispatcher, not that a particular code path reaches it under every condition. Coverage of the rule's own logic is the job of the test the doctrine names, not of the wiring check.

## Why it exists

The failure this prevents is invisible from any single file. A rule is written down, a script implements it, a test proves the script, and nothing invokes the script. Every artifact is present and the enforcement is fiction. `test/cases/18_doctrine_wiring.sh` breaks each link in a throwaway copy and fails unless `doctor` goes red — a verifier that survives broken wiring proves nothing.
{% endraw %}
