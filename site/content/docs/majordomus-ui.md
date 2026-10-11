+++
title = "`majordomus ui`"
description = "`majordomus ui`, the primary surface: how it differs from the Cockpit, where every value on it comes from and how a missing one reads, how the first screen stays fast with slow panels on routes of their own, the page without JavaScript, the assets and gates that hold it, and how a screen is added"
weight = 49
[extra]
source = "docs/MAJORDOMUS_UI.md"
+++

{% raw %}

How the delivery is shown to a person: what the UI is, how it relates to the Cockpit, where
every value on it comes from, how it stays fast and current, and how a screen is added. It
describes behaviour as implemented and tested; where the two disagree, this document is
wrong and changes in the same commit as the fix.

- The decision is
  [ADR 0135](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0135-majordomus-ui-is-the-primary-surface.md).
- The rule the UI is held to is `project.ui-derived-state`.
- The feature record is `.ai/repo/features/ui.md`.
- The owner's brief, with its reference screens, is
  `campaigns/majordomus-ui-complete-opus-fable-v3-20261011/`.

## The product and its technical reference

The shared server of a checkout serves two surfaces a person reads.

<div class="overflow-x-auto" tabindex="0">

| Surface | Mount | What it is |
|---|---|---|
| The UI | `/ui` | the product: the delivery in the project's own words, dense, linked and clickable |
| The Cockpit | `/cockpit` | its technical reference: the registry rendered, one page per capability, object, graph and route |

</div>


The UI is where a person is sent first, and it is not a view of the registry: its navigation
is its screens, its words are the delivery's. It refers to the Cockpit in exactly two places:
- the "technical view" link on each source line, to the Cockpit's page for that capability;
- the Cockpit entry among the other surfaces in the top bar.

Both surfaces are projections (ADR 0012): they hold no data, they compose capabilities
through the executor every transport calls, and the same process serves them under one
content-security policy. The UI is a native surface of the web topology (`web.surfaces`
lists it), declared beside the Cockpit in `apps/majordomus-cli/src/web/discover.rs`.

## The screens

<div class="overflow-x-auto" tabindex="0">

| Screen | Route | Reads |
|---|---|---|
| Dashboard | `/ui` | `repository.info`, `health.report`, `release.version`, `distribution.releases`, `plan.status`, `plan.roadmap`, `mesh.peers`; the queue on its own route |
| Roadmap | `/ui/roadmap` | `plan.roadmap`, `plan.status`, `intents.list` |
| Issues & PRs | `/ui/work` | `plan.status`, `plan.issues`; the queue on its own route |
| Mesh | `/ui/mesh` | `mesh.peers`, `mesh.events` |
| CI/CD & Releases | `/ui/delivery` | `distribution.releases`, `release.version`, `deploy.list`, `gates.model` |
| Evidence | `/ui/evidence` | `evidence.report` |
| Governance | `/ui/governance` | `rules.report` |

</div>


The navigation is `SCREENS` in `apps/majordomus-cli/src/ui/mod.rs` and nothing else. A test
compares it with the dispatcher's arms, so a screen without a route, or a route without a
screen, fails. A screen of the owner's concept that has no producer yet is not in the
navigation: a link to nothing is a dead end.

Every screen has the same shape:
- a hero band, with the screen's glyph, title, tagline and lead, the brand mark lit behind
  them, and a row of KPI tiles;
- a grid of panels;
- the frame around both: the sidebar of screens with the version card at its foot, and the top
  bar with the search, the time the screen was read, and the other surfaces (documentation,
  API, Cockpit).

KPI tiles lead to the screen that explains them. A panel whose whole is on another screen
says "View all". A milestone row leads to that milestone on the Roadmap, where the row it
lands on is lit.

## Where a value comes from

Every value is a reading: the value at an RFC 6901 pointer into one capability's answer
(`apps/majordomus-cli/src/ui/source.rs`). A tile shows its capability and pointer under its
value. A panel closes with its sources. Each source is named with how long it took, and with
every other way to ask the same question, as the capability declares them through
`capabilities.describe`:
- **API**: the route, linked, so a browser opens the same answer as JSON;
- **CLI**: the command a person types, `majordomus <path>`;
- **MCP**: the tool an agent calls;
- **technical view**: the capability's page in the Cockpit.

Nothing is computed on the UI's behalf:
- A list it shows is a list a capability returned.
- A count is the number at a pointer, or the length of the list there.
- A meter is a picture of the two numbers written beside it.
- A donut draws the counts its legend lists.
- A status word is the source's own word, styled by the design system's status vocabulary.

