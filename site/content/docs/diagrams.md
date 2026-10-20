+++
title = "Diagrams and charts"
description = "diagrams and charts on the site: one viewer for every Mermaid diagram (zoom, pan, full screen, download, tracing), the addresses `#diagram-<name>` and `#diagram-<name>:<node>`, node links inferred at build from the identities the generated data carries, and charts drawn from the tables a page already renders"
weight = 70
[extra]
source = "docs/DIAGRAMS.md"
+++

{% raw %}

Every diagram this site publishes is drawn by one viewer, and every chart by one component.
Neither is configured per page. A diagram is a ` ```mermaid ` fence in the one document that
states the relationship, or Mermaid source a template writes from generated data; a chart is
a table the page already renders. What makes them interactive, addressable and linked is
derived at build time from what the repository already knows. The decision is
[ADR 0152](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/adrs/0152-a-diagram-and-a-chart-on-the-site-are-one-viewer-each-over-w.md);
the rule that a diagram is drawn and never typed is
[`project.a-diagram-is-drawn-not-typed`](https://github.com/korczis/prismatic-majordomus/blob/@source-ref@/.ai/repo/rules/project/a-diagram-is-drawn-not-typed.v2.md).

## The path of one diagram

<pre class="mermaid">
flowchart LR
    fence["a mermaid fence&lt;br/&gt;&lt;small&gt;docs, a decision, a rule&lt;/small&gt;"]
    data["site/data/generated&lt;br/&gt;&lt;small&gt;diagrams.json, plan.json&lt;/small&gt;"]
    zola["Zola&lt;br/&gt;&lt;small&gt;pre class=mermaid&lt;/small&gt;"]
    pass["scripts/lib/diagrams.mjs&lt;br/&gt;&lt;small&gt;name, nodes, toolbar, links&lt;/small&gt;"]
    check["scripts/ci/link-check&lt;br/&gt;&lt;small&gt;every link and every address&lt;/small&gt;"]
    viewer["site/diagrams.js&lt;br/&gt;&lt;small&gt;draw, zoom, pan, trace, address&lt;/small&gt;"]
    fence --&gt; zola
    data --&gt; zola
    zola --&gt; pass
    data -- "identities and routes" --&gt; pass
    pass --&gt; check
    pass --&gt; viewer
</pre>


Nothing in that path is typed twice. The fence is the only place the relationship is
written; the identities a node can link to are the ones `scripts/generate-site-data` already
wrote; and the viewer reads what the build prepared rather than a configuration of its own.

## What the build adds to a diagram

`scripts/site-build` runs `scripts/lib/diagrams.mjs` over the pages Zola wrote. For each
`<pre class="mermaid">` it adds four things:

<div class="overflow-x-auto" tabindex="0">

| Added | From | Why |
|---|---|---|
| `id="diagram-<name>"` | the `data-diagram` name a template declared, else the id of the heading the diagram sits under, else its ordinal on the page; a second diagram under one heading is `<name>-2` | an address a link can land on, with or without JavaScript |
| `data-nodes` | the node ids the source declares (flowcharts and state diagrams) | a link to one node can be refused when the diagram has no such node |
| a toolbar, written `hidden` | the pass itself | a control the markup declares is one `scripts/ci/interaction-check` can hold to a behaviour; the viewer reveals it once there is a drawing to control |
| a list of links beside it | the identities in `site/data/generated` | the links work without JavaScript and are checked like every other link |

</div>


A page that would carry two diagrams under one declared name is refused, and the build
stops (exit 10). The diagram's source is never changed.

## Links are inferred

A node becomes a link when its id, its label, or one line of its label is the identity of an
object this build wrote a page for:

<div class="overflow-x-auto" tabindex="0">

| Identity in the diagram | Links to | Read from |
|---|---|---|
| `I2653`, `M003` | the issue or the milestone in the plan | `plan.json` |
| `ADR 0152`, `adr-0152` | the decision | `entities.json` |
| a rule id, with or without its version | the rule | `entities.json` |
| `majordomus doctor`, `majordomus plan next` | the command | `commands.json` |
| a capability id such as `mesh.claim` | the capability | `executable.json` |
| a doctrine id | the doctrine | `doctrines.json` |
| `docs/DESIGN.md` | the document | `docs.json` |

</div>


An identity inside a longer label resolves too, but only when the label names one object.
A label that names two, or that names an object this site has no page for, is linked to
neither: a link that picked the one it could resolve would be a guess. A bare file name
inside a sentence is not an identity. An identity whose page the build did not write is not
offered at all.

Nobody writes an `href` into a diagram. Mermaid's `click` directive is not used: a link
exists because an identity appears in a diagram, and it disappears at the next build when
the object does.

## The viewer

`site/diagrams.js` is loaded only by pages that carry a diagram, and fetches the vendored
Mermaid build after the page has been parsed. It draws each diagram in the page's own
colours, read from the design tokens, and draws them again when the theme changes.

<div class="overflow-x-auto" tabindex="0">

| To | Do |
|---|---|
| zoom | the toolbar's `−` and `+`; `+` and `-` on the keyboard; Ctrl or Cmd with the wheel; a pinch |
| pan | drag; the arrow keys |
| see the whole diagram | `Fit`; `0`; a double click on empty space |
| fill the screen | `Expand`; `F`; `Escape` leaves |
| keep the drawing | `SVG` saves `<name>.svg` |
| follow a node | click it, or move the focus to it and press Enter, when the build linked it |
| trace a node | point at it or move the focus to it: its edges and neighbours stay, the rest steps back |
| select a node | click a node that is not a link; Alt and click, or `S` with the focus on it, for one that is |
| copy an address | `Link` copies the diagram's address, or the selected node's |

</div>


A diagram too large to be read whole, such as the dependency graph of every issue, opens at
a readable size from its top left corner and is panned from there. A plain wheel always
scrolls the page.

The frame is the diagram's own `<pre>`: it keeps its id and its place in the tab order. A
diagram that fails to draw keeps its source visible, gets no controls, and reports the
error on the browser console, which `scripts/interaction-probe` treats as a finding.

## Addresses

<div class="overflow-x-auto" tabindex="0">

| Address | Opens |
|---|---|
| `#diagram-<name>` | the page at that diagram |
| `#diagram-<name>:<node>` | that diagram with the node selected, centred and traced |

