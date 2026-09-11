// The graph viewer. The nodes and edges come from /api/v1/graph, which is the canonical
// Rust model; this file translates that model into Cytoscape's own shape at the boundary
// and nowhere else, so a different renderer would need no change on the server.
//
// Progressive enhancement in the strict sense: the page already lists every node and every
// edge as a table. This adds a second view of the same data. If Cytoscape is not vendored,
// or fails, the tables stay and nothing is lost.
//
// What this file decides, and what it does not:
//
//   the layout      is chosen from the graph's own measured shape, because there is no one
//                   layout that is right for a 91-node tree and a 1442-node hairball, and
//                   `breadthfirst` for both is what made every large graph a horizontal
//                   line about one pixel tall. `choose()` below is the whole decision, and
//                   `measure()` is the evidence it decides on.
//   the colours     are not decided here. A node kind's colour is `--mj-series-<n>`, which
//                   share/design/tokens.yaml declares; this file only decides *which* of
//                   the series a kind gets, and it decides that by hashing the kind's name
//                   so that the answer does not move when another kind appears.
//   the type sizes  are the declared scale, read from `--text-meta`.
//
// Only the built-in layouts are available: share/cockpit/vendor/ holds cytoscape.min.js and
// nothing else, the Cockpit's content-security policy names no remote origin, and a layout
// extension would be a second vendored file for one page. So the four `choose()` picks
// between are `circle`, `breadthfirst`, `cose` and `concentric`.

import { api, vendor, palette, explainMissing, motionAllowed } from './cockpit.js';

/** At or under this many nodes every label is painted at every zoom; above it, not. */
const LABELS_EVERYWHERE_UNDER = 80;
/**
 * How many labels can share one frame and still be read. A label at the declared `meta`
 * step is about 90x12 device pixels and the frame is about 1110x630; the arithmetic says
 * 600 would fit edge to edge, and 600 overlapping identifiers is the grey smear the `why`
 * graph rendered when this was decided by zoom alone. Sixty is the count at which they are
 * separate words.
 */
const LABEL_BUDGET = 60;
/** However far out the reader is, this many of the busiest nodes stay named. */
const ALWAYS_NAMED = 12;

const frame = document.querySelector('[data-mj-graph]');
if (frame) {
  draw(frame).catch((error) => explainMissing(frame, error));
}

