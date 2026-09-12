+++
title = "The site's navigation is declared once, and every projection derives from that declaration"
description = "The site's navigation is declared once, and every projection derives from that declaration"
weight = 94
[extra]
kind = "rule"
slug = "project-navigation-is-declared-once-1"
identity = "project.navigation-is-declared-once@1"
status = "active"
source = ".ai/repo/rules/project/navigation-is-declared-once.v1.md"
+++
{% raw %}

## Rationale

Navigation looks like presentation and is not. It is a claim about what this project is
for — the handful of questions a visitor arrives with, and the pages that answer them — and
it is rendered in at least three places on every page of the site. That makes it exactly the
kind of fact this repository is careful about: stated once, projected many times, and worth
nothing the moment a projection starts stating it too.

Two failures measured it, and both were invisible for as long as they were true.

The first is a declaration that lied about being one. `site/data/nav.toml` carried a header
saying the footer rendered from it. The footer did not read `nav` at all: it was five links
typed by hand, so the site had two navigations, and the second one could not be changed by
editing the file that claimed to own it. A file that announces itself as the single source is
more dangerous than one that does not, because a reader who believes it stops looking — the
shape `project.derived-once` names, wearing the costume of the fix.

The second is the same route declared twice under two names. `/docs/cli/` and `/docs/api/`
each appeared in two groups, labelled differently in each, because the categories they sat
under — `Executable` and `Reference` — answered the same question. A visitor met the same
page twice and had to guess which label was the real one, which is the information
architecture handing its job to the reader. It survived because nothing compared the file
with itself. That version of the file carried nine top-level groups; the duplication was not
a coincidence of it, it was a consequence: categories multiply until two of them mean the
same thing, and then their contents overlap.

## Required behaviour

**`site/data/nav.toml` is the one declaration.** It states the `[[groups]]`, the `items` of
each, and the single `[cta]`. Nothing else states them, and nothing enumerates a route the
generator owns — a feature, a module, a capability or a command page is reached through its
index, never listed here by hand.

**No href appears twice in it.** Not across groups, not within one. There is exactly one
exception and it is a rendering decision rather than a second entry: a group's own `href` may
repeat as the `href` of its **first** item, which is the dropdown's landing link — the page
reached by clicking the group itself. The same route under a second label anywhere else is
the defect above.

**The primary navigation stays small: at most six groups.** A navbar is not a sitemap. Not
every page deserves a place in it, and a category that exists because a page needed a home is
the beginning of the duplication this rule is about.

**Every projection derives.** A template that renders site navigation reads `nav.groups`; it
does not type routes beside it. That covers `site/templates/partials/navbar.html` and
`site/templates/partials/footer.html` today and any surface that joins them. Two literal
forms count as typing a route — `href="/docs/"` and `get_url(path="/docs/")` — and two things
are exempt by declaration: the home link `/`, which every page carries and no group ever is,
and anything the declaration itself already carries, which is what declaring it is for. A
route built from a variable is a computation, not a typed route, and an external URL never
begins with `/`.

**The call to action exists and resolves.** It is an action rather than a sixth category, and
it is the one link on the bar that may not be dead.

## Failure behaviour

`scripts/ci/nav-check`, registered as the `nav-check` gate on the `site` path class, fails
with exit 10 and names each finding: the two places a duplicated href was written, the group
count against the limit, the template and the route it typed, the template that renders
navigation without reading `nav.groups`, and a call to action that no page answers. A missing
or unparseable `site/data/nav.toml` is exit 12 — unusable rather than clean, because a gate
that cannot reach its subject must say so instead of passing.

The remedy is always the same direction: move the fact into the declaration and render it.
Never the other way.

## Verification

```sh
scripts/ci/nav-check        # the gate
bash test/run.sh 273_navigation_is_declared_once
```

`test/cases/273_navigation_is_declared_once.sh` builds a fixture site for each shape the gate
has to tell apart — a clean navigation whose groups legitimately repeat their own href as
their first item, a route declared in two groups, a route declared twice inside one group,
seven groups, a call to action pointing at no page, a template that types a route and one
that resolves the same route through `get_url`, a template that renders navigation without
reading the declaration, and a declaration that is absent or malformed — so the gate is
measured by what it decides rather than by what this repository happens to contain. The last
assertion in the case runs the gate against this repository itself.
{% endraw %}
