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
//                   so that the answer does not move when another kind appears. The softer
//                   fill a hub wears is that same token mixed into the page's raised ground
//                   by the browser (`color-mix`), measured back the way every token is.
//   the type        is the declared scale and the declared mono family, read from
//                   `--text-meta` and `--font-mono`.
//   the motion      is the declared `--mj-motion-fast` and its multiples, and none at all
//                   for a reader who asked for less.
//
// How it answers a hand, in one vocabulary shared with the trains and the UI: pointing at a
// node lights it and its neighbours and names its relations; a click, or the keyboard,
// selects it and opens the drawer under the frame; the legend both explains a colour and
// takes a kind out of the picture; the search lights what matches and says how many.
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
/**
 * Above this many nodes the drawing stops animating its state changes, hides its edges and
 * paints a texture while the reader pans, and offers a map of the whole. Cose's own bound,
 * and the size at which a restyle of every element starts to be felt under the pointer.
 */
const BUSY_GRAPH = 400;
/**
 * A node this many pixels across or more is drawn as a ring around a soft fill, with room
 * for the reader to see the kind's colour as an outline; a smaller one stays a solid dot,
 * because a 10px ring is a smudge.
 */
const RING_FROM = 24;
/** Two of the declared series are the ones a colour-blind reader confuses: these get a ring. */
const RINGED_SLOTS_FROM = 6;
/** How far Shift and an arrow pan, in rendered pixels. */
const PAN_STEP = 48;
/** An arrow moves the selection to a node within this many degrees of its direction. */
const STEER_DEGREES = 60;
/** Each arrow key as a direction in the drawing's own coordinates (y grows downward). */
const ARROWS = { ArrowRight: [1, 0], ArrowLeft: [-1, 0], ArrowDown: [0, 1], ArrowUp: [0, -1] };

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
  // Everything the page's palette is, re-read whenever the theme changes: `restyle()` below
  // assigns these again and the style sheet's functions read them at paint time.
  let read = palette();
  let series = seriesColours(read);
  // the page's palette, as channels: see `channels()` for why a token cannot go straight in
  let colours = drawable(read);
  let softs = softFills(series, read);
  let type = typeScale();

  const degree = degrees(graph);
  const shape = measure(graph, degree);
  const kinds = Object.keys(graph.node_kinds || {});
  const colourOf = (kind) => series[slot(kind, series.length)];
  const softOf = (kind) => softs[slot(kind, series.length)];
  const ringed = (n) => size(n.data('weight'), shape.busiest) >= RING_FROM;

  const still = !motionAllowed();
  // state changes ease only where restyling every element is cheap enough to animate
  const lively = !still && shape.nodes <= BUSY_GRAPH;
  const busy = shape.nodes > BUSY_GRAPH;
  const quick = motionMs();
  // a tooltip and a map are for a pointer that can hover; a touch screen gets the drawer
  const hovers = window.matchMedia('(hover: hover) and (pointer: fine)').matches;

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

  // The frame, its overlays and its map share one positioned box, so the hover card and the
  // map are placed against the drawing without a measurement of the page. Built only once
  // the data has arrived: a failure above leaves the page exactly as the server sent it.
  const stage = document.createElement('div');
  stage.className = 'mj-graph-stage';
  frame.parentNode.insertBefore(stage, frame);
  stage.append(frame);

  // The style sheet as a function of the palette, so a theme change can hand the drawing
  // the same sheet over the new colours (`restyle()`), and nothing else has to be rebuilt.
  const motion = lively
    ? {
        'transition-property': 'opacity, underlay-opacity, border-width, line-color, width',
        'transition-duration': quick,
      }
    : {};
  const styleSheet = () => [
    {
      selector: 'node',
      style: {
        // a hub is a ring around a soft fill of its kind's colour; anything smaller is a
        // solid dot, which at 8 to 20 pixels is the only form whose colour can be read
        'background-color': (n) =>
          ringed(n) ? softOf(n.data('kind')) : colourOf(n.data('kind')).rgb,
        // the two confusable slots carry a second cue at every size: a fg ring on a dot, and
        // on a hub, whose ring every slot already wears, a double ring in place of a single
        'border-width': (n) => {
          const confusable = slot(n.data('kind'), series.length) >= RINGED_SLOTS_FROM;
          if (ringed(n)) return confusable ? 4 : 2;
          if (n.data('external') === 'yes') return 2;
          return confusable ? 1.5 : 0;
        },
        'border-style': (n) => {
          if (n.data('external') === 'yes') return 'dashed';
          return ringed(n) && slot(n.data('kind'), series.length) >= RINGED_SLOTS_FROM
            ? 'double'
            : 'solid';
        },
        'border-color': (n) => {
          if (ringed(n)) return colourOf(n.data('kind')).rgb;
          return n.data('external') === 'yes' ? colours.muted : colours.text;
        },
        label: 'data(label)',
        color: colours.text,
        'font-size': type.meta,
        'font-family': type.mono,
        // a label the zoom has made smaller than this is not painted at all: the level of
        // detail below decides by what is in view, this by what could still be read
        'min-zoomed-font-size': 6,
        'text-valign': 'bottom',
        'text-margin-y': 4,
        'text-wrap': 'ellipsis',
        'text-max-width': '120px',
        'text-background-color': colours.background,
        'text-background-opacity': 0.78,
        'text-background-padding': 2,
        'text-background-shape': 'roundrectangle',
        // a node is as big as it is connected. A constant 12px said every node was
        // equally important, which in a graph derived from a registry is the one thing
        // that is never true: the reader is looking for the hubs.
        width: (n) => size(n.data('weight'), shape.busiest),
        height: (n) => size(n.data('weight'), shape.busiest),
        'underlay-color': colours.accent,
        'underlay-opacity': 0,
        'underlay-padding': 5,
        'underlay-shape': 'ellipse',
        ...motion,
      },
    },
    {
      selector: 'edge',
      style: {
        width: 1,
        'line-color': colours.border,
        'line-opacity': 0.7,
        'target-arrow-color': colours.border,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.6,
        'curve-style': 'bezier',
        ...motion,
      },
    },
    // A relation's kind is never carried by colour alone: the dependencies are dashed, the
    // settled relations dotted and the writes heavier, as on every other relation surface.
    {
      selector: 'edge[label = "uses"], edge[label = "depends_on"]',
      style: { 'line-style': 'dashed' },
    },
    {
      selector: 'edge[label = "satisfied"], edge[label = "merged"]',
      style: { 'line-style': 'dotted' },
    },
    { selector: 'edge[label = "writes"]', style: { width: 1.6 } },
    // Level of detail. 1442 labels painted at once are not information, they are a grey
    // smear; a label appears when the reader has zoomed close enough to read it, and the
    // busiest handful stay named at every zoom so the drawing is never anonymous.
    { selector: 'node.mj-unnamed', style: { label: '' } },
    // everything outside what the reader is pointing at, selecting or searching for, and
    // everything outside the kind a legend chip is previewing
    { selector: '.mj-dim, .mj-aside', style: { opacity: 0.15, 'text-opacity': 0 } },
    { selector: '.mj-hidden', style: { display: 'none' } },
    // the one interaction colour: a halo for what is pointed at or found, a ring and a
    // stronger halo for what is selected. Selection outranks hover.
    { selector: 'node.mj-near, node.mj-match', style: { 'underlay-opacity': 0.14, 'z-index': 10 } },
    {
      selector: 'node.mj-focus',
      style: {
        'border-width': 3,
        'border-style': 'solid',
        'border-color': colours.accent,
        'underlay-opacity': 0.22,
        'z-index': 20,
      },
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
        'font-family': type.mono,
        'min-zoomed-font-size': 6,
        color: colours.muted,
        'text-background-color': colours.background,
        'text-background-opacity': 0.86,
        'text-background-padding': 2,
        'text-background-shape': 'roundrectangle',
        'text-rotation': 'autorotate',
        width: 2.2,
        'line-color': colours.accent,
        'line-opacity': 1,
        'target-arrow-color': colours.accent,
        'z-index': 15,
      },
    },
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
    // a pan over 1442 nodes and 3254 edges redraws a texture of the last frame and leaves
    // the edges out until the reader stops, which is what keeps the largest graph smooth
    textureOnViewport: busy,
    hideEdgesOnViewport: busy,
    style: styleSheet(),
  });

  // --------------------------------------------------------------- level of detail
  const busiest = cy
    .nodes()
    .sort((a, b) => b.data('weight') - a.data('weight'))
    .slice(0, ALWAYS_NAMED);
  // What the reader is pointing at, has selected or has searched for: its nodes are named
  // ahead of the budget below, within the budget itself.
  let lit = null;
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
      if (lit) {
        const nodes = lit.nodes();
        const named =
          nodes.length <= LABEL_BUDGET
            ? nodes
            : nodes.sort((a, b) => b.data('weight') - a.data('weight')).slice(0, ALWAYS_NAMED);
        named.removeClass('mj-unnamed');
      }
    });
  };
  // a pan fires this on every frame; the decision is worth making once the reader stops
  let settling = 0;
  const decideSoon = () => {
    clearTimeout(settling);
    settling = setTimeout(decideLabels, 120);
  };
  cy.on('viewport', decideSoon);

  // ------------------------------------------------------------------ the viewport
  // Has the reader moved the view since the drawing was last fitted? Only while they have
  // not does a resized frame refit itself: a reader's own zoom is never taken from them.
  let moved = false;
  cy.on('scrollzoom pinchzoom dragpan', () => {
    moved = true;
  });
  /** Fit these elements, ending any viewport animation still running so the last ask wins. */
  const fitTo = (eles, padding) => {
    if (!eles.length) return;
    moved = false;
    cy.stop(true, false);
    if (still) cy.fit(eles, padding);
    else cy.animate({ fit: { eles, padding }, duration: quick * 2, easing: 'ease-out-cubic' });
  };
  /** Zoom by `factor` about the frame's centre, within the drawing's own bounds. */
  const zoomBy = (factor) => {
    moved = true;
    const level = Math.min(cy.maxZoom(), Math.max(cy.minZoom(), cy.zoom() * factor));
    const zoom = { level, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } };
    cy.stop(true, false);
    if (still) cy.zoom(zoom);
    else cy.animate({ zoom, duration: quick, easing: 'ease-out-cubic' });
  };
  /** Bring a node into view when the keyboard has moved the selection off the screen. */
  const reveal = (node) => {
    const view = cy.extent();
    const p = node.position();
    if (p.x >= view.x1 && p.x <= view.x2 && p.y >= view.y1 && p.y <= view.y2) return;
    moved = true;
    cy.stop(true, false);
    if (still) cy.center(node);
    else cy.animate({ center: { eles: node }, duration: quick * 2, easing: 'ease-out-cubic' });
  };

  // What the frame says when a filter or a search leaves nothing to draw.
  const nothing = document.createElement('p');
  nothing.className = 'mj-graph-empty';
  nothing.textContent = 'Nothing matches';
  nothing.hidden = true;
  stage.append(nothing);

  // ----------------------------------------------------------------- the hover card
  const tip = document.createElement('div');
  tip.className = 'mj-graph-tip';
  // it repeats what the drawer and the tables say; assistive technology reads those
  tip.setAttribute('aria-hidden', 'true');
  tip.hidden = true;
  stage.append(tip);
  const hideTip = () => {
    tip.hidden = true;
  };
  const showTip = (node) => {
    if (!hovers) return;
    const data = node.data();
    tip.textContent = '';
    const title = document.createElement('span');
    title.className = 'mj-graph-tip-title mj-mono';
    title.textContent = data.label;
    const row = document.createElement('span');
    row.className = 'mj-graph-tip-row';
    const name = document.createElement('span');
    name.className = 'mj-graph-legend-name';
    name.append(swatchOf(colourOf(data.kind)), ' ', data.kind);
    const value = document.createElement('span');
    value.className = 'mj-graph-tip-value';
    value.textContent = data.weight + (data.weight === 1 ? ' relation' : ' relations');
    row.append(name, value);
    tip.append(title, row);
    if (data.external === 'yes') {
      const meta = document.createElement('span');
      meta.className = 'mj-graph-tip-meta';
      meta.textContent = 'named here, held elsewhere';
      tip.append(meta);
    }
    const hint = document.createElement('span');
    hint.className = 'mj-graph-tip-hint';
    // the trains' words: one Cockpit, one way to open a node
    hint.textContent = data.route
      ? 'Double-click, or select and press Enter, to open it'
      : 'Click to select';
    tip.append(hint);
    tip.hidden = false;
    // beside the node and clear of it, flipped below when the top of the frame is near and
    // held inside the frame at the sides; one read of the card's size per hover
    const at = node.renderedPosition();
    const r = node.renderedWidth() / 2;
    const w = tip.offsetWidth;
    const h = tip.offsetHeight;
    const x = Math.max(4, Math.min(at.x + r + 8, cy.width() - w - 4));
    const above = at.y - r - h - 8;
    const y = above >= 4 ? above : Math.min(at.y + r + 8, cy.height() - h - 4);
    tip.style.transform = 'translate(' + Math.round(x) + 'px, ' + Math.round(y) + 'px)';
  };
  cy.on('viewport grab', hideTip);
  frame.addEventListener('pointerleave', hideTip);

  // --------------------------------------------------------------------- emphasis
  let selected = null;
  let hovered = null;
  let found = null;
  // The one function that paints what the reader is attending to: the selection if there
  // is one, else the node under the pointer, else what the search found, else nothing.
  // Every handler below changes the state and calls this, so no two of them can disagree.
  // It is one batched pass per change, so the drawing restyles once and not per class; the
  // library still re-evaluates every element the pass names, which on the largest graph
  // (1442 nodes, 3254 edges) measures about 5ms, well inside a hover.
  // A search that matched nothing dims everything, under the frame's "Nothing matches".
  const light = () => {
    const node = selected || hovered;
    lit = null;
    let shown = null;
    if (node) {
      lit = node.closedNeighborhood();
      shown = lit;
    } else if (found) {
      lit = found;
      shown = found.union(found.connectedEdges());
    }
    cy.batch(() => {
      cy.elements('.mj-near, .mj-focus, .mj-named, .mj-match').removeClass(
        'mj-near mj-focus mj-named mj-match',
      );
      if (shown) {
        shown.removeClass('mj-dim');
        cy.elements().not(shown).addClass('mj-dim');
      } else {
        cy.elements('.mj-dim').removeClass('mj-dim');
      }
      if (node) {
        node.connectedEdges().addClass('mj-named');
        node.addClass(selected ? 'mj-focus' : 'mj-near');
      } else if (found) {
        found.addClass('mj-match');
      }
    });
    decideLabels();
  };
  /** Select one node: ring it, light its neighbourhood, and describe it under the frame. */
  const select = (node) => {
    selected = node;
    light();
    describe(stage, node.data());
  };
  /** Let go of the selection and of the drawer describing it; the caller repaints. */
  const forget = () => {
    selected = null;
    const drawer = document.querySelector('[data-mj-graph-details]');
    if (drawer) drawer.remove();
  };
  const clearSelection = () => {
    forget();
    light();
  };

  // Leaving a node lets the picture go a moment later, so a pointer travelling from one node
  // to the next moves the light across rather than flashing the whole graph back on first.
  let leaving = 0;
  cy.on('mouseover', 'node', (event) => {
    clearTimeout(leaving);
    hovered = event.target;
    if (!selected) light();
    showTip(event.target);
  });
  cy.on('mouseout', 'node', () => {
    hovered = null;
    hideTip();
    clearTimeout(leaving);
    leaving = setTimeout(() => {
      if (!hovered && !selected) light();
    }, quick || 60);
  });
  // The library's mousedown takes focus off the page, so a click would select a node the
  // keyboard cannot then open; the frame takes focus back, and Enter means what it says.
  cy.on('tap', 'node', (event) => {
    select(event.target);
    frame.focus({ preventScroll: true });
  });
  cy.on('dbltap', 'node', (event) => {
    const route = event.target.data('route');
    if (route) window.location.assign(route);
  });
  cy.on('mouseover', 'edge', (event) => event.target.addClass('mj-named'));
  cy.on('mouseout', 'edge', (event) => {
    const node = selected || hovered;
    if (node && event.target.connectedNodes().contains(node)) return;
    event.target.removeClass('mj-named');
  });
  cy.on('tap', (event) => {
    if (event.target === cy) clearSelection();
  });

  // ---------------------------------------------------------------------- controls
  const fit = document.querySelector('[data-mj-graph-fit]');
  if (fit) {
    fit.addEventListener('click', () => fitTo(cy.elements(':visible'), 24));
    // − and + beside Fit, one segmented group; built here because they act on the drawing
    // and are no use to a page without one
    const group = document.createElement('div');
    group.className = 'mj-graph-zoom';
    group.setAttribute('role', 'group');
    group.setAttribute('aria-label', 'Zoom');
    const button = (text, label, handler) => {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'mj-button mj-button--quiet';
      b.setAttribute('aria-label', label);
      b.textContent = text;
      b.addEventListener('click', handler);
      return b;
    };
    fit.parentNode.insertBefore(group, fit);
    group.append(
      button('−', 'Zoom out', () => zoomBy(0.8)),
      button('+', 'Zoom in', () => zoomBy(1.25)),
      fit,
    );
  }

  const search = document.querySelector('[data-mj-graph-search]');
  let count = null;
  // what Escape on the drawing does once nothing is selected: the search lets go too
  let clearSearch = () => false;
  // What the search finds among the nodes still drawn, and the count of it. A search and a
  // filter both ask it again, so the count, the note and the light describe one picture.
  let match = () => {};
  if (search) {
    count = document.createElement('span');
    count.className = 'mj-graph-count';
    count.setAttribute('aria-live', 'polite');
    search.insertAdjacentElement('afterend', count);
    match = () => {
      const needle = search.value.trim().toLowerCase();
      if (!needle) {
        found = null;
        count.textContent = '';
        return;
      }
      const pool = cy.nodes(':visible');
      // busiest first, so Enter takes the match a reader most likely meant
      found = pool
        .filter((n) => (n.data('label') + ' ' + n.data('summary')).toLowerCase().includes(needle))
        .sort((a, b) => b.data('weight') - a.data('weight'));
      // the wording every graph uses, which the site's interaction spec reads
      count.textContent = found.length + ' of ' + pool.length;
    };
    const find = () => {
      match();
      if (!found) {
        nothing.hidden = cy.nodes(':visible').length > 0;
        light();
        return;
      }
      nothing.hidden = found.length > 0;
      // a search replaces the selection: what is lit is what matches
      forget();
      light();
      if (found.length) fitTo(found, 48);
    };
    let typing = 0;
    clearSearch = () => {
      clearTimeout(typing);
      if (!search.value && !found) return false;
      search.value = '';
      find();
      return true;
    };
    search.addEventListener('input', () => {
      clearTimeout(typing);
      typing = setTimeout(find, 150);
    });
    search.addEventListener('keydown', (event) => {
      if (event.key === 'Enter') {
        clearTimeout(typing);
        find();
        if (found && found.length) {
          select(found[0]);
          frame.focus();
        }
      } else if (event.key === 'Escape') {
        clearSearch();
      } else {
        return;
      }
      event.preventDefault();
    });
  }

  // -------------------------------------------------------------------------- map
  // A map of the whole for a graph too big to see whole at a legible zoom: one picture
  // per layout or filter, and a rectangle for the view moved once per burst of viewport
  // events. Nothing runs while the reader is idle. Fit is its accessible equivalent.
  const map =
    busy && hovers && window.matchMedia('(min-width: 640px)').matches ? miniature(cy, stage) : null;
  const drawMap = () => {
    if (map) map.draw(read.background);
  };

  // ------------------------------------------------------------------------ legend
  // A hidden node can be neither selected nor found: the selection lets go, drawer and all,
  // and the search is asked again over what is drawn now, so a kind shown again is lit again.
  const filtered = () => {
    if (selected && !selected.visible()) forget();
    match();
    light();
    const left = cy.elements(':visible');
    nothing.hidden = left.length > 0 && !(found && !found.length);
    // never refit to an empty set: the reader's last view stays under the note
    if (left.length) fitTo(left, 24);
    drawMap();
  };
  legend(stage, cy, kinds, graph, colourOf, filtered);

  // ---------------------------------------------------------------------- keyboard
  // One key map with the trains and the site's diagrams. The server renders the frame as an
  // image; once there is a drawing to operate it becomes one, keeping the server's label.
  const keys = document.createElement('p');
  keys.className = 'mj-note';
  keys.id = 'mj-graph-keys';
  keys.textContent =
    'With the drawing focused: the arrows move between neighbouring nodes, Shift and an ' +
    'arrow pans, + and − zoom, 0 fits, Home and End go to the busiest and the quietest ' +
    'node, Enter opens the selected node and Escape clears.';
  frame.setAttribute('tabindex', '0');
  frame.setAttribute('role', 'application');
  frame.setAttribute('aria-roledescription', 'graph');
  frame.setAttribute('aria-describedby', keys.id);
  frame.setAttribute(
    'aria-keyshortcuts',
    'ArrowUp ArrowDown ArrowLeft ArrowRight Shift+ArrowUp Shift+ArrowDown Shift+ArrowLeft ' +
      'Shift+ArrowRight Plus Minus 0 Home End Enter Escape',
  );
  frame.addEventListener('keydown', (event) => {
    // the browser's own chords (Ctrl or Cmd with + − 0 zoom the page) stay the browser's
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const key = event.key;
    const arrow = ARROWS[key];
    if (arrow && event.shiftKey) {
      moved = true;
      cy.panBy({ x: -arrow[0] * PAN_STEP, y: -arrow[1] * PAN_STEP });
    } else if (arrow) {
      const next = selected ? steer(cy, selected, arrow) : centremost(cy);
      if (next) {
        select(next);
        reveal(next);
      }
    } else if (key === 'Home' || key === 'End') {
      const order = cy.nodes(':visible').sort((a, b) => b.data('weight') - a.data('weight'));
      if (order.length) {
        const pick = key === 'Home' ? order[0] : order[order.length - 1];
        select(pick);
        reveal(pick);
      }
    } else if (key === '+' || key === '=') {
      zoomBy(1.25);
    } else if (key === '-' || key === '_') {
      zoomBy(0.8);
    } else if (key === '0') {
      fitTo(cy.elements(':visible'), 24);
    } else if (key === 'Enter') {
      if (selected && selected.data('route')) window.location.assign(selected.data('route'));
    } else if (key === 'Escape') {
      hideTip();
      // one press lets go of one thing: the selection first, then the search
      if (selected) clearSelection();
      else clearSearch();
    } else {
      return;
    }
    event.preventDefault();
  });

  // ---------------------------------------------------------------------- the theme
  // The page's theme is a class on <html>. When it changes every token is read again and the
  // same sheet is handed back to the drawing, in place: no layout, no replayed entrance.
  const restyle = () => {
    read = palette();
    series = seriesColours(read);
    colours = drawable(read);
    softs = softFills(series, read);
    type = typeScale();
    cy.style().fromJson(styleSheet()).update();
    drawMap();
  };
  new MutationObserver(restyle).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ['class'],
  });

  // A frame that changes size refits, unless the reader has moved the view since the last
  // fit; one decision per animation frame, however many resizes the browser reports.
  let resizing = 0;
  new ResizeObserver(() => {
    cancelAnimationFrame(resizing);
    resizing = requestAnimationFrame(() => {
      cy.resize();
      if (!moved) cy.fit(cy.elements(':visible'), 24);
    });
  }).observe(frame);

  // the layout has run by the time this fires; fit what it actually drew
  cy.ready(() => {
    cy.fit(cy.elements(), 24);
    decideLabels();
    drawMap();
  });

  // under the legend, the drawer and the drawing: the last thing the card says
  const last = document.querySelector('[data-mj-graph-legend]') || stage;
  last.insertAdjacentElement('afterend', keys);

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

