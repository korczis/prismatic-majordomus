---
id: project.ui-uses-flowbite-pro
version: 1
kind: rule
title: Every user interface is built on Flowbite Pro
description: Every user interface this repository builds or generates uses the owner's licensed Flowbite Pro shell, blocks and components on Tailwind and Flowbite, and no other UI kit; `majordomus design kit --check` judges the tree against the flowbite-pro profile in CI and before every push.
statement: Build every user interface in this repository on Flowbite Pro, starting from the matching Pro template or block, with Flowbite as the dependency, its Tailwind plugin and source configured, Flowbite's JavaScript behind every interactive attribute, no second kit, and the Pro shell on every root layout; declare every exemption with its reason in the design_kit policy block, and never publish a Pro source file.
status: active
class: blocking
depends_on: []
tags: [ui, flowbite, design-system]

x-majordomus:
  enforced_by: [majordomus design kit --check]
---

# Rationale

The owner holds a Flowbite Pro licence and requires it for every project. One design system
across products keeps them consistent, and the licence already pays for the components a
project would otherwise reinvent. A rule a reviewer has to remember drifts: kits were mixed,
shells were rebuilt by hand and Tailwind was loaded from a CDN in projects that all claimed
to follow it. This rule is therefore judged by a command, from markers in the tree.

# Required behaviour

- **Dependency.** `flowbite` is a dependency of the package manifest that builds the
  interface (`package.json` or `assets/package.json`); a React or Next.js interface uses
  `flowbite-react`.
- **Plugin and source.** Under Tailwind 4, the stylesheet that imports Tailwind carries
  `@plugin "flowbite/plugin"` and an `@source` over Flowbite. Under Tailwind 3,
  `tailwind.config.*` requires `flowbite/plugin` and its `content` globs Flowbite. Tailwind
  compiled in the browser (the Play CDN) and Flowbite fetched only from a CDN are not a build.
- **Flowbite JS.** Any `data-drawer-*`, `data-dropdown-*`, `data-modal-*`, `data-tabs-*`,
  `data-collapse-*` or `data-tooltip-*` attribute is driven by Flowbite's JavaScript, loaded
  by the build. A script that reads those attributes itself re-implements Flowbite.
- **One kit.** daisyUI, Bootstrap, Preline, TW Elements, shadcn/ui and Radix, Petal
  Components, Material UI, Ant Design, Bulma, Chakra UI, Foundation and Materialize are
  not used. Alpine.js is reported, not refused.
- **The Pro shell on every root layout.** A fixed top navbar; an `aside` labelled
  `Sidebar` whose id both `data-drawer-target` and `data-drawer-toggle` name; a
  `nav[aria-label="Breadcrumb"]` page header on application pages; the page, surface and
  border roles (`bg-gray-50`/`dark:bg-gray-900`, `bg-white`/`dark:bg-gray-800`,
  `border-gray-200`/`dark:border-gray-700`, or the role aliases a token rule declares). A
  brand's colour is Tailwind's `primary` scale on the Pro structure (ADR 0229).
- **Exemptions are declared, never silent.** `.ai/repo/policy.yaml` carries
  `design_kit: {profile: flowbite-pro, exempt: [{path, because}]}`. An exemption with
  `shell: marketing` judges a landing or marketing page by the Pro marketing blocks instead:
  a navbar and a footer.
- **Licence.** Pro markup is adapted into the owner's products. Pro source files (templates,
  blocks, bundles) are never committed to a public repository or published verbatim.

# Failure behaviour

`majordomus design kit --check` exits 10 and names each finding with its signal, path,
evidence and remedy; CI and the pre-push hook run it (enforcement entries `design-kit` and
`design-kit-on-push`). A policy that cannot be read, or an exemption without a reason, exits
2. A repository with no user interface passes vacuously.

# Verification

`majordomus design kit --check` in CI and before every push; `majordomus doctor` reports
whether both entries are wired. A review of a UI change compares it with the Pro template or
block it starts from: spacing, radius, typography, cards, tables, badges and buttons.
