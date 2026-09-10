---
id: project.design-tokens-declared-once
version: 2
kind: rule
title: The design is one typed declaration and every first-party surface consumes its projections
description: Roles, status semantics, type scale, layout, radius, motion, theme contract and brand are declared once in share/design/tokens.yaml, read by the crate as a typed model, and projected by `majordomus generate design` into the stylesheets both Tailwind builds import, the tokens and declaration compiled into the executable, every copy of the brand, the site's dataset and the reference; a first-party surface names a token and never chooses a value, a state word is coloured by the meaning it is filed under, and a token added to the declaration reaches every surface with no consumer edited.
statement: A visual decision is made in one place and every surface derives it; a surface that carries a colour, a size, a status colour, a theme key or a mark of its own is a bug, an exemption is a reason rather than a path, and a new token is available to every supported surface the moment it is declared.
status: active
class: blocking
depends_on: [project.derived-files-regenerated@1, project.interfaces-are-projections@1, project.generated-artifacts-are-typed@1, project.ui-conformance@1]
tags: [design, ui, derived, projections, cockpit, site]
---

# Rationale

This repository serves four surfaces to a person — the site published to GitHub Pages, the
Cockpit, the pages the executable renders itself, and the Swagger shell. Version 1 of this
rule made the palette and the type stacks one declaration and found seven palettes on the
way. The audit for version 2 found the rest of the design where it had always been.

The site's colours were Flowbite's default theme, a third-party file, and the declaration
restated them for the Cockpit — two owners of one decision. Status was three palettes:
the declaration's emerald/amber/rose, the Cockpit's green-700/amber-700/red-700, the site's
emerald-900/orange-900/rose-900 — and which word meant which status was written twice, as
selector lists in one stylesheet and as `if`/`elif` chains in four template macros. The
Cockpit's dark palette sat under `:where(.dark)`, lost to `:root` on specificity, and had
never applied. The Cockpit had a favicon and a letter-mark of its own beside the site's
prism. Two theme keys. Six bracketed pixel sizes. And nothing anywhere could say what a
token was for.

None of that was a wrong value. It was a design with no canonical home for most of its
decisions, so every surface made them again, and no check could tell obedience from
invention. ADR 0031 records the first repair; ADR 0036 records this one.

# Required behaviour

**One file decides.** `share/design/tokens.yaml` holds the identity and the marks, the two
type stacks, the raw palette, the semantic roles, the status meanings and the vocabulary
filed under them, the named type scale and letter-spacings, the shared layout values, the
radii, the durations, the theme contract, the audit widths, and the site's vocabulary as
aliases. Nothing else decides any of these; everything else derives.

**The crate reads it as a typed model.** `crate::design::DesignSystem` parses the
declaration through the layer's own YAML subset, keeps declaration order, validates it and
fingerprints it. It refuses a role naming no palette entry, a status referencing neither a
palette entry nor a role, a state word filed under two meanings or under the name of another
status, an alias of nothing, a type step without a unit or a positive line height, a
non-ascending width list, and any value the CSS parser would drop in silence — an empty
value, an unbalanced quote. A refused declaration projects nothing.

**`majordomus generate design` writes every projection.** The Tailwind `@theme` both
builds import, carrying the type stacks, the scale, the letter-spacings, the radii and the
site's vocabulary as `var(--mj-<role>)` after Flowbite's own theme; the surface sheet with
every role and status colour as `--mj-*` in the base layer, light on `:root` and dark under
the theme class *as a class selector after it*; the status sheet with one selector group
per state word setting the three indirection properties the primitives read; the same
tokens as plain custom properties compiled into the executable; the declaration itself
compiled into the executable; every copy of the brand any surface serves; the site's
dataset with the vocabulary, the theme contract, the pre-paint statement and the widths;
and `docs/generated/design.{json,yaml,md}`. Each carries a provenance banner and is
compared by `generate --check` like every other generated artifact.

