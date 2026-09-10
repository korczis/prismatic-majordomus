# The design system — one declaration, every surface

How the site published to GitHub Pages, the Cockpit, the pages the executable renders
itself and the Swagger shell come to look like one tool: where the visual identity is
decided, how it reaches each surface, how it is extended, how drift is caught, and what to
do when a page looks wrong. Behaviour as implemented and tested; where implementation and
this document disagree, the document is wrong and changes in the same commit as the fix.
The rule is `project.design-tokens-declared-once@2`; the decisions are
[ADR 0031](../.ai/repo/adrs/0031-the-design-is-one-declaration-and-every-surface-is-a-project.md)
and
[ADR 0036](../.ai/repo/adrs/0036-the-design-system-is-a-typed-declaration-and-every-first-party-surface-consumes-its-projections.md).

## Why

Four surfaces put HTML in front of a person, and each had grown a design of its own. The
site was Flowbite's default theme over Tailwind's palette; the Cockpit a second Tailwind
build with its own palette, a dark theme at a different hue (which, it turned out, never
applied), its own favicon, its own theme key and six bracketed pixel sizes; the report pages
seven hexadecimal literals; the Swagger shell whatever a third party shipped. Status was
three palettes, and which word meant which status was written in a stylesheet's selector
lists *and* in four template macros. Nothing could say what `--mj-text-faint` was for.

None of it was a wrong value. It was a design with no canonical home, so every surface
decided it again, and no check could tell obedience from invention.

## The source of truth

`share/design/tokens.yaml`. One file, in the layer's own YAML subset, holding:

| section | what it decides |
|---|---|
| `identity` | the name and the marks under `share/design/brand/` |
| `font` | the system stack and the monospace stack |
| `palette` | the raw colours — Tailwind's, quoted; internal, never read by a consumer |
| `roles` | the semantic surface: `bg`, `raised`, `sunken`, `line`, `fg`, `body`, `muted`, `faint`, `accent`, `accent-strong`, `accent-soft`, `accent-line`, `accent-fill`, `on-accent`, each a palette entry per theme |
| `status.roles` | five meanings — `ok`, `warn`, `bad`, `info`, `neutral` — each with text, ground and border per theme |
| `status.states` | the vocabulary: every state word any surface renders, filed under one meaning |
| `type` | the named scale (`meta`, `label`, `small`, `dense`) and letter-spacings |
| `layout`, `radius`, `motion` | the shared measures, corner radii and durations |
| `theme` | the class that means dark and the `localStorage` key of the reader's choice |
| `viewports` | the widths every browser audit measures at |
| `alias.flowbite` | the site's vocabulary, each Flowbite name a synonym of a role or a status colour |

The crate reads it as a typed model (`apps/majordomus-cli/src/design/`): declaration order
kept, every reference resolved, every inconsistency refused before anything is written, and
a SHA-256 fingerprint over the canonical form. The first twelve digits of that fingerprint
are what every stylesheet carries as `--mj-design`.

## The pipeline

```text
share/design/tokens.yaml
        │  majordomus generate design   (crate::design::render)
        ▼
share/design/theme.css       Tailwind @theme + Flowbite's names as var(--mj-*)   ┐
share/design/surface.css     every role and status colour, light and dark        ├─► site/tailwind.css ──► site/static/app.css ──► GitHub Pages
share/design/status.css      one selector group per state word                   ├─► share/cockpit/src/cockpit.css ──► share/cockpit/cockpit.css ──► Cockpit
share/design/base.css        (hand-written) focus ring, reduced motion            │
share/design/primitives.css  (hand-written) badge, table, card, code, form …      ┘
apps/majordomus-cli/src/web/tokens.css          the same tokens, no Tailwind ──► reports, home page, Swagger shell
apps/majordomus-cli/src/design/tokens.yaml      the declaration, compiled in ──► design.* capabilities, Cockpit shell and Design page
apps/majordomus-cli/src/cockpit/logo-mark.svg   the mark, compiled in         ──► Cockpit top bar
share/cockpit/favicon.svg, site/static/favicon.svg, site/static/images/*.svg    every copy of the brand
site/data/registry/design.json                  vocabulary, theme contract, widths ──► templates, site-probe, ui audit
docs/generated/design.{json,yaml,md}            the inventory
```

