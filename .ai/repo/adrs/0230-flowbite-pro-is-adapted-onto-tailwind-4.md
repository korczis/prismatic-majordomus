---
schema: adr/v1
id: adr-0230
kind: adr
title: Flowbite Pro is adapted onto Tailwind 4 and Flowbite 4, a brand's colour is the primary scale, and one capability judges every repository's interface against it
status: proposed
date: 2026-10-28
tags:
  - design
  - ui
  - enforcement
related:
  - file:share/design/kits/flowbite-pro.yaml
  - file:share/design/kits/flowbite-pro.rule.md
  - file:apps/majordomus-cli/src/design/kit.rs
  - file:.ai/repo/project/issues/I4334.yaml
  - test:test/cases/4334_the_ui_kit_is_checked_by_one_capability.sh
provenance:
  origin: authored
---

# 230. Flowbite Pro is adapted onto Tailwind 4 and Flowbite 4, a brand's colour is the primary scale, and one capability judges every repository's interface against it

## Context

On 2026-10-28 the owner ordered that every one of their projects with a user interface is
built on their licensed Flowbite Pro: documented, enforced, back-propagated to every
repository and verified. An audit of about thirty repositories that day found every way of
missing it. Some used free Flowbite with a hand-rolled shell. Some loaded Tailwind from the
Play CDN. Some mixed in daisyUI, Preline, TW Elements or shadcn/Radix. Some wired the
drawer through a hand-written script instead of Flowbite's JavaScript. This repository's
own site re-implements Flowbite's navigation in `site/nav.js`.

Two facts make the order more than a checklist:

1. **The licensed templates target an older stack.** The Admin Dashboard 1.1.1 that the
   owner's adaptation (poppyhead) starts from targets `tailwindcss ^3` and `flowbite ^1.5`.
   It configures the theme in `tailwind.config.js`: the plugin under `plugins`, Flowbite
   under `content`, `primary` under `theme.extend.colors`, and `darkMode: "class"`. The
   repositories that are current build with Tailwind 4, which is configured in CSS, and
   with Flowbite 4.
2. **Several products are brand-driven.** catharsis, astronomists, unable.cz, u-tygra and
   vomaste.cz each have a colour of their own, and some are dark only. Pro's palette is one
   blue `primary` over grays.

A rule nobody can run drifts the way the audit found. The owner's skill ships an interim
scanner (`verify.mjs`), but it checks only dependencies and whether the rule file exists.

## Decision

1. **Pro's structure, adapted onto Tailwind 4 and Flowbite 4.** A repository takes Pro's
   markup and class lists and builds them with Tailwind 4 and Flowbite 4. It does not pin
   Pro's Tailwind 3 stack. The v3 configuration maps one to one:

   | Pro (Tailwind 3) | Tailwind 4 / Flowbite 4 |
   |---|---|
   | `plugins: [require('flowbite/plugin')]` | `@plugin "flowbite/plugin";` in the stylesheet that imports Tailwind |
   | `content: ['./node_modules/flowbite/**/*.js']` | `@source "../node_modules/flowbite";` |
   | `theme.extend.colors.primary` | `@theme { --color-primary-50: …; … --color-primary-950: …; }` |
   | `darkMode: 'class'` | `@custom-variant dark (&:where(.dark, .dark *));` |
   | `fontFamily.sans: ['Inter', …]` | `@theme { --font-sans: 'Inter', …; }` |
   | `transitionProperty.width` | `transition-[width]` |
   | `safelist: [...]` | `@source inline("...");` |

   Pro's markup uses utilities that Tailwind 4 renamed, so the markup is renamed when it
   is adapted, never shimmed:

   | Tailwind 3 | Tailwind 4 |
   |---|---|
   | `shadow-sm` | `shadow-xs` |
   | `shadow` | `shadow-sm` |
   | `rounded-sm` | `rounded-xs` |
   | `rounded` | `rounded-sm` |
   | `ring` | `ring-3` |
   | `outline-none` | `outline-hidden` |
   | `flex-shrink-0` | `shrink-0` |
   | `bg-opacity-50` | the colour's alpha, as in `bg-gray-900/50` |

   The `data-*` attributes that Flowbite's JavaScript drives (drawer, dropdown, modal, tabs,
   collapse, tooltip) are unchanged, and so is the shell: a fixed navbar, the sidebar
   drawer, the breadcrumb page header, `gray-50`/`gray-900` pages, `white`/`gray-800`
   surfaces and `gray-200`/`gray-700` borders.
2. **A brand's colour is Tailwind's `primary` scale.** A brand-driven product declares its
   colour as `--color-primary-*` and keeps Pro's structure and grays. It never adds a
   second accent, and its surfaces stay on the Pro shell. A design system that names
   surfaces by role, such as this repository's `--mj-*` tokens, declares role aliases
   (`bg-page`, `bg-surface`, `border-default`, or its own under `design_kit.roles`). The
   check accepts an alias in place of the raw grays, so a token rule and the kit do not
   compete.
