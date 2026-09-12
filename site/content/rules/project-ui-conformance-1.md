+++
title = "Every page of every web surface is audited, and the audit names no page"
description = "Every page of every web surface is audited, and the audit names no page"
weight = 129
[extra]
kind = "rule"
slug = "project-ui-conformance-1"
identity = "project.ui-conformance@1"
status = "active"
source = ".ai/repo/rules/project/ui-conformance.v1.md"
+++
{% raw %}

## Rationale

A UI standard that lives in review comments is enforced on the pages somebody looked at.
This site had 483 pages and one reviewer's memory; the first machine audit of all of them
found 1455 failures, and every one of the large classes came from a single shared source —
one highlighting palette put 411 contrast failures on 134 pages, one typography rule put 491
unreachable scroll boxes on 197, one link idiom put 312 colour-only links on 92. None of
those was a page's mistake. Each was a decision made once, applied everywhere, and never
measured.

A rule that listed the pages would go stale the day a page was added, which is the same
defect as the registration this repository already refuses for web surfaces. So the audit is
derived from what exists: the pages from the built site's filesystem and its sitemap,
unioned, so an orphan is still visited; the widths from the media queries the CSS build
emitted, so a breakpoint added to the theme is audited without a test changing.

## Required behaviour

**The target set is discovered.** `scripts/ui pages` and `scripts/ui viewports` answer what
will be audited and where each value came from. Neither reads a list. A page that renders
is audited; a width the theme declares is visited, along with the pixel below it, where a
layout discontinuity hides.

**The invariants are executable.** `scripts/ui audit` drives the repository's own server and
a real browser over that set and checks: no horizontal overflow at any width, one `main` and
one `h1` per document, no heading level skipped, no duplicate id, every component trigger
resolving to the target it names and carrying an accessible name, no console error, no
failed same-origin request, and the WCAG 2.0 A/AA, 2.1 A/AA and 2.2 AA rules of an
accessibility engine. The verdict is arithmetic over the findings; nothing decides that a
finding does not count.

**Markup we do not write is normalised, not excused.** A syntax highlighter's `<pre>`, a
typography plugin's `<table>` and a markdown task list's checkbox arrive from third parties
with no attribute we can set at the source. The build normalises the rendered output —
which elements scroll is read from the stylesheet the build just compiled, and the palette
is raised to the contrast threshold against the background the same stylesheet declares, hue
and saturation untouched. A palette that already passes is rewritten to itself, and a second
pass changes nothing.

**Markup we do write is held at its source.** `scripts/ui check` costs milliseconds, needs
no browser and no built site, and refuses a scrolling box a keyboard cannot reach wherever
this repository writes one — a template, a generator, a Rust view. It is the half of the
audit that can stand between a person and a commit.

**A finding is a shared source until proven otherwise.** The remediation of a class of
finding changes the one place it came from. A fix applied page by page to a defect that has
one cause is how a repository acquires 483 copies of a decision.

## Failure behaviour

`scripts/ui check` exits 10 on any failure and runs in the `ui` gate. `scripts/ui audit`
exits 10 on any finding, writes its results document, and `majordomus web report ui` renders
it at `/tests/ui` beside the test run — the page names every finding with the route, the
width, the rule and the element, and a run narrowed for local iteration says so on its own
page, so a partial run can never be read as a clean one.

No command decides whether a page *should* exist or what it should look like. That is
intent, a person states it, and this rule only holds every page that exists to the same
standard.

## Verification

`test/cases/85_ui_conformance.sh` proves it by mutation: a template that writes a scrolling
box without keyboard access is refused; a page added to the site appears in the discovered
set without a list changing; a breakpoint added to the theme appears in the visited widths;
a palette below the threshold is raised, and raising it twice changes nothing. The unit tests
of `scripts/lib/ui-*.mjs` hold the discovery, the scanner, the normaliser and the contrast
arithmetic. ADR 0022 records the decision and what it rejected.
{% endraw %}
