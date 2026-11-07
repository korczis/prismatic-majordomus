---
schema: context/v1
id: ai.repo.identity.providers
kind: context
title: Identity providers
description: One declarative object per OpenID Connect provider a person may sign in with; striking a provider is deleting its file.
status: active
scope: subtree
providers: ["*"]
audience: [human, agent]
composition: extend
order: 100
tracks: [share/schemas/majordomus/identity-provider/identity-provider.v1.schema.json, share/allow/identity-provider.txt, share/kinds.yaml]
---

# Identity providers

Each `*.yaml` file here is one `identity-provider` object: a provider a person may sign in
with, stated as its issuer, its endpoints and discovery document, the scopes it is asked for,
the claim that carries a stable subject, the environment variables its client id and client
secret arrive in, its display name and its place in the sign-in offer. The file's name is its
`id`.

## What is authoritative

**Authoritative:** the `*.yaml` files in this directory, and nothing else. The set of providers
is expected to change; a provider is struck by deleting its file, and added by writing one.

**Never here:** a credential. `credentials.client_id_env` and `credentials.client_secret_env`
are the *names* of environment variables, `[A-Z][A-Z0-9_]*`, and the schema refuses anything
else in those fields, so a pasted client id or secret fails validation instead of being
committed.

## The contract

`share/schemas/majordomus/identity-provider/identity-provider.v1.schema.json`
(`identity-provider/v1`) is the contract, and `share/allow/identity-provider.txt`, generated
from it by `majordomus generate`, is what the shell tool checks keys against. An unknown key
is refused, never carried.

The schema expresses one flow, the OpenID Connect authorization code flow with PKCE (S256) and
a nonce, and every field of it is one that every provider of the kind fills. There is no
per-provider block and no free-form extension. A provider that needs a field the others do not
is evidence the shape is wrong, and the answer is a new version of the schema, not a hatch in
one object:

- `issuer` is the value an ID Token's `iss` must equal.
- `endpoints` names the authorization, token and key-set endpoints. They are declared even
  where a `discovery` document exists, because a discovery document need not carry every
  endpoint the code flow uses: Facebook's publishes no token endpoint.
- `scopes` always includes `openid`.
- `subject_claim` is the claim an account is bound to, `sub` under OpenID Connect Core; never
  an e-mail address or a name.
- `order` places the provider in the sign-in offer; two providers do not share one.

## Regenerate

    majordomus generate
    majordomus generate --check    # what CI runs
