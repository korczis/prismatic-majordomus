+++
title = "UI conformance"
description = "UI conformance: the page set and the width set discovered rather than listed, the responsive, semantic, component and WCAG invariants a browser checks over every page, what the build normalises in markup it did not write, and where the report is"
weight = 30
[extra]
source = "docs/UI.md"
+++

{% raw %}

How every page this repository serves is held to one standard, and why the standard is an
audit rather than a checklist. Behaviour as implemented and tested; where implementation
and this document disagree, the document is wrong and changes in the same commit as the
fix. The rule is `project.ui-conformance@1`; the decision and what it rejected are
[ADR 0017](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0017-the-ui-standard-is-an-audit-over-a-discovered-page-set.md).

## The problem this solves

A UI standard that lives in review comments is enforced on the pages somebody looked at.
This site is generated: hundreds of pages from a few dozen templates and one theme, so a
decision made once appears everywhere and is measured nowhere. The first machine audit of
every page, at every width the theme declares, found that four decisions accounted for the
overwhelming majority of failures — one highlighting palette, one typography rule, one link
idiom, one route mounted over a section of the site. Not one of them is a page's mistake.

So the standard is executable, it runs over every page rather than a sample, and a finding
is treated as a shared source until proven otherwise.

## The commands

<div class="overflow-x-auto" tabindex="0">

| Command | What it answers |
|---|---|
| `scripts/ui pages [--json]` | every page that will be audited, its tier, and where it was found |
| `scripts/ui viewports [--json]` | every width it will be audited at, and the media queries they came from |
| `scripts/ui check` | the static invariants, over markup, with no browser and no built site |
| `scripts/ui audit [--report]` | every page in a real browser; writes the results document, optionally renders `/tests/ui` |
| `scripts/ui test` | the discovery's own tests |

</div>


`majordomus web report ui --from target/web/run-ui.json` renders the results as a section of
the test surface. Exit codes follow the repository's convention: 0 clean, 10 findings, 12 a
missing precondition, 13 a missing dependency.

## What is discovered, and what that buys

**The pages** are the union of two sources: every `index.html` under the built site, and
every `<loc>` of its sitemap. Neither alone is right — a page the sitemap omits is an orphan
and still has to work, and a page the sitemap advertises has to exist. The sitemap's entries
carry the published base URL, so the common prefix is inferred and stripped rather than
configured.

**The widths** are the `min-width` media queries the CSS build emitted, converted from `rem`
where the theme used them, each contributing the boundary *and* the pixel below it, plus a
reflow floor of 320 and a desktop width. A breakpoint added to the theme is audited the day
it compiles.

**The tiers** are derived too. Every page is visited at the floor, a middle width and the
desktop end; one page per section of the site — the first path segment — takes the full
sweep across every boundary. Sections are where templates change, so the sweep buys
structural coverage without anybody naming a page.

Nothing in any of this is a list. `scripts/ui pages` prints its own provenance, and so does
the generated report.

## What the audit checks

Beyond the accessibility engine (axe-core, the WCAG 2.0 A/AA, 2.1 A/AA and 2.2 AA rule sets):

- **`responsive.horizontal-overflow`** — the document is no wider than the viewport, and a
  failure names the elements that reach past the edge, skipping anything inside its own
  scrolling box, which is allowed to be wider.
- **`semantics.main` / `semantics.h1` / `semantics.heading-order`** — one main landmark, one
  level-one heading, no level skipped.
- **`semantics.duplicate-id`** — a duplicate id breaks every reference to it, a component's
  included.
- **`component.target-missing` / `component.trigger-unnamed`** — every Flowbite trigger
  (`data-collapse-toggle`, `data-dropdown-toggle`, `data-modal-target`, `data-drawer-target`,
  `data-accordion-target`, `data-tabs-toggle`, `data-tooltip-target`, `data-popover-target`,
  and the toggles beside them) names a target that exists and has an accessible name.
- **`runtime.console-error` / `runtime.asset-failed`** — the page loaded without an error and
  without a same-origin request failing. A *cancelled* request is not a failed one:
  `net::ERR_ABORTED` is the browser saying it no longer needs the response, which is what a
  lazily loaded asset in flight when the audit moves on produces, and it says nothing about
  the site.
- **`page.status` / `page.unreachable` / `page.audit-failed`** — the page answered, and it
  answered in time. A page that does not is a finding about that page, never the end of the
  run.

## The origin the audit drives

The audit runs against `majordomus serve` — this repository's own server, so the audit holds
no second opinion about how the site is served, and so a route the executable answers itself
is visible to it.

Zola writes absolute URLs. A site built for the published base URL asks the *published*
origin for its stylesheets while the local server answers for its pages, and an audit of
that measures two different sites: it passes defects production has already fixed and fails
ones it has not. So `scripts/ui audit` builds the site for the origin it is about to serve,
audits it, and rebuilds it for the configured base URL afterwards, leaving the checkout as it
found it. `--no-build` skips both, for a caller that has already built for the origin;
`--origin URL` audits a server somebody else is running.

## Remediation: at the source, and only at the source

**Markup this repository writes** is held at its source. `scripts/ui check` reads every
tracked `.html` and every rendered page and refuses a scrolling box a keyboard cannot reach
(WCAG 2.1.1). It costs milliseconds, needs no browser and no built site, and runs in every
CI plan.

**Markup this repository does not write** is normalised in the build, because there is no
source to fix:

- the syntax highlighter's `<pre>` and the typography plugin's `<table>` are put in the tab
  order — and *which* elements scroll is read from the stylesheet the build just compiled, so
  a theme that stops making `<pre>` scroll stops the pass touching it;
- a markdown task list's checkbox is given the accessible name of the text beside it;
- the generated highlighting palettes are raised to a 4.5:1 contrast ratio against the
  background the same stylesheet declares, keeping hue and saturation and giving up only as
  much lightness as the threshold demands.

Both passes are idempotent: a conforming input is rewritten to itself, and a second pass
changes nothing. The audit proves it from the outside, which is the only reason to trust it.

## What the audit refuses to attribute to a page

Three failures look like findings and are not, and each one cost a run before it was
classified:

- a **cancelled** request (`net::ERR_ABORTED`) is the browser saying it no longer needs the
  response — a lazily loaded asset in flight when the audit moves on — not a failed one;
- a **refused connection** means the server the audit drives is gone, so the run stops and
  says how many visits it actually measured, rather than recording every remaining page as
  unreachable;
- a **visit that exceeds its deadline** is reported as that page's finding, and the tab it
  was using is discarded, because a deadline stops waiting and not the work it abandoned.

## Where the report is

`/tests/ui`, inside the test surface — a conformance run is a test run, and the web topology
refuses a surface mounted inside another's subtree, so the architecture had already answered
where this belongs. The page carries every finding with the route, the width, the rule and
the element, the machine-readable `results.json` beside it, and the provenance of the target
set. A run narrowed for local iteration says so on its own page, so a partial run can never
be read as a clean audit.

## Adding a page, a width, or an invariant

A page: add it. It is audited when it renders.

A width: change the theme. It is visited when the CSS compiles.

An invariant: add it to `scripts/lib/ui-audit.mjs` if it needs a browser, or to
`scripts/lib/ui-static.mjs` if it can be decided from markup, and give it a rule name in the
same `area.thing` shape as the others. Nothing else changes — not the report, not the gate,
not this document's tables, because none of them enumerate rules.
{% endraw %}