// ---------------------------------------------------------------------- the keyboard

/**
 * The nearest visible node in an arrow's direction, within `STEER_DEGREES` of it. Nodes in
 * and around the view are asked first, so a step costs what is on screen and not the whole
 * graph; only when nothing there lies that way is every visible node asked.
 */
function steer(cy, from, arrow) {
  const p = from.position();
  const view = cy.extent();
  const margin = Math.max(view.w, view.h) / 2;
  const spread = Math.tan((STEER_DEGREES * Math.PI) / 180);
  const best = (near) => {
    let pick = null;
    let score = Infinity;
    cy.nodes(':visible').forEach((n) => {
      if (n.id() === from.id()) return;
      const q = n.position();
      const off =
        q.x < view.x1 - margin ||
        q.x > view.x2 + margin ||
        q.y < view.y1 - margin ||
        q.y > view.y2 + margin;
      if (near && off) return;
      const dx = q.x - p.x;
      const dy = q.y - p.y;
      const along = dx * arrow[0] + dy * arrow[1];
      if (along <= 0) return;
      const across = Math.abs(dx * arrow[1] - dy * arrow[0]);
      if (across > along * spread) return;
      // straight ahead is preferred to the same distance off to one side
      const s = along + 2 * across;
      if (s < score) {
        score = s;
        pick = n;
      }
    });
    return pick;
  };
  return best(true) || best(false);
}

