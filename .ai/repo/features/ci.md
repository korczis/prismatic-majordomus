---
schema: feature/v1
id: ci
kind: feature
title: CI planned from one model, the site deployed from its verified run
short_title: CI
headline: A change runs the gates its paths can affect, every gate is the same script a person runs, and the public site is published only from a commit whose derived data proved current.
summary: The gate model names every validation gate and which classes of paths select it; the workflow is a thin adapter over that plan; the publication path proves the committed derived data current by its input hash and renders it; and the tool supervises its own repository with its own hooks.
status: stable
weight: 190
featured: false
areas: [verification]
commands: [doctor, watch]
rules: [project.blocking-checks-cheap, project.tests-run-in-disposable-repos, project.rust-cli-evidence, project.rust-command-tested-in-file]
docs: [docs/CI.md, docs/GITHUB_PAGES_PERFORMANCE.md]
adrs: [adr-0006]
claims: [ci-planned-gates, ci-verdict, suite-parallel, site-deploys-from-verified-run, site-deploy-one-path, rust-evidence-gates]
use_cases: [gate-ci-on-the-tool-itself]
related: [doctrine, benchmarks]
tags: [ci, deployment]
---

## What it does

`scripts/ci-plan` reads the gate model and nothing else to decide what a change must run; a
changed path no class matches escalates the whole plan and names the path. The pre-commit
hook runs `doctor`, the worktree guard and the derived-data fingerprint gate, so a stale
generated file is refused before the commit exists rather than after the merge.

## What it does not do

Publication carries only what can make the published bytes wrong; merging is decided
elsewhere. Nothing here trusts a green run it did not observe.