Two hand-written files sit beside the generated ones and are imported by both Tailwind entry
points: `base.css` (the accessible defaults) and `primitives.css` (the `.mj-*` components
both surfaces render). They name tokens and choose nothing.

Both Tailwind builds import the generated sheets in the same order, after Flowbite's theme.
The site keeps Flowbite's vocabulary in its templates — `text-heading`, `border-default`,
`bg-brand` — and the generated theme declares each of those names as `var(--mj-<role>)`,
unlayered and after Flowbite's own `.dark` block, so the name is a synonym and the value is
the declaration's in both themes. The Cockpit reads `--mj-*` directly. A report page and
the Swagger shell compile the same tokens in and fetch nothing.

Status works through one indirection. The generated status sheet emits, for each meaning,
one selector group over the meaning and every word filed under it —
`.mj-badge--succeeded, .mj-alert--succeeded, .mj-status--succeeded, …` — setting
`--mj-status-fg`, `--mj-status-bg` and `--mj-status-line`; the shared badge, alert and
status primitives read those three with a neutral fallback. So a surface writes
`mj-badge--<word>` and nothing else, and the colour is whatever the word is filed under.

Nothing is run by hand. `scripts/derive` (and `just derive`) runs `majordomus generate`,
which includes the design target; `scripts/cockpit-assets` and `scripts/site-build` compile
the two Tailwind bundles from the generated sheets; the pre-commit hook and the CI plan
refuse a tree whose projections are stale.

## Extending it

Every extension is one edit to `share/design/tokens.yaml`, then `majordomus generate design`
(or `just derive`), then the two asset builds. Nothing else is edited.

- **A semantic colour.** Add a role under `roles` with `about`, `light` and `dark` naming
  palette entries (add the entries under `palette` if the colour is new — Tailwind's value,
  quoted). It is `--mj-<name>` on every surface, a swatch class for the inspector, a row on
  the Cockpit's Design page and in the inventory, and answered by `explain`.
- **A status semantic.** Add a meaning under `status.roles` with `fg`, `bg` and `line` per
  theme, and file its words under `status.states`. Every `mj-badge--<word>` is styled.
- **A state word.** Add it to the list under its meaning. That is the whole change:
  `test/cases/109_design_system.sh` does exactly this to prove nothing else moves.
- **A type step.** Add it under `type.scale` with `size` and `leading`; `text-<name>` exists
  in both Tailwind builds and `--text-<name>` on every page.
- **A spacing or layout value.** Add it under `layout`; it is `--mj-<name>`.
- **A component primitive.** Write it once in `share/design/primitives.css` over tokens and
  scale steps; both surfaces have it. The Cockpit's own stylesheet keeps only what the
  Cockpit alone has.
- **A mark.** Replace the file under `share/design/brand/`; every copy is regenerated.
- **A Flowbite name the site uses that should follow a role.** Add it under
  `alias.flowbite`. A name absent there keeps Flowbite's value, which is a statement that
  it is not a shared decision.

The process never includes editing the site's templates and the Cockpit's stylesheet for the
same reason. If it does, the architecture has a hole; file it.

## Validation

| what | where | when |
|---|---|---|
| the declaration is consistent | `crate::design::DesignSystem::validate`, run by `generate` | before anything is written |
| every projection is current | `majordomus generate --check` (`rust-integration` / `generation-converges`) | every plan that touches the crate, the share or the generated tree |
| no source carries a design decision of its own | `scripts/ci/design-check` (`design` gate, structure job) | every plan that touches a stylesheet, template, script or the crate's web code |
| the compiled Cockpit stylesheet matches its source, and the palette matches the installed Tailwind | `scripts/cockpit-assets --check` (`cockpit-assets` gate) | the site job |
| every badge word on the site is filed, and every page carries the fingerprint | `scripts/site-check` (`site-build` gate) | the site job |
| the stylesheet, the page and the dataset agree, at every declared width | `scripts/site-probe` (`site-probe` gate) | the site job |
| the stylesheet, the page and the executable agree; every badge word is filed; no overflow at every declared width | `scripts/cockpit-probe` (`cockpit-probe` gate) | the cockpit job |
| every page is keyboard-operable and passes contrast at every width | `scripts/ui audit` (`ui-audit` gate) | the site job |
| the model, the renderer and the zero-registration property | the crate's tests; `test/cases/107_design_tokens.sh`; `test/cases/109_design_system.sh` | the rust and suite jobs |