/** Where the keyboard starts when nothing is selected: the visible node nearest the centre. */
function centremost(cy) {
  const view = cy.extent();
  const midX = (view.x1 + view.x2) / 2;
  const midY = (view.y1 + view.y2) / 2;
  let pick = null;
  let best = Infinity;
  cy.nodes(':visible').forEach((n) => {
    const q = n.position();
    const d = (q.x - midX) * (q.x - midX) + (q.y - midY) * (q.y - midY);
    if (d < best) {
      best = d;
      pick = n;
    }
  });
  return pick;
}

// --------------------------------------------------------------------------- the map

/**
 * A picture of the whole drawing at the frame's corner, with the view drawn on it. The
 * picture is taken once per layout, filter or theme; the rectangle moves in one animation
 * frame per burst of viewport events and never on a timer. A click on it pans there.
 */
function miniature(cy, stage) {
  const box = document.createElement('div');
  box.className = 'mj-graph-mapbox';
  box.setAttribute('aria-hidden', 'true');
  box.hidden = true;
  const img = document.createElement('img');
  img.className = 'mj-graph-map';
  img.alt = '';
  const view = document.createElement('div');
  view.className = 'mj-graph-map-view';
  box.append(img, view);
  stage.append(box);

  let bounds = null;
  let scale = 0;
  const place = () => {
    if (!bounds || !scale) return;
    const e = cy.extent();
    const w = img.width;
    const h = img.height;
    const x1 = Math.max(0, (e.x1 - bounds.x1) * scale);
    const y1 = Math.max(0, (e.y1 - bounds.y1) * scale);
    const x2 = Math.min(w, (e.x2 - bounds.x1) * scale);
    const y2 = Math.min(h, (e.y2 - bounds.y1) * scale);
    view.style.transform = 'translate(' + Math.round(x1) + 'px, ' + Math.round(y1) + 'px)';
    view.style.width = Math.max(4, Math.round(x2 - x1)) + 'px';
    view.style.height = Math.max(4, Math.round(y2 - y1)) + 'px';
  };
  let queued = 0;
  cy.on('viewport', () => {
    if (queued) return;
    queued = requestAnimationFrame(() => {
      queued = 0;
      place();
    });
  });
  img.addEventListener('load', () => {
    box.hidden = false;
    // the picture's own width over the drawing's: read once per picture, not per frame
    scale = img.width / Math.max(bounds.w, 1);
    place();
  });
  img.addEventListener('click', (event) => {
    if (!bounds || !scale) return;
    const x = bounds.x1 + event.offsetX / scale;
    const y = bounds.y1 + event.offsetY / scale;
    const pan = { x: cy.width() / 2 - x * cy.zoom(), y: cy.height() / 2 - y * cy.zoom() };
    cy.stop(true, false);
    if (motionAllowed()) cy.animate({ pan, duration: motionMs() * 2, easing: 'ease-out-cubic' });
    else cy.pan(pan);
  });

  return {
    draw: (ground) => {
      const shown = cy.elements(':visible');
      if (!shown.length) {
        box.hidden = true;
        return;
      }
      bounds = shown.boundingBox();
      img.src = cy.png({ full: true, maxWidth: 160, maxHeight: 120, bg: ground });
    },
  };
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
    // vocabulary as bands and finds the hubs at the front of each. By code unit, not by
    // `localeCompare`: every comparator in this repository orders by code unit, so that the
    // drawing a reader sees does not depend on the locale of the machine rendering it
    // (`scripts/ci/order-check`, question 3).
    sort: (a, b) => byCodeUnit(a.data('kind'), b.data('kind')) || b.data('weight') - a.data('weight'),
  };
}

