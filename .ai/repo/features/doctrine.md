---
schema: feature/v1
id: doctrine
kind: feature
title: Rules a machine decides, wired, tested and CI-blocking
short_title: Doctrine
headline: A rule is enforced when a validator decides it, a command runs it, a test proves it and CI blocks on it; anything less is documentation, and the tool knows the difference.
summary: Every rule is a portable Markdown object with front matter; a rule the tool enforces names its validator, the commands that dispatch it and the tests that prove it, and doctor walks that chain from the source, refusing a validator nobody declares and a declaration nothing runs; a blocking violation stops the command and an advisory one is reported.
status: stable
weight: 70
featured: true
areas: [governance, verification]
modules: [health]
commands: [rules, doctrine, doctor, check, watch]
kinds: [rule]
rules: [project.rule-is-a-doctrine, majordomus.doctrine-wiring-integrity, majordomus.enforcement-wiring, majordomus.rule-package-integrity, project.no-claim-without-test]
docs: [docs/DOCTRINE.md, docs/DOGFOODING.md]
claims: [doctrine-registry, doctrine-class-decides, dispatcher-wiring, wiring-reconciliation, vendored-rule-package, rule-resolution, drift-watch]
use_cases: [prove-a-rule-is-enforced, read-the-rules-the-tool-applies, gate-ci-on-the-tool-itself, find-out-what-drifted]
cockpit: [health]
related: [finish-contract, policy]
tags: [rules, doctrine, enforcement]
---

## What it does

The effective rule set is the vendored baseline the tool ships plus the rules this
repository wrote, resolved as a dependency graph: a missing dependency, a cycle or two rules
claiming one identity is an error, and a set that does not resolve is not applied at all. A
rule's class is `blocking` or `advisory` and there is no third; the class is read at
dispatch time and is what routes a finding, so changing one word in a rule file changes
whether `majordomus check` exits zero — and a test asserts exactly that.

`majordomus doctor` reads the source rather than a rule's description of itself: the
validator function must exist, the commands the rule names must dispatch it, a blocking
rule must be able to exit non-zero, a test must prove it, and CI must run that test without
swallowing its exit code. A validator no rule declares is enforcement running under no
rule, and fails.

## What it does not do

A rule without a validator is normative for whoever reads it and enforced by nobody, and it
says so; the tool does not pretend otherwise. There is no severity ladder, no baseline and
no override: a project rule may add a constraint and may never weaken a vendored one.