Classes that overlap, such as a rule report's coverage, are listed and never summed. What is
missing says so:

<div class="overflow-x-auto" tabindex="0">

| The source | The UI shows |
|---|---|
| answered, and the pointer holds a value | the value (a fraction rounded to two places) |
| answered, and the pointer holds nothing | `none` |
| answered a row without its status word | `unknown` |
| could not answer | `not answered`, with the reason |

</div>


None of them is ever drawn as zero or healthy. The `ui-integrity` and `cockpit-projection`
gates hold the UI's sources exactly as they hold the Cockpit's:
`scripts/ci/cockpit-projection-check` scans both directories. Cases 389 and 547 prove it
refuses, in the UI, a read of the index or the registry and an absent count drawn as zero.

## Fast, and current

A screen asks all of its sources at once, each on a thread of its own, so it costs its slowest
source rather than the sum of them (`source::ask_all`; the Cockpit's overview does the same
since I2460).

A source too slow for a first paint is a **panel with a route of its own**. The pull-request
queue (`integration.queue`) is `/ui/panels/pull-requests`:
- with `?fragment` the route answers the panel alone, and `ui.js` puts it in place once the
  screen has painted;
- without JavaScript the placeholder is a link to the same route.

While a screen is visible, `ui.js` asks the server for it again every minute, puts the new
rendering in place, and updates the time in the top bar. That time is when the sources were
read. This is a refresh and is said as one; nothing claims a live stream.

## Relationship graphs

A relationship the source records is drawn as a graph, for example the dependencies between
milestones. The server puts the nodes and edges on the page (`view::graph`). The vendored
Cytoscape, loaded only by a screen that has a graph, draws them:
- in the tones of the design system, read from the stylesheet;
- with a layout that does not animate under reduced motion;
- with nodes that lead where their row is.

The table of the same edges stays beneath the graph, and it is what a reader without
JavaScript, or without the library, reads. Only nodes that take part in an edge are drawn.

## Without JavaScript

Every screen is complete HTML, and the menu of a narrow screen opens with a checkbox and its
label. `share/ui/ui.js` is the only script of the UI's own: it pulls in slow panels, refreshes
the screen, draws graphs and reveals the theme toggle. The one inline script is the theme
statement the policy allows by its digest, the same one the Cockpit ships. A served test
asserts that the UI and the Cockpit carry one identical policy.

## Assets and design

The UI's files live in `share/ui/`.

<div class="overflow-x-auto" tabindex="0">

| File | How it is made | Committed |
|---|---|---|
| `ui.css` | compiled from `share/ui/src/ui.css` by `scripts/cockpit-assets`, beside the Cockpit's stylesheet; `--check` fails when either is stale | yes |
| `favicon.svg` | written from the canonical mark by `majordomus generate` | yes |
| `vendor/cytoscape.min.js` | copied from the pinned npm package by `scripts/cockpit-assets` | no |

</div>


The `ui` class of the CI model (`.ai/repo/ci/gates.yaml`) selects the asset check, the probe,
the design gate and the UI audit whenever a file under `share/ui/` or
`apps/majordomus-cli/src/ui/` changes.

The stylesheet imports the tokens, the status vocabulary, the primitives and the kit from
`share/design/`, where the site and the Cockpit import the same files. Every colour is a role,
a status or a tone; none is a literal. The markup names what a thing is, never a utility
class, because the stylesheet compiles only what is written in it. The icons are the kit's,
declared once in `site/data/kit/icons.toml` and compiled into the crate by `majordomus
generate` (`apps/majordomus-cli/src/ui/icons.toml`); a test holds the two equal.

## Adding a screen

1. Add a module under `apps/majordomus-cli/src/ui/` that asks its sources with
   `source::ask_all` and composes `view` components from readings, never a value of its own.
2. Add it to `SCREENS` in `ui/mod.rs` with its icon, and its arm to the dispatcher.
3. A source too slow for a first paint becomes a panel route in `PANELS`, drawn as a
   placeholder with `data-panel-src`.
4. Lead somewhere: a tile with `kpi_to`, a panel with `panel_to`, a row with a link.

## What it does not do yet

- `majordomus ui` as a command, which ensures the shared server and opens this surface, is
  the next slice. Until then, open `/ui` on the address `majordomus serve status` prints.
- Command (a schema-driven runner), Capabilities, Deployments, Documentation and Settings
  screens are not built yet. Until they are, they are not in the navigation.
- No capability records per-run CI timings, so the CI/CD screen draws no build-time chart and
  says why.
- Milestones carry no dates, so the roadmap is a sequence and a graph, never a calendar.
{% endraw %}