/**
 * Order two strings by code unit. The repository's one ordering rule: a comparator orders by
 * code unit and never by the reader's locale, so the same tree renders the same everywhere.
 */
function byCodeUnit(a, b) {
  if (a === b) return 0;
  return a < b ? -1 : 1;
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
  // a value the canvas cannot parse leaves its last fill in place, which would read back as
  // black; a mix an older engine does not know is the fallback instead
  if (window.CSS && CSS.supports && !CSS.supports('color', value)) return fallback;
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
    // one token in the forms its readers need: the canvas needs channels, and the legend's
    // swatch is painted with the token's name, so it follows a theme change unredrawn
    const rgb = channels(value, channels(colours.accent, [0, 0, 0]));
    found.push({ css: value, name: '--mj-series-' + n, rgb });
  }
  return found.length
    ? found
    : [{ css: colours.accent, name: '--mj-accent', rgb: channels(colours.accent, [0, 0, 0]) }];
}

/**
 * The hub's fill: each series mixed 14% into the page's raised ground, by the browser, and
 * measured back like any token. Soft enough that the ring and the caption carry the node,
 * and still the kind's own hue. Where the browser cannot mix, the hub is the solid colour.
 */
function softFills(series, colours) {
  const mix = (value) => 'color-mix(in oklch, ' + value + ' 14%, ' + colours.surface + ')';
  return series.map((s) => channels(mix(s.css), s.rgb));
}

