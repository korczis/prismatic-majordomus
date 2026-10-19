+++
title = "An architectural relationship in a published document is drawn, never typed in box art"
description = "An architectural relationship in a published document is drawn, never typed in box art"
weight = 68
[extra]
kind = "rule"
slug = "project-a-diagram-is-drawn-not-typed-2"
identity = "project.a-diagram-is-drawn-not-typed@2"
status = "active"
source = ".ai/repo/rules/project/a-diagram-is-drawn-not-typed.v2.md"
+++
{% raw %}

## Rationale

This repository is careful about facts that are written twice. A diagram typed in
`─│┌└┼` and `-->` is exactly that, and it escaped the care for a long time because it does
not look like data.

It is a projection of the system's structure, made by hand, and it has every failure of a
hand-made projection and one of its own:

- **Nothing can read it.** A `stateDiagram` that a machine can parse can be compared with the
  transitions the code actually declares. A picture of the same states cannot, so it drifts
  from them silently while continuing to look authoritative — the shape `DYNAMICITY.md` names.
- **Nothing can redraw it.** The same relationship on the website, in the Cockpit, in a
  terminal and in an API response needs four renderings. Typed box art is one of them,
  frozen, and the other three either do without or re-type it — which is how one architecture
  comes to be maintained in four independent pictures.
- **It cannot wrap.** Box art has no space to break at. A diagram wide enough to be useful on
  a desktop is a horizontal scrollbar on a phone, and this repository holds its surfaces to
  320px.
- **It cannot be themed or read aloud.** The design system supplies colours from tokens and
  the site is checked for contrast in both themes; a picture made of characters takes part in
  neither, and a screen reader is handed a wall of punctuation.

Mermaid fixes the last three immediately and makes the first one *possible*: a parseable
topology is something a later gate can diff against the registry that owns it. That is why
the target is mermaid and not an image — an SVG would render and still be unreadable.

## Required behaviour

