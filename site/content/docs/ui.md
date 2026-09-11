+++
title = "UI conformance"
description = "UI conformance: the page set and the width set discovered rather than listed, the responsive, semantic, component and WCAG invariants a browser checks over every page, what the build normalises in markup it did not write, and where the report is"
weight = 48
[extra]
source = "docs/UI.md"
+++

{% raw %}

How every page this repository serves is held to one standard, and why the standard is an
audit rather than a checklist. Behaviour as implemented and tested; where implementation
and this document disagree, the document is wrong and changes in the same commit as the
fix. The rule is `project.ui-conformance@1`; the decision and what it rejected are
[ADR 0022](https://github.com/korczis/prismatic-majordomus/blob/master/.ai/repo/adrs/0022-the-ui-standard-is-an-audit-over-a-discovered-page-set.md).

## The problem this solves

A UI standard that lives in review comments is enforced on the pages somebody looked at.
This site is generated: hundreds of pages from a few dozen templates and one theme, so a
decision made once appears everywhere and is measured nowhere. The first machine audit of
every page, at every width the theme declares, found that four decisions accounted for the
overwhelming majority of failures — one highlighting palette, one typography rule, one link
idiom, one route mounted over a section of the site. Not one of them is a page's mistake.

So the standard is executable, it runs over every page rather than a sample, and a finding
is treated as a shared source until proven otherwise.

The widths every page is audited at are the media queries the theme compiled *and* the
widths `share/design/tokens.yaml` declares (`viewports`), unioned; the two browser probes
(`scripts/site-probe`, `scripts/cockpit-probe`) measure at the declared widths alone and
also assert the design contract — that the stylesheet a page loaded, the page itself and
the executable carry one fingerprint. How the design itself is declared and projected is
[`DESIGN_SYSTEM.md`](@/docs/design-system.md).

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

**The pages of a built surface** are the union of two sources: every `index.html` under the
built site, and every `<loc>` of its sitemap. Neither alone is right — a page the sitemap
omits is an orphan and still has to work, and a page the sitemap advertises has to exist. The
sitemap's entries carry the published base URL, so the common prefix is inferred and stripped
rather than configured.

**The pages of a surface the executable renders** — the Cockpit, the server's home page, the
Swagger shell — cannot come from a directory, because there is not one. They come from the
surface itself: its own `<a href>` anchors, crawled from its mount and staying inside it. A
page a reader can reach is a page something links to, so that closure *is* the page set, and
a page linked from nowhere is out of the crawl's reach for the same reason it is out of a
reader's. Reading anchors rather than every `href` is also why nothing needs a list of file
extensions to tell a page from a stylesheet: the markup already made that distinction.

Whether a surface has pages at all is the server's answer rather than a list here: a mount
that returns a document to an `Accept: text/html` request is a page surface, and `/api/v1`,
`/events` and `/mcp` are not — none of them had to be named to be left out.

The crawl is bounded by what it learns rather than by a depth. A route is expanded while
routes of its shape are still yielding routes nobody had seen, so a paginated listing is
followed to its last page and a thousand leaf pages whose only links go back to the shell
cost two fetches between them. Measured: **2660 Cockpit routes derived in 108 requests**, in
about fourteen seconds, with no route named anywhere.

That derivation lives in `scripts/lib/ui-routes.mjs` and it is the only one there is:
`scripts/cockpit-probe` imports the same module, so the two browser instruments over this
repository cannot disagree about what a Cockpit route is. What differs is what each asserts —
the probe owns the shell, the security headers, the design fingerprint, the interactions and
the claim that the browser layer is optional; the audit owns contrast, the accessibility
engine, the landmarks, the components and the width sweep.

**The widths** are the `min-width` media queries the CSS build emitted, converted from `rem`
where the theme used them, each contributing the boundary *and* the pixel below it, plus a
reflow floor of 320 and a desktop width. A breakpoint added to the theme is audited the day
it compiles.

**The tiers** are derived too. Every page is visited at the floor, a middle width and the
desktop end; one page per section takes the full sweep across every boundary. On a built
surface a section is the first path segment, because that is where templates change; on a
crawled surface it is the *family* — the shape a route shares with its siblings, `?`-keys
when it has a query and the parent path when it does not. Both are where a renderer changes,
so the sweep buys structural coverage without anybody naming a page.

**The sample** is derived as well, and only a crawled surface has one. Every route the
surface advertises is visited, because each of those is its own page; of every family the
audit takes four members spread across the sorted set, because a family is one renderer over
many records and what varies is the record. Four is a budget spent on whatever members
exist, not a list — a family with fewer members is visited whole, which is why no navigation
entry is ever dropped. The Cockpit's 2660 routes come to 108 pages this way, and the report
prints both numbers so that a sample can never be read as a total.

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

### A subtree that is not ours to fix

A page may carry a component tree this repository did not write and has decided not to
restyle: pinned at a version, its markup and its stylesheet arriving together. The Swagger UI
widget is the one such tree here, and ADR 0036 already recorded that decision.

The *page* declares the boundary — `data-mj-foreign="swagger-ui-dist@5.17.14"` on the element
that holds it — and the engine is told to skip it. Nothing in the audit knows what a widget is
called. What is around it is measured as usual, which is how the Swagger shell's missing
`<main>` and missing `<h1>` were found and fixed the day the audit first reached it.

Every declaration is reported, once, under **Not measured, and why**, because "not measured"
and "measured and clean" are different claims and a report that conflated them would be worth
nothing. A subtree without the declaration is measured like anything else, so this cannot be
used to quiet a finding — only to say, on the page, whose finding it is.

### How the engine gets into a page that forbids scripts

The accessibility engine is evaluated through the debugging protocol, not appended as a
`<script>` element. A `<script>` element is *in the document*, so the document's own Content
Security Policy decides whether it may run — and the Cockpit's policy is `script-src 'self'`
with a hash per script, exactly as it should be. The engine would be refused on every page of
the strictest surface here, which is the surface whose accessibility was least measured; the
first run over the Cockpit produced 507 `page.audit-failed` findings that were all one
refusal. The debugger's evaluation is the instrument speaking rather than the page, so it
runs without the policy being relaxed — which matters, because a page audited with its policy
disabled is not the page.

## What it costs

The full audit is 996 pages and 3483 visits, and it drives a pool of browser tabs rather than
one: a visit is mostly waiting — for a navigation, for the accessibility engine inside the
page — so a serial run leaves the machine idle for most of it. `MJ_UI_JOBS=N` sets the pool;
the default follows the machine, between two and four.

Reaching the served surfaces cost 117 pages and 516 visits — 91s at six jobs — on top of the
878 the built documentation contributes. That is the price of the sampling: the Cockpit's
2645 routes visited in full at three widths each would be eight thousand visits on their own.

Measured on one laptop, same tree, same 2430 visits: **873s at one job, 260s at four**. The
audit covers half again as many pages as it did when it read a single directory and finishes
in less than a third of the time.

## The origin the audit drives, and where its pages are

The audit runs against `majordomus serve` — this repository's own server, so the audit holds
no second opinion about how the site is served, and so a route the executable answers itself
is visible to it.

**A built directory does not know where it is served from.** The same documentation is
generated once and mounted at `/docs` by the executable and at the root by the published
site. So the audit does not name a directory: it reads `majordomus web list`, takes every
surface the executable *serves*, and visits each one's pages under the mount the topology
gives it. That is why `/tests` and `/benchmarks` are audited too — they are surfaces like any
other, and nothing had to be added to a list to include them.

It is also why `scripts/ui pages` now starts a server. Half the page set only exists while
the executable is running, so "which pages will be audited" became a question only the server
can answer; `--origin URL` points at one somebody else is running, and `--no-build` skips the
build.

That build is now two: the documentation, and the Cockpit's own assets. Half of those are
deliberately not committed — two megabytes of drawing libraries, each loaded lazily by one
page — and a checkout that has not built them serves graph pages that ask for a library that
is not there. The audit read that as 35 console errors saying nothing about the Cockpit, so
it builds what it measures. A page measured without its assets is a page no reader gets.

It builds with `scripts/site-build --serve`, whose base URL is a path rather than an origin,
so the pages resolve their assets wherever they are served. `--no-build` skips that, for a
caller that has already built; `--origin URL` audits a server somebody else is running.

The published tree (`site/public`) is `published-only` and is not served, so the audit does
not visit it. Both trees come from one source and one generator; auditing the served one
covers the same templates and the same data.

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

The results document's contract is `ui-audit/v1`, read by `majordomus web report ui`. It
gained two fields when the audit reached the served surfaces, both additive and both on the
page rather than only in the file:

- **`surfaces[]`** — `id`, `mount`, `kind`, and for a crawled surface the `routes` it has,
  the `families` they fell into, how many were `sampled`, and whether the crawl was
  `truncated` by its budget. Rendered as *The surfaces this run measured*, so a sample can
  never be read as a total.
- **`foreign[]`** — every subtree a page declared `data-mj-foreign`, once per declaration,
  with the element and the first route it was seen on. Rendered as *Not measured, and why*.

## Adding a page, a width, or an invariant

A page: add it. It is audited when it renders.

A width: change the theme. It is visited when the CSS compiles.

An invariant: add it to `scripts/lib/ui-audit.mjs` if it needs a browser, or to
`scripts/lib/ui-static.mjs` if it can be decided from markup, and give it a rule name in the
same `area.thing` shape as the others. Nothing else changes — not the report, not the gate,
not this document's tables, because none of them enumerate rules.
{% endraw %}
