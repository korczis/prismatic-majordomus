---
schema: context/v1
id: ai.repo.identity
kind: context
title: Identity
description: How a person proves who they are to this repository's web surfaces, declared as data; no credential is ever here.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
---

# Identity

Who may sign in to the surfaces this repository serves, and through what, is a fact of the
repository like any other: declared once, in a file, and projected. It is not a list in the
executable. A provider that is a file is added by adding the file and struck by deleting it;
a provider that is a match arm is struck by a release.

| section | holds |
|---|---|
| `providers/` | one `identity-provider` object per provider a person may sign in with, in the order the sign-in offer shows them |

## What is never here

No client id, no client secret, no token, no cookie and no path to one. An object names the
environment variables its credentials arrive in; the values live in the environment of the
process that serves the repository and nowhere in its history. The schema refuses a value
where a name belongs.

Reading these objects, the sign-in flow and the session it opens belong to the executable
(I1202 onwards); this directory only declares.
