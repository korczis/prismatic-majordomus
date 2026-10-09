---
id: project.navigation-reaches-every-width
version: 1
kind: rule
title: Every navigation surface is reachable and operable at every width the design declares
description: A surface that navigates — the Cockpit's sections, the site's menu — is reached by touch, by keyboard and without JavaScript at every width share/design/tokens.yaml declares, and the probe that says so asserts what a person can do there, not only that nothing overflows. Below the width where the Cockpit's sidebar sits beside the page it is the same element as a drawer, with one state owner.
statement: Hold a narrow screen to what a person can reach and do on it, measured in a browser at every declared width; a check that asserts only the absence of overflow passes on a page with nothing on it, which is how a phone went without the Cockpit's navigation while every check was green.
status: active
class: blocking
depends_on: [project.every-link-and-control-is-tested@1, project.navigation-is-declared-once@1, project.derived-once@1]
tags: [cockpit, site, navigation, mobile, accessibility, testing]

x-majordomus:
  tests: [scripts/cockpit-probe, scripts/lib/cockpit-probe.mjs, scripts/lib/interaction-specs/nav-collapse.mjs, apps/majordomus-cli/src/cockpit/view.rs]
---

# Rationale

Below Tailwind's `lg` width the Cockpit's sidebar was `hidden lg:block`, and nothing showed it.
On a phone the Cockpit had no section navigation at all. Every narrow-width check passed: the
Cockpit probe swept 139 routes at 320 and 390 for horizontal overflow, and a page whose
navigation is gone does not overflow. The checks asserted what does not go wrong; nothing
asserted what a person can reach.

The repair exposed a second lesson. The first drawer was correct and its first probe was
green, and a seeded random walk against a model of the drawer still found a defect in its
twentieth step: after following an entry and pressing Tab, the focused skip link slid over
the new trigger and took the next tap. Scripted transitions are the ones somebody thought
of; a walk finds the orders nobody wrote down.

# Required behaviour

**Reachable at every declared width.** At every width in `share/design/tokens.yaml`
`viewports` below `lg`, and in phone landscape, the navigation's trigger is visible, at least
44 by 44 CSS pixels, and opens the full catalogue — the same entries the wide layout shows,
from the same rendering. Nothing about a narrow screen is a second navigation.

**One state owner, written once.** The Cockpit drawer's state lives in the `cockpit`
component and is written into the page synchronously in one function; the stylesheet derives
the backdrop from it. No second piece of state — a reactive binding, a Flowbite controller —
may own any part of it.

**Modal only where it is modal.** Open as a drawer it is a named dialog with `aria-modal`,
the rest of the page `inert`, the page behind it not scrolling, focus inside. Beside the page
it is a plain `nav`.

**Every way out undoes everything.** Close control, backdrop, Escape, an entry followed, the
back-forward cache, widening past `lg`, opening the palette: each leaves no lock, no `inert`
element and nothing covering the page.

**Without the script, and with Alpine missing.** The trigger is a link the stylesheet answers
through `:target`, so it works with JavaScript disabled and when Alpine fails to load.

**The site's phone menu closes on Escape** and hands focus back to its toggle.

# Failure behaviour

`scripts/cockpit-probe` (and `--drawer` alone) fails with exit 10 and names each finding: the
width, the transition, the state it left (`open`, `locked`, `inert` count, what covers the
page, where focus is). A failing random walk prints its seed, its path and the command that
replays it. `scripts/interaction-probe --spec nav-collapse` fails when Escape leaves the site's
menu open. The `cockpit::view` unit tests fail when the shell stops carrying the trigger, the
close control or a backdrop directly after the sidebar, or renders the sidebar as a dialog.

The remedy is never a wider viewport list in a test or a longer timeout: it is the control a
person could not reach.

# Verification

```sh
scripts/cockpit-probe --drawer
COCKPIT_PROBE_SEED=<seed> scripts/cockpit-probe --drawer   # replay a walk
scripts/interaction-probe --spec nav-collapse
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --lib cockpit::view
```
