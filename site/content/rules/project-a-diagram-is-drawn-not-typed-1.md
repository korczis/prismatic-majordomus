+++
title = "An architectural relationship in a published document is drawn, never typed in box art"
description = "An architectural relationship in a published document is drawn, never typed in box art"
weight = 56
[extra]
kind = "rule"
slug = "project-a-diagram-is-drawn-not-typed-1"
identity = "project.a-diagram-is-drawn-not-typed@1"
status = "active"
source = ".ai/repo/rules/project/a-diagram-is-drawn-not-typed.v1.md"
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

**An architectural relationship in a document under `docs/` is a ```mermaid block.** That is
lifecycles, state machines, pipelines, dependency and governance graphs, projections from a
declaration to its surfaces, and anything else whose content is *what connects to what*.

**Three things made of the same characters are not diagrams, and are exempt by declaration:**

- **literal terminal output** — what a command actually printed. Fence it `console`, `bash`,
  `sh` or `shell-session`. Redrawing a terminal as a graph would be a lie about what the
  terminal showed, and this repository shows real output rather than mock-ups;
- **a file listing** — `├── lib/` says where files are, not how they relate, and a reader
  compares it against their own `ls`. A tree's `# …` annotations are prose, so an arrow
  inside one is a sentence, not an edge;
- **quoted file content** — a block showing what a generated header looks like contains
  `<!-- … -->`, and an HTML comment is not an arrow.

**Mermaid is a renderer, not a second source.** Where the relationship already exists as data
— the outcomes a workflow declares, the surfaces a capability is exposed on, the rules a
doctrine dispatches — the diagram is generated from that data and never typed beside it. A
mermaid block written by hand is for the explanatory relationship that no registry holds, and
even then it is written **once**, in the document that states it, and not repeated in ASCII
next to it for the terminal.

## Failure behaviour

`scripts/ci/diagram-check`, registered as the `diagram-check` gate, fails when the number of
hand-drawn diagrams under `docs/` rises above `.ai/repo/diagram-baseline.txt`, listing each
one with its file, line and first line of content. It also fails when the count falls *below*
the baseline without the baseline being lowered — a ratchet above the truth is a gate that
has stopped looking, and the only way to go down is to record that you did.

The baseline exists because the debt predates the rule. It is meant to reach zero; a change
that raises it is refused rather than accommodated.

## Verification

```sh
scripts/ci/diagram-check          # the gate
scripts/ci/diagram-check --list   # every hand-drawn diagram, for converting
bash test/run.sh 272_diagrams_are_drawn
```

`test/cases/272_diagrams_are_drawn.sh` builds the blocks the gate must tell apart in a
fixture of its own — a box-art architecture, a mermaid block, a `console` transcript, a file
tree, a tree whose annotation contains an arrow, and a block quoting an HTML comment — so the
gate is measured by what it decides rather than by what this repository happens to contain.
{% endraw %}
