---
schema: context/v1
id: ai.repo.deployments
kind: context
title: Deployments
description: One canonical object per deployment of this repository's executable; every artifact built from it — the container image definition and its ignore file — is generated from it and none of them is authoritative.
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
paths its image is built from, and the budgets it is held to. It names no hosting
provider: none is a target. What ships is the release binaries, GitHub Pages and the
container image built from this object and published to GHCR; Fly.io was named here once,
never ran, and was removed from the contract on 2026-10-12.

## What is authoritative and what is generated

**Authoritative:** the `*.yaml` files in this directory. Nothing else.

**Generated from them:** the container image definition (`deploy/Dockerfile`) and its
ignore file (`.dockerignore`). No provider configuration is generated. Each carries a
provenance header naming this directory and the command that regenerates it, and
`majordomus generate --check` fails when one is edited by hand.

A hand-written `Dockerfile` would restate the port and the build inputs the repository
already knows, and the pair would drift the first time one half changed. This is the same
rule capabilities live under: declared once, projected everywhere (ADR 0004).

## Regenerate

    majordomus generate
    majordomus generate --check    # what CI runs

## The contract

`share/schemas/majordomus/deployment/deployment.v1.schema.json` (`deployment/v1`) is the contract, and
`share/allow/deployment.txt` — generated from it — is what the shell tool checks keys
against. An unknown key is refused, never carried.

No credential belongs here. The object names what is deployed and how much of it runs; a
token that authorises publishing comes from outside the repository and is never written
into it, into a generated file, into an image layer or into a log.

## Validation beyond the schema

The schema decides shape. `majordomus deploy doctor` decides sense: a health route that no
capability registers, a package or binary the workspace does not contain, a build input
that does not resolve, a minimum running count above the machine count. Each refusal names
the file, the key, the value it found and the correction.