3. **One capability judges it, from markers.** `design.kit` (CLI `majordomus design kit`,
   MCP `majordomus_design_kit`, HTTP `GET /api/v1/design/kit`) judges a repository's tree
   against a kit profile in `share/design/kits/`. It has six signals:
   - S0: whether there is a user interface at all;
   - S1: the dependency;
   - S2: the plugin and its source;
   - S3: Flowbite's JavaScript behind every interactive attribute;
   - S4: no other kit;
   - S5: the shell on every root layout.

   S0 cuts the tree into surfaces: the root's manifest (or the one in `assets/`), every
   workspace the root declares, and every nested app manifest with markup in its own
   subtree. Published pages count as an interface whatever builds them. A manifest that is
   no surface is printed as a notice, never judged. The profile's markers are of two
   kinds. Wiring markers connect the kit to a build: the package, the plugin and source,
   the script that loads it. Usage markers show it in use: the attributes its JavaScript
   drives, its React components, its API. A surface passes S1 to S3 only with usage in its
   own files, because a dependency nobody uses proves nothing.

   Each finding names its signal, path, evidence and remedy. `--check` exits 10 on a
   finding. A repository without a user interface passes vacuously. A policy that cannot be
   read exits 2. An exemption is a `{path, because}` entry in the `design_kit` policy block,
   and every answer lists it. A landing or marketing layout is exempted with
   `shell: marketing`, and is then judged by a navbar and a footer.
4. **A repository adopts it with one command.** `majordomus design kit adopt flowbite-pro`
   writes three things and nothing that is already there:
   - the rule `project.ui-uses-flowbite-pro`, from the distribution's template;
   - the `design_kit` block, once the distribution's policy schema declares it;
   - the enforcement entries `design-kit-on-push` (pre-push) and `design-kit` (the CI
     workflow).

   `majordomus doctor` names each entry until the hook and the workflow invoke it. Doctor
   also refuses a policy key the schema does not declare, so until the policy keys of the
   design licence (I4336) carry the block, adoption writes none and says so: flowbite-pro is
   the profile judged when the policy names none. Those keys and this block's shape
   (`layouts`, `roles`, an exemption's `signals` and `shell`) are agreed before the block
   moves under them.
5. **Licence: markers only.** The profile names attributes, class lists, package names and
   file shapes. No Pro template, block, stylesheet or script is copied into this
   repository, the distribution or a published page.

   A licensed kit's profile also declares its licensed markers: what in a tree is the
   licensed distribution itself, such as a directory or a manifest named after a Pro
   product. Where a tree carries one, the licence evidence decides it. The judge receives
   that evidence as one of four states: `active`, `inactive`, `not_judged` or `refused`.
   Only `active` passes. A variant the judge cannot decide is a finding no exemption moves,
   never a pass. The design licence (I4336) reads the evidence. Until it lands, the one
   function that answers, `licence_state`, says `not_judged`.
6. **The adopted rule keeps a section of the repository's own.** The template ends with
   `# In this repository`. Adoption writes it once, and a later adoption brings the rest of
   the rule up to the template without overwriting that section.

## Consequences

- Every supervised repository gets the same gate in one command, and an unsupervised one
  can be judged with `--repo` without Majordomus being installed into it. The decision to
  supervise it stays the owner's.
- The judgement is pure over a tree built in memory, so each signal is a unit test, and
  case 4334 proves each signal by mutation in a disposable repository.
- This repository does not adopt the rule in the same change. Its site's navigation is
  hand-written (`site/nav.js`), and the site would fail S3 and S5 today. Adoption here is a
  later slice that moves the site, the Cockpit and `/ui` onto the Pro shell, mapping
  Tailwind 4 onto the `--mj-*` roles as decision 2 allows.
- The markers are heuristics over text, not a renderer. A shell that is assembled at run
  time from components the profile does not name will read as missing. Such a layout is
  declared under `design_kit.layouts` or exempted with its reason, never passed silently.

## Alternatives rejected

- **Pinning Pro's Tailwind 3 stack.** It would freeze every repository on a major version
  that the current ones have left, and it would fork the configuration of each product
  that already builds with Tailwind 4.
- **Copying the Pro templates into `share/` for the check to compare against.** That is
  the one thing the licence forbids, and it would publish the templates with every release
  of the distribution.
- **The skill's scanner as the gate.** It runs on one machine, it is not a projection of
  the registry, and it cannot be required by CI or by a pre-push hook in the repositories
  it judges.

## Verification

- `cargo test --lib design::kit` covers every signal, the vacuous pass, exemptions, the
  marketing shell, policy parsing and idempotent adoption.
- `test/cases/4334_the_ui_kit_is_checked_by_one_capability.sh` mutates a conformant
  disposable repository once per signal and expects each finding and exit code. It adopts
  twice to prove idempotence, and refuses an exemption without a reason. It also proves
  a published page, a workspace that is wired and uses nothing, a manifest that is only a
  notice, a licensed marker nobody could judge, and the rule's own section surviving a
  second adoption.
