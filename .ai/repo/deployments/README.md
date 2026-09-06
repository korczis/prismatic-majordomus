---
schema: context/v1
id: ai.repo.deployments
kind: context
title: Deployments
description: One canonical object per deployment of this repository's executable; every container and provider artifact is generated from it and none of them is authoritative.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/deployment/deployment.v1.schema.json, share/kinds.yaml]
---

# Deployments

A deployment object states, once, everything a deployment of this repository's executable
needs: which application it is, which package and binary are shipped, the port the process
listens on and the interface it listens on, the routes a platform polls for liveness and
readiness, the resources and the machine count it is granted, the region it runs in, the
paths its image is built from, and the budgets it is held to. Provider-specific facts —
what has no meaning outside Fly.io — live in the `provider` block and nowhere else.

## What is authoritative and what is generated

**Authoritative:** the `*.yaml` files in this directory. Nothing else.

**Generated from them:** the container image definition, its ignore file, the provider
configuration (`fly.toml`), the smoke suite's expectations and the deployment pages of the
generated documentation. Every one of those carries a provenance header naming this
directory and the command that regenerates it, and `majordomus generate --check` fails
when one is edited by hand.

A hand-written `Dockerfile` and a hand-written `fly.toml` would restate the port, the
routes, the resources and the build inputs the repository already knows, and every pair
would drift the first time one half changed. This is the same rule capabilities live
under: declared once, projected everywhere (ADR 0004).

## Regenerate

    majordomus generate
    majordomus generate --check    # what CI runs

## The contract

`share/schemas/majordomus/deployment/deployment.v1.schema.json` (`deployment/v1`) is the contract, and
`share/allow/deployment.txt` — generated from it — is what the shell tool checks keys
against. An unknown key is refused, never carried.

No credential belongs here. The object names what is deployed and how much of it runs; the
token that authorises a deployment comes from outside the repository and is never written
into it, into a generated file, into an image layer or into a log.

## Validation beyond the schema

The schema decides shape. `majordomus deploy doctor` decides sense: a health route that no
capability registers, a package or binary the workspace does not contain, a build input
that does not resolve, a minimum running count above the machine count. Each refusal names
the file, the key, the value it found and the correction.
