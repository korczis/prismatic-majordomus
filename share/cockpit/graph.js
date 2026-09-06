// The graph viewer. The nodes and edges come from /api/v1/graph, which is the canonical
// Rust model; this file translates that model into Cytoscape's own shape at the boundary
// and nowhere else, so a different renderer would need no change on the server.
//
// Progressive enhancement in the strict sense: the page already lists every node and every
// edge as a table. This adds a second view of the same data. If Cytoscape is not vendored,
// or fails, the tables stay and nothing is lost.

import { api, vendor, palette, explainMissing, motionAllowed } from './cockpit.js';

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
  const colours = palette();

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
      },
    })),
    ...graph.edges.map((e, i) => ({
      data: { id: 'e' + i, source: e.source, target: e.target, label: e.kind },
    })),
  ];

  // one hue per node kind, taken from the page's own accent by rotation: the vocabulary is
  // the graph's, so the legend cannot go out of step with the drawing
  const kinds = Object.keys(graph.node_kinds);
  const hue = (kind) => {
    const at = kinds.indexOf(kind);
    return 'hsl(' + Math.round((at / Math.max(kinds.length, 1)) * 300 + 200) + ' 55% 55%)';
  };

  const cy = cytoscape({
    container: frame,
    elements,
    layout: {
      name: 'breadthfirst',
      directed: true,
      padding: 24,
      spacingFactor: 1.1,
      animate: false,
    },
    style: [
      {
        selector: 'node',
        style: {
          'background-color': (n) => hue(n.data('kind')),
          'border-width': (n) => (n.data('external') === 'yes' ? 2 : 0),
          'border-style': 'dashed',
          'border-color': colours.muted,
          label: 'data(label)',
          color: colours.text,
          'font-size': 9,
          'font-family': 'ui-monospace, SFMono-Regular, Menlo, monospace',
          'text-valign': 'bottom',
          'text-margin-y': 3,
          'text-wrap': 'ellipsis',
          'text-max-width': '120px',
          width: 12,
          height: 12,
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
      {
        selector: '.mj-dim',
        style: { opacity: 0.12 },
      },
      {
        selector: '.mj-focus',
        style: { 'border-width': 3, 'border-style': 'solid', 'border-color': colours.accent },
      },
    ],
  });

  if (!motionAllowed()) cy.autounselectify(false);

  // selection: focus the neighbourhood and show what the node is
  cy.on('tap', 'node', (event) => {
    const node = event.target;
    cy.elements().addClass('mj-dim');
    node.closedNeighborhood().removeClass('mj-dim');
    cy.elements('node').removeClass('mj-focus');
    node.addClass('mj-focus');
    describe(frame, node.data());
  });
  cy.on('tap', (event) => {
    if (event.target === cy) {
      cy.elements().removeClass('mj-dim').removeClass('mj-focus');
      const drawer = document.querySelector('[data-mj-graph-details]');
      if (drawer) drawer.remove();
    }
  });

  const fit = document.querySelector('[data-mj-graph-fit]');
  if (fit) fit.addEventListener('click', () => cy.fit(undefined, 24));

  const search = document.querySelector('[data-mj-graph-search]');
  if (search) {
    search.addEventListener('input', () => {
      const needle = search.value.trim().toLowerCase();
      if (!needle) {
        cy.elements().removeClass('mj-dim');
        return;
      }
      cy.elements().addClass('mj-dim');
      const matching = cy
        .nodes()
        .filter((n) => (n.data('label') + ' ' + n.data('summary')).toLowerCase().includes(needle));
      matching.removeClass('mj-dim');
      matching.connectedEdges().removeClass('mj-dim');
      if (matching.length) cy.fit(matching, 48);
    });
  }
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
