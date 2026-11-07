---
schema: adr/v1
id: adr-0036
kind: adr
title: The design system is a typed declaration in the crate and every first-party surface consumes its projections
status: accepted
date: 2026-09-10
tags: [design, ui, cockpit, site, projections, derived]
related:
  - rule:project.design-tokens-declared-once
  - rule:project.ui-conformance
  - rule:project.interfaces-are-projections
  - rule:project.generated-artifacts-are-typed
  - file:share/design/tokens.yaml
  - file:apps/majordomus-cli/src/design/mod.rs
  - file:apps/majordomus-cli/src/design/render.rs
  - file:apps/majordomus-cli/src/capability/builtin/design.rs
  - file:share/design/primitives.css
  - file:scripts/ci/design-check
  - file:docs/DESIGN_SYSTEM.md
  - file:.ai/repo/project/issues/I4316.yaml
  - rule:project.ui-uses-flowbite-pro
  - file:.ai/repo/adrs/0230-flowbite-pro-is-adapted-onto-tailwind-4.md
  - test:test/cases/107_design_tokens.sh
  - test:test/cases/109_design_system.sh
provenance:
  origin: authored
---

# 36. The design system is a typed declaration in the crate and every first-party surface consumes its projections

## Context

ADR 0031 made the palette and the type stacks one declaration, `share/design/tokens.yaml`,
projected by a shell script into a Tailwind theme, a surface block and a compiled-in token
sheet. That closed the largest hole — seven palettes — and left the rest of the design in
the places it had always been, which an audit against the mandate to give the site and the
Cockpit one visual language made plain:

- **The site's colours were not the declaration's.** The site is written in Flowbite's
  vocabulary — `text-heading`, `border-default`, `bg-brand`, `text-fg-success-strong` — and
  those names resolved to Flowbite's default theme, a third-party file. The declaration
  *restated* the values Flowbite resolved to, for the Cockpit, and called the light theme
  "what the site already renders". That is two owners of one decision, one of them in
  `node_modules`. Where the two disagreed they had already diverged: the site's dark theme
  was Flowbite's gray-950 family and the Cockpit's was a hand-picked hue 250; the site's
  body text was gray-600 and the Cockpit's gray-900; the site's warning was orange and the
  Cockpit's amber.
- **Status was three palettes.** The declaration said `ok`/`warn`/`bad` were emerald,
  amber and rose; the Cockpit's stylesheet coloured `succeeded` with Tailwind's
  `text-green-700`, `cancelled` with `text-amber-700` and `failed` with `text-red-700`; the
  site coloured `DONE` with Flowbite's emerald-900, `VERIFY` with orange-900 and `BLOCKED`
  with rose-900. Which words meant which status was written twice — as selector lists in
  one stylesheet and as `if`/`elif` chains in four template macros — and a new word had to
  be added to both or it was coloured by whichever surface somebody remembered.
- **The Cockpit's dark palette had never applied.** Its dark values sat under
  `:where(.dark)`, which has no specificity, and its light values under `:root`, which
  has some; both match `<html class="dark">`, so light won on every element. The theme
  toggle flipped the class, the utilities with `dark:` variants followed, and the page
  stayed white. No gate had a way to notice.
- **Two brands.** The site's mark was a prism drawn in `currentColor`, kept under `assets/`
  and copied by the site build; the Cockpit's favicon was a blue square with an "M" in it,
  its top-bar mark a CSS square with the letter M, and the navbar carried a third,
  hand-inlined copy of the prism's paths.
- **Two theme contracts.** The site stored the reader's choice under Flowbite's
  `color-theme`; the Cockpit under `mj-theme`; each shipped its own pre-paint statement.
- **Sizes were bracketed numbers.** The Cockpit's stylesheet set type in `text-[13.5px]`,
  `text-[11px]`, `text-[10px]`, `text-[12px]`, `text-[12.5px]`, `text-[11.5px]` and spaced
  labels in `tracking-[0.1em]`, `[0.12em]`, `[0.14em]`, `[0.16em]`; the report pages the
  executable renders set `15px/1.55`, `62rem`, `.5rem`. None of it was a decision anyone
  could find, change or explain.