async function draw(frame) {
  const [cytoscape, answer] = await Promise.all([
    vendor('cytoscape.min.js', 'cytoscape'),
    api(frame.dataset.mjGraphSrc),
  ]);
  if (!answer.ok) throw new Error('the graph endpoint answered ' + answer.status);
  const graph = answer.body;
  const read = palette();
  const series = seriesColours(read);
  // the page's palette, as channels: see `channels()` for why a token cannot go straight in
  const colours = drawable(read);
  const type = typeScale();

  const degree = degrees(graph);
  const shape = measure(graph, degree);
  const kinds = Object.keys(graph.node_kinds || {});
  const colourOf = (kind) => series[slot(kind, series.length)];

  // the one translation to the drawing library's shape
  const elements = [
    ...graph.nodes.map((n) => ({
      data: {
        id: n.id,
        label: n.label,
        kind: n.kind,
        summary: n.summary || '',
        route: n.route || '',
        external: n.external ? 'yes' : 'no',
        weight: degree.get(n.id) || 0,
      },
    })),
    ...graph.edges.map((e, i) => ({
      data: { id: 'e' + i, source: e.source, target: e.target, label: e.kind },
    })),
  ];

  const cy = cytoscape({
    container: frame,
    elements,
    layout: choose(shape),
    // not lower: at 0.04 a 20px node renders under one pixel, which is the defect this
    // replaces wearing a different layout. A drawing that will not fit at a legible zoom
    // should overflow the frame and be panned, not be shrunk into a smear.
    minZoom: 0.1,
    maxZoom: 4,
    textureOnViewport: shape.nodes > 400,
    style: [
      {
        selector: 'node',
        style: {
          'background-color': (n) => colourOf(n.data('kind')).rgb,
          'border-width': (n) => (n.data('external') === 'yes' ? 2 : 0),
          'border-style': 'dashed',
          'border-color': colours.muted,
          label: 'data(label)',
          color: colours.text,
          'font-size': type.meta,
          'font-family': 'ui-monospace, SFMono-Regular, Menlo, monospace',
          'text-valign': 'bottom',
          'text-margin-y': 3,
          'text-wrap': 'ellipsis',
          'text-max-width': '120px',
          'text-background-color': colours.background,
          'text-background-opacity': 0.72,
          'text-background-padding': 1,
          // a node is as big as it is connected. A constant 12px said every node was
          // equally important, which in a graph derived from a registry is the one thing
          // that is never true: the reader is looking for the hubs.
          width: (n) => size(n.data('weight'), shape.busiest),
          height: (n) => size(n.data('weight'), shape.busiest),
        },
      },
      {
        selector: 'edge',
        style: {
          width: 1,
          'line-color': colours.border,
          'target-arrow-color': colours.border,
          'target-arrow-shape': 'triangle',
          'arrow-scale': 0.6,
          'curve-style': 'bezier',
        },
      },
      // Level of detail. 1442 labels painted at once are not information, they are a grey
      // smear; a label appears when the reader has zoomed close enough to read it, and the
      // busiest handful stay named at every zoom so the drawing is never anonymous.
      { selector: 'node.mj-unnamed', style: { label: '' } },
      { selector: '.mj-dim', style: { opacity: 0.12, 'text-opacity': 0 } },
      { selector: '.mj-hidden', style: { display: 'none' } },
      {
        selector: '.mj-focus',
        style: { 'border-width': 3, 'border-style': 'solid', 'border-color': colours.accent },
      },
      // The relation's type, on the edges of whatever the reader is pointing at or has
      // selected. `data.label` carried the edge kind before and no style ever rendered it,
      // so the type of every relation was invisible. It is not painted on all 3254 edges
      // at once because that is unreadable above a few dozen edges and because an edge's
      // kind is a question asked about one node at a time; the legend cannot carry it
      // either, since the same two node kinds are joined by several different relations.
      {
        selector: 'edge.mj-named',
        style: {
          label: 'data(label)',
          'font-size': type.meta,
          'font-family': 'ui-monospace, SFMono-Regular, Menlo, monospace',
          color: colours.muted,
          'text-background-color': colours.background,
          'text-background-opacity': 0.82,
          'text-background-padding': 2,
          'text-rotation': 'autorotate',
          width: 1.6,
          'line-color': colours.accent,
          'target-arrow-color': colours.accent,
        },
      },
    ],
  });

  if (!motionAllowed()) cy.autounselectify(false);

  // --------------------------------------------------------------- level of detail
  const busiest = cy
    .nodes()
    .sort((a, b) => b.data('weight') - a.data('weight'))
    .slice(0, ALWAYS_NAMED);
  // What is labelled is decided by what is *on screen*, not by the zoom and not by the size
  // of the graph. A zoom threshold cannot know that 266 nodes still overlap when all of them
  // are in view, and it cannot know that forty of a thousand are readable once the reader has
  // panned into a corner. So: count the nodes inside the viewport, and if they are within the
  // budget name all of them; otherwise name only the busiest, which keeps the drawing from
  // ever being anonymous.
  const decideLabels = () => {
    if (shape.nodes <= LABELS_EVERYWHERE_UNDER) return;
    const view = cy.extent();
    const inside = cy.nodes().filter((n) => {
      const p = n.position();
      return p.x >= view.x1 && p.x <= view.x2 && p.y >= view.y1 && p.y <= view.y2;
    });
    cy.batch(() => {
      cy.nodes().addClass('mj-unnamed');
      if (inside.length && inside.length <= LABEL_BUDGET) inside.removeClass('mj-unnamed');
      else busiest.removeClass('mj-unnamed');
    });
  };
  // a pan fires this on every frame; the decision is worth making once the reader stops
  let settling = 0;
  const decideSoon = () => {
    clearTimeout(settling);
    settling = setTimeout(decideLabels, 120);
  };
  cy.on('viewport', decideSoon);

  // --------------------------------------------------------------------- selection
  cy.on('tap', 'node', (event) => {
    const node = event.target;
    cy.elements().addClass('mj-dim');
    const near = node.closedNeighborhood();
    near.removeClass('mj-dim');
    near.nodes().removeClass('mj-unnamed');
    cy.elements('edge').removeClass('mj-named');
    node.connectedEdges().addClass('mj-named');
    cy.elements('node').removeClass('mj-focus');
    node.addClass('mj-focus');
    describe(frame, node.data());
  });
  cy.on('mouseover', 'edge', (event) => event.target.addClass('mj-named'));
  cy.on('mouseout', 'edge', (event) => {
    if (!event.target.connectedNodes().hasClass('mj-focus')) event.target.removeClass('mj-named');
  });
  cy.on('tap', (event) => {
    if (event.target === cy) {
      cy.elements().removeClass('mj-dim').removeClass('mj-focus').removeClass('mj-named');
      decideLabels();
      const drawer = document.querySelector('[data-mj-graph-details]');
      if (drawer) drawer.remove();
    }
  });

  // ---------------------------------------------------------------------- controls
  const fit = document.querySelector('[data-mj-graph-fit]');
  if (fit) fit.addEventListener('click', () => cy.fit(cy.elements(':visible'), 24));

  const search = document.querySelector('[data-mj-graph-search]');
  if (search) {
    search.addEventListener('input', () => {
      const needle = search.value.trim().toLowerCase();
      if (!needle) {
        cy.elements().removeClass('mj-dim');
        decideLabels();
        return;
      }
      cy.elements().addClass('mj-dim');
      const matching = cy
        .nodes()
        .filter((n) => (n.data('label') + ' ' + n.data('summary')).toLowerCase().includes(needle));
      matching.removeClass('mj-dim').removeClass('mj-unnamed');
      matching.connectedEdges().removeClass('mj-dim');
      if (matching.length) cy.fit(matching, 48);
    });
  }

  legend(frame, cy, kinds, graph, colourOf, decideLabels);

  // the layout has run by the time this fires; fit what it actually drew
  cy.ready(() => {
    cy.fit(cy.elements(), 24);
    decideLabels();
  });

  // What the drawing occupies, for anything that needs to ask whether it drew a graph or a
  // hairline. A probe cannot read a canvas's meaning, and counting canvas elements reported
  // three healthy layers over a one-pixel line for as long as this page has existed; this
  // is the same question answered by the renderer, which knows.
  frame.mjGraph = {
    layout: shape.layout,
    shape,
    extent: () => {
      const box = cy.elements(':visible').renderedBoundingBox();
      const size = frame.getBoundingClientRect();
      return {
        width: box.w,
        height: box.h,
        frameWidth: size.width,
        frameHeight: size.height,
        nodes: cy.nodes().length,
        zoom: cy.zoom(),
      };
    },
  };
  frame.dataset.mjGraphLayout = shape.layout;
}

