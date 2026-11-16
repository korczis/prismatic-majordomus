+++
title = "Every user interface here is built on Flowbite Pro, through the mj tokens"
description = "Every user interface here is built on Flowbite Pro, through the mj tokens"
weight = 183
[extra]
kind = "rule"
slug = "project-ui-uses-flowbite-pro-1"
identity = "project.ui-uses-flowbite-pro@1"
status = "active"
source = ".ai/repo/rules/project/ui-uses-flowbite-pro.v1.md"
+++
{% raw %}

## Rationale

The owner decided on 2026-10-28 that every project's user interface is built on their licensed
Flowbite Pro, and, for this repository, that it is built "through the mj tokens": Pro is the
starting point for a surface's structure and components, while this repository's design stays
one typed declaration (`share/design/tokens.yaml`, ADR 0031 and ADR 0036) that every surface
consumes. Open-source Flowbite already sits under that declaration, with its theme overridden
by the tokens; Pro adds the shell, blocks and components the licence pays for.

This repository and its site are public, and Majordomus is released under MIT. The Flowbite Pro
licence allows adapting its markup into the owner's products and forbids publishing its source
files. So what reaches this tree is adapted markup on the tokens, never a Pro file.

## Required behaviour

- A new or changed surface starts from the matching Flowbite Pro template or block (shell, page
  header, cards, tables, tabs, drawers, empty states, forms) from the owner's licensed sources
  (`~/dev/poppyhead/assets/vendor/flowbite/`, condensed by the `flowbite-pro` skill), and adapts
  its markup and class lists rather than inventing a component the Pro set already has.
- Every colour, type and spacing value is a `--mj-*` token or a named step of the declaration.
  No raw Tailwind palette utility, no literal colour and no second declaration of a canonical
  custom property: that is what `scripts/ci/design-check` refuses.
- No Flowbite Pro source file — a bundle, a template or a block as shipped — is committed, and
  none is copied into a generated or published page.
- ECharts, Cytoscape and this repository's own tokens are not other UI kits: they are the
  libraries and the declaration a Pro surface is drawn with (ADR 0135).

## Failure behaviour

A surface that does not start from Flowbite Pro, or that carries a design value outside the
tokens, is a review finding and is reworked before it lands. A committed Pro source file is a
licence finding and is removed before the branch is pushed.

## Verification

`scripts/ci/design-check` refuses raw palette utilities, literal colours and second
declarations of the tokens, so a surface on Pro markup cannot bring Pro's or Tailwind's own
values with it. Two checks are in flight and not yet on the trunk. Pro fidelity as a check
rather than a review is `majordomus design kit --check` (I4334): the Pro shell on each root
layout, Flowbite JavaScript where its attributes are used, and no other kit. Refusing a Pro
source file in this public tree is the never-publish gate (I4336). Until they land, the
owner-level checker `node ~/.claude/skills/flowbite-pro/verify.mjs <repo>` reports the same
signals, and review enforces the rest.
{% endraw %}
