---
id: project.design-tokens-declared-once
version: 1
kind: rule
title: A design value is chosen once and every stylesheet is a projection of it
description: Type, palette and accent are declared in one canonical file and generated into the Tailwind theme both builds import, the surface block the Cockpit is written against, and the custom properties compiled into the executable; a file that puts a colour on a screen may name a token and may not invent one, and a token it names is one something declares.
statement: A design value is chosen in one place and every surface derives it; a surface that carries a colour of its own is a bug, and an exemption is a reason rather than a path.
status: active
class: blocking
depends_on: [project.derived-files-regenerated@1, project.interfaces-are-projections@1, project.ui-conformance@1]
tags: [design, ui, derived, projections]
---

# Rationale

This repository serves four surfaces to a person — the published site, the Cockpit, the
pages the executable renders itself, and the Swagger shell — and before this rule each one
carried its own design. Not a different design anybody had chosen: the same design, decided
again by whoever needed it next, in whatever syntax was to hand.

Counting them is the argument. The site named a font it never loaded. The Cockpit declared
a second Tailwind theme and, under it, a second surface palette whose dark half sat at a
different hue from everything else. A report page held seven hexadecimal literals and
switched on a media query while every other surface switched on a class. And three more
appeared only once a gate looked: hexadecimal fallbacks in the Cockpit's script, sixteen
literals in a Mermaid theme, and — the one worth remembering — a graph view reading seven
custom properties **no stylesheet has ever declared**, so every lookup missed, every drawing
used the fallback table in its own source, and the comment above it read "Palette read from
the page, not duplicated here."

That is the shape of the failure. Not a wrong value anywhere; a value with no canonical
home, so inventing one locally was the only thing a surface could do, and no check could
tell obedience from invention.

# Required behaviour

**One file chooses.** `share/design/tokens.yaml` is where type, the accent ramp and the
semantic surface are decided. Nothing else decides; everything else derives.

**Three projections, generated.** `scripts/design-tokens` writes the Tailwind `@theme` both
Tailwind entry points import, the surface block in the layer and under the selector the
Cockpit's stylesheet is written against, and the custom properties compiled into the
executable for pages that have no Tailwind and fetch no asset. Each carries a provenance
header. None is edited by hand.

**A second name is an alias, never a second value.** A surface with its own vocabulary —
`--mj-bg`, `--mj-graph-text` — declares it as `var(--<canonical>)` in the generated block.
An alias has no value of its own and therefore cannot drift.

**A surface names tokens; it does not invent them.** Any tracked file that can put a colour
on a screen — a stylesheet, a template, a script that supplies CSS values, Rust that writes
a `<style>` block — may reference a token and may not write a colour literal. A script that
reads a token from the page falls back to a colour the page already computed, never to a
literal: a fallback that only appears when the stylesheet failed is a copy nobody would
notice going stale.

**An exemption is a reason.** Four exist and each is stated where the gate is written: the
generated projections, which are made of literals because that is their job; third-party
bytes; an SVG icon, which a browser renders outside any document and which therefore cannot
reach a custom property; and colour arithmetic — a contrast library searching between pure
black and pure white paints nothing and cannot go stale. A path added to that list without a
reason is the defect this rule replaces, wearing a different hat.

**A token that is read is a token something declares.** An undeclared custom property is
not an error in CSS; it is the empty string. A surface that reads one gets nothing, uses
whatever fallback stands beside it, and looks deliberate for ever. The gate asks the
`--mj-` namespace — this repository's own — and refuses a read nothing answers. It found
`--mj-text-faint` on its first run: the Cockpit's stylesheet had asked for it since the
day it was written, nothing had ever declared it, and every element wearing it had
rendered as muted.

**The generator validates what it writes.** A stale projection fails loudly; a *malformed*
one is dropped by the CSS parser in silence, with every check downstream still green. The
generator refuses to emit a declaration with an unbalanced quote or no value, because both
have happened here.

# Failure behaviour

`scripts/ci/design-check` exits 10 when a projection is stale or a surface carries a colour
of its own, and runs as the `design` gate. It needs no browser and no built site.

This rule governs where a value is *declared*, never what the value should be. Whether the
accent should be blue is intent; a person states it, in the one file that holds it.

# Verification

`test/cases/107_design_tokens.sh` proves it by mutation: a hand-edited projection is
refused; a value changed canonically reaches every projection; an alias pointing at
nothing is refused; a stylesheet with a literal fails and the same stylesheet naming a
token passes; `color-mix()` over tokens passes and a literal inside one does not; a read of a token
nothing declares fails and the same read passes once it is declared; and neither
`href="#features"` nor `Merge pull request #117` is mistaken for a colour, because a gate
that cries wolf is a gate somebody turns off. Two of its assertions are regressions for real bugs in the generator —
a quote stripper that broke a font stack, and a comment stripper that ate `#fff` — both of
which produced CSS that failed in silence. ADR 0031 records the decision and what it left
undone.
