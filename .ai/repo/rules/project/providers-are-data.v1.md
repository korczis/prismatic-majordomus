---
id: project.providers-are-data
version: 1
kind: rule
title: What the tool knows about a provider is declared once and projected everywhere
description: A provider's title, the client configuration it reads and the scratch roots it creates checkouts under are declared in share/providers.yaml beside its template and nowhere else; the table a person reads is docs/generated/providers.md, generated from the same value the product, the site and the worktree topology answer from; no document enumerates providers by hand.
statement: Declare a provider in share/providers.yaml and a template beside it; let `majordomus generate` write docs/generated/providers.md and every other projection; when a document has to name the providers, point at the generated table rather than listing them, and let scripts/ci/providers-check refuse a list written by hand.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.derived-once@1, project.no-counts-in-prose@1]
tags: [documentation, provider, derivation]
---

# Rationale

A provider was known in three places before this rule, and none of them was data. Its title
and the client configuration it reads were a table compiled into the executable; the scratch
roots it creates checkouts under were a list compiled into the worktree topology; and the
set of providers was named by hand in a dozen documents — the README, the design document,
the MCP guide, a feature's headline, a use case's summary — each of which was correct on the
day it was written and wrong the day a provider was added (ADR 0024). The number rule,
`project.no-counts-in-prose`, exists because a count in prose is stale the moment the thing
it counts changes; a list of names has the same property and the same remedy.

One declaration beside the templates, and one generated table that every document points
at, is the shape everything else in this repository already has: the capabilities have
`docs/generated/capabilities.md`, the command line has `docs/generated/cli.md`, the
artifacts have their manifest. A provider list written by hand in a document is a second
declaration of the same fact, and a second declaration is a design defect
(`docs/CAPABILITIES.md`, ADR 0004).

# Required behaviour

1. **One declaration.** `share/providers.yaml` declares, per provider, the title a person
   knows it by, the file it reads a project-scoped MCP client configuration from, and the
   scratch roots it creates checkouts under. The set of providers is the templates under
   `share/providers/`; a declaration without a template is an error and a template without
   a declaration a warning, both reported by `majordomus product validate`.
2. **Every projection reads the declaration.** `majordomus product providers`, the MCP tool
   `majordomus_providers`, `GET /api/v1/product/providers`, the site's provider cards, the
   worktree topology's scratch roots and `docs/generated/providers.{md,json,yaml}` are
   derived from it. No source file carries a provider's title, configuration file or scratch
   root as a literal.
3. **Documents point, they do not list.** A document that has to say which providers exist
   names `docs/generated/providers.md`. A line that names three or more declared providers
   is a list, and the file that carries it must point at the generated table; records of a
   moment — decisions, claims, the changelog — are exempt, because they say what was true
   when they were written.
4. **Adding a provider is data and a template.** An entry in `share/providers.yaml`, a
   template beside it, and — when this repository projects a bootstrap through it — a
   projection in `.ai/repo/policy.yaml` with the target under `merge=derived`. Then
   `scripts/derive`. Nothing else is edited.

# Failure behaviour

`scripts/ci/providers-check` is the gate: it fails on a declaration without a template or a
template without a declaration, on a projected bootstrap the merge driver does not cover, on
a policy projection through a provider that does not exist, and on a document that
enumerates providers by hand without pointing at the generated table; with the executable
at hand it also fails when `docs/generated/providers.*` are stale, and without it
`scripts/derive-check` does. The gate runs in the `structure` job for changes under
`share/`, `docs/`, `.ai/`, the crate and the generated artifacts.

# Verification

`scripts/ci/providers-check` on this tree, and `majordomus generate --check providers`.
`apps/majordomus-cli/tests/product.rs` proves the provider table is the templates the
distribution ships decorated by the policy; the worktree service's tests prove the scratch
roots are the declarations expanded.