- **Nothing could explain a token.** The declaration was data a shell script read once.
  No capability, route, MCP tool or Cockpit page could say what `--mj-text-faint` was for,
  what it resolved to in the dark theme, or which surfaces read it.

The generator itself was a bash script with an embedded Python parser. It could not be
asked anything at run time, it could not be typed or schema-backed, and it duplicated the
YAML subset the executable already parses.

## Decision

**The design system is a typed model in the crate, `share/design/tokens.yaml` is its one
declaration, `majordomus generate design` writes every projection, and every first-party
surface consumes a projection and decides nothing.**

Concretely:

1. **The model.** `crate::design::DesignSystem` reads the declaration through the layer's
   own YAML subset, keeps declaration order, refuses inconsistency (a role naming no palette
   entry, a state word filed under two meanings, an alias of nothing, a value the CSS parser
   would drop in silence), fingerprints itself, and answers `explain`. The executable
   carries a generated copy of the declaration, so its answers are the declaration its
   stylesheets came from and the crate builds when packaged alone.
2. **Semantic roles over raw palette.** The declaration holds a raw palette — Tailwind's
   values, quoted, proved equal to the installed Tailwind by `scripts/cockpit-assets` — and
   fourteen roles over it (`bg`, `raised`, `sunken`, `line`, `fg`, `body`, `muted`,
   `faint`, `accent`, `accent-strong`, `accent-soft`, `accent-line`, `accent-fill`,
   `on-accent`), each with a light and a dark palette entry. A consumer reads
   `--mj-<role>`; nothing reads the palette.
3. **The site's vocabulary is an alias.** Every Flowbite name the site's templates use is
   declared in the generated theme as `var(--mj-<role>)`, after Flowbite's own theme and
   its `.dark` block, so `text-heading` and `--mj-fg` are one value in both themes. The
   templates keep their vocabulary; the values move under the declaration's ownership.
   The role values are the site's own — Flowbite's defaults, named — so the published site
   does not move; the Cockpit comes to it, dark theme included.
4. **Status is one vocabulary.** Five meanings (`ok`, `warn`, `bad`, `info`, `neutral`),
   each with text, ground and border per theme; and every state word any surface renders,
   filed under one meaning. The generated status sheet emits one selector group per word
   (`.mj-badge--succeeded`, `.mj-alert--stale`, `.mj-status--live`) that sets three
   indirection properties; the shared badge, alert and status primitives read those. A word
   is coloured by the meaning it is filed under and by nothing else. The site's badge
   macros, the Cockpit's `badge()` and the scripts all emit `mj-badge--<word>`; the site
   check and the Cockpit probe refuse a word the declaration does not file.
5. **Shared primitives.** The components both surfaces render — badge, tag, alert, card,
   table, facts, chips, link, mono, kbd, code block, details, form controls, swatch — live
   in `share/design/primitives.css`, imported by both Tailwind entry points beside a shared
   accessible base (focus ring, reduced motion). The Cockpit's stylesheet keeps what only
   the Cockpit has: the shell, the palette, the runner, the log, the canvases.
6. **Type, layout, radius, motion are named.** Four steps (`meta`, `label`, `small`,
   `dense`) become `text-<step>` utilities and `--text-<step>` properties; one
   letter-spacing, three layout values, three radii and one duration likewise. A bracketed
   pixel size in a first-party stylesheet fails the gate.
7. **One theme contract and one brand.** The class that means dark and the storage key are
   two values in the declaration; the pre-paint statement is generated from them and shipped
   identically by the site and the Cockpit; the scripts read both from the page. The marks
   live under `share/design/brand/`; every copy a surface serves — the Cockpit's favicon and
   compiled-in mark, the site's favicon and images — is a generated artifact of the same
   file, and the gate compares them byte for byte.
8. **Introspection is derived.** A `design` capability module (`design.system`,
   `design.tokens`, `design.explain`) projects into MCP, HTTP, OpenAPI and a Cockpit page
   like every other module; the Cockpit's Design page renders the model with swatches for
   both themes; `docs/generated/design.{json,yaml,md}` is the inventory; the site's dataset
   carries the vocabulary, the contract and the audit widths.