</div>


`scripts/ci/link-check` decides both offline. The first is an ordinary fragment: the id must
exist on the page. The second resolves only when the page carries the diagram and the
diagram declares the node, so a link to a node that was renamed fails the build that
renamed it.

## Charts

A chart is a table the page already renders, declared as one:

```html
<figure id="chart-milestones" data-chart="bar" data-chart-stack data-chart-horizontal data-chart-unit="issues">
  <table>
    <thead><tr><th>milestone</th><th data-series="done">…</th></tr></thead>
    <tbody><tr id="chart-milestones:m000"><td><a href="…">M000</a></td><td>15</td></tr></tbody>
  </table>
</figure>
```

`site/charts.js` draws the table with Apache ECharts: the first column is the category and
every column whose header carries `data-series` is a series. The numbers are read from the
cells the page shows, so a chart cannot disagree with its table and there is no second copy
of the data. The table stays on the page; it is what a reader without JavaScript, a screen
reader and a printout get.

<div class="overflow-x-auto" tabindex="0">

| Declaration | Meaning |
|---|---|
| `data-chart="bar"` or `"line"` | what to draw |
| `data-chart-stack` | the series are parts of one whole |
| `data-chart-horizontal` | categories run down the side |
| `data-chart-unit` | the unit, on the axis and in the tooltip |
| `data-chart-label` | what the picture is of, for a reader who cannot see it |
| `data-series` on a header cell | this column is drawn |
| `data-value` on a cell | the unrounded number, when the cell shows a rounded one |
| a link in a row's first cell | the row's mark opens the same page |
| `id="chart-<name>:<row>"` on a row | the row's address, which also highlights its mark |

</div>


A series takes the colour its header is drawn in when the header carries a status badge, so
a state is drawn in the colour the design declares for it; any other series takes the next
of the design's series colours. ECharts is vendored at build from the pinned npm package and
is fetched the first time a chart nears the viewport.

Two charts are drawn this way today:

<div class="overflow-x-auto" tabindex="0">

| Chart | Page | Table it reads | Data |
|---|---|---|---|
| `#chart-milestones` | `/plan/` | issues of each milestone by derived state | `plan.json` |
| `#chart-context-history` | `/economics/` | the context reduction ratio at each recorded run | `economics.json` |

</div>


## What is checked, and by what

<div class="overflow-x-auto" tabindex="0">

| Claim | Check |
|---|---|
| what a source declares and what a node resolves to | `node --test scripts/lib/diagrams.test.mjs` |
| names, nodes, toolbar, links, idempotence, the refusal of a duplicate name, and both kinds of address through `link-check` | `bash test/run.sh 3006_diagrams_are_interactive_and_linkable` |
| every link the pass wrote resolves | `scripts/ci/link-check`, on every build |
| the toolbar, the keys, the address and the linked nodes behave in a browser | the `diagram` spec of `scripts/interaction-probe` |
| no relationship is typed as box art | `scripts/ci/diagram-check` |

</div>


The charts have no behaviour spec yet: they declare no control, so the probe has nothing to
claim, and what a chart read from its table is asserted by no gate. That is issue I2654.

## Limits

- Node links and node addresses exist for flowcharts and state diagrams. A sequence diagram
  or any other kind is named, drawn, zoomed and addressable as a whole, and has no nodes to
  link.
- Edges are traced in flowcharts only.
- The homepage carries no diagram and no chart. Its sections and its byte budget are
  declared in `site/data/homepage.toml`, and adding either is a change to that declaration
  (issue I2655).
- The Cockpit and `majordomus ui` draw their own graphs and do not use this viewer
  (issue I2656).
{% endraw %}