**An architectural relationship in anything this repository publishes is a ```mermaid
block.** That is lifecycles, state machines, pipelines, dependency and governance graphs,
projections from a declaration to its surfaces, and anything else whose content is *what
connects to what*.

**Published means every form a reader meets it in**, not only the Markdown. The canonical
trees are:

- `docs/**`, the pages written straight into `site/content-src/**`, and the templates those
  pages are rendered through, `site/templates/**`;
- the decisions, `.ai/repo/adrs/**`, and the rules, `.ai/repo/rules/**`. Both are published
  as pages of the site and read in the handbook's reading room, so a drawing typed into one
  is a drawing on two surfaces;
- `handbook/**`, the chapters and the page they are assembled into;
- the markdown `majordomus ui` renders: the Rust sources under `apps/majordomus-cli/src/ui/**`,
  whose doc comments rustdoc publishes and whose strings the screens draw, and the scripts
  under `share/ui/**`.

`site/content/**` is a projection of `docs/` and `site/content-src/` and the generator
overwrites it, so a finding reported there would name a file its author cannot fix.

And in each of them, both forms a block is written in — a ```fenced block and an HTML `<pre>`/`<code>` block. The second is not a technicality: the
most-published document in this repository is a template, and for the rule's first weeks a
hand-drawn control loop sat in one, in HTML, where a gate reading fences only could not see
it at any scope.

**An HTML block declares its language the way HTML does**, in `class` or `data-lang`:
`class="console"` is exactly as exempt as a ```console fence, `language-bash` and
`data-lang="mermaid"` are read the same way, and a `<pre>` wearing only layout classes
declares nothing and is judged on its contents.

**Four things made of the same characters are not diagrams, or are not published, and are
exempt by declaration:**

- **literal terminal output** — what a command actually printed. Fence it `console`, `bash`,
  `sh` or `shell-session`. Redrawing a terminal as a graph would be a lie about what the
  terminal showed, and this repository shows real output rather than mock-ups;
- **a file listing** — `├── lib/` says where files are, not how they relate, and a reader
  compares it against their own `ls`. A tree's `# …` annotations are prose, so an arrow
  inside one is a sentence, not an edge. A listing keeps every glyph in its indentation,
  before an entry's name; `executor ──┬── MCP` connects two things after a name, and a block
  that does that is a drawing however tree-like its left margin looks;
- **quoted file content** — a block showing what a generated header looks like contains
  `<!-- … -->`, and an HTML comment is not an arrow;
- **the campaign packs** — `campaigns/**` holds operator prompt packs kept byte for byte as
  provenance and recorded in `campaigns/MANIFEST.sha256`. Nothing there is published or
  loaded, and rewriting a brief would falsify the record of what was asked.

**Every diagram is the interactive viewer, and a node is named by what it is.** On the site a
mermaid block reaches the one viewer of ADR 0152 (`site/diagrams.js`), and the build
(`scripts/lib/diagrams.mjs`) links every node whose id or label is the identity of an object
the site has a page for: a rule id, `ADR 0152`, an issue, a capability such as
`executions.get`, a command such as `majordomus doctor`, a document path. So a node that is
such an object carries its identity in its label, and is clickable without anybody writing
an `href`. The handbook draws the same blocks with its own Mermaid, in the page's colours,
and again when the theme changes; in a handbook chapter the block is a
`<pre class="mermaid-src" data-lang="mermaid">`.

**Mermaid is a renderer, not a second source.** Where the relationship already exists as data
— the outcomes a workflow declares, the surfaces a capability is exposed on, the rules a
doctrine dispatches — the diagram is generated from that data and never typed beside it. A
mermaid block written by hand is for the explanatory relationship that no registry holds, and
even then it is written **once**, in the document that states it, and not repeated in ASCII
next to it for the terminal.

## Failure behaviour

`scripts/ci/diagram-check`, registered as the `diagram-check` gate, fails when the number of
hand-drawn diagrams in the published tree rises above `.ai/repo/diagram-baseline.txt`,
listing each with its file, line and first line of content. It also fails when it falls *below*
the baseline without the baseline being lowered — a ratchet above the truth is a gate that
has stopped looking, and the only way to go down is to record that you did.

The baseline exists because the debt predates the rule. It is meant to reach zero; a change
that raises it is refused rather than accommodated.

## Verification

```sh
scripts/ci/diagram-check          # the gate
scripts/ci/diagram-check --list   # every hand-drawn diagram, for converting
bash test/run.sh 272_diagrams_are_drawn 3106_every_published_tree_draws_its_diagrams
```

`test/cases/272_diagrams_are_drawn.sh` builds the blocks the gate must tell apart in a
fixture of its own — a box-art architecture, a mermaid block, a `console` transcript, a file
tree, a tree whose annotation contains an arrow, and a block quoting an HTML comment — so the
gate is measured by what it decides rather than by what this repository happens to contain.
Each of those is asked again of the HTML form: a template's `<pre><code>` box art and
entity-encoded arrows must be caught, and a `<pre class="console">` holding real terminal
output, a `<pre>` holding a file tree, a `<pre class="mermaid">`, `class="language-bash"`, an
inline `<code>` in a sentence and a ```html fence quoting the markup of a drawing must not.

`test/cases/3106_every_published_tree_draws_its_diagrams.sh` plants box art in each tree
version 2 added (a decision, a rule, a handbook chapter, a UI module's doc comment and a UI
script) and requires the gate to name every one. A mermaid block, a file listing and a
campaign pack in the same places stay clean, and the real tree stays at zero.

## History

Version 1 covered `docs/**`, `site/content-src/**` and `site/templates/**`. Box art survived
outside them, in a decision (ADR 0033), the handbook and a UI module. Inside them, a drawing
whose only arrows were `▼` and `↓` passed as a file listing. Version 2 (ADR 0166, decision 6)
adds the decisions, the rules, the handbook and the markdown `majordomus ui` renders. It reads
`▼ ▲ ↓ ↑ ← ◄ ►` as arrows, and it accepts a block as a listing only when the glyphs stay in
the indentation.
{% endraw %}
