+++
title = "Automation is a capability"
description = "This repository's own automation lived in shell, and every recurring defect class it had recorded about itself had a shell instance. Why ADR 0069 makes new automation a typed or scripted capability of the one registry, how a check refuses shell nobody declared, and what is not built yet."
template = "page.html"
[extra]
lang = "en"
related = ["written-once-derived-everywhere/", "the-bet-on-a-layer/", "proof-carrying-documentation/"]
[extra.translations]
cs = "automatizace-je-schopnost/"
+++

[ADR 0069](@/adrs/adr-0069.md) (proposed), dated 15 September 2026, starts from where this repository's automation lived: in shell, from most of `scripts/` and `lib/` to every behavioural test case. Every recurring defect class the repository had recorded about itself had a shell instance: a BSD or GNU flag that behaves differently on the runner, a pipeline whose count is lost in a subshell, a gate that cannot read its own pattern and reports clean.

That day the repository owner decided that shell is to be replaced gradually by Rhai, the scripting language embedded in Rust, and that Perl is never an option. The obvious reading, an automation subsystem with a registry of its own, is refused: the Rust executable already owns one registry from which every interface is derived, and [project.no-new-nouns](@/rules/project-no-new-nouns-1.md) forbids a second. The decision is one line: a capability may be scripted, and nothing else is added.

## What shell cost

The first cost was portability. A user needed Bash to run any of the automation, and Perl reached the tree without anyone choosing it, through `shasum`, which on macOS is a Perl script.

The second was an inventory nobody kept. Nothing refused a new shell file and the existing ones were described only in an audit report, yet a migration converges only if new shell stops arriving while the old is moved.

The third was behaviour nobody could ask about. A capability declares its input and output as JSON Schemas, its effect and whether it can be stopped, and every surface shows that declaration. For a script, what it does, who calls it and what it assumes about the platform were facts the migration audit had to collect.

## A capability may be scripted

In the ADR, a scripted capability is declared in a file of the layer instead of in Rust code, and its handler is a Rhai script instead of a compiled function. Its declaration is a new kind that states its input and output as JSON Schema, its effect (`read`, `process state` or `repository mutation`) and whether it can be cancelled, with the script beside it. It runs inside the Rust executable, already installed wherever Majordomus is, while Perl, Python and Node were rejected as runtimes a user must install.

A script reaches the repository only through capabilities, by one host function that calls `Context::execute`, so schema validation, redaction, caching, counters and the execution plane apply to every call it makes. Where it needs something no capability offers, the answer is a typed capability in Rust, not a larger script, and no function takes a shell command string.

The engine is a sandbox, not the `eval` that [project.no-network-no-eval](@/rules/project-no-network-no-eval-1.md) forbids. A script is a tracked file, compiled from that file and never from a string a caller supplied; Rhai's own `eval` is disabled, the engine has no file, process or network function of its own, its work is bounded, and cancellation is checked on every operation. The repository's knowledge base adds that this is a language boundary, not process isolation.

## One declaration, every surface

[ADR 0004](@/adrs/adr-0004.md) (accepted) made one declaration the whole definition of a capability: every projection is derived from it, and nothing is written twice. A scripted capability is discovered by the index like any object of the layer, becomes a capability of the one registry, and reaches the command line, the HTTP route, the OpenAPI operation, the MCP tool, the Cockpit's runner form and the generated reference. Adding one registers it nowhere, and it owes what a compiled capability owes: a schema-valid definition, a test, benchmark cases, generated documentation and a coverage gate.

The check that guards the migration already works this way. `shell.check` is declared once in Rust and serves one verdict as `majordomus shell check`, as the MCP tool `majordomus_shell_check` and at `GET /api/v1/shell/check`.

## The inventory and its exemptions

The migration is tracked in `.ai/repo/automation/inventory.jsonl`, one JSON object per line, in canonical order. It holds every unit the migration audit found, from shell files and the awk, jq and JavaScript they call to the shell embedded in workflows, recipes and hooks. Each record carries a disposition and its reason: A moves into a typed capability, B becomes a scripted one, C is bootstrap glue, D is dead, E is generated and F is a platform adaptor.

A shell file under a governed directory must also carry an exemption, which says why it has to be shell today and the condition under which it is removed, and which is read in review as the whole argument for the file. Even `bin/majordomus`, the shell tool's entry point, has one: it is deleted once a typed capability owns the behaviour and every caller asks it.

Nothing mechanical is recorded, because the check derives line counts and interpreters from the tree and a recorded copy would go stale unnoticed. The exemptions live in the inventory itself, since a second list would be the repeated definition ADR 0004 names as a defect. The list only shrinks: a migrated unit's record is deleted in the same change as the unit.

## New shell is refused

The bootstrap every worker reads is explicit: new repository automation is a typed or scripted capability, not shell, and `majordomus shell check` refuses a shell unit `.ai/repo/automation/` does not exempt. A shell unit is a file under `bin/`, `lib/`, `scripts/`, `share/`, `.githooks/` or `.claude/hooks/` whose first line names a shell or whose name ends in `.sh`, and the check reads the files git would commit, so a new script is refused before it is added. Its finding names the file and gives the decision as the remedy: a scripted or typed capability, or, where shell is genuinely required, as for an installer run as `curl | sh`, a record with an exemption.

Deleting a migrated file without its record fails as `shell.stale`, the ratchet that keeps the list from outliving its units. The check reads git's index and the first bytes of each file, builds nothing and uses no network, and exit `12`, a tree it could not measure, is not a pass. In CI it is the `shell-inventory` gate, typed Rust because a new shell gate is exactly what it refuses.

The shell that remains is held to [project.portable-shell](@/rules/project-portable-shell-1.md), active and blocking: bash 3.2 and BSD userland are the floor, with no associative arrays, no `mapfile`, no GNU-only flags, and `shellcheck -x -s bash` passing on every script. The tool runs on a Mac as often as on a Linux host, and a construct that works on only one is, in the rule's words, a bug that shows up only where nobody is looking. No command decides the rule; a reviewer does, with shellcheck running in CI.

## What is not built yet

ADR 0069 is proposed, and its work lands as slices of the `automation` milestone. The check and the inventory reached master the day the ADR is dated, a case has refused Perl anywhere the repository runs since 30 September, and the sandboxed engine has landed with a test for each of its guarantees.

Nothing runs a script yet. The engine's module says the milestone's next change wires it to a capability and only its tests build one until then, and every exemption on an orchestration unit says no scripted capability runs it yet. That change, which makes a scripted capability a registered kind runnable through `majordomus run` and projected like any other, is ready in the plan, and the first migration of `scripts/ci` gates waits on it. Until then, the ADR's promise of automation run by the installed executable alone describes where the milestone ends, not where it stands.

ADR 0004 is accepted, ADR 0069 is proposed, and project.portable-shell is active and blocking. How one declaration reaches every surface is in [Written once, derived everywhere](@/written-once-derived-everywhere.md), why Majordomus is a layer and not another agent in [The bet on a layer, not a model](@/the-bet-on-a-layer.md), how a repository refuses its own documentation in [Proof-carrying documentation](@/proof-carrying-documentation.md), and the check and the inventory in [`docs/SHELL.md`](@/docs/shell.md).
