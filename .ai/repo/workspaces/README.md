---
schema: context/v1
id: ai.repo.workspaces
kind: context
title: External workspaces
description: One declaration per external workspace this repository is authorised to read; the content it names is this checkout's state and is never here.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/workspace/workspace.v1.schema.json, share/kinds.yaml]
---

# External workspaces

A workspace is a body of content held by another vendor, over which the operator of this
repository is already authenticated, and which this repository reads. A ChatGPT Project is
one. A Claude Project would be another.

**A workspace is not a provider.** A provider is a tool that works *in* this repository —
the bootstrap it reads, the client configuration that starts the shared MCP server for it,
the hooks through which it hands Majordomus a prompt (ADR 0024, `share/providers.yaml`). A
workspace is the arrow pointing the other way. Both are named after the same vendors, which
is precisely why the distinction is written down: Claude Code is a provider, and a Claude
Project would be a workspace.

## What is authoritative and what is not here at all

**Authoritative:** the `*.yaml` files in this directory. Each one declares which workspace
exists, who holds it, what the operator asserts they authorised, the origins an adapter may
observe, the browser profile that reaches it and the capabilities the adapter is expected
to negotiate.

**Not here at all:** the content. A workspace's conversations, messages and attachments are
synced into `.ai/local/workspaces/<workspace>/`, which is this checkout's state and never a
source (ADR 0005, ADR 0025). Nothing under it is indexed, generated from, published, or
carried by a branch. It becomes the repository's own statement only when a person promotes
a piece of it into `.ai/repo/`, deliberately, one piece at a time.

**Not here either:** any credential. `access.browser_profile` is a label the operator's own
configuration resolves, outside this repository. No cookie, token, authorisation header, or
path to one belongs in this directory, in the store, in a fixture, in a log or in a
snapshot.

## The contract

`share/schemas/majordomus/workspace/workspace.v1.schema.json` (`workspace/v1`) is the
contract, and `share/allow/workspace.txt` — generated from it — is what the shell tool
checks keys against. An unknown key is refused, never carried.

## What a declaration is for

It is read before anything is fetched, and it bounds what may be:

- `vendor.origins` is the scope of observation. Traffic to anything not named there is
  never recorded — not filtered afterwards, not recorded.
- `authorisation.statement` is a person's assertion in their own words, and the adapter may
  not exceed it. `authorisation.write` is false everywhere until a separate decision says
  otherwise, and a read adapter never consults it.
- `capabilities` is an expectation, not a permission. One the vendor withdrew is a
  diagnostic; an adapter that cannot provide one reports it unavailable rather than
  returning an empty success.

## Regenerate

    majordomus generate
    majordomus generate --check    # what CI runs
