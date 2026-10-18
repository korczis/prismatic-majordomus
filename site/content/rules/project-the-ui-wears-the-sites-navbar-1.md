+++
title = "majordomus.dev is the reference for how the product looks, and every page of the UI and the Cockpit wears the site's own navbar"
description = "majordomus.dev is the reference for how the product looks, and every page of the UI and the Cockpit wears the site's own navbar"
weight = 176
[extra]
kind = "rule"
slug = "project-the-ui-wears-the-sites-navbar-1"
identity = "project.the-ui-wears-the-sites-navbar@1"
status = "active"
source = ".ai/repo/rules/project/the-ui-wears-the-sites-navbar.v1.md"
+++
{% raw %}

## Rationale

The owner's order of 2026-10-16: majordomus.dev is the reference for the product's styling
and its interface rules, and the Cockpit and the UI at `/ui` derive from it — the same
layout, a pixel-identical navbar — rather than keep a second design by hand.

Measured that day, the site and the UI shared their tokens (`--mj-design` was the same
fingerprint on both) and nothing else at the top of the page. The site's bar is 73 pixels
tall at 1440 wide and 65 at 390, carries the brand and the release, four menus, a search, a
theme toggle, the product, GitHub and the call to action, and is set in 16-pixel type. The
UI's was a 67-pixel tool bar beside a 240-pixel sidebar with a brand of its own — 119 pixels
tall at 390 — with a different search, a different theme control and none of the site's
questions, in 14-pixel type. The Cockpit's was a third: 53 pixels, 13.5-pixel type, a brand
suffixed "Cockpit", links to Swagger and OpenAPI, and a text button for the theme. Three bars
for one product, each kept by hand, and each a place where the next change to the others
would not arrive.

A rule that said "keep them alike" would describe that state rather than end it. The
navigation is already declared once (`project.navigation-is-declared-once`) and the design is
already one declaration (`project.design-tokens-declared-once`). What was missing is that the
bar itself — the markup a declaration is rendered into — had one renderer, and every other
surface had to imitate its output.

## Required behaviour

1. **One renderer.** The bar is rendered by the site's partial
   (`site/templates/partials/navbar.html`) over the site's declaration
   (`site/data/nav.toml`) and nothing else. `scripts/ui navbar --write` cuts it out of the
   built site — its not-found page, the one page on which no entry is current — into
   `apps/majordomus-cli/src/ui/navbar.html`, which the executable compiles in and both the UI
   and the Cockpit render (`ui/navbar.rs`). Two things of
   that build are made into what the executable fills in: the release the badge names becomes
   a slot, and a link to the site map at the foot of every site page leads to the site's front
   page, because a screen of the UI has no such foot. Nobody edits the copy.
2. **The same styles.** The UI's and the Cockpit's stylesheets compile exactly the utilities
   the copy names, from the same Tailwind, the same Flowbite theme and the same generated
   design sheets the site's `app.css` compiles them from (`share/ui/src/ui.css` and
   `share/cockpit/src/cockpit.css`, each `@source` the copy). The bar
   sets its own text size, so a page that sets another on its body does not change it.
3. **The same behaviour.** The bar's menus and theme toggle run on the site's own scripts,
   `site/nav.js` and `site/theme.js`, served from `share/ui/` and `share/cockpit/` as byte copies that
   `scripts/cockpit-assets` writes and `scripts/cockpit-assets --check` holds to their
   originals; the page carries the theme contract on `<html>`, where the site's script reads
   it. The bar's search button leads to the UI's own search, and in the Cockpit to its palette.
4. **What the executable decides.** The version the badge names is the executable's; an
   entry the site leads to the published snapshot of a surface this process serves (the
   `data-nav-primary` and `data-nav-snapshot` marks the partial writes) leads to the live
   surface and stops saying "snapshot". Every other link leads to the site at its own address.
5. **Nothing beside it.** Neither surface draws a second brand, a second theme toggle or a
   second bar that stays at the top; the UI's own navigation — the screens — and the
   Cockpit's — its sections — are the sidebar beneath it, and each one's tool bar — the UI's
   search and the other surfaces of the process, the Cockpit's name, palette and API links —
   is a row beneath it that does not stay on screen.

## Failure behaviour

`scripts/ui navbar` (and `scripts/ui audit`, which runs it first, as the `ui-audit` gate of
the `ui` job) fails with exit 10 and names each difference:

- the copy is not the bar the built site renders — the remedy is `scripts/site-build &&
  scripts/ui navbar --write`, then `scripts/cockpit-assets`;
- an element of the bar the site draws on its front page and the one drawn on `/ui` or on
  `/cockpit`
  differs in its tag, its classes, its text, its rendered box (beyond one pixel) or any of the
  computed styles listed in `scripts/lib/ui-navbar.mjs`, at a declared width
  (`site/data/registry/design.json` `viewports`), in a theme, with the bar closed or with a
  menu open. The version text is set to one value on both pages before measuring, and the
  snapshot note is left out; nothing else is excused.

An absent site build is exit 12, and a run without a browser is exit 13 — refused on CI,
because a comparison nobody ran is not a comparison that holds.

The remedy is always in one direction: change the site's partial or the declaration, rebuild,
and let the copy follow. Never edit the copy, and never style the bar from the UI's sheet.

## Verification

```sh
scripts/site-build && scripts/ui navbar      # the copy, then the browser comparison
scripts/ui test                               # the cut and the comparison, on fixtures
scripts/cockpit-assets --check                # the scripts are the site's bytes
cargo test --manifest-path apps/majordomus-cli/Cargo.toml ui::navbar ui::shell
```
{% endraw %}