**A surface names tokens; it does not invent them.** A first-party stylesheet, template,
script that supplies CSS values, or Rust that writes a `<style>` block may reference a
token and may not write a colour literal, a raw Tailwind palette utility
(`text-green-700`), a bracketed size (`text-[11px]`), a second declaration of a canonical
custom property, the theme key or the theme class. A script that reads a token from the
page falls back to a colour the page already computed, never to a literal.

**Status is one vocabulary.** A state word is coloured by the meaning it is filed under and
by nothing else. Every surface renders a state as `mj-badge--<word>`, `mj-alert--<word>` or
`mj-status--<word>` and lets the generated sheet colour it; a word the declaration does not
file renders neutral, and the site check and the Cockpit probe refuse it.

**Shared primitives, shared base.** The components more than one surface renders live in
`share/design/primitives.css` over the tokens; the accessible base — a visible focus ring
on everything the keyboard reaches, reduced motion honoured — lives in
`share/design/base.css`; both Tailwind entry points import both. A surface keeps only what
it alone has.

**One theme contract, one brand.** The class that means dark and the storage key come from
the declaration; every surface ships the generated pre-paint statement and its scripts read
the key and the class from the page. The marks live under `share/design/brand/`; every copy
is generated and compared byte for byte; no other mark exists.

**A new token needs no registration.** A role, a status colour, a state word, a type step,
a layout value added to the declaration and regenerated is in every stylesheet, the site's
dataset, the inventory, `explain` and the Cockpit's Design page with no consumer edited.
`test/cases/109_design_system.sh` and the crate's render tests prove it by doing it.

**The page can tell.** Every generated sheet carries `--mj-design`, the declaration's
fingerprint; the executable stamps the same fingerprint on every page it renders and answers
it from `design.system`. The Cockpit compares the two on load; the site probe and the
Cockpit probe assert stylesheet, page and executable agree at every declared width. A
stylesheet compiled before the declaration moved, or an executable built before it was
regenerated, is visible on the page rather than in a diff nobody ran.

**An exemption is a reason.** The generated projections are made of literals, because that
is their job; third-party bytes and compiled artifacts are not written here; the canonical
marks under `share/design/brand/` cannot reach a custom property; one script computes a
categorical hue per node kind at run time; three measure or compute colour and paint
nothing. Each is stated where the gate is written. A path added without a reason is the
defect this rule replaces, wearing a different hat.

# Failure behaviour

`scripts/ci/design-check` exits 10 when any first-party source carries a design decision of
its own, when a copy of the brand differs from the canonical file, or — in this repository,
with a built executable at hand — when a projection is stale; it runs as the `design` gate
and needs no browser and no node. `majordomus generate --check` refuses a stale projection
in the `rust` job and `scripts/cockpit-assets --check` refuses a compiled stylesheet that
differs from its source or a palette entry that differs from the installed Tailwind, in the
`site` job. `scripts/site-check` refuses a badge word the declaration does not file and a
page that does not carry the declaration's fingerprint. `scripts/site-probe` and
`scripts/cockpit-probe` refuse a page whose stylesheet carries a different fingerprint from
the dataset or the executable, or renders in a stack other than the declared one.

This rule governs where a value is *declared*, never what the value should be. Whether the
accent should be blue is intent; a person states it, in the one file that holds it.

# Verification

`test/cases/107_design_tokens.sh` proves the gate by mutation: a colour literal, a raw
palette utility, a bracketed size, a second declaration, a read of a token nothing emits, a
hand-edited copy of the mark, a theme key named by hand — each refused; the same file
naming a token, and every documented exemption, passing. `test/cases/109_design_system.sh`
proves the projection through the built executable: a state word and a role added to a
fixture's declaration reach every stylesheet, the site's dataset and the inventory in one
`generate design`; an inconsistent declaration projects nothing; `generate --check` refuses
a stale projection; and the capabilities answer the same fingerprint the sheets carry. The
crate's own tests hold the validator's refusals, the renderer's shape — including that the
dark block is a class after the root block — and the zero-registration property. ADR 0036
records the decision and what it left undone.