// ------------------------------------------------------------------- the measured shape

/** How many edges touch each node: the one number the drawing ranks and sizes by. */
function degrees(graph) {
  const d = new Map();
  for (const n of graph.nodes) d.set(n.id, 0);
  for (const e of graph.edges) {
    if (d.has(e.source)) d.set(e.source, d.get(e.source) + 1);
    if (d.has(e.target)) d.set(e.target, d.get(e.target) + 1);
  }
  return d;
}

/**
 * The graph's shape, measured rather than assumed: how big it is, how tightly it is wired,
 * how deep a breadth-first walk goes and — the number that matters most — how many nodes
 * end up on the widest of those levels.
 *
 * That last number is the whole diagnosis of the defect this replaces. `breadthfirst` puts
 * one BFS level on one row; in the composed graph one level held about 1400 nodes, so the
 * drawing was a row 1400 nodes wide and `fit()` zoomed out until that row fitted the
 * frame's width — a horizontal hairline. Measuring `widest` is what lets the choice below
 * refuse breadthfirst before it happens rather than after.
 */
function measure(graph, degree) {
  const nodes = graph.nodes.length;
  const edges = graph.edges.length;
  const out = new Map();
  const indegree = new Map();
  for (const n of graph.nodes) {
    out.set(n.id, []);
    indegree.set(n.id, 0);
  }
  for (const e of graph.edges) {
    if (out.has(e.source)) out.get(e.source).push(e.target);
    if (indegree.has(e.target)) indegree.set(e.target, indegree.get(e.target) + 1);
  }
  let roots = graph.nodes.filter((n) => indegree.get(n.id) === 0).map((n) => n.id);
  if (!roots.length && nodes) roots = [graph.nodes[0].id];

  const level = new Map();
  let queue = roots.slice();
  for (const r of roots) level.set(r, 0);
  while (queue.length) {
    const next = [];
    for (const id of queue) {
      for (const to of out.get(id) || []) {
        if (level.has(to)) continue;
        level.set(to, level.get(id) + 1);
        next.push(to);
      }
    }
    queue = next;
  }
  const width = new Map();
  for (const l of level.values()) width.set(l, (width.get(l) || 0) + 1);
  // anything the walk never reached is still a node breadthfirst has to place, in a row of
  // its own; count it in the widest row rather than pretend it is not there
  const unreached = nodes - level.size;
  const widest = Math.max(unreached, ...(width.size ? [...width.values()] : [0]));

  let busiest = 1;
  for (const d of degree.values()) busiest = Math.max(busiest, d);

  return {
    nodes,
    edges,
    density: nodes ? edges / nodes : 0,
    levels: width.size,
    widest,
    busiest,
    kinds: Object.keys(graph.node_kinds || {}).length,
    layout: '',
  };
}

