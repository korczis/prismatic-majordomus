---
schema: adr/v1
id: adr-0021
kind: adr
title: The UI standard is an audit over a discovered page set, and its failures are fixed at their shared source
status: accepted
date: 2026-09-06
tags: [ui, accessibility, web, projections]
related:
  - rule:project.ui-conformance
  - rule:project.web-surface-topology
  - file:scripts/ui
  - file:scripts/lib/ui-discover.mjs
  - file:scripts/lib/ui-audit.mjs
  - file:scripts/lib/ui-static.mjs
  - file:scripts/lib/ui-contrast.mjs
  - file:apps/majordomus-cli/src/web/report/ui.rs
  - file:apps/majordomus-cli/src/web/validate.rs
  - test:test/cases/85_ui_conformance.sh
provenance:
  origin: authored
---

# 21. The UI standard is an audit over a discovered page set, and its failures are fixed at their shared source

## Context

The repository had a UI standard in the sense that a person had opinions about it. There was
a mobile-first case that measured horizontal overflow on every route in a headless browser,
and beyond that, review. The site had grown to 483 pages across nine web surfaces, generated
from templates by a build nobody reads page by page.

The first machine audit of all of them, at every width the compiled theme declares, found
1455 failures. The distribution is the whole argument:

- 491 scrollable regions no keyboard could reach, on 197 pages — one typography rule, which
  makes `pre` and `table` scroll, and a highlighter that emits `pre`.
- 411 contrast failures, on 134 pages — one generated highlighting palette, 18 of whose 19
  colours sat below 4.5:1 on the background the same stylesheet declared.
- 312 links distinguished from their surrounding text by colour alone, on 92 pages — one
  link idiom, `text-fg-brand hover:underline`, copied 140 times.
- 165 pages answering 404 under the running executable — one native route mounted over a
  section of the application. (That route has since moved, under
  `project.web-surface-declared-once`; what this decision keeps is the *validator* that
  finds the next one.)
- 36 heading skips on 3 pages, 27 undersized targets on 14, 12 unnamed form controls on 1,
  and 1 page overflowing at 320px.

Not one of the large classes is a page's mistake. Each is a decision made once, applied
everywhere, and never measured. A reviewer cannot find them, because finding them means
visiting 1683 page-and-width combinations and computing contrast ratios.

## Decision

The UI standard is an executable audit over a page set and a width set that are **discovered**,
and its findings are fixed at the source they share.

**Nothing lists a page or a width.** The pages come from the built site — its filesystem and
its sitemap, unioned, so an orphan page is still visited. The widths come from the media
queries the CSS build emitted, so a breakpoint added to the theme is audited without a test
changing. `scripts/ui pages` and `scripts/ui viewports` answer what will be visited and where
each value came from, and neither reads a list.

**The audit is a browser.** `scripts/ui audit` drives the repository's own `majordomus serve`
and a real Chrome over that set, checking what only a browser can answer — layout at a width,
the accessibility tree, whether a component's trigger names a target that exists, whether the
page logged an error — and it holds no second opinion about how the site is served.

**Markup this repository does not write is normalised at build time; markup it writes is held
at its source.** A syntax highlighter's `pre`, a typography plugin's `table` and a markdown
task list's checkbox arrive with no attribute we can set upstream, so the build normalises the
rendered output: which elements scroll is read from the stylesheet the build just compiled,
and the palette is raised to the contrast threshold against the background that stylesheet
declares, hue and saturation untouched. Everything this repository writes itself is refused by
`scripts/ui check`, which costs milliseconds and needs no browser.

**The report is a section of the test surface.** A conformance run is a test run, `/tests/ui`
is where a reader looks for it, and the web topology already refuses a surface mounted inside
another's subtree. So `majordomus web report ui` writes into the surface the test report owns,
declares nothing of its own, and never overwrites the enclosing producer's declaration.

**A native route mounted over the application's own pages is a finding.** The topology exempts
the root application from the nesting rule, correctly — answering what nothing else claims is
its job. That exemption was blind to the application having *files* at particular paths, and
no reasoning about mounts can see it: the answer is on disk. `web validate` now asks the
application's own output whether it has pages under a native route's mount, and warns with the
count. It warns rather than refuses because which of the two should move is intent, and a
person decides that. The one this found has already been decided — `/swagger`, under
`project.web-surface-declared-once` — and the validator is what will find the next one before
an audit has to.

## Consequences

A page added to the site is audited the day it renders, and a breakpoint added to the theme is
visited the day it compiles. Neither costs an edit anywhere.

The build now rewrites its own output twice — the palette and the scrolling markup. That is
derived state being normalised, not a source being edited, and both passes are idempotent: a
conforming palette is rewritten to itself, and a second pass changes nothing. The audit proves
it from the outside, which is the only reason to trust it.

The generated highlighting palette no longer matches the upstream theme it was generated from.
It matches its hue and its saturation; it does not match its lightness where lightness was
below the threshold. Choosing a different theme would have moved the number rather than fixed
the class of defect, because the next theme is chosen the same way — for how it reads, by
somebody who is not measuring contrast.

A shadowing is now reported by `web validate` with the page count, rather than discovered by
an audit of the rendered site an hour later. What to do about one — rename the route or move
the section — stays a person's decision, and the warning is what that decision has to look at.

## Alternatives rejected

**A list of pages to audit, with a rule that it be kept current.** The same defect as the
per-surface registration this repository already refuses: a list is right on the day it is
written. The audit would have covered the pages somebody remembered.

**Choosing a different highlighting theme.** It would have moved 411 findings to some other
number and left the repository with no way to know which. The palette is generated; generated
output is where a threshold belongs.

**Fixing the contrast, the scroll boxes and the links page by page.** 483 pages, three
decisions. The remediation of a class of finding changes the one place it came from, or the
repository acquires 483 copies of a decision it made once.

**Making the audit a blocking check on every commit.** It needs a built site, a browser and
several minutes. The half that can stand in front of a commit — the markup this repository
writes — is `scripts/ui check`, and it does; the browser audit is a gate, which is what the
rule about cheap blocking checks already required.