/**
 * The declared motion step in milliseconds, whichever unit the stylesheet wrote it in (the
 * compiled sheet minifies `150ms` to `.15s`). Missing, it is no motion at all.
 */
function motionMs() {
  const root = getComputedStyle(document.documentElement);
  const value = root.getPropertyValue('--mj-motion-fast').trim();
  const n = parseFloat(value);
  if (!Number.isFinite(n) || n < 0) return 0;
  return /ms$/.test(value) ? n : n * 1000;
}

/** A kind's swatch: the declared token itself, so CSS resolves it in either theme. */
function swatchOf(colour) {
  const swatch = document.createElement('span');
  swatch.className = 'mj-graph-swatch';
  swatch.style.background = 'var(' + colour.name + ')';
  return swatch;
}

/**
 * The declared type scale and mono family, for a canvas that cannot be given a CSS class.
 * `font-size: 9` was a size the scale does not name; `meta` at 10px is the smallest step it
 * does. The family is `--font-mono`, read once, so the canvas sets identifiers in the face
 * the tables beside it use rather than a font stack copied into this file.
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
  const mono =
    root.getPropertyValue('--font-mono').trim() || getComputedStyle(document.body).fontFamily;
  return { meta, mono };
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
 * a swatch per kind with the number of nodes that carry it, and a press that hides that
 * kind and refits what is left (`filtered`). Pointing at a chip previews it: every node of
 * another kind steps back until the pointer leaves.
 *
 * It is built here rather than rendered by the server because it is a legend *of the
 * drawing*: with no drawing there is nothing to explain, and the page already names every
 * kind and every edge kind in "What the shapes mean" for a reader who has no JavaScript.
 * `aria-pressed="true"` means the kind is out of the picture, as on the trains.
 */
