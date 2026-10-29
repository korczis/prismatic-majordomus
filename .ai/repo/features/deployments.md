---
schema: feature/v1
id: deployments
kind: feature
title: A deployment is one canonical object, and every artifact built from it is generated from it
short_title: Deployments
headline: The port, the routes and the build inputs of the executable's container image are stated once, and the image definition and its ignore file are projections that cannot drift from it.
summary: One deployment object per deployment of the executable, validated against the registry this process built; the container image definition, its ignore file and the files of the provider it names are generated from it and checked for drift.
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

It does not deploy. A deployment object may name the provider that hosts it (ADR 0219;
this repository's names Gigalixir), and `majordomus generate deployment` renders that
provider's files under `deploy/<provider>/` — for Gigalixir the Procfile and the inline
buildpack's hooks — but nothing here pushes to the provider, holds its credential or
contacts it; assembling and publishing the slug is a workflow's, and observing what it
serves is `served.observe`'s. A provider the contract does not know is refused (Fly.io was
named once, never ran, and stays no target since 2026-10-12).