9. **Drift is asked on the page.** Every generated sheet carries `--mj-design: "<twelve
   hex>"`, the fingerprint of the declaration; the executable stamps the same fingerprint
   on every page it renders. The Cockpit compares the two on load and says so; the site
   probe and the Cockpit probe assert the stylesheet, the page and the executable agree —
   the cross-surface contract test, in a browser, at the widths the declaration names.

## Consequences

The site and the Cockpit now resolve every colour, type step, status meaning, theme
contract and mark from one file, and a change there reaches both — and the report pages
and the Swagger shell — on `majordomus generate design` followed by the two asset builds.
Adding a role or a state word is one edit to the declaration: the stylesheets, the site's
dataset, the inventory, `explain` and the Cockpit page all carry it with no consumer
touched. `test/cases/109_design_system.sh` and the render tests prove that by doing it.

Behaviour that moved, on purpose:

- The Cockpit's dark theme is now the site's (gray-950 page, gray-900 raised, gray-800 wells
  and rules, gray-400 body, white headings). ADR 0031 kept the hue-250 dark palette as "the
  better of the two"; the mandate names the site as the reference and, since that palette
  never actually applied, nothing a person had seen is lost.
- The Cockpit's body text is the site's reading grey; headings, identifiers and values keep
  the strong text colour, which is the site's own arrangement.
- Status colours are the site's tints (emerald, orange, rose, blue at Flowbite's shades);
  the Cockpit's green-700/amber-700/red-700 badges go.
- Type steps are four named sizes; 12.5px becomes 12px and 11.5px becomes 11px.
- The theme key is `color-theme` everywhere; a reader who had chosen a Cockpit theme under
  `mj-theme` chooses once more.

What was removed: `scripts/design-tokens`, the Cockpit's own favicon and letter-mark, the
`assets/` directory, the navbar's inlined copy of the mark, the `--mj-graph-*` and
`--mj-text-*`/`--mj-bg-*`/`--mj-border` alias layer (the scripts read the roles directly),
every raw palette utility and bracketed size in the Cockpit's stylesheet, the `x-cloak`
style block in the site's head, and both hand-written pre-paint statements.

Alternatives considered and rejected:

- *Copy the site's CSS into the Cockpit.* The site is Flowbite utilities in templates; the
  Cockpit is semantic classes over tokens. Copying would give two stylesheets that looked
  alike today and had no shared owner tomorrow.
- *One large shared stylesheet for both.* The two surfaces need different densities,
  layouts and components; a union would be a third design neither surface used whole.
- *Keep the two implementations and compare screenshots.* Pixel baselines are brittle
  across machines and prove sameness after the fact; a shared declaration makes sameness
  structural, and the fingerprint contract proves it in the browser without an image.
- *Tokens declared independently per surface, kept equal by a test.* A test that equates two
  copies is a test somebody updates twice; the second copy is the defect.
- *Rewrite the site's templates from Flowbite's vocabulary to `.mj-*` classes.* Six hundred
  occurrences of `text-heading` for no visual gain; the alias makes the vocabulary a
  synonym, which is all that ownership requires.
- *Keep the shell generator.* It could not be typed, could not be asked a question at run
  time, and duplicated a parser the crate already has.

Left undone and named: the Swagger UI's own component tree keeps its third-party
stylesheet; the site's Flowbite names that are not shared decisions (`neutral-tertiary`,
`brand-strong`, the `fg-*` non-strong variants) keep Flowbite's values; the browser audit
(`scripts/ui`) still discovers only static-directory surfaces, so the Cockpit's
accessibility is measured by its own probe and not by the audit's contrast pass.

## Amendment, 2026-11-07: a review surface starts from a Flowbite Pro block, on the tokens (I4316)

