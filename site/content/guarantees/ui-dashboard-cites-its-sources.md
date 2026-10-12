+++
title = "The UI's dashboard at /ui links every capability it read to that capability's page, serves under the Cockpit's security policy, and never waits for the pull-request queue, which is a page and a fragment of its own"
description = "majordomus ui, the primary surface at /ui (ADR 0135), shows a person how the delivery"
weight = 196
[extra]
claim_id = "ui-dashboard-cites-its-sources"
status = "guaranteed"
source = "docs/claims/ui-dashboard-cites-its-sources.md"
+++
{% raw %}

## What it means

`majordomus ui`, the primary surface at `/ui` (ADR 0135), shows a person how the delivery
stands. Every source the dashboard reads is cited on the page, each with a link to that
capability's page in the Cockpit, where its schema, routes and provenance are. A reader who
doubts a number can follow it to the capability that answered it, and from there call that
capability over the API, the command line or MCP and get the same answer.

The dashboard is served under the same content-security policy as the Cockpit, so the UI
adds no looser surface. The pull-request queue is the slowest source the delivery has. It is
not something the first paint waits for: it is a panel at a route of its own, which answers
both as a whole page and as a fragment that a screen pulls in after it has painted.

## How it works

The dashboard (`apps/majordomus-cli/src/ui/dashboard.rs`) declares the sources it reads.
It asks them through the one executor that serves MCP and HTTP, all at once, and renders
each answer with a source line naming the capability and linking
`/cockpit/capabilities/<id>`. A source that fails reads "not answered", never zero
(`project.ui-derived-state`). The queue is left out of the first paint. A
`data-panel-src="/ui/panels/pull-requests"` placeholder is filled by the UI's one script, and
a reader without JavaScript gets it as a link.

`apps/majordomus-cli/tests/ui_surface.rs` proves the claim over a real socket against a
disposable repository. `the_dashboard_is_served_at_ui_under_the_cockpits_policy` asks for
`/ui` and asserts:

- the page is HTML under the Cockpit's own `content-security-policy`, with `nosniff`;
- it links to the Cockpit;
- it cites `health.report`, `release.version`, `distribution.releases`, `plan.status`,
  `plan.roadmap` and `mesh.peers` through their Cockpit pages;
- the queue is referenced only as `/ui/panels/pull-requests`, never as `integration.queue`.

`the_queue_panel_is_a_page_and_a_fragment` asks for that panel's route, once as a page and
once with `?fragment`.

## How to see it

```bash
majordomus serve                       # logs the URL of the shared server
# open <url>/ui and follow any "source" line under a tile or panel
cd apps/majordomus-cli && cargo test --test ui_surface
```

## What it does not cover

It proves the dashboard, not every screen. That every screen renders its hero and panels
with a source line is held by `ui::tests::every_screen_renders_its_hero_and_panels` in the
crate, and that no screen shows a value a capability did not answer is held by the
`ui-integrity` gate (case 547). Neither is this claim's test.

It does not prove that the cited capability's answer is correct. That is proven by the
capability's own tests. Nor does it prove the layout at every width, which the `ui-audit`
gate and the design rules hold.

## Why it exists

The UI is the surface a person is sent to first, so it is the one most likely to be believed
without checking. A dashboard that showed numbers without naming where they came from would
be a second authority over the delivery, which is what ADR 0012 and ADR 0088 forbid. The claim
makes the citation part of the contract a reader relies on, and the test holds it over a real
server rather than over a rendered template.
{% endraw %}
