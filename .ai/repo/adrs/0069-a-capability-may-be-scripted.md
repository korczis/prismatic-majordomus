---
schema: adr/v1
id: adr-0069
kind: adr
title: A capability may be scripted, and repository automation is scripted capabilities rather than shell
status: proposed
date: 2026-09-15
tags:
  - automation
  - capabilities
  - architecture
  - portability
  - security
related:
  - rule:project.no-network-no-eval
  - rule:project.no-new-nouns
  - rule:project.optional-complexity
  - rule:project.interfaces-are-projections
  - rule:project.executions-carry-no-secret
  - file:docs/CONCEPTS.md
  - file:SECURITY.md
---

# 69. A capability may be scripted, and repository automation is scripted capabilities rather than shell

## Context

Repository automation lives in shell: 122 files under `scripts/` (80 of them shell, 43 of 44
in `scripts/ci/`), 45 under `lib/` (38 shell), four under `bin/`, and 216 shell test cases.
Every recurring defect class this repository has recorded about itself has a shell
instance: a BSD/GNU flag that behaves differently on the runner, a pipeline whose count is
lost in a subshell, a gate that cannot read its own pattern and reports clean, a sort whose
order depends on the machine's locale, a file a derivation could not see because it was not
yet in the git index. The Rust executable, meanwhile, already owns one registry from which
the command line, the HTTP API, OpenAPI, MCP and the Cockpit are derived.

The repository owner decided on 2026-09-15 that shell is to be replaced gradually by Rhai,
the scripting language embedded in Rust, and that Perl is never an option. This record says
what that becomes in this repository's own vocabulary, because the obvious reading — "an
automation subsystem with an automation registry" — is a second registry beside the one the
vocabulary already has, and `project.no-new-nouns` refuses it.

## Decision

**A capability may be scripted.** Nothing else is added.

`docs/CONCEPTS.md` already says the capability registry "indexes the tool's own surface … and
is derived from the layer and the code on every start", and that a capability which reports
as it goes, called once, is an *execution*. A scripted capability is a capability declared in
a file of the layer instead of in Rust code, whose handler is a Rhai script instead of a
compiled function. It is discovered by the index like any other object of the layer, becomes
a capability in the one registry, and from there reaches every surface by the projections
that already exist — the command line, the HTTP route, the OpenAPI operation, the MCP tool,
the Cockpit's generated runner form, the generated reference. Adding one registers it
nowhere.

**The definition is a kind.** A new kind in `share/kinds.yaml`, with a versioned schema under
`share/schemas/majordomus/`, declares a scripted capability: its id, title and description,
its input and output as JSON Schema, its effect (`read`, `process state`, `repository
mutation`), whether it can be cancelled, and the script beside it. The repository's own
scripted capabilities live under `.ai/repo/`; the ones the distribution ships live under
`share/` and travel in the release archive unchanged. Identity, validation of unknown keys,
the `majordomus://` URI and the MCP resource come from the index, not from a loader of its own.

**Infrastructure stays typed Rust.** A script reaches the repository only through capabilities,
by one host function that calls `Context::execute` — so schema validation, redaction, caching,
counters and the execution plane apply to every call a script makes, exactly as they apply to
a call from HTTP or MCP. Where a script needs something no capability offers (git status of a
tree, a glob over tracked files, running a program with arguments), the answer is a typed
capability in Rust, not a larger script. No function takes a shell command string. A program
is run by name with an argument list, a working directory, an environment delta and a bound.

**What a script may do is declared and enforced.** A scripted capability declares its effect,
and the host refuses at run time a call to any capability whose effect exceeds it. The
exposure ceiling that today exists only in the command graph moves into
`CapabilityRegistry::build`, where it holds every capability — compiled or scripted — to the
same rule: an effect above the ceiling is not projected onto HTTP or MCP. This also closes a
gap found while writing this record: `plan.transition` and `recover.orphans` write the
repository and are served over both today, while their own comments say a ceiling excludes
them.

**The interpreter is a sandbox, and it is not `eval`.** `project.no-network-no-eval` forbids
eval because a string assembled at run time and executed with the tool's full authority is a
program nobody reviewed. A scripted capability is the opposite on every count: the script is
a tracked file, reviewed like any other; it is compiled once from that file, never from a
string a caller supplied; Rhai's own `eval` function is disabled; the engine has no file,
process or network access of its own and reaches the world only through capabilities whose
effects it declared; and the engine enforces limits on operations, call depth, expression
depth and the sizes of strings, arrays and maps, with cancellation and a deadline checked on
every operation. `SECURITY.md` states this as a declared exception in the rule's own terms, and
the rule's scan is taught to tell a script file from an eval call.

**Configuration.** `rhai` 1.26 with `sync` (the shared server runs handlers on threads),
`serde` (typed values cross the boundary through the schemas that already describe them),
`metadata` (the host functions a script may call are introspectable) and `no_time` (time
reaches a script through its context, never a hidden clock). `unchecked`, `internals` and
`debugging` stay off. Modules resolve only among discovered scripts, never from the
filesystem at large.

## Alternatives rejected

**An automation subsystem with its own registry, runtime and API.** It would be the second
registry `project.no-new-nouns` forbids, and every surface would have to learn it; the
existing registry already projects to all of them.

**Transliterating each shell script into Rhai.** Most scripts are domain logic — parsing
YAML, classifying git state, walking a registry — that belongs in typed Rust. A Rhai file that
reimplements `git status` parsing is the same defect in a different syntax.

**A generic `shell` or `exec` capability.** It would make command-string composition the
easiest path and put an unreviewed program behind every surface. If a compatibility escape
is ever unavoidable it is a separately named capability whose effect is the highest there is,
never projected beyond the command line, and every use of it is reported.

**Perl, Python or Node as the portable layer.** Each is a runtime a user must install; the
Rust executable is already installed wherever Majordomus is.

## Consequences

- The shell tool shrinks by migration, not by rewrite: each script is classified — domain logic
  to a Rust capability, orchestration to a scripted capability, unavoidable bootstrap kept
  with a declared exemption, dead code deleted — and a gate refuses new shell under governed
  paths without an entry in a machine-readable exemption list.
- Scripted capabilities are held to what compiled ones are: a schema-valid definition, a test,
  benchmark cases, generated documentation, and a coverage gate on the Rust that runs them.
- Diagnostics gain a source position once, in the shared diagnostic type, so a script error
  names its file, line and column on every surface.
- A user runs repository automation with the installed executable alone: no Bash, no Rust
  toolchain, no separate Rhai installation.
- The work lands as the slices of the `automation` milestone, each merged on its own.