/**
 * Pick a layout from that shape. Five branches, each with a reason, and each boundary
 * measured against a 1110x630 frame rather than guessed:
 *
 *   circle       (24 nodes or fewer) everything fits on one ring at a readable size, every
 *                node is equidistant from the centre, and no node hides behind another. A
 *                force-directed run over twenty nodes only wobbles.
 *   breadthfirst (a layered graph whose widest layer fits across a frame) this is what
 *                breadthfirst is for, and it is the only built-in layout that shows
 *                *direction*: a graph rooted in a few kinds reads top to bottom. The guard
 *                is `widest` — the moment one level holds more nodes than a frame can show
 *                side by side, breadthfirst is a horizontal line. 48 across a 1440px frame
 *                is about 30px per node, which is the last width at which it is a drawing.
 *   cose         (400 nodes or fewer) force-directed, and the only built-in layout that
 *                puts a cluster together: the reader sees *communities* — the rules that
 *                depend on each other, the documents that cite each other. Its cost grows
 *                with the square of the node count, which is what bounds it here.
 *   concentric   (up to 900 nodes, and only when the degrees are actually peaked) rank by
 *                degree: hubs at the centre, leaves on the rim. It is a sort rather than an
 *                iteration, so it is instant. The hub test is the point — on a flat degree
 *                distribution every node lands in the same ring and concentric is a circle
 *                with nothing in it.
 *   grid         (everything else) sorted by kind, then by degree. Measured: `composed` is
 *                1452 nodes, and concentric gave a disc about 19000px across, so `fit()`
 *                hit `minZoom` and a 20px node rendered under one pixel — the hairline
 *                again, in a different shape. A 1110x630 frame is 700k square pixels; 1452
 *                nodes leave 480 each, which is a 22px cell. At that density there is no
 *                arrangement in which a node is both placed by structure *and* still a
 *                visible mark, so the drawing stops pretending: one cell per node, kinds in
 *                contiguous blocks, hubs first within each. What it conveys is the
 *                vocabulary and its proportions — which is real, and which the grey disc
 *                did not convey either. The structure is then reached through the legend's
 *                filter (one kind at a time is a graph that fits) and through selecting a
 *                node, which lights its neighbourhood and names its relations.
 */
function choose(shape) {
  const common = { padding: 24, animate: false, fit: true };
  if (shape.nodes <= 24) {
    shape.layout = 'circle';
    return { ...common, name: 'circle' };
  }
  if (shape.levels >= 3 && shape.widest <= 48 && shape.density < 2) {
    shape.layout = 'breadthfirst';
    return { ...common, name: 'breadthfirst', directed: true, spacingFactor: 1.1 };
  }
  if (shape.nodes <= 400) {
    shape.layout = 'cose';
    return {
      ...common,
      name: 'cose',
      // bounded: a Cockpit page may not spend a minute on physics. Cytoscape's own
      // defaults with the iteration count cut and randomisation on, which is what stops a
      // dense graph collapsing into its own centre.
      numIter: 400,
      randomize: true,
      nodeRepulsion: () => 12000,
      idealEdgeLength: () => 48,
      nodeOverlap: 8,
      gravity: 0.4,
      componentSpacing: 60,
    };
  }
  // peaked, not flat: the busiest node carries many times the average degree, so ranking by
  // degree actually separates the nodes into rings instead of piling them into one
  const peaked = shape.busiest >= Math.max(8, shape.density * 8);
  if (shape.nodes <= 900 && peaked) {
    shape.layout = 'concentric';
    return {
      ...common,
      name: 'concentric',
      concentric: (n) => n.data('weight'),
      levelWidth: (nodes) => Math.max(1, nodes.maxDegree() / 8),
      minNodeSpacing: 12,
      avoidOverlap: true,
    };
  }
  shape.layout = 'grid';
  return {
    ...common,
    name: 'grid',
    avoidOverlap: true,
    condense: true,
    // kinds in contiguous blocks, and within a kind the busiest first: the eye reads the
    // vocabulary as bands and finds the hubs at the front of each
    sort: (a, b) =>
      a.data('kind').localeCompare(b.data('kind')) || b.data('weight') - a.data('weight'),
  };
}