function legend(anchor, cy, kinds, graph, colourOf, filtered) {
  const host = document.createElement('div');
  host.className = 'mj-graph-legend';
  host.setAttribute('data-mj-graph-legend', '');
  host.setAttribute('role', 'group');
  host.setAttribute('aria-label', 'Node kinds: press one to hide or show it');

  const counts = new Map();
  for (const n of graph.nodes) counts.set(n.kind, (counts.get(n.kind) || 0) + 1);
  // busiest kind first: the reader's eye wants the thing most of the drawing is made of
  const ordered = kinds.slice().sort((a, b) => (counts.get(b) || 0) - (counts.get(a) || 0));
  const hidden = new Set();
  const ofKind = (kind) => cy.nodes().filter((n) => n.data('kind') === kind);

  for (const kind of ordered) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'mj-graph-legend-entry';
    button.setAttribute('aria-pressed', 'false');
    button.title = (graph.node_kinds[kind] || kind) + ' — press to hide or show this kind';

    const name = document.createElement('span');
    name.className = 'mj-graph-legend-name';
    name.textContent = kind;
    const count = document.createElement('span');
    count.className = 'mj-graph-legend-count';
    count.textContent = String(counts.get(kind) || 0);
    // the DOM reads the declaration's own token; only the canvas needs channels
    button.append(swatchOf(colourOf(kind)), name, count);

    button.addEventListener('pointerenter', () => {
      if (hidden.has(kind)) return;
      const of = ofKind(kind);
      cy.batch(() => cy.elements().not(of).not(of.connectedEdges()).addClass('mj-aside'));
    });
    button.addEventListener('pointerleave', () => {
      cy.batch(() => cy.elements('.mj-aside').removeClass('mj-aside'));
    });
    button.addEventListener('click', () => {
      if (hidden.has(kind)) hidden.delete(kind);
      else hidden.add(kind);
      button.setAttribute('aria-pressed', hidden.has(kind) ? 'true' : 'false');
      button.classList.toggle('is-hidden', hidden.has(kind));
      cy.batch(() => {
        cy.elements('.mj-aside').removeClass('mj-aside');
        const of = ofKind(kind);
        if (hidden.has(kind)) of.addClass('mj-hidden');
        else of.removeClass('mj-hidden');
      });
      filtered();
    });
    host.append(button);
  }

  anchor.insertAdjacentElement('afterend', host);
}

/**
 * A small drawer under the graph: what the selected node is, and where it lives. A polite
 * live region, so a reader moving the selection with the arrows hears where they are.
 */
function describe(anchor, data) {
  let drawer = document.querySelector('[data-mj-graph-details]');
  if (!drawer) {
    drawer = document.createElement('div');
    drawer.setAttribute('data-mj-graph-details', '');
    drawer.setAttribute('aria-live', 'polite');
    drawer.className = 'mj-alert mj-alert--info';
    anchor.insertAdjacentElement('afterend', drawer);
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
