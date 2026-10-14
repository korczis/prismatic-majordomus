---
schema: feature/v1
id: deployments
kind: feature
title: A deployment is one canonical object, and every artifact built from it is generated from it
short_title: Deployments
headline: The port, the routes and the build inputs of the executable's container image are stated once, and the image definition and its ignore file are projections that cannot drift from it.
summary: One deployment object per deployment of the executable, validated against the registry this process built; the container image definition and its ignore file are generated from it and checked for drift.
status: draft
weight: 200
domain: surfaces
featured: false
areas: [governance]
modules: [deploy, delivery]
kinds: [deployment]
claims: [deployment-contract-refuses-by-name]
rules: [majordomus.deployment-contract]
docs: [docs/DELIVERY.md]
adrs: [adr-0071]
tags: [deployment]
---

## What it does

`deploy.list`, `deploy.get` and `deploy.check` read the deployment objects the index holds
and decide whether each would work against the capability registry and the workspace it
sits in.

`delivery.report` and `delivery.feature` decide whether each product feature exists: on
the trunk, contained in what the public site serves, publicly verified, tested, its evidence
published and linked — every dimension computed, and unknown never a pass (ADR 0071).

## What it does not do

No hosting provider is a target. What ships is the release binaries, GitHub Pages and the
container image built from the generated `deploy/Dockerfile` and published to GHCR; nothing
runs from the deployment object on a hosted machine, and no provider configuration is
generated (Fly.io was named once, never ran, and was removed on 2026-10-12).
