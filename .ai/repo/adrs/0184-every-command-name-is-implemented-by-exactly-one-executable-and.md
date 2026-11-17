---
schema: adr/v1
id: adr-0184
kind: adr
title: "Every command name is implemented by exactly one executable, and the other only delegates"
status: proposed
date: 2026-10-18
tags:
  - context-knowledge
  - chatgpt-project
related:
  - file:.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md
  - file:.ai/repo/adrs/0047-the-session-domain-is-typed-and-its-identities-are-not-interchangeable.md
  - file:.ai/repo/adrs/0069-a-capability-may-be-scripted.md
  - rule:project.commands-are-projections
  - rule:project.operation-transport-parity
  - file:.ai/repo/project/issues/I3156.yaml
  - file:.ai/repo/project/milestones/repository-automation-is-typed.yaml
  - file:.ai/repo/project/issues/I3141.yaml
  - file:.ai/repo/project/intents/repository-automation-is-typed.yaml
provenance:
  origin: extracted
  derived_from:
    - issue:I3100
    - file:.ai/repo/adrs/0040-development-semantics-are-capabilities-of-one-runtime.md
    - file:.ai/repo/adrs/0047-the-session-domain-is-typed-and-its-identities-are-not-interchangeable.md
    - file:.ai/repo/adrs/0069-a-capability-may-be-scripted.md
    - file:.ai/repo/project/issues/I3156.yaml
    - file:.ai/repo/project/milestones/repository-automation-is-typed.yaml
    - file:.ai/repo/project/issues/I3141.yaml
    - file:.ai/repo/project/intents/repository-automation-is-typed.yaml
---

# 184. Every command name is implemented by exactly one executable, and the other only delegates

## Context

The shell tool bin/majordomus and the Rust executable share the name; share/commands.yaml describes one and the capability registry the other. A shell fallback once evaluated the model differently (318 vs 26 findings, 2026-10-12).

## Decision

Ownership is per command path (the program plus the full word sequence cli.yaml and share/commands.yaml declare) and is derived from the command graph's existing join of the two origins; a path both implement outside a declared delegation is a finding. Delegation stays read at run time from the generated projection docs/generated/cli.yaml (rule project.commands-are-projections, bin/majordomus:100-103): no delegation line is generated into or hand-written in bin/majordomus. The Rust executable is the target owner of every path, moved path by path with exit codes and machine output preserved.

## Alternatives rejected

- Keeping both implementations with a parity test forever
- An all-at-once rewrite of the shell tool

## Consequences

The session cutover (ADR 0047) and the automation milestone move paths one by one; doctor's agreement check becomes an ownership check over the command graph; a shared top-level word handed over by subcommand word is not a finding.

## Sources

Promoted on 2026-10-18 from the owner's ChatGPT project (workspace `chatgpt-majordomus`, issue I3100). The owner's acceptance is at `chatgpt-majordomus:c192#5`; accepting this record is still the owner's act.

- `chatgpt-majordomus:c233#2`
- `chatgpt-majordomus:c268#2`