The gate's questions are the failure modes that were found: a colour literal, a raw palette
utility (`text-green-700`), a bracketed size (`text-[11px]`), a second declaration of a
canonical property, a read of a `--mj-` token nothing emits, a copy of the brand that
differs from the canonical file or lives elsewhere, and a theme key or class named by hand.
Each exemption in the gate is a reason, stated where the gate is written.

## Introspection

| surface | how |
|---|---|
| MCP | `majordomus_design`, `majordomus_design_tokens`, `majordomus_design_explain`; resource `majordomus://design` |
| HTTP | `GET /api/v1/design`, `/api/v1/design/tokens?kind=role`, `/api/v1/design/explain?token=fg` |
| OpenAPI, Swagger | the same three operations, from the registry |
| Cockpit | `/cockpit/design`: every role with both themes' swatches, every status with its words as badges, the type scale at its own sizes, the projections — and the badge that says whether the stylesheet the page loaded agrees with the executable |
| reference | `docs/generated/design.md`, and the JSON and YAML beside it |

`explain` takes a role, a status, a state word, a type step, a palette entry, or the custom
property any of them becomes:

```text
majordomus_design_explain {"token": "--mj-ok"}
→ kind: status, about: healthy, done, passing …,
  parts: fg --mj-ok emerald-900 / emerald-300, bg --mj-ok-bg …, line --mj-ok-line …,
  states: ok pass passed succeeded completed done …,
  aliases: --color-fg-success-strong --color-success-soft --color-success-subtle,
  projections: share/design/theme.css share/design/surface.css …
```

## Troubleshooting

- **A page looks wrong on one surface.** Read `--mj-design` from the page
  (`getComputedStyle(document.documentElement).getPropertyValue('--mj-design')`) and
  compare with `GET /api/v1/design` (`design`) or `site/data/registry/design.json`. The
  Cockpit does this itself and shows an alert on mismatch. A different fingerprint means a
  stylesheet compiled before the declaration moved: run `majordomus generate design`, then
  `scripts/cockpit-assets` or `scripts/site-build`, and restart the server.
- **A colour is missing.** An undeclared custom property is the empty string, never an
  error. `scripts/ci/design-check` names every `--mj-` read nothing emits; run it.
- **A badge is grey that should not be.** Its word is filed under no status. `explain` it;
  if it answers nothing, file the word under `status.states`. `scripts/site-check` and
  `scripts/cockpit-probe` refuse an unfiled word.
- **The Cockpit is unstyled.** `share/cockpit/cockpit.css` is missing or stale:
  `scripts/cockpit-assets`. The shell says so in a banner.
- **Dark mode does not switch.** The class the page toggles is `theme.class` from the
  declaration, written onto `<html>` (site) or `<body>` (Cockpit) as `data-theme-class`;
  the scripts read it there. A page without the attribute has an invalid compiled
  declaration: `GET /api/v1/design` says why.
- **A visual regression probe fails.** The finding names the route, the width and the
  assertion. An overflow is a layout defect on that page; a `design` finding is a stale
  bundle or a page not carrying `data-design`; a `font` finding means a stack other than the
  declared one — usually a third-party stylesheet loaded after ours.
- **`generate` refuses the declaration.** The message names the key path and the reason
  (`roles.sunken.light: 'gray-51' is not a palette entry`). Nothing was written.

## What is deliberately not shared

The two surfaces share a language, not a layout. The site is Flowbite components in
templates at reading density; the Cockpit is semantic classes over the same tokens at
operational density. The Swagger UI's own component tree keeps its third-party stylesheet
inside a frame that is ours. Flowbite names the site uses that are not shared decisions
(`neutral-tertiary`, `brand-strong`, the non-strong `fg-*` variants) keep Flowbite's values
until somebody files them under a role.
