+++
title = "Every link the site publishes resolves, and every control on it does what a spec says"
description = "Every link the site publishes resolves, and every control on it does what a spec says"
weight = 83
[extra]
kind = "rule"
slug = "project-every-link-and-control-is-tested-1"
identity = "project.every-link-and-control-is-tested@1"
status = "active"
source = ".ai/repo/rules/project/every-link-and-control-is-tested.v1.md"
+++
{% raw %}

## Rationale

The site's only link check grepped for root-relative hrefs. Zola writes absolute ones, so the check reported
that every internal link resolved while it looked at none. The first check that decided every link found links
on the published site that 404ed, and every one came from a shared producer: a template building a route from
an identifier instead of the slug the reference carries, a graph giving object kinds module pages they do not
have, and a document projection that left Markdown links written for GitHub resolving against the page URL.

Interactive controls had the same shape. A browser probe measured the menu, the dropdowns and the theme toggle
on one page. The copy buttons, tab strips, filters, the diagnosis, the contract demonstration, the changelog
chips, the graph's filters and the keyboard scroll regions were shipped with no test of what they do.

A list of links or of controls would go stale the day a page changed. So both sets are discovered from the
build, and the tests are the rule's own enforcement rather than a review somebody remembers.

## Required behaviour

**Every link is decided offline.** `scripts/ci/link-check` classifies every href and src the site emits, and
every sitemap entry, and decides each one without the network:
- A same-origin link resolves to a file under `site/public`.
- A fragment names an id its target page carries.
- A link into this repository on its forge resolves against git: a blob is a tracked file, a tree a tracked
  directory, a commit is in the history being published, a tag exists.
- Anything that cannot be decided offline is refused: another host, `javascript:`, an empty href, a bare `#`.
  A shallow clone refuses to report clean.

`scripts/site-check` runs it.

**A projected document's links are resolved at their source.** `scripts/lib/project-links.awk` resolves every
relative link of a projected document against the file it came from:
- A document with a page becomes its route.
- A tracked file or directory becomes its forge blob or tree.
- A path git does not track keeps its text and loses its link.

**Every control is claimed.** `scripts/ci/interaction-check` discovers every control on every page. A control is
a form control, a summary, an element an Alpine directive or a Flowbite binding makes act, an ARIA widget role,
a graph binding, or a keyboard-reachable scroll region. Disabled elements are not controls, and a control inside
`<template>` is counted. Each control must be claimed by exactly one spec under `scripts/lib/interaction-specs/`.
An unclaimed control fails, a doubly claimed one fails, and so does a spec that claims nothing. Every Alpine
expression on every page must compile: text interpolated into a JavaScript string breaks on the first apostrophe,
and Alpine then drops the directive without a sound, which is how the doctrines and use-case filters stopped
filtering. Text a directive needs is carried in a data attribute, which the template escapes, and read through
`$el.dataset`. `scripts/site-check` runs it.

**Every control does what its spec says.** `scripts/interaction-probe` opens every page that carries a control and
serves the build under test at its published origin. The browser classifies the live document with the same rule,
and each spec drives every control it claims. A page fails when:
- a behaviour does not hold
- the live count of a spec's controls differs from the markup
- the console reports an error or an exception goes uncaught
- a request leaves the site's origin
- a spec drives fewer controls than it claims

It runs in CI as the `interaction-probe` gate, in a job of its own. With `MJ_PROBE_LIVE=1` it drives the published
site, which is how a deployment is validated.

## Failure behaviour

- `link-check` exits 10 with one finding per broken link, naming the page, the link and why it does not resolve.
  It exits 12 when the site is missing or the clone is too shallow to decide.
- `interaction-check` exits 10 naming the page and the signature of the unclaimed or doubly claimed control, the
  spec that claims nothing, or the directive whose expression does not compile.
- `interaction-probe` exits 10 naming the page, the spec and the behaviour that did not hold. Without a browser
  it skips locally and exits 12 under `CI=true`.

## Verification

- `scripts/ci/link-check` and `scripts/ci/interaction-check` over the built site, both run by `scripts/site-check`.
- `scripts/interaction-probe` over the built site. After a deployment, `MJ_PROBE_LIVE=1 scripts/interaction-probe`
  checks the commit that `scripts/pages verify` says is served.
- `test/cases/335_every_link_resolves.sh`, `test/cases/336_every_control_is_claimed.sh` and
  `test/cases/337_every_control_does_what_it_says.sh` each break what their check refuses, and prove the
  refusal.
{% endraw %}
