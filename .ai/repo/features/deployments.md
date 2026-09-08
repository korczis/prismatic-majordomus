---
schema: feature/v1
id: deployments
kind: feature
title: A deployment is one canonical object, and every provider artifact is generated from it
short_title: Deployments
headline: The port, the routes, the resources and the build inputs of a hosted executable are stated once, and the container definition and the provider configuration are projections that cannot drift from it.
summary: One deployment object per hosted instance of the executable, validated against the registry this process built; the container image definition, the provider configuration and the smoke expectations are generated from it and checked for drift.
status: draft
weight: 200
featured: false
areas: [governance]
modules: [deploy]
kinds: [deployment]
rules: [majordomus.deployment-contract]
tags: [deployment]
---

## What it does

`deploy.list`, `deploy.get` and `deploy.check` read the deployment objects the index holds
and decide whether each would work against the capability registry and the workspace it
sits in.

## What it does not do

Nothing is deployed by this repository yet; the objects describe what a deployment would
be, and the milestone that hosts one is on the roadmap.
