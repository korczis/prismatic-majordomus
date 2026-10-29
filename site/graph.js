// Interactive graph rendering for the derived graph data (architecture.json, claims-graph.json).
//
// Progressive enhancement, in the strict sense: the page ships the whole graph as a rendered
// list of nodes and their edges, which is what a reader without JavaScript, a crawler, and a
// screen reader get. This script only adds a second view of the same data. If Cytoscape fails
// to load, the list stays and nothing is lost. Where a page links to the list instead of
// carrying it (partials/graph.html's graph_src), the link is what stays.
//
// The library is 400 kB, so it is not in the base layout: it is fetched the first time a graph
// scrolls into view, once per page, and never on a page without one.
//
// What a reader can do with the drawing, all of it reachable from the keyboard as well:
//   - point at a node: it and its neighbours stay lit, everything else steps back, a card
//     names it, and its edges are drawn in the accent and say what they are
//   - select a node (a tap, a name in the list, the arrow keys): the panel beside the drawing
//     describes it, in its own aria-live region
//   - find a node by name, kind or summary; zoom with the buttons, a pinch, or Ctrl/Cmd and the
//     wheel (a plain wheel scrolls the page, as it does over a diagram); Fit and Reset
//   - point at or focus an edge kind's chip to see that kind alone, and hide it with its box
// Seen whole, the drawing reads as a map: strokes keep their weight however far out the view
// stands, a node too small to carry its name is filled with its role instead, and Fit always
// takes in the whole graph. Selecting a node closes in until its name can be read.
// Interaction has one colour, the accent. Every colour is a token of the page read back from
// the page, and every size and duration a step of the declared scale.
(function () {
  var containers = document.querySelectorAll('[data-graph]');
  if (!containers.length) { return; }

  var root = document.documentElement;
  function media(q) { return Boolean(window.matchMedia && window.matchMedia(q).matches); }
  var reduced = media('(prefers-reduced-motion: reduce)');
  // a card that follows the pointer is no use under a finger, and covers what was touched
  var coarse = media('(pointer: coarse)');
  // past this many nodes a transition per element costs more than it shows, and a pan draws
  // a picture of the graph rather than every edge
  var LARGE = 400;
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

  function isDark() { return root.classList.contains(root.dataset.themeClass); }

  // A colour as six-digit sRGB hex, whatever notation the stylesheet declared it in: the
  // browser resolves it and one painted pixel is read back. The same reading site/charts.js
  // and site/diagrams.js take, for the same reason: the declared values are OKLCH, which
  // Cytoscape's own parser rejects (it logs the property invalid and paints its default, the
  // defect share/cockpit/graph.js documents), so the library is handed a colour it can parse,
  // and no colour is written in this file.
  var probe = document.createElement('span');
  var pixel = document.createElement('canvas');
  pixel.width = pixel.height = 1;
  var paint = pixel.getContext('2d', { willReadFrequently: true });
  function hex(cssColour) {
    if (!paint || !cssColour) { return ''; }
    paint.save();
    paint.clearRect(0, 0, 1, 1);
    paint.fillStyle = cssColour;
    paint.fillRect(0, 0, 1, 1);
    var px = paint.getImageData(0, 0, 1, 1).data;
    paint.restore();
    return '#' + [px[0], px[1], px[2]].map(function (c) { return (c < 16 ? '0' : '') + c.toString(16); }).join('');
  }
  function declared(name) { return Boolean(getComputedStyle(root).getPropertyValue(name).trim()); }
  // any CSS colour expression, resolved by the browser: a token, or a mix of tokens
  function resolve(expr) {
    probe.style.color = '';
    probe.style.color = expr;
    if (!probe.style.color) { return ''; }
    document.body.appendChild(probe);
    var resolved = getComputedStyle(probe).color;
    probe.remove();
    return hex(resolved);
  }
  function token(name) { return declared(name) ? resolve('var(' + name + ')') : ''; }
  // A derived colour: `pct` of a colour over a ground token, opaque, resolved by the browser.
  // The colour is a token's name or a colour already read back from the page; never a literal.
  function mix(colour, pct, ground) {
    var named = colour.indexOf('--') === 0;
    if (!declared(ground) || (named && !declared(colour))) { return named ? token(colour) : colour; }
    return resolve('color-mix(in oklch, ' + (named ? 'var(' + colour + ')' : colour) + ' ' + pct + '%, var(' + ground + '))');
  }

  // Type and time, from the declaration, never a number written here: a size is a step of the
  // scale, a duration a multiple of the one motion token.
  function font(name) {
    return getComputedStyle(root).getPropertyValue(name).trim() || getComputedStyle(document.body).fontFamily;
  }
  function length(name) {
    var raw = getComputedStyle(root).getPropertyValue(name).trim();
    var n = parseFloat(raw);
    if (!isFinite(n)) { return NaN; }
    return /rem$/.test(raw) ? n * parseFloat(getComputedStyle(root).fontSize) : n;
  }
  function size(name) {
    var n = length(name);
    if (!isFinite(n)) { n = length('--text-small'); }
    return isFinite(n) ? n : parseFloat(getComputedStyle(document.body).fontSize);
  }
  function motion() {
    if (reduced) { return 0; }
    var raw = getComputedStyle(root).getPropertyValue('--mj-motion-fast').trim();
    var n = parseFloat(raw);
    if (!isFinite(n)) { return 0; }
    return /ms$/.test(raw) ? n : n * 1000;
  }

  // Palette read from the page, not duplicated here: the graph uses the same tokens as the
  // rest of the site, so the theme toggle moves it without a second colour table. The
  // fallback is the page's own computed text colour, never a table.
  function palette() {
    var fallback = hex(getComputedStyle(document.body).color);
    function v(name) { return token(name) || fallback; }
    var p = {
      dark: isDark(),
      text: v('--mj-fg'),
      muted: v('--mj-muted'),
      line: v('--mj-line'),
      surface: v('--mj-raised'),
      ground: v('--mj-bg'),
      accent: v('--mj-accent'),
      warn: v('--mj-warn'),
      good: v('--mj-ok'),
      // an edge is recessive but must still be seen on the lightest ground, where the line
      // role alone (a hairline grey) all but vanishes: two fifths of the way to the muted ink
      edge: mix('--mj-muted', 40, '--mj-line') || v('--mj-line'),
      fills: {},
      states: {}
    };
    // a node's ground: its role at 14% over the raised surface, so the ring carries the role
    // and the label stands on a ground the contrast derivation already measured
    p.soft = function (colour) {
      if (!p.fills[colour]) { p.fills[colour] = mix(colour, 14, '--mj-raised') || p.surface; }
      return p.fills[colour];
    };
    // a node's fill when its label is too small to draw: the role itself, softened a fifth
    // toward the ground, since a status's badge ink is the theme's darkest step of it
    p.solid = function (colour) {
      var key = 'solid ' + colour;
      if (!p.fills[key]) { p.fills[key] = mix(colour, 80, '--mj-bg') || colour; }
      return p.fills[key];
    };
    return p;
  }

  var KIND_COLOUR = {
    entry: 'accent', module: 'text', artifact: 'good',
    claim: 'accent', source: 'text', implementation: 'muted', test: 'good'
  };

  function badgeClass(status) { return 'mj-badge--' + String(status).toLowerCase().replace(/[^a-z0-9-]+/g, '-'); }

  // A status is the colour the design files that word under, read from the badge the page
  // would draw for it, so nothing here maps a state to a colour. A word filed under no status
  // answers the badge's neutral ink, which is no status colour at all.
  function statusInk(p, host, status) {
    var key = badgeClass(status);
    if (Object.prototype.hasOwnProperty.call(p.states, key)) { return p.states[key]; }
    var badge = document.createElement('span');
    badge.className = 'mj-badge ' + key;
    badge.hidden = true;
    host.appendChild(badge);
    var ink = hex(getComputedStyle(badge).color);
    badge.remove();
    p.states[key] = ink && ink !== p.text ? ink : '';
    return p.states[key];
  }

  // A label smaller than this on screen is not drawn at all (Cytoscape's min-zoomed-font-size):
  // a word that small is a smudge, not a word.
  var LEGIBLE = 7;

  // `view` is how far out the reader stands: `k` multiplies every stroke so that a line keeps
  // the weight it has at full size however far out the fit or the reader goes (at a fitted 0.2
  // a hairline is a fifth of a pixel and the edges vanish), and `far` is true once the labels
  // are too small to draw, when a node is filled with its role outright so the picture still
  // reads as a map of colours and words come back as the reader closes in.
  function stylesheet(p, host, large, view) {
    var mono = font('--font-mono');
    var corner = length('--radius-md');
    function stroke(w) { return function () { return w * view.k; }; }
    // a state change eases at the motion token's pace, and not at all for a large graph or a
    // reader who asked for stillness
    var t = large ? 0 : motion();
    function eased(style) {
      style['transition-property'] = 'opacity, border-color, border-width, underlay-opacity, line-color, width';
      style['transition-duration'] = t;
      return style;
    }
    // The role is the datum's when it carries one: a graph that invents a class of node
    // should not also have to be taught its colour here. Then its status, when the design
    // files the word under one, and KIND_COLOUR for the graphs that predate both fields.
    function ink(n) {
      var role = n.data('role');
      if (role && p[role]) { return p[role]; }
      var status = n.data('status') ? statusInk(p, host, n.data('status')) : '';
      return status || p[KIND_COLOUR[n.data('kind')] || 'muted'];
    }
    return [
      { selector: 'node', style: eased({
        'background-color': function (n) { return view.far ? p.solid(ink(n)) : p.soft(ink(n)); },
        'border-width': stroke(1.5),
        'border-color': ink,
        'label': 'data(label)',
        'color': p.text,
        'font-size': size('--text-label'),
        'font-family': mono,
        'font-weight': 500,
        'min-zoomed-font-size': LEGIBLE,
        'text-valign': 'center',
        'text-halign': 'center',
        'text-wrap': 'wrap',
        'text-max-width': 110,
        'padding': 8,
        'shape': 'round-rectangle',
        'corner-radius': isFinite(corner) ? corner : 'auto',
        'width': 'label',
        'height': 'label',
        'underlay-color': p.accent,
        'underlay-opacity': 0,
        'underlay-padding': stroke(6),
        'underlay-shape': 'round-rectangle'
      }) },
      { selector: 'edge', style: eased({
        'width': stroke(1.2),
        'line-color': p.edge,
        'target-arrow-color': p.edge,
        'target-arrow-shape': 'triangle',
        'arrow-scale': 0.75,
        'curve-style': 'bezier',
        'opacity': 0.7,
        'font-family': mono,
        'font-size': size('--text-meta'),
        'color': p.text,
        'text-rotation': 'autorotate',
        'text-background-color': p.ground,
        'text-background-opacity': 0.9,
        'text-background-shape': 'roundrectangle',
        'text-background-padding': 2,
        'min-zoomed-font-size': 6
      }) },
      // a relation keeps the encoding that is not its colour: a dash, a dot, a weight
      { selector: 'edge[kind = "writes"]', style: { 'line-color': p.warn, 'target-arrow-color': p.warn, 'width': stroke(1.8) } },
      { selector: 'edge[kind = "uses"], edge[kind = "depends_on"]', style: { 'line-style': 'dashed' } },
      { selector: 'edge[kind = "satisfied"], edge[kind = "merged"]', style: { 'line-style': 'dotted' } },
      // a search's match is lit in the one interaction colour
      { selector: 'node[?match]', style: { 'border-color': p.accent, 'border-width': stroke(2) } },
      // stepping back: a node keeps its shape and loses its words, so what is lit reads alone
      { selector: 'node[?dimmed], node[?faded]', style: { 'opacity': 0.15, 'text-opacity': 0 } },
      { selector: 'edge[?dimmed], edge[?faded]', style: { 'opacity': 0.15, 'text-opacity': 0 } },
      // under the pointer the neighbourhood is lit, over any dim, and the node itself glows
      { selector: 'node[?hot]', style: { 'opacity': 1, 'text-opacity': 1 } },
      { selector: 'node[?pointed]', style: { 'underlay-opacity': 0.14 } },
      { selector: 'edge[?hot], edge[?focus]', style: {
        'opacity': 1, 'width': stroke(2.2), 'line-color': p.accent, 'target-arrow-color': p.accent,
        'label': 'data(kind)', 'z-index': 2
      } },
      // a selection outranks all of it: a ring and a held glow in the accent
      { selector: 'node:selected', style: {
        'opacity': 1, 'text-opacity': 1, 'border-width': stroke(2.5), 'border-color': p.accent, 'underlay-opacity': 0.22
      } }
    ];
  }

  // The data is the page's own <script type="application/json"> when it carries one, and the
  // published JSON named by data-graph-src when it does not: a page that only summarises a graph
  // (the homepage) fetches it on first sight rather than shipping it inline to every visitor.
  function graphData(el) {
    var payload = document.getElementById(el.getAttribute('data-graph'));
    if (payload) {
      try { return Promise.resolve(JSON.parse(payload.textContent)); } catch (e) { return Promise.reject(e); }
    }
    var url = el.getAttribute('data-graph-src');
    if (!url || !window.fetch) { return Promise.reject(new Error('no graph data')); }
    return fetch(url).then(function (r) {
      if (!r.ok) { throw new Error('graph data ' + r.status); }
      return r.json();
    });
  }

  function mount(el, cytoscape, data) {
    if (!data || !data.nodes || !data.nodes.length) { return; }

    var base = (el.getAttribute('data-graph-base') || '').replace(/\/$/, '');
    var canvas = el.querySelector('[data-graph-canvas]');
    var detail = el.querySelector('[data-graph-detail]');
    // the empty description is what the page was built with, in the page's own language: a
    // copy's translation pass rewrites the text a reader sees, not an attribute beside it
    var empty = detail ? detail.innerHTML : '';
    if (!canvas) { return; }

    var byId = {};
    data.nodes.forEach(function (n) { byId[n.id] = n; });
    var elements = data.nodes.map(function (n) { return { group: 'nodes', data: n }; })
      .concat(data.edges.filter(function (e) { return byId[e.source] && byId[e.target]; })
        .map(function (e) { return { group: 'edges', data: { id: e.source + '|' + e.target + '|' + e.kind, source: e.source, target: e.target, kind: e.kind } }; }));

    var large = data.nodes.length > LARGE;
    var theme = isDark();
    var p = palette();
    var view = { k: 1, far: false };
    var cy = cytoscape({
      container: canvas,
      elements: elements,
      style: stylesheet(p, el, large, view),
      layout: { name: 'preset' },   // the real layout is run below, once roots can be resolved
      // out far enough to see the whole of the largest graph, in close enough to read a label
      minZoom: 0.15,
      maxZoom: 3,
      wheelSensitivity: 0.2,
      autoungrabify: false,
      // a large graph pans as a picture of itself and draws its edges again when the hand stops
      hideEdgesOnViewport: large,
      textureOnViewport: large
    });

    // Where the data has a natural depth — entry, then modules, then the artifacts they name —
    // rings read far better in a wide short box than a three-row tree, which degenerates into
    // one long line. Everything else gets a force layout. Neither animates: deterministic and
    // cheap, and already what reduced motion asks for.
    var order = (el.getAttribute('data-graph-rings') || '').split(',').filter(Boolean);
    var opts = order.length
      ? { name: 'concentric', animate: false, padding: 12, avoidOverlap: true,
          minNodeSpacing: 14, equidistant: false, startAngle: -Math.PI / 2, fit: true,
          concentric: function (n) { var i = order.indexOf(n.data('kind')); return i === -1 ? 0 : order.length - i; },
          levelWidth: function () { return 1; } }
      : { name: 'cose', animate: false, randomize: false, idealEdgeLength: 120,
          nodeRepulsion: 9000, edgeElasticity: 60, nodeOverlap: 20, padding: 16,
          componentSpacing: 110, gravity: 0.3, numIter: reduced ? 500 : 1500, fit: true };
    cy.layout(opts).run();

    // Fitted until the reader zooms or pans: a resize of the responsive grid refits the drawing
    // while it is, and keeps the reader's own view once they have chosen one.
    var fitted = true;
    // A fit frames what it is given and comes no closer than half again the drawing's own
    // size: one search match, or a graph of three nodes, is shown at a reading size rather than
    // blown up to the zoom ceiling.
    var CLOSEST = 1.5;
    function settle(eles, padding, animated) {
      if (!eles.length) { return; }               // nothing to frame: the view stays where it is
      var bb = eles.boundingBox();
      var z = Math.min((cy.width() - 2 * padding) / bb.w, (cy.height() - 2 * padding) / bb.h);
      z = Math.max(cy.minZoom(), Math.min(CLOSEST, cy.maxZoom(), isFinite(z) ? z : CLOSEST));
      var to = { zoom: z, pan: { x: (cy.width() - z * (bb.x1 + bb.x2)) / 2, y: (cy.height() - z * (bb.y1 + bb.y2)) / 2 } };
      var t = animated ? motion() * 2 : 0;
      cy.stop(true, true);
      if (t) { cy.animate(to, { duration: t, easing: 'ease-out-cubic' }); } else { cy.viewport(to); }
    }
    // The floor is 0.15 unless the whole graph does not fit the frame at it (the claims graph's
    // 586 nodes need about 0.06): then it is as far out as fitting takes, so Fit always shows
    // everything and is never a view of one corner.
    var FLOOR = cy.minZoom();
    function floor() {
      var bb = cy.elements().boundingBox(), pad = 16;
      var need = Math.min((cy.width() - 2 * pad) / bb.w, (cy.height() - 2 * pad) / bb.h);
      cy.minZoom(isFinite(need) && need > 0 ? Math.min(FLOOR, need * 0.95) : FLOOR);
    }
    function fit(animated) { floor(); settle(cy.elements(':visible'), 16, animated); fitted = true; }

    // Strokes and fills follow the distance the reader stands at (see stylesheet()). Restyled
    // once the zoom has come to rest, and only when the weight moved by a tenth or the labels
    // came or went: a pinch is not a restyle per frame.
    var labelPx = size('--text-label');
    var restyle = 0;
    function scale() {
      restyle = 0;
      var z = cy.zoom();
      // three quarters of full weight far out: a dense graph's edges stay lines, not a mesh
      var k = Math.max(1, 0.75 / z), far = z * labelPx < LEGIBLE;
      if (far === view.far && Math.abs(k - view.k) < view.k / 10) { return; }
      view.k = k;
      view.far = far;
      cy.style().update();
    }
    cy.on('zoom', function () { clearTimeout(restyle); restyle = setTimeout(scale, 120); });

    fit(false);
    scale();
    cy.on('scrollzoom pinchzoom dragpan', function () { fitted = false; });
    if (window.ResizeObserver) {
      var ro = new ResizeObserver(function () { cy.resize(); floor(); if (fitted) { settle(cy.elements(':visible'), 16, false); } });
      ro.observe(canvas);
    }

    function href(d) { return d.external ? d.route : base + d.route; }

    // a node's kind and state, as the page's own tag and badge: the word, never colour alone
    function marks(d) {
      return '<span class="mj-tag">' + esc(d.kind) + '</span>' +
        (d.status ? '<span class="mj-badge ' + badgeClass(d.status) + '"><span class="mj-badge-dot"></span>' + esc(d.status) + '</span>' : '');
    }

    function describe(node) {
      if (!detail) { return; }
      var d = node.data();
      var parts = ['<p class="font-mono text-sm font-medium text-heading break-words">' + esc(d.label) + '</p>'];
      parts.push('<p class="mt-2 flex flex-wrap items-center gap-1.5">' + marks(d) + '</p>');
      if (d.summary) { parts.push('<p class="mt-3 text-sm text-body">' + esc(d.summary) + '</p>'); }
      if (d.note) { parts.push('<p class="mt-2 text-sm text-body">' + esc(d.note) + '</p>'); }
      var out = node.outgoers('edge'), inc = node.incomers('edge');
      if (out.length || inc.length) {
        parts.push('<ul class="mt-3 space-y-1 border-t border-default pt-3 text-xs text-body-subtle">');
        out.forEach(function (e) { parts.push('<li><span class="font-mono text-heading">' + esc(e.data('kind')) + '</span> → ' + esc(e.target().data('label')) + '</li>'); });
        inc.forEach(function (e) { parts.push('<li>' + esc(e.source().data('label')) + ' <span class="font-mono text-heading">' + esc(e.data('kind')) + '</span> → this</li>'); });
        parts.push('</ul>');
      }
      if (d.route) {
        parts.push('<a class="mt-3 inline-block text-sm font-medium text-fg-brand underline" href="' + esc(href(d)) + '">Open the page for this</a>');
      }
      detail.innerHTML = parts.join('');
    }

    // A state flag is set false, never removed: Cytoscape's removeData() does not restyle (3.33
    // ignores its updateStyle option), so a removed flag left the drawing dimmed or lit after
    // the state it drew had gone. data() restyles; the selectors read false as unset.
    function clearFocus() {
      cy.elements().data({ dimmed: false, focus: false });
      if (detail) { detail.innerHTML = empty; }
    }

    function focus(node) {
      var keep = node.closedNeighborhood();
      cy.batch(function () {
        // the previous selection's edges must lose their focus first: edge[?focus] outranks
        // edge[?dimmed] in the stylesheet, so an edge that kept both stayed lit under the new one
        cy.elements().data({ focus: false, match: false, dimmed: true });
        keep.data('dimmed', false);
        keep.edges().data('focus', true);
      });
      // a selection ends a search: the query and its count go with the matches it lit, so the
      // box never reports a result the drawing no longer shows
      clearTimeout(timer);
      if (search) { search.value = ''; }
      if (count) { count.textContent = ''; }
      describe(node);
    }

    // A selection the reader can see and read: brought to the centre when it is out of view,
    // and closed in on, just far enough for its name to be drawn, when the view stands so far
    // out that the labels are not. A view already close enough is never zoomed out.
    function bring(node) {
      var e = cy.extent(), b = node.boundingBox();
      var seen = b.x1 >= e.x1 && b.x2 <= e.x2 && b.y1 >= e.y1 && b.y2 <= e.y2;
      var legible = cy.zoom() * labelPx >= LEGIBLE;
      if (seen && legible) { return; }
      var z = legible ? cy.zoom() : Math.min(cy.maxZoom(), (LEGIBLE + 2) / labelPx);
      var at = node.position();
      var to = { zoom: z, pan: { x: cy.width() / 2 - at.x * z, y: cy.height() / 2 - at.y * z } };
      var t = motion() * 2;
      cy.stop(true, true);
      if (t) { cy.animate(to, { duration: t, easing: 'ease-out-cubic' }); } else { cy.viewport(to); }
      fitted = false;
    }

    // every selection goes through here, so the aria-live panel announces each one
    function pick(node) {
      cy.elements().unselect();
      node.select();
      focus(node);
      bring(node);
    }

    // ------------------------------------------------------------------ the pointer and its card
    var tip = null;
    if (!coarse) {
      tip = document.createElement('div');
      tip.className = 'mj-graph-tip';
      tip.hidden = true;
      tip.setAttribute('aria-hidden', 'true');   // the panel is what is announced; this previews it
      canvas.parentNode.appendChild(tip);
    }
    function hideTip() { if (tip) { tip.hidden = true; } }
    function showTip(node) {
      if (!tip) { return; }
      var d = node.data();
      var links = node.connectedEdges(':visible').length;
      tip.innerHTML = '<p class="mj-graph-tip-title">' + esc(d.label) + '</p>' +
        '<p class="mj-graph-tip-row">' + marks(d) +
        '<span class="mj-graph-tip-value">' + links + (links === 1 ? ' link' : ' links') + '</span></p>' +
        (d.route ? '<p class="mj-graph-tip-foot">Click to select · Enter opens</p>' : '');
      tip.hidden = false;
      // one read of the card's box per pointing, then one write: no layout loop
      var w = tip.offsetWidth, h = tip.offsetHeight;
      var rp = node.renderedPosition(), nh = node.renderedOuterHeight();
      var fw = canvas.clientWidth, fh = canvas.clientHeight, gap = 8;
      var x = Math.max(gap, Math.min(fw - w - gap, rp.x - w / 2));
      var y = rp.y - nh / 2 - h - gap;
      if (y < gap) { y = Math.max(gap, Math.min(fh - h - gap, rp.y + nh / 2 + gap)); }
      tip.style.transform = 'translate(' + Math.round(canvas.offsetLeft + x) + 'px, ' + Math.round(canvas.offsetTop + y) + 'px)';
    }

    cy.on('mouseover', 'node', function (evt) {
      var node = evt.target;
      // Cytoscape follows the pointer by its container's rectangle, not by what is on top: a
      // pointer on the sticky header over a scrolled frame is not pointing into the drawing
      if (!canvas.matches(':hover')) { return; }
      canvas.classList.add('is-pointing');
      cy.batch(function () {
        node.data('pointed', true);
        // a selection outranks the pointer: what is held stays the picture
        if (!cy.$('node:selected').length) {
          cy.elements().data('faded', true);
          node.closedNeighborhood().data({ faded: false, hot: true });
        }
      });
      showTip(node);
    });
    function unpoint() {
      canvas.classList.remove('is-pointing');
      cy.batch(function () { cy.elements().data({ pointed: false, hot: false, faded: false }); });
      hideTip();
    }
    cy.on('mouseout', 'node', unpoint);
    // a pointer that leaves the frame in one jump is never seen leaving the node by Cytoscape,
    // which only follows a pointer inside its container: the frame says so instead
    canvas.addEventListener('mouseleave', function () { if (canvas.classList.contains('is-pointing')) { unpoint(); } });
    cy.on('viewport grab', hideTip);

    cy.on('tap', 'node', function (evt) {
      focus(evt.target);
      // the keyboard carries on from what the pointer chose
      if (document.activeElement !== canvas) { canvas.focus({ preventScroll: true }); }
    });
    // the ground lets go of everything: the selection, the search and what either had lit
    cy.on('tap', function (evt) { if (evt.target === cy) { cy.elements().unselect(); clearSearch(); clearFocus(); } });

    // filters: one checkbox per edge kind present, rendered by the template from the data
    el.querySelectorAll('[data-graph-filter]').forEach(function (input) {
      input.addEventListener('change', function () {
        var off = [];
        el.querySelectorAll('[data-graph-filter]').forEach(function (i) { if (!i.checked) { off.push(i.getAttribute('data-graph-filter')); } });
        cy.edges().forEach(function (e) { e.style('display', off.indexOf(e.data('kind')) === -1 ? 'element' : 'none'); });
      });
    });
    // The legend is a preview as well as a filter, as a chart's is: pointing at a chip, or
    // focusing its box, steps every edge of the other kinds back with the hover's own flag.
    // A selection outranks it, as it outranks the pointer, and leaving always restores.
    el.querySelectorAll('.mj-graph-kind').forEach(function (chip) {
      var input = chip.querySelector('[data-graph-filter]');
      if (!input) { return; }
      var kind = input.getAttribute('data-graph-filter');
      function preview() {
        if (!input.checked || cy.$('node:selected').length) { return; }
        cy.batch(function () {
          cy.edges().forEach(function (e) { e.data('faded', e.data('kind') !== kind); });
        });
      }
      function restore() { cy.batch(function () { cy.edges().data('faded', false); }); }
      chip.addEventListener('pointerenter', preview);
      chip.addEventListener('pointerleave', restore);
      chip.addEventListener('focusin', preview);
      chip.addEventListener('focusout', restore);
    });

    // ------------------------------------------------------------------ find a node
    var search = el.querySelector('[data-graph-search]');
    var count = el.querySelector('[data-graph-count]');
    var timer = 0;
    function clearSearch() {
      clearTimeout(timer);
      if (search) { search.value = ''; }
      if (count) { count.textContent = ''; }
      var held = cy.$('node:selected');
      if (held.length) { focus(held[0]); return; }
      cy.batch(function () { cy.elements().data({ dimmed: false, match: false }); });
    }
    // Matches stay lit and are fitted into view; everything else steps back. A query that
    // matches nothing says so and keeps the view: there is nothing to fit to.
    function find(move) {
      var q = search ? search.value.trim().toLowerCase() : '';
      if (!q) { clearSearch(); return cy.collection(); }
      var hits = cy.nodes().filter(function (n) {
        var d = n.data();
        return [d.label, d.kind, d.summary].some(function (s) { return s != null && String(s).toLowerCase().indexOf(q) !== -1; });
      });
      cy.batch(function () {
        cy.elements().data({ focus: false, match: false, dimmed: true });
        hits.data({ dimmed: false, match: true });
      });
      if (count) { count.textContent = hits.length ? hits.length + ' of ' + cy.nodes().length : 'Nothing matches'; }
      if (hits.length && move !== false) { settle(hits, 32, true); fitted = false; }
      return hits;
    }
    if (search) {
      search.addEventListener('input', function () {
        clearTimeout(timer);
        timer = setTimeout(find, 120);
      });
      search.addEventListener('keydown', function (ev) {
        if (ev.key === 'Escape') { ev.preventDefault(); clearSearch(); }
        else if (ev.key === 'Enter') {
          ev.preventDefault();
          clearTimeout(timer);
          var hits = find(false);
          if (hits.length) { pick(hits[0]); canvas.focus({ preventScroll: true }); }
        }
      });
    }

    // ------------------------------------------------------------------ zoom, fit, reset
    // around the centre of the frame, within the bounds the drawing was made with
    function zoomBy(factor) {
      cy.stop(true, true);
      var level = Math.max(cy.minZoom(), Math.min(cy.maxZoom(), cy.zoom() * factor));
      cy.zoom({ level: level, renderedPosition: { x: cy.width() / 2, y: cy.height() / 2 } });
      fitted = false;
    }
    el.querySelectorAll('[data-graph-zoom]').forEach(function (b) {
      b.addEventListener('click', function () { zoomBy(b.getAttribute('data-graph-zoom') === 'out' ? 0.8 : 1.25); });
    });
    var fitButton = el.querySelector('[data-graph-fit]');
    if (fitButton) { fitButton.addEventListener('click', function () { fit(true); }); }

    function resetAll() {
      clearTimeout(timer);
      if (search) { search.value = ''; }
      if (count) { count.textContent = ''; }
      cy.batch(function () { cy.elements().unselect().data('match', false); });
      clearFocus();
      fit(true);
    }
    var reset = el.querySelector('[data-graph-reset]');
    if (reset) { reset.addEventListener('click', resetAll); }

    // the fallback list and the canvas are the same graph: clicking a name selects the node
    el.querySelectorAll('[data-graph-node]').forEach(function (a) {
      a.addEventListener('click', function (ev) {
        var n = cy.getElementById(a.getAttribute('data-graph-node'));
        if (!n || !n.length) { return; }
        ev.preventDefault();
        pick(n);
        canvas.scrollIntoView({ block: 'nearest', behavior: reduced ? 'auto' : 'smooth' });
      });
    });

    // ------------------------------------------------------------------ the keyboard
    // One map with site/diagrams.js and the Cockpit's trains: an arrow moves the selection to
    // the nearest node that way, Shift with an arrow pans, + and - zoom, 0 fits, Home and End
    // go to the ends, Enter opens, Escape clears. `/` stays the site search's.
    var DIRECTION = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
    function nearest(from, dir) {
      var o = from.position(), best = null, score = Infinity;
      cy.nodes(':visible').forEach(function (n) {
        if (n.same(from)) { return; }
        var q = n.position(), dx = q.x - o.x, dy = q.y - o.y, dist = Math.sqrt(dx * dx + dy * dy);
        if (!dist) { return; }
        var cos = (dx * dir[0] + dy * dir[1]) / dist;
        if (cos < 0.5) { return; }                       // within 60 degrees of the direction
        var s = dist * (2 - cos);                        // nearer, and straighter, wins
        if (s < score) { score = s; best = n; }
      });
      return best;
    }
    canvas.addEventListener('keydown', function (ev) {
      if (ev.altKey || ev.ctrlKey || ev.metaKey) { return; }
      var visible = cy.nodes(':visible');
      var held = cy.$('node:selected');
      var dir = DIRECTION[ev.key];
      if (dir && ev.shiftKey) { cy.panBy({ x: -dir[0] * 48, y: -dir[1] * 48 }); fitted = false; }
      else if (dir) {
        var next = held.length ? nearest(held[0], dir) : visible[0];
        if (next) { pick(next); }
      } else if (ev.key === 'Home' || ev.key === 'End') {
        if (visible.length) { pick(ev.key === 'Home' ? visible[0] : visible[visible.length - 1]); }
      } else if (ev.key === '+' || ev.key === '=') { zoomBy(1.25); }
      else if (ev.key === '-' || ev.key === '_') { zoomBy(0.8); }
      else if (ev.key === '0') { fit(true); }
      else if (ev.key === 'Enter') {
        if (held.length && held[0].data('route')) { window.location.href = href(held[0].data()); }
        else if (!held.length && visible.length) { pick(visible[0]); }
      } else if (ev.key === 'Escape') {
        // Escape lets go and leaves the view where the reader put it: only Reset and 0 refit
        cy.elements().unselect(); clearSearch(); clearFocus();
      }
      else { return; }
      ev.preventDefault();
    });

    // A plain wheel over the drawing scrolls the page, as it does over a diagram: the event is
    // stopped on its way down to Cytoscape and its default, the scroll, is left alone. A pinch
    // arrives as a wheel with ctrlKey, so it still zooms, and so do Ctrl and Cmd with the wheel.
    el.addEventListener('wheel', function (ev) {
      if (!ev.ctrlKey && !ev.metaKey) { ev.stopPropagation(); }
    }, { capture: true, passive: true });

    // Draw again in the other palette when the theme changes: the charts.js idiom, the class
    // read from the page. Restyled in place; the layout and the reader's view stay put.
    new MutationObserver(function () {
      if (isDark() === theme) { return; }
      theme = isDark();
      p = palette();
      cy.style(stylesheet(p, el, large, view));
    }).observe(root, { attributes: true, attributeFilter: ['class'] });

    // the drawing is a control now: reachable, named after its caption, and its keys described
    var caption = el.querySelector('figcaption');
    var keys = el.querySelector('[data-graph-keys]');
    canvas.removeAttribute('aria-hidden');
    canvas.setAttribute('tabindex', '0');
    canvas.setAttribute('role', 'application');
    canvas.setAttribute('aria-roledescription', 'graph');
    if (caption) { canvas.setAttribute('aria-label', caption.textContent.trim()); }
    if (keys && keys.id) { canvas.setAttribute('aria-describedby', keys.id); }
    // the controls the build wrote hidden are shown once there is a drawing to control
    el.querySelectorAll('[data-graph-tool]').forEach(function (t) { t.hidden = false; });

    canvas.setAttribute('data-graph-ready', '1');
    // the drawing, reachable from its own figure and nowhere else: scripts/lib/interaction-specs/graph.mjs
    // asserts that a filter hides the edges of its kind and that a name in the list focuses its node,
    // which only the drawing itself can answer
    el.mjGraph = cy;
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
    Promise.all([loadCytoscape(src), graphData(el)]).then(function (r) { mount(el, r[0], r[1]); }).catch(function (err) {
      // said in the console for whoever looks, and on the page for the reader
      if (window.console) { console.warn('graph: the drawing could not be made:', err); }
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