/** A node's diameter, from its degree. Square-rooted, so a hub is bigger and not absurd. */
function size(weight, busiest) {
  const share = Math.sqrt(Math.max(weight, 0) / Math.max(busiest, 1));
  return 8 + Math.round(share * 22);
}

// ----------------------------------------------------------------------- the vocabulary

/**
 * The sRGB channels of whatever a token resolves to, measured by painting it.
 *
 * The declaration's values are Tailwind v4's palette, which is `oklch`, and Cytoscape's own
 * colour parser does not know `oklch`: handed one it logs "the style property ... is
 * invalid" and falls back to black. Every token this file read was already landing that way
 * before the series existed — the borders, the label grounds — and nobody saw it because the
 * node fill was the one colour the old code computed itself, in a syntax the library could
 * read. So the value is painted onto a one-pixel canvas and its channels are read back, and
 * the drawing gets the triple. Cytoscape accepts `[r, g, b]` directly, which is why nothing
 * here composes a colour string: this is colour arithmetic at a library boundary, the same
 * category as scripts/lib/ui-contrast.mjs, and the decision is still tokens.yaml's.
 */
function channels(value, fallback) {
  if (!value) return fallback;
  try {
    const canvas = document.createElement('canvas');
    canvas.width = 1;
    canvas.height = 1;
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    ctx.clearRect(0, 0, 1, 1);
    ctx.fillStyle = value;
    ctx.fillRect(0, 0, 1, 1);
    const px = ctx.getImageData(0, 0, 1, 1).data;
    if (!px[3]) return fallback;
    return [px[0], px[1], px[2]];
  } catch (e) {
    return fallback;
  }
}

/** Every colour the drawing uses, as channels the library can read. */
function drawable(colours) {
  const black = [0, 0, 0];
  const out = {};
  for (const name of ['background', 'surface', 'border', 'text', 'muted', 'accent']) {
    out[name] = channels(colours[name], black);
  }
  return out;
}

/**
 * The categorical series, read from the declaration: `--mj-series-1` upward until the page
 * stops answering, so how many colours there are is share/design/tokens.yaml's decision and
 * not a number written here. If the stylesheet is not there the drawing is the page's own
 * accent throughout — monochrome and honest, which is the same contract `palette()` keeps.
 * Nothing in this file is a colour literal; the hue rotation this replaces was one.
 */
function seriesColours(colours) {
  const style = getComputedStyle(document.documentElement);
  const found = [];
  for (let n = 1; n <= 64; n += 1) {
    const value = style.getPropertyValue('--mj-series-' + n).trim();
    if (!value) break;
    // both forms of the same token: the canvas needs channels, the legend's swatch is a
    // DOM element and CSS reads the declaration's own value perfectly well
    found.push({ css: value, rgb: channels(value, channels(colours.accent, [0, 0, 0])) });
  }
  return found.length
    ? found
    : [{ css: colours.accent, rgb: channels(colours.accent, [0, 0, 0]) }];
}

/**
 * The declared type scale, for a canvas that cannot be given a CSS class. `font-size: 9`
 * was a size the scale does not name; `meta` at 10px is the smallest step it does.
 */