On 2026-10-28 the owner made Flowbite Pro the kit of every user interface they own and, asked
how that meets this declaration, chose "Flowbite Pro through the mj tokens". Nothing above
changes: the declaration still owns every colour, type step, status meaning, theme contract
and mark, and the site already builds on open-source Flowbite 4 over Tailwind 4 with the
Flowbite vocabulary aliased onto the roles (decision 3). What the owner's choice adds is a
starting point and a boundary, recorded here for the surfaces of milestone
`a-review-is-published-at-its-commit`: the /quality/ page, a /trace/ page, the hotspot view
and the /ui status charts. ADR 0230 records how Pro's Tailwind 3 markup is renamed for
Tailwind 4 and is not restated here.

1. **The starting point is the matching Pro block.** A new page or screen of the review
   surfaces starts from the Flowbite Pro block that matches it in the owner's licensed
   sources — a table with search for the findings, stat cards for the scorecard, a page
   header with breadcrumb for the frame — and adapts its markup and class lists. It does not
   invent a component the Pro set already has. The template names the block it adapts in a
   comment beside the markup, `flowbite-pro: <category>/<block>`
   (`{# flowbite-pro: application-ui/tables/table-with-search #}` in a site template,
   `// flowbite-pro: …` in a screen the executable renders), so a reviewer can open the
   licensed original beside the adaptation.
2. **Every colour and type value is a token.** Adapting a block replaces its values with the
   declaration's; the mapping is the alias layer of decision 3:

   | A Pro block writes | The adaptation writes |
   |---|---|
   | `bg-gray-50 dark:bg-gray-900` (page) | `bg-neutral-primary` (`--mj-bg`) |
   | `bg-white dark:bg-gray-800` (card, surface) | `bg-neutral-primary-soft` (`--mj-raised`) |
   | `bg-gray-100 dark:bg-gray-700` (well, table head) | `bg-neutral-secondary` (`--mj-sunken`) |
   | `border-gray-200 dark:border-gray-700` | `border-default` (`--mj-line`) |
   | `text-gray-900 dark:text-white` | `text-heading` (`--mj-fg`) |
   | `text-gray-500 dark:text-gray-400` | `text-body` (`--mj-body`), `text-body-subtle` (`--mj-muted`) |
   | `bg-primary-700 hover:bg-primary-800 text-white` (button) | `mj-btn mj-btn--accent` (`--mj-accent-fill`, `--mj-on-accent`) or `bg-brand` |
   | `text-primary-600 dark:text-primary-500` (link) | `text-fg-brand` (`--mj-accent`) |
   | `bg-green-100 text-green-800` (status badge) | `mj-badge mj-badge--<word>`, the word filed under its meaning (decision 4) |
   | `text-[13px]`, `tracking-[0.12em]`, a font of its own | `text-dense`, `tracking-caps`, `font-sans` (decision 6) |

   A value the table does not answer is a role or a step added to `share/design/tokens.yaml`,
   never a value kept in the template.
3. **No Pro source is committed.** This repository and its site are public and the
   Flowbite Pro licence forbids publishing its source files. What enters the tree is adapted
   markup on the tokens; no Pro file — a block, a page, a stylesheet, a script, an image, as
   shipped — is committed, vendored, or copied into a generated or published page.

`scripts/ci/design-check` holds the parts a machine can hold. Question 2 refuses Tailwind's
palette families in every first-party file and now names the class it refuses. Question 10
refuses a review surface that names no block, and in every file that names one it refuses
Pro's `primary-<shade>`, `white` and `black` colour utilities and a type value of its own (a
bracketed size, family or spacing that reads no token, or a literal in a style). Question 11
compares every tracked file of a kind the kit ships with the owner's kit-evidence/v1
fingerprints, handed in with `--evidence` or, in CI, through the secret
`MAJORDOMUS_KIT_EVIDENCE_FLOWBITE_PRO`, and refuses one that is a licensed file byte for byte,
naming it and the licensed source; without evidence it says it did not judge rather than
passing. `test/cases/107_design_tokens.sh` plants each of these and watches it refused.

The boundary in words is wider than the check. A check by byte identity misses a copy with
one character changed, and the never-publish gate (`majordomus design licence`, I4336), which
also matches a reformatted copy, judges only a tree whose policy declares a kit, which this
one does not yet. So the review of each review surface keeps the licence boundary as well:
markup adapted onto the tokens is the owner's product; a Pro file with its values intact,
whatever its bytes, is the licensed source and does not land.
