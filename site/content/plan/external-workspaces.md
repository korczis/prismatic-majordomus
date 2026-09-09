+++
title = "external-workspaces — An external workspace is read the way this repository reads anything else, and its content never becomes public by accident"
description = "A workspace is a declared object of the layer — vendor, identity, what the operator authorised, which browser profile reaches it, which capabilities it is expected to have — and rides the existing kind pipeline onto every surface with no code that special-cases it. Its content is synced by the Node tooling layer that already drives the system browser, lands under `.ai/local/workspaces/`, carries provenance and a support level on every record, and is read back through exactly one capability shaped like `continuity`: served, never published. A sync is incremental from a durable checkpoint, idempotent on replay, fails closed when an observed contract breaks, and never writes a credential anywhere."
weight = 16
template = "milestone.html"
[extra]
plan_id = "external-workspaces"
source = ".ai/repo/project/milestones/external-workspaces.yaml"
+++