function typeScale() {
  const root = getComputedStyle(document.documentElement);
  const step = (name) => {
    const value = parseFloat(root.getPropertyValue(name));
    return Number.isFinite(value) && value > 0 ? value : 0;
  };
  // the fallback is another declared step and then the page's own computed size — never a
  // number chosen here, for the same reason `palette()` falls back to a computed colour
  const meta = step('--text-meta') || step('--text-label') || parseFloat(getComputedStyle(document.body).fontSize);
  return { meta };
}

/**
 * Which colour of the series a kind gets. A hash of the kind's *name*, so `capability` is
 * the same colour in the registry graph and in the composed graph, and is still that colour
 * the day a thirty-fifth kind is declared. The code this replaces indexed into the kind
 * list, which meant declaring one new kind recoloured every kind after it: node identity
 * that changed between two versions of the same page, for no reason a reader could see.
 *
 * FNV-1a, 32-bit: four lines, and the same answer in every engine.
 */
function slot(kind, count) {
  let hash = 0x811c9dc5;
  for (let i = 0; i < kind.length; i += 1) {
    hash ^= kind.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash % Math.max(count, 1);
}

/**
 * The legend, which is also the filter. At 34 node kinds a reader needs two things: what a
 * colour means, and the ability to take a kind out of the picture. One control does both —
 * a swatch per kind with the number of nodes that carry it, and a click that hides that
 * kind and refits what is left.
 *
 * It is built here rather than rendered by the server because it is a legend *of the
 * drawing*: with no drawing there is nothing to explain, and the page already names every
 * kind and every edge kind in "What the shapes mean" for a reader who has no JavaScript.
 */
function legend(frame, cy, kinds, graph, colourOf, decideLabels) {
  const host = document.createElement('div');
  host.className = 'mj-graph-legend';
  host.setAttribute('data-mj-graph-legend', '');

  const counts = new Map();
  for (const n of graph.nodes) counts.set(n.kind, (counts.get(n.kind) || 0) + 1);
  // busiest kind first: the reader's eye wants the thing most of the drawing is made of
  const ordered = kinds.slice().sort((a, b) => (counts.get(b) || 0) - (counts.get(a) || 0));
  const hidden = new Set();

  for (const kind of ordered) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'mj-graph-legend-entry';
    button.setAttribute('aria-pressed', 'false');
    button.title = (graph.node_kinds[kind] || kind) + ' — click to hide this kind';

    const swatch = document.createElement('span');
    swatch.className = 'mj-graph-swatch';
    // the DOM reads the declaration's own value; only the canvas needs channels
    swatch.style.background = colourOf(kind).css;
    const name = document.createElement('span');
    name.className = 'mj-graph-legend-name';
    name.textContent = kind;
    const count = document.createElement('span');
    count.className = 'mj-graph-legend-count';
    count.textContent = String(counts.get(kind) || 0);
    button.append(swatch, name, count);

    button.addEventListener('click', () => {
      if (hidden.has(kind)) hidden.delete(kind);
      else hidden.add(kind);
      button.setAttribute('aria-pressed', hidden.has(kind) ? 'true' : 'false');
      button.classList.toggle('is-hidden', hidden.has(kind));
      cy.batch(() => {
        const of = cy.nodes().filter((n) => n.data('kind') === kind);
        if (hidden.has(kind)) of.addClass('mj-hidden');
        else of.removeClass('mj-hidden');
      });
      const left = cy.elements(':visible');
      if (left.length) cy.fit(left, 24);
      decideLabels();
    });
    host.append(button);
  }

  frame.insertAdjacentElement('afterend', host);
}

/** A small drawer under the graph: what the selected node is, and where it lives. */
function describe(frame, data) {
  let drawer = document.querySelector('[data-mj-graph-details]');
  if (!drawer) {
    drawer = document.createElement('div');
    drawer.setAttribute('data-mj-graph-details', '');
    drawer.className = 'mj-alert mj-alert--info';
    frame.insertAdjacentElement('afterend', drawer);
  }
  drawer.textContent = '';

  const title = document.createElement('strong');
  title.textContent = data.label;
  drawer.append(title, ' — ', data.kind);
  if (data.summary) drawer.append(': ' + data.summary);
  if (data.route) {
    const a = document.createElement('a');
    a.className = 'mj-link';
    a.href = data.route;
    a.textContent = 'open';
    drawer.append(' ', a);
  }
  if (data.external === 'yes') {
    drawer.append(' (named by this repository and not held by it)');
  }
}
