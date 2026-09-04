// Interactive graph rendering for the derived graph data (architecture.json, claims-graph.json).
//
// Progressive enhancement, in the strict sense: the page ships the whole graph as a rendered
// list of nodes and their edges, which is what a reader without JavaScript, a crawler, and a
// screen reader get. This script only adds a second view of the same data. If Cytoscape fails
// to load, the list stays and nothing is lost.
//
// The library is 400 kB, so it is not in the base layout: it is fetched the first time a graph
// scrolls into view, once per page, and never on a page without one.
(function () {
  var containers = document.querySelectorAll('[data-graph]');
  if (!containers.length) { return; }

  var reduced = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var loading = null;

  function loadCytoscape(src) {
    if (window.cytoscape) { return Promise.resolve(window.cytoscape); }
    if (loading) { return loading; }
    loading = new Promise(function (resolve, reject) {
      var s = document.createElement('script');
      s.src = src;
      s.onload = function () { resolve(window.cytoscape); };
      s.onerror = function () { reject(new Error('cytoscape did not load')); };
      document.head.appendChild(s);
    });
    return loading;
  }

  // Palette read from the page, not duplicated here: the graph uses the same tokens as the
  // rest of the site, so the theme toggle moves it without a second colour table.
  function palette() {
    var s = getComputedStyle(document.documentElement);
    var dark = document.documentElement.classList.contains('dark');
    function v(name, fallback) { var x = s.getPropertyValue(name).trim(); return x || fallback; }
    return {
      dark: dark,
      text: v('--mj-graph-text', dark ? '#e5e7eb' : '#111827'),
      muted: v('--mj-graph-muted', dark ? '#9ca3af' : '#6b7280'),
      line: v('--mj-graph-line', dark ? '#4b5563' : '#d1d5db'),
      surface: v('--mj-graph-surface', dark ? '#1f2937' : '#ffffff'),
      accent: v('--mj-graph-accent', dark ? '#818cf8' : '#4f46e5'),
      warn: v('--mj-graph-warn', dark ? '#fbbf24' : '#b45309'),
      good: v('--mj-graph-good', dark ? '#34d399' : '#047857')
    };
  }

  var KIND_COLOUR = {
    entry: 'accent', module: 'text', artifact: 'good',
    claim: 'accent', source: 'text', implementation: 'muted', test: 'good'
  };

  function stylesheet(p) {
    return [
      { selector: 'node', style: {
        'background-color': p.surface,
        'border-width': 1.5,
        'border-color': function (n) { return p[KIND_COLOUR[n.data('kind')] || 'muted']; },
        'label': 'data(label)',
        'color': p.text,
        'font-size': 11,
        'font-family': 'ui-monospace, SFMono-Regular, Menlo, monospace',
        'text-valign': 'center',
        'text-halign': 'center',
        'text-wrap': 'wrap',
        'text-max-width': 120,
        'padding': 6,
        'shape': 'round-rectangle',
        'width': 'label',
        'height': 'label'
      } },
      { selector: 'node[?dimmed]', style: { 'opacity': 0.2 } },
      { selector: 'node:selected', style: { 'border-width': 3, 'border-color': p.accent, 'color': p.accent } },
      { selector: 'edge', style: {
        'width': 1.2,
        'line-color': p.line,
        'target-arrow-color': p.line,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.7,
        'curve-style': 'bezier',
        'opacity': 0.75
      } },
      { selector: 'edge[kind = "writes"]', style: { 'line-color': p.warn, 'target-arrow-color': p.warn, 'width': 1.8 } },
      { selector: 'edge[kind = "uses"]', style: { 'line-style': 'dashed' } },
      { selector: 'edge[?dimmed]', style: { 'opacity': 0.06 } },
      { selector: 'edge[?focus]', style: { 'opacity': 1, 'width': 2.4, 'line-color': p.accent, 'target-arrow-color': p.accent } }
    ];
  }

  function mount(el, cytoscape) {
    var payload = document.getElementById(el.getAttribute('data-graph'));
    if (!payload) { return; }
    var data;
    try { data = JSON.parse(payload.textContent); } catch (e) { return; }
    if (!data.nodes || !data.nodes.length) { return; }

    var base = (el.getAttribute('data-graph-base') || '').replace(/\/$/, '');
    var canvas = el.querySelector('[data-graph-canvas]');
    var detail = el.querySelector('[data-graph-detail]');
    if (!canvas) { return; }

    var byId = {};
    data.nodes.forEach(function (n) { byId[n.id] = n; });
    var elements = data.nodes.map(function (n) { return { group: 'nodes', data: n }; })
      .concat(data.edges.filter(function (e) { return byId[e.source] && byId[e.target]; })
        .map(function (e) { return { group: 'edges', data: { id: e.source + '|' + e.target + '|' + e.kind, source: e.source, target: e.target, kind: e.kind } }; }));

    var p = palette();
    var cy = cytoscape({
      container: canvas,
      elements: elements,
      style: stylesheet(p),
      layout: {
        name: 'cose',
        animate: false,          // deterministic and cheap; also what reduced motion wants
        randomize: false,        // same input, same picture, every build
        idealEdgeLength: 90,
        nodeOverlap: 12,
        padding: 24,
        componentSpacing: 80,
        nestingFactor: 0.8,
        gravity: 0.4,
        numIter: reduced ? 400 : 1200
      },
      minZoom: 0.3,
      maxZoom: 2.5,
      wheelSensitivity: 0.2,
      autoungrabify: false
    });

    function describe(node) {
      if (!detail) { return; }
      var d = node.data();
      var parts = ['<p class="font-mono text-sm text-heading break-words">' + esc(d.label) + '</p>'];
      parts.push('<p class="mt-1 text-xs uppercase tracking-wide text-body-subtle">' + esc(d.kind) + (d.status ? ' · ' + esc(d.status) : '') + '</p>');
      if (d.summary) { parts.push('<p class="mt-2 text-sm text-body">' + esc(d.summary) + '</p>'); }
      if (d.note) { parts.push('<p class="mt-2 text-sm text-body">' + esc(d.note) + '</p>'); }
      var out = node.outgoers('edge'), inc = node.incomers('edge');
      if (out.length || inc.length) {
        parts.push('<ul class="mt-3 space-y-1 text-xs text-body-subtle">');
        out.forEach(function (e) { parts.push('<li><span class="font-mono text-heading">' + esc(e.data('kind')) + '</span> → ' + esc(e.target().data('label')) + '</li>'); });
        inc.forEach(function (e) { parts.push('<li>' + esc(e.source().data('label')) + ' <span class="font-mono text-heading">' + esc(e.data('kind')) + '</span> → this</li>'); });
        parts.push('</ul>');
      }
      if (d.route) {
        var href = d.external ? d.route : base + d.route;
        parts.push('<a class="mt-3 inline-block text-sm font-medium text-primary-700 underline dark:text-primary-400" href="' + esc(href) + '">Open the page for this</a>');
      }
      detail.innerHTML = parts.join('');
    }

    function clearFocus() {
      cy.elements().removeData('dimmed').removeData('focus');
      if (detail) { detail.innerHTML = detail.getAttribute('data-empty') || ''; }
    }

    function focus(node) {
      var keep = node.closedNeighborhood();
      cy.elements().data('dimmed', true);
      keep.removeData('dimmed');
      keep.edges().data('focus', true);
      describe(node);
    }

    cy.on('tap', 'node', function (evt) { focus(evt.target); });
    cy.on('tap', function (evt) { if (evt.target === cy) { cy.elements().unselect(); clearFocus(); } });

    // filters: one checkbox per edge kind present, rendered by the template from the data
    el.querySelectorAll('[data-graph-filter]').forEach(function (input) {
      input.addEventListener('change', function () {
        var off = [];
        el.querySelectorAll('[data-graph-filter]').forEach(function (i) { if (!i.checked) { off.push(i.getAttribute('data-graph-filter')); } });
        cy.edges().forEach(function (e) { e.style('display', off.indexOf(e.data('kind')) === -1 ? 'element' : 'none'); });
      });
    });

    var reset = el.querySelector('[data-graph-reset]');
    if (reset) { reset.addEventListener('click', function () { cy.elements().unselect(); clearFocus(); cy.fit(undefined, 24); }); }

    // the fallback list and the canvas are the same graph: clicking a name selects the node
    el.querySelectorAll('[data-graph-node]').forEach(function (a) {
      a.addEventListener('click', function (ev) {
        var n = cy.getElementById(a.getAttribute('data-graph-node'));
        if (!n || !n.length) { return; }
        ev.preventDefault();
        n.select(); focus(n); cy.center(n);
      });
    });

    var toggle = document.getElementById('theme-toggle');
    if (toggle) { toggle.addEventListener('click', function () { setTimeout(function () { cy.style(stylesheet(palette())); }, 0); }); }

    canvas.setAttribute('data-graph-ready', '1');
    return cy;
  }

  function esc(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }

  var src = document.body.getAttribute('data-cytoscape-src');
  if (!src) { return; }

  function start(el) {
    loadCytoscape(src).then(function (cytoscape) { mount(el, cytoscape); }).catch(function () {
      var note = el.querySelector('[data-graph-note]');
      if (note) { note.hidden = false; }         // the list below is the graph; say so and stop
    });
  }

  if (!('IntersectionObserver' in window)) {
    containers.forEach(start);
    return;
  }
  var io = new IntersectionObserver(function (entries) {
    entries.forEach(function (e) {
      if (!e.isIntersecting) { return; }
      io.unobserve(e.target);
      start(e.target);
    });
  }, { rootMargin: '200px' });
  containers.forEach(function (el) { io.observe(el); });
})();
