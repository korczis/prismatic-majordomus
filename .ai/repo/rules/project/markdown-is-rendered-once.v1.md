---
id: project.markdown-is-rendered-once
version: 1
kind: rule
title: A Markdown body is drawn by one renderer, wherever it is shown
description: Every Markdown body a served surface shows — a rule, a decision record, a skill, a use case, a document, a field of a plan record, a pull request's description — is drawn by the crate's one renderer, which writes nothing a record holds as markup, follows only the links it was told lead somewhere, and draws a diagram as its source until the page's script draws the figure.
statement: A served surface draws a Markdown body through `crate::markdown` and through nothing else; the parser is named in that one module, a record's raw HTML is shown as text, a link leads to a web address, a fragment or an object the index holds and otherwise nowhere, a table's alignment is a class and never an inline style, and a fenced diagram is its source marked for drawing.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.ui-conformance@1, project.ui-derived-state@1, project.derived-once@1]
tags: [ui, cockpit, markdown, projections, security]

x-majordomus:
  tests: [test/cases/1101_markdown_is_rendered_once.sh, apps/majordomus-cli/tests/markdown_bodies.rs]
---

# Rationale

Most of what this repository says is Markdown, and until 2026-10-12 none of it was drawn. The
Cockpit's page for an object showed a rule or a decision record as the characters it is
written in: a table as rows of pipes, a diagram as a paragraph of arrows, a link as brackets.
The UI showed a milestone's problem as one escaped string, and when it first drew markup it
did so with a parser of its own, written for that one screen — the second renderer on the
day there was a first.

Two renderers are two answers to every question a renderer answers, and three of those
questions are not about looks:

- **What a record may write into the page.** A body is text somebody typed, or text a forge
  returned. Whether its `<script>` is shown or run is decided by whichever renderer drew it.
- **Where a link leads.** A body links to its neighbours by path. A renderer that turns every
  address into an anchor draws dead links and `javascript:` ones alike.
- **What the content policy sees.** A Markdown table's alignment becomes an inline `style` in
  a renderer that has no stylesheet to name. One right-aligned column in one decision record
  stopped three publications of the site on 2026-10-12.

So these are answered once, in one module, and every surface that shows a body calls it.

# Required behaviour

1. **One renderer.** `apps/majordomus-cli/src/markdown.rs` is the only source file of the
   crate that names the Markdown parser. A surface draws a body with `markdown::render` and a
   sentence a record carries in a field with `markdown::inline`.
2. **Nothing a record holds is markup.** The parser's events are drawn through the crate's
   HTML builder, which escapes every byte. Raw HTML in a body is shown as the text it is; an
   HTML comment is dropped.
3. **A link leads somewhere this decided, or nowhere.** An `http` or `https` address is
   followed, with `rel="noopener noreferrer"`; a fragment stays on the page; any other
   address is asked of the surface, which answers only for an object the index holds
   (`objects.list`, `objects.get`). What nobody resolves is drawn as its label with the
   address as its title, never as an anchor. A picture is described and linked, not fetched.
4. **One outline.** A body's headings are drawn below the level the surface says its own
   heading has, so a body never adds a second `h1` to a page.
5. **No inline style.** A table's column alignment is a class of the shared primitives.
6. **A diagram is still its source.** A fenced `mermaid` block is drawn as its text, marked
   `data-mj-diagram`, in a box that scrolls in the tab order; the page's script draws the
   figure from the vendored library with the library's strict security level, and a page
   without the script or the library shows the source.
7. **One look.** The typography of a body is the `.mj-md*` classes of
   `share/design/primitives.css`, which the site, the Cockpit and the UI all import.

# Enforcement

`test/cases/1101_markdown_is_rendered_once.sh` holds that the parser is named in one file,
that the Cockpit's object page and the UI's prose go through the renderer, and that the
primitives carry the classes the renderer writes. `apps/majordomus-cli/tests/markdown_bodies.rs`
serves a repository whose document carries a script, a `javascript:` link, a link to a
neighbour, an aligned table and a diagram, and asks the object's page what it drew.
