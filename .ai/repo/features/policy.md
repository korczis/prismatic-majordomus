---
schema: feature/v1
id: policy
kind: feature
title: One policy, projected into every tool's instruction file
short_title: Policy
headline: Every provider the tool declares — an agent, an orchestrator that runs agents, any tool that reads AGENTS.md — gets the same rules from the same place, because their instruction files are generated from one policy and never written by hand; the providers are the table in docs/generated/providers.md.
summary: A provider-neutral policy and four execution profiles are the one source; majordomus update renders each provider bootstrap from its template, stamps it with the policy hash and the hash of its own content, and doctor fails a bootstrap that was hand-edited, that carries a rule corpus of its own, or that exceeds the always-loaded budget.
status: stable
weight: 90
featured: false
areas: [governance, context]
commands: [init, update, doctor]
kinds: [policy, profile]
rules: [majordomus.policy-integrity, majordomus.policy-completeness, majordomus.projection-integrity, majordomus.bootstrap-integrity, majordomus.profile-requirements, majordomus.context-budget, project.unknown-keys-are-errors]
docs: [docs/SCHEMAS.md, docs/ADOPTION.md]
adrs: [adr-0005]
claims: [policy-parse, profile-validate, projection-generation, projection-fingerprint, region-projection, bootstrap-chain, shared-policy, context-budget, profile-axes, capability-class, effort-escalation, provider-projections-one-renderer, init-refuses, no-silent-overwrite]
use_cases: [adopt-an-existing-repository, keep-the-bootstrap-thin-and-within-budget, trust-the-policy-before-reading-it]
related: [doctrine, declare-once]
tags: [policy, providers, projections]
---

## What it does

`.ai/repo/policy.yaml` names the projections: a provider and a target file for each. The
templates ship with the tool and a repository may override one under its own layer; the
Rust executable and the shell tool render the same bytes, stamp included, so both agree on
every stamp. A bootstrap says how to find the policy and never what the policy is: it points
at the layer and the rules, stays inside the line budget the policy sets, and carries no
rule of its own, so a rule that exists for one provider and not another cannot happen.

Profiles set capability class, effort, verbosity, presentation, context toggles and
verification as independent fields, and a task carries the profile it started with.
Unknown keys anywhere are errors, so a typo fails loudly.

## What it does not do

It never names a vendor model: capability classes are what the projection asks a worker to
map onto the closest its environment offers. It does not edit a provider's own settings,
and a hand edit of a generated file is detected and refused rather than merged.
