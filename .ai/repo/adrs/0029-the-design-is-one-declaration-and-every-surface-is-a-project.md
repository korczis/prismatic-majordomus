---
schema: adr/v1
id: adr-0029
kind: adr
title: The design is one declaration and every surface is a projection of it
status: accepted
date: 2026-09-09
tags: [design, ui, projections, derived]
related:
  - rule:project.design-tokens-declared-once
  - rule:project.ui-conformance
  - rule:project.interfaces-are-projections
  - rule:project.derived-files-regenerated
  - file:share/design/tokens.yaml
  - file:scripts/design-tokens
  - file:scripts/ci/design-check
  - file:apps/majordomus-cli/src/web/html.rs
  - file:apps/majordomus-cli/src/http/swagger.rs
  - test:test/cases/104_design_tokens.sh
provenance:
  origin: authored
---

# 29. The design is one declaration and every surface is a projection of it

## Context

Four surfaces of this repository put HTML in front of a person: the site published to
GitHub Pages, the Cockpit, the pages the executable renders itself — its home page and
every generated report — and the Swagger UI shell over the OpenAPI document.

Each carried its own design.

The site was Flowbite's default theme over Tailwind's palette, and named Inter first in a
font stack that never loaded it. The Cockpit was a second Tailwind entry point with an
`@theme` of its own — the same idea, restated — and, underneath it, a second surface
palette in `--mj-*` whose dark half was hand-picked at a different hue from anything else
in the repository. The report pages carried seven hexadecimal literals in a `<style>` block
and switched on `prefers-color-scheme` while every other surface switched on a class. The
Swagger shell inherited whatever a third party shipped that week.

That was the state anybody could see. Writing the gate found three more. `cockpit.js`
carried hexadecimal fallbacks for the tokens it reads from the page. `site/graph.js` read
seven `--mj-graph-*` properties that **no stylesheet has ever declared**, so every lookup
missed and every drawing was painted from the literals in its own fallback table — under a
comment reading "Palette read from the page, not duplicated here". `site/diagrams.js` held
a light and a dark Mermaid theme, sixteen more literals, plus a third restatement of the
type stack.

Seven palettes. Not one of them was a design anybody chose twice; each was the same design
decided again by whoever needed it next, and they had drifted exactly as far as nobody
looking would predict.

Nothing was wrong with the *code*. The defect was that a design value had no canonical
home, so every surface that needed one had to invent it, and no check could tell the
difference between a surface obeying the design and a surface with a design of its own.

## Decision

**A design value is chosen once, in `share/design/tokens.yaml`, and every stylesheet is a
projection of it.**

`scripts/design-tokens` writes three projections from that one file:

- `share/design/theme.css`, the Tailwind `@theme` — imported by the site's entry point and
  by the Cockpit's, after Flowbite's theme, so the two Tailwind builds cannot resolve a
  different type or a different accent.
- `share/design/surface.css`, the semantic surface in the layer and under the selector the
  Cockpit's stylesheet is written against, carrying the `--mj-*` and `--mj-graph-*` names
  as *aliases* of the canonical properties. An alias has no value of its own, so it cannot
  drift; it is a synonym, never a second decision.
- `apps/majordomus-cli/src/web/tokens.css`, the same values as plain custom properties,
  compiled into the executable with `include_str!` for the pages that have no Tailwind and
  fetch no asset. It lives inside the crate so the crate still builds when packaged alone.

`scripts/ci/design-check` is the gate, and it asks two questions. Are the three projections
current? And does any file that can put a colour on a screen name a colour outside the
canonical declaration? A tracked source with a hexadecimal, `rgb()`, `hsl()` or `oklch()`
literal fails, unless it is one of four stated exemptions — the generated projections
themselves, third-party bytes, an SVG icon a browser renders outside any document, and the
colour *arithmetic* in the contrast library, which searches between pure black and pure
white and paints nothing. Each exemption is a reason. A path added without one is the
failure mode this replaces.

The values were chosen so that the published site does not change. Light is Tailwind's
gray, which is what the site's theme already resolved to. The type stack is the one the
site always rendered in, since Inter was named and never loaded. Dark is the Cockpit's,
kept because it was chosen for a dense control plane read for hours. Convergence went
towards what was already there rather than towards a new opinion.

## Consequences

The Cockpit's compiled stylesheet recompiled **byte for byte identical** when its own
`@theme` was deleted and replaced by the shared import. That is the evidence the first move
was a refactor and not a redesign.

A value changed in `tokens.yaml` and not regenerated cannot land: the gate fails. A surface
that grows a palette of its own cannot land either. The four surfaces are now one design in
the only sense that survives a year of edits — there is one place to change it, and a
machine notices when there is a second.

The generator validates what it emits before writing it. This is not defensive
programming; it is a bug this generator had. An unbalanced quote produced
`--font-sans: ...Emoji;` — a declaration the CSS parser drops in silence, so the build
succeeded, the drift check passed, every gate stayed green, and one surface lost its type
stack. `test/cases/104_design_tokens.sh` holds that mutation and the other one, where a
comment stripper ate `#fff` and wrote a custom property with no value at all.

What this does **not** do: it does not restyle the inside of Swagger UI. The page's frame —
its type, its accent, its links — is now this repository's; the operation blocks and schema
tables are a third party's stylesheet at a pinned version, and reskinning somebody else's
component tree is a maintenance bill this page does not need. The page pins itself light,
because Swagger UI 5 ships no dark theme and a dark frame around a permanently light widget
is worse than a light one.

Two things are deliberately left for later and named rather than hidden. The Swagger UI
assets are still fetched from the unpkg CDN, which is the one part of the HTTP projection
that is not available offline — vendoring them is a separate decision about distribution
size, not about design. And `scripts/ui` still discovers only `static-directory` surfaces,
so the Cockpit, the home page and the Swagger shell are outside the browser audit even now
that they share a design; the audit's page set comes from a filesystem, and a route rendered
at request time has no file. Extending it is what would make ADR 0022's guarantee reach
every surface rather than most of them.
