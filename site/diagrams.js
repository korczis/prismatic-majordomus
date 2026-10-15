// The diagram viewer: one for every diagram on the site (ADR 0152).
//
// A diagram reaches a page as <pre class="mermaid">, projected from a canonical ```mermaid
// fence or written by a template from site/data/generated. The build (scripts/lib/
// diagrams.mjs) has already named it (`id="diagram-<name>"`), listed its nodes
// (`data-nodes`) and written beside it the links it inferred from the identities the
// generated data carries. This file adds what only a browser can:
//
//   - it fetches the vendored Mermaid build after the page has been parsed, never before,
//     and draws each diagram in the page's own colours, again when the theme changes;
//   - zoom (the toolbar, + and -, Ctrl or Cmd with the wheel, a pinch), pan (drag, the arrow
//     keys), fit (0, a double click), full screen (F) and the drawing as an SVG file;
//   - a node's edges and neighbours traced while the pointer or the focus is on it;
//   - every node the build linked is a link in the drawing, to the same page;
//   - an address: `#diagram-<name>` opens on that diagram, `#diagram-<name>:<node>` on that
//     node, selected and centred, and the toolbar copies whichever is current.
//
// Rendering is a progressive enhancement in the strict sense. The <pre> stays the element
// it was: it keeps its id and its place in the tab order and becomes the viewer's frame, so
// a link to it lands on the same thing with or without this script. A diagram that fails to
// draw keeps its source visible and gets no controls, and never breaks the page.
(function () {
  'use strict';

  var blocks = Array.prototype.slice.call(document.querySelectorAll('pre.mermaid'));
  if (!blocks.length || !window.Promise) { return; }

  var PREFIX = 'diagram-';
  var SVG_NS = 'http://www.w3.org/2000/svg';
  var MAX_SCALE = 8;
  var STEP = 1.25;
  var PAN_STEP = 48;
  var DRAG_THRESHOLD = 4;
  var STATUS_FOR_MS = 2400;
  // the smallest scale a diagram opens at: below it a label is a smudge
  var READABLE = 0.4;

  var script = document.currentScript;
  var mermaidSrc = script ? script.getAttribute('data-mermaid-src') : null;
  var root = document.documentElement;
  var loading = null;
  var queue = Promise.resolve();
  var seq = 0;
  var states = [];

  function isDark() { return root.classList.contains(root.dataset.themeClass); }
  function clamp(v, lo, hi) { return Math.min(hi, Math.max(lo, v)); }

  // ---------------------------------------------------------------- the page's own colours

  // One read of the page's own custom properties. `getPropertyValue` answers the empty
  // string for a name no stylesheet declares, which is how the previous palette here went
  // unnoticed: every lookup missed and every fallback was used.
  function tok(name) {
    return getComputedStyle(root).getPropertyValue(name).trim();
  }

  // A colour token as six-digit sRGB hex. The stylesheet declares its palette in OKLCH
  // notation, which Mermaid's colour parser rejects with an uncaught "Unsupported color
  // format" while rendering, so a token is resolved by the browser (var() chains included)
  // and read back from one painted pixel, which is sRGB whatever notation the stylesheet used.
  // No colour is written here: the pixel is the token.
  var probe = document.createElement('span');
  var canvas = document.createElement('canvas');
  canvas.width = canvas.height = 1;
  var paint = canvas.getContext('2d', { willReadFrequently: true });
  function colour(name) {
    if (!tok(name) || !paint) { return ''; }
    probe.style.color = 'var(' + name + ')';
    document.body.appendChild(probe);
    var resolved = getComputedStyle(probe).color;
    probe.remove();
    // a computed colour is always a valid one, so nothing needs resetting between tokens;
    // save/restore keeps one token's paint from leaking into the next
    paint.save();
    paint.clearRect(0, 0, 1, 1);
    paint.fillStyle = resolved;
    paint.fillRect(0, 0, 1, 1);
    var px = paint.getImageData(0, 0, 1, 1).data;
    paint.restore();
    return '#' + [px[0], px[1], px[2]].map(function (c) { return (c < 16 ? '0' : '') + c.toString(16); }).join('');
  }

  // The named colour tokens that actually resolve, plus whatever is passed through verbatim.
  function vars(names, extra) {
    var out = {}, key;
    for (key in extra) { if (Object.prototype.hasOwnProperty.call(extra, key)) { out[key] = extra[key]; } }
    for (key in names) {
      if (!Object.prototype.hasOwnProperty.call(names, key)) { continue; }
      var value = colour(names[key]);
      if (value) { out[key] = value; }
    }
    return out;
  }

  // ---------------------------------------------------------------- Mermaid

  function usable(lib) { return Boolean(lib) && typeof lib.initialize === 'function' && typeof lib.render === 'function'; }

  function loadMermaid() {
    // a heading called "Mermaid" makes `window.mermaid` that heading: a page's ids are
    // properties of its window, so the library is known by what it can do, not by its name
    if (usable(window.mermaid)) { return Promise.resolve(window.mermaid); }
    if (loading) { return loading; }
    loading = new Promise(function (resolve, reject) {
      if (!mermaidSrc) { reject(new Error('no Mermaid build is named on this page')); return; }
      var s = document.createElement('script');
      s.src = mermaidSrc;
      s.onload = function () {
        if (usable(window.mermaid)) { resolve(window.mermaid); } else { reject(new Error('the Mermaid build defined nothing')); }
      };
      s.onerror = function () { reject(new Error('the Mermaid build did not load')); };
      document.head.appendChild(s);
    });
    return loading;
  }

  function configure(mermaid) {
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      // a diagram that does not parse is reported on its own frame, not drawn into the page
      suppressErrorRendering: true,
      theme: isDark() ? 'dark' : 'neutral',
      fontFamily: tok('--font-sans'),
      // the viewer scales the drawing itself; Mermaid's own fit-to-width would fight it
      flowchart: { useMaxWidth: false },
      state: { useMaxWidth: false },
      sequence: { useMaxWidth: false },
      er: { useMaxWidth: false },
      class: { useMaxWidth: false },
      gantt: { useMaxWidth: false },
      // Read from the page, not written here. These names are declared once, in
      // share/design/tokens.yaml, and reach this script through the stylesheet the page
      // already loaded; the block that used to sit here was a private colour table that
      // moved whenever nobody remembered it existed. A token that does not resolve is
      // omitted rather than guessed, and Mermaid's own theme answers for it.
      themeVariables: vars({
        primaryColor: '--mj-raised',
        primaryTextColor: '--mj-fg',
        // edge labels: without these Mermaid's dark theme puts light grey text on a mid-grey
        // label (4.43:1, below WCAG AA); the page's own text on its raised surface passes
        textColor: '--mj-fg',
        edgeLabelBackground: '--mj-raised',
        primaryBorderColor: '--mj-line',
        lineColor: '--mj-muted',
        secondaryColor: '--mj-sunken',
        tertiaryColor: '--mj-sunken',
        background: '--mj-bg'
      }, { fontSize: '14px' })
    });
  }

  // ---------------------------------------------------------------- the view: fit, zoom, pan

  function expanded(state) {
    return document.fullscreenElement === state.wrap || state.wrap.classList.contains('is-expanded');
  }

  // The view a diagram opens on. Outside full screen the frame takes the drawing's height, up
  // to three quarters of the window, so a small diagram is not a tall empty box and a tall one
  // does not push the page away. A drawing too large to be read whole (a dependency graph of
  // hundreds of issues) opens at a scale its labels can be read at, from its top left corner,
  // and is panned from there; Fit shows all of it.
  function home(state) {
    if (!state.width) { return; }
    var frame = state.pre;
    var available = frame.clientWidth;
    // inside a closed disclosure the frame has no width yet; it is fitted when it gets one
    if (!available) { return; }
    if (expanded(state)) { frame.style.height = ''; fit(state, false); return; }
    var tallest = Math.max(240, window.innerHeight * 0.75);
    var whole = Math.min(1, available / state.width, tallest / state.height);
    var scale = Math.max(whole, Math.min(READABLE, 1));
    frame.style.height = Math.max(96, Math.ceil(Math.min(tallest, state.height * scale))) + 'px';
    state.fit = Math.min(1, available / state.width, frame.clientHeight / state.height);
    state.scale = scale;
    state.x = Math.max(0, (available - state.width * scale) / 2);
    state.y = Math.max(0, (frame.clientHeight - state.height * scale) / 2);
    state.lastWidth = available;
    apply(state, false);
  }

  // The whole drawing inside the frame as it stands.
  function fit(state, animate) {
    if (!state.width) { return; }
    var frame = state.pre;
    var available = frame.clientWidth;
    if (!available || !frame.clientHeight) { return; }
    var scale = Math.min(expanded(state) ? 2 : 1, available / state.width, frame.clientHeight / state.height);
    state.fit = scale;
    state.scale = scale;
    state.x = (available - state.width * scale) / 2;
    state.y = (frame.clientHeight - state.height * scale) / 2;
    apply(state, animate);
  }

  function apply(state, animate) {
    state.stage.classList.toggle('is-animated', Boolean(animate));
    state.stage.style.transform = 'translate(' + state.x + 'px, ' + state.y + 'px) scale(' + state.scale + ')';
    state.pre.setAttribute('data-diagram-scale', String(Math.round(state.scale * 1000) / 1000));
  }

  function zoomAt(state, factor, cx, cy, animate) {
    var next = clamp(state.scale * factor, Math.min(0.1, state.fit), MAX_SCALE);
    var ratio = next / state.scale;
    state.x = cx - (cx - state.x) * ratio;
    state.y = cy - (cy - state.y) * ratio;
    state.scale = next;
    apply(state, animate);
  }

  function zoomCentre(state, factor) {
    zoomAt(state, factor, state.pre.clientWidth / 2, state.pre.clientHeight / 2, true);
  }

  function pan(state, dx, dy, animate) {
    state.x += dx;
    state.y += dy;
    apply(state, animate);
  }

  function localPoint(state, event) {
    var rect = state.pre.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  }

  // Bring one node to the middle of the frame, at a size its label can be read at.
  function centreOn(state, node) {
    var frame = state.pre.getBoundingClientRect();
    var box = node.getBoundingClientRect();
    if (!box.width || !state.scale) { return; }
    var cx = (box.left + box.width / 2 - frame.left - state.x) / state.scale;
    var cy = (box.top + box.height / 2 - frame.top - state.y) / state.scale;
    var scale = Math.max(state.scale, 1);
    state.scale = scale;
    state.x = state.pre.clientWidth / 2 - cx * scale;
    state.y = state.pre.clientHeight / 2 - cy * scale;
    apply(state, true);
  }

  // ---------------------------------------------------------------- full screen, download, link

  function setExpanded(state, on) {
    state.wrap.classList.toggle('is-expanded', on);
    window.requestAnimationFrame(function () { home(state); });
  }

  function toggleFullscreen(state) {
    if (document.fullscreenElement === state.wrap) { document.exitFullscreen(); return; }
    if (state.wrap.classList.contains('is-expanded')) { setExpanded(state, false); return; }
    if (state.wrap.requestFullscreen) {
      state.wrap.requestFullscreen().catch(function () { setExpanded(state, true); });
    } else {
      setExpanded(state, true);
    }
  }

  function download(state) {
    var svg = state.stage.querySelector('svg');
    if (!svg) { return; }
    var copy = svg.cloneNode(true);
    copy.setAttribute('xmlns', SVG_NS);
    // the page's ground, so the file reads the same opened on its own
    copy.style.background = colour('--mj-bg');
    var blob = new Blob([new XMLSerializer().serializeToString(copy)], { type: 'image/svg+xml' });
    var url = URL.createObjectURL(blob);
    var link = document.createElement('a');
    link.href = url;
    link.download = state.name + '.svg';
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.setTimeout(function () { URL.revokeObjectURL(url); }, 1000);
    say(state, 'Saved ' + state.name + '.svg');
  }

  // The address of what the reader is looking at: the diagram, or the node selected in it.
  function fragment(state) {
    return '#' + state.id + (state.selected ? ':' + encodeURIComponent(state.selected) : '');
  }

  function address(state) {
    var here = window.location;
    return here.origin + here.pathname + here.search + fragment(state);
  }

  // The address bar always gets the link, so a browser that refuses the clipboard still
  // leaves the reader with something to copy.
  function copyLink(state) {
    var url = address(state);
    try { window.history.replaceState(null, '', fragment(state)); } catch (e) { /* a sandboxed page */ }
    var done = function () { say(state, state.selected ? 'Link to node ' + state.selected + ' copied' : 'Link to this diagram copied'); };
    var fallback = function () { say(state, 'The link is in the address bar'); };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(url).then(done, fallback);
    } else {
      fallback();
    }
  }

  function say(state, text) {
    if (!state.status) { return; }
    state.status.textContent = text;
    window.clearTimeout(state.statusTimer);
    state.statusTimer = window.setTimeout(function () { state.status.textContent = state.rest; }, STATUS_FOR_MS);
  }

  // ---------------------------------------------------------------- nodes: identity, tracing, selection

  // A node's id as the source declares it. Mermaid writes it into the element's own id
  // between a kind and a counter (`flowchart-I0001-12`, `state-active-3`), prefixed with the
  // drawing's id in some versions; the build's `data-nodes` is the list to match against, so
  // an id that itself ends in a number is not mistaken for a counter.
  function nodeKey(state, el) {
    var known = el.getAttribute('data-node');
    if (known) { return known; }
    var raw = el.getAttribute('data-id') || el.id || '';
    if (state.declared[raw]) { return raw; }
    var best = '';
    for (var key in state.declared) {
      if (key.length > best.length && new RegExp('(^|-)' + key.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '(-\\d+)?$').test(raw)) { best = key; }
    }
    return best || null;
  }

  // An edge's two ends. Mermaid names an edge `L_<from>_<to>_<n>`; ids may hold `_`, so the
  // split is the one whose halves are both nodes of this diagram.
  function edgeEnds(state, edge) {
    var match = /L[_-](.+)[_-]\d+$/.exec(edge.getAttribute('data-id') || edge.id || '');
    if (!match) { return null; }
    var body = match[1];
    for (var i = 1; i < body.length; i++) {
      if (body[i] !== '_' && body[i] !== '-') { continue; }
      var from = body.slice(0, i), to = body.slice(i + 1);
      if (state.nodes[from] && state.nodes[to]) { return [from, to]; }
    }
    return null;
  }

  function trace(state, key) {
    untrace(state);
    var node = state.nodes[key];
    if (!node) { return; }
    var hot = [node];
    state.edges.forEach(function (edge) {
      if (edge.ends[0] !== key && edge.ends[1] !== key) { return; }
      hot.push(edge.el, state.nodes[edge.ends[0] === key ? edge.ends[1] : edge.ends[0]]);
      if (edge.label) { hot.push(edge.label); }
    });
    hot.forEach(function (el) { if (el) { el.classList.add('is-hot'); } });
    state.hot = hot;
    state.pre.classList.add('is-tracing');
  }

  function untrace(state) {
    (state.hot || []).forEach(function (el) { if (el) { el.classList.remove('is-hot'); } });
    state.hot = null;
    state.pre.classList.remove('is-tracing');
  }

  // What the frame shows when nothing is hovered: the selected node's neighbourhood, or all.
  function rest(state) {
    if (state.selected) { trace(state, state.selected); } else { untrace(state); }
  }

  function select(state, key, opts) {
    var options = opts || {};
    Object.keys(state.nodes).forEach(function (k) { state.nodes[k].classList.remove('is-selected'); });
    state.selected = key && state.nodes[key] ? key : null;
    if (state.selected) {
      state.nodes[state.selected].classList.add('is-selected');
      if (options.centre) { centreOn(state, state.nodes[state.selected]); }
    }
    rest(state);
    state.rest = state.selected ? 'Node ' + state.selected + ' selected' : '';
    if (state.status) { state.status.textContent = state.rest; }
    if (options.address) {
      try { window.history.replaceState(null, '', fragment(state)); } catch (e) { /* a sandboxed page */ }
    }
  }

  // Read the drawing: which element is which declared node, which path joins which two, and
  // which nodes the build linked. A linked node is wrapped in an SVG <a>, so it is a link to
  // the keyboard, to a screen reader and to "open in a new tab" exactly as one in the text is.
  function index(state) {
    var svg = state.stage.querySelector('svg');
    state.nodes = {};
    state.edges = [];
    Array.prototype.forEach.call(svg.querySelectorAll('g.node, g.statediagram-state'), function (el) {
      var key = nodeKey(state, el);
      if (!key || state.nodes[key]) { return; }
      el.setAttribute('data-node', key);
      state.nodes[key] = el;
    });
    Array.prototype.forEach.call(svg.querySelectorAll('.edgePaths path, path.flowchart-link, path.transition'), function (el) {
      var ends = edgeEnds(state, el);
      if (!ends) { return; }
      var id = el.getAttribute('data-id') || el.id;
      var label = id ? svg.querySelector('.edgeLabel [data-id="' + id + '"], .edgeLabel [id="' + id + '"]') : null;
      state.edges.push({ el: el, ends: ends, label: label ? label.closest('.edgeLabel') : null });
    });
    Object.keys(state.links).forEach(function (key) {
      var node = state.nodes[key];
      if (!node || node.parentNode.namespaceURI !== SVG_NS) { return; }
      var a = document.createElementNS(SVG_NS, 'a');
      a.setAttribute('href', state.links[key].href);
      a.setAttribute('aria-label', state.links[key].text + ' (' + state.links[key].kind + ')');
      node.parentNode.insertBefore(a, node);
      a.appendChild(node);
    });
  }

  // ---------------------------------------------------------------- the frame and its controls

  // The frame is assembled once, on the first drawing that succeeds: a diagram that never
  // draws keeps the markup it arrived with. The toolbar is the build's (scripts/lib/
  // diagrams.mjs wrote it, hidden, right after the diagram); nothing here creates a control.
  function frame(state) {
    if (state.wrap) { return; }
    var pre = state.pre;
    var wrap = document.createElement('div');
    wrap.className = 'mj-diagram';
    wrap.setAttribute('data-diagram-viewer', state.name);
    pre.parentNode.insertBefore(wrap, pre);
    if (state.bar) { wrap.appendChild(state.bar); state.bar.hidden = false; }
    wrap.appendChild(pre);
    // the inferred links stay with the diagram they belong to, under its frame
    if (state.list) { wrap.appendChild(state.list); }
    pre.classList.add('mj-diagram-canvas');
    pre.setAttribute('role', 'group');
    pre.setAttribute('aria-roledescription', 'diagram');
    pre.setAttribute('aria-label', 'Diagram ' + state.name + '. Plus and minus zoom, the arrow keys pan, 0 fits, F is full screen.');
    if (!pre.hasAttribute('tabindex')) { pre.tabIndex = 0; }
    state.wrap = wrap;
    wire(state);
  }

  function wire(state) {
    var pre = state.pre;
    var pointers = {};
    var drag = null;
    var pinch = null;
    var suppressClick = false;

    if (state.bar) state.bar.addEventListener('click', function (event) {
      var button = event.target.closest('[data-diagram-action]');
      if (!button || !state.width) { return; }
      var action = button.getAttribute('data-diagram-action');
      if (action === 'zoom-in') { zoomCentre(state, STEP); }
      else if (action === 'zoom-out') { zoomCentre(state, 1 / STEP); }
      else if (action === 'fit') { fit(state, true); }
      else if (action === 'expand') { toggleFullscreen(state); }
      else if (action === 'download') { download(state); }
      else if (action === 'link') { copyLink(state); }
    });

    pre.addEventListener('wheel', function (event) {
      // a plain wheel scrolls the page, not the diagram
      if (!state.width || !(event.ctrlKey || event.metaKey || expanded(state))) { return; }
      event.preventDefault();
      var point = localPoint(state, event);
      zoomAt(state, Math.exp(-event.deltaY * 0.0025), point.x, point.y, false);
    }, { passive: false });

    pre.addEventListener('pointerdown', function (event) {
      if (!state.width || (event.pointerType === 'mouse' && event.button !== 0)) { return; }
      pointers[event.pointerId] = localPoint(state, event);
      var ids = Object.keys(pointers);
      if (ids.length === 2) {
        var a = pointers[ids[0]], b = pointers[ids[1]];
        pinch = { distance: Math.hypot(a.x - b.x, a.y - b.y) || 1 };
        drag = null;
      } else if (ids.length === 1) {
        drag = { x: event.clientX, y: event.clientY, moved: false, id: event.pointerId };
      }
    });

    pre.addEventListener('pointermove', function (event) {
      if (!pointers[event.pointerId]) { return; }
      pointers[event.pointerId] = localPoint(state, event);
      var ids = Object.keys(pointers);
      if (pinch && ids.length === 2) {
        var a = pointers[ids[0]], b = pointers[ids[1]];
        var distance = Math.hypot(a.x - b.x, a.y - b.y) || 1;
        zoomAt(state, distance / pinch.distance, (a.x + b.x) / 2, (a.y + b.y) / 2, false);
        pinch.distance = distance;
        suppressClick = true;
        return;
      }
      if (!drag) { return; }
      var dx = event.clientX - drag.x, dy = event.clientY - drag.y;
      if (!drag.moved) {
        if (Math.hypot(dx, dy) < DRAG_THRESHOLD) { return; }
        // on touch, outside full screen, a vertical swipe belongs to the page
        if (event.pointerType !== 'mouse' && !expanded(state) && Math.abs(dy) > Math.abs(dx)) {
          drag = null;
          delete pointers[event.pointerId];
          return;
        }
        drag.moved = true;
        pre.classList.add('is-panning');
        if (pre.setPointerCapture) { try { pre.setPointerCapture(event.pointerId); } catch (e) { /* the pointer is gone */ } }
      }
      drag.x = event.clientX;
      drag.y = event.clientY;
      pan(state, dx, dy, false);
    });

    function release(event) {
      delete pointers[event.pointerId];
      if (Object.keys(pointers).length < 2) { pinch = null; }
      if (drag && drag.id === event.pointerId) {
        if (drag.moved) { suppressClick = true; }
        drag = null;
        pre.classList.remove('is-panning');
      }
    }
    pre.addEventListener('pointerup', release);
    pre.addEventListener('pointercancel', release);

    function nodeOf(target) {
      return target && target.closest ? target.closest('[data-node]') : null;
    }

    // A drag that ends on a node must not follow its link. A click on a node the build did
    // not link selects it; on a linked one the link is followed, and Alt selects instead.
    pre.addEventListener('click', function (event) {
      if (suppressClick) {
        event.preventDefault();
        event.stopPropagation();
        suppressClick = false;
        return;
      }
      var node = nodeOf(event.target);
      if (!node) { return; }
      var linked = Boolean(node.closest('a'));
      if (linked && !event.altKey) { return; }
      event.preventDefault();
      var key = node.getAttribute('data-node');
      select(state, state.selected === key ? null : key, { address: true });
    }, true);

    pre.addEventListener('dblclick', function (event) {
      if (state.width && !nodeOf(event.target)) { fit(state, true); }
    });

    pre.addEventListener('keydown', function (event) {
      if (!state.width || event.ctrlKey || event.metaKey || event.altKey) { return; }
      var focused = nodeOf(event.target);
      var handled = true;
      switch (event.key) {
        case '+': case '=': zoomCentre(state, STEP); break;
        case '-': case '_': zoomCentre(state, 1 / STEP); break;
        case '0': fit(state, true); break;
        case 'ArrowLeft': pan(state, PAN_STEP, 0, true); break;
        case 'ArrowRight': pan(state, -PAN_STEP, 0, true); break;
        case 'ArrowUp': pan(state, 0, PAN_STEP, true); break;
        case 'ArrowDown': pan(state, 0, -PAN_STEP, true); break;
        case 'f': case 'F': toggleFullscreen(state); break;
        // S selects the node the focus is on, which is how a linked node is addressed
        // from the keyboard without following its link
        case 's': case 'S':
          if (focused) { select(state, focused.getAttribute('data-node'), { address: true }); } else { handled = false; }
          break;
        case 'Escape':
          if (state.wrap.classList.contains('is-expanded')) { setExpanded(state, false); }
          else if (state.selected) { select(state, null, { address: true }); }
          else { handled = false; }
          break;
        default: handled = false;
      }
      if (handled) { event.preventDefault(); }
    });

    pre.addEventListener('mouseover', function (event) {
      var node = nodeOf(event.target);
      if (node) { trace(state, node.getAttribute('data-node')); }
    });
    pre.addEventListener('mouseleave', function () { rest(state); });
    pre.addEventListener('focusin', function (event) {
      var node = nodeOf(event.target) || (event.target.querySelector ? event.target.querySelector('[data-node]') : null);
      if (node && event.target !== pre) { trace(state, node.getAttribute('data-node')); }
    });
    pre.addEventListener('focusout', function () { rest(state); });
  }

  // ---------------------------------------------------------------- drawing

  function draw(state) {
    queue = queue.then(function () {
      return loadMermaid().then(function (mermaid) {
        configure(mermaid);
        seq += 1;
        return mermaid.render('mjd-' + seq, state.source);
      }).then(function (result) {
        var stage = document.createElement('div');
        stage.className = 'mj-diagram-stage';
        stage.innerHTML = result.svg;
        var svg = stage.querySelector('svg');
        if (!svg) { throw new Error('Mermaid returned no drawing'); }
        frame(state);
        state.pre.textContent = '';
        state.pre.appendChild(stage);
        state.stage = stage;
        var box = svg.viewBox && svg.viewBox.baseVal;
        var measured = box && box.width ? box : svg.getBBox();
        state.width = measured.width;
        state.height = measured.height;
        svg.setAttribute('width', String(state.width));
        svg.setAttribute('height', String(state.height));
        svg.style.maxWidth = 'none';
        index(state);
        state.hot = null;
        state.rendered = true;
        state.pre.setAttribute('data-diagram-rendered', isDark() ? 'dark' : 'light');
        home(state);
        select(state, state.selected, {});
      }).catch(function (error) {
        // leave the source visible, as it was before this script ran
        if (!state.rendered) { state.pre.textContent = state.source; }
        state.pre.setAttribute('data-diagram-error', String(error && error.message || error));
        if (window.console) { window.console.error('[diagrams]', state.name, error); }
      });
    });
    return queue;
  }

  blocks.forEach(function (pre, i) {
    var name = pre.getAttribute('data-diagram') || (pre.id && pre.id.indexOf(PREFIX) === 0 ? pre.id.slice(PREFIX.length) : String(i + 1));
    if (!pre.id) { pre.id = PREFIX + name; }
    var declared = {};
    (pre.getAttribute('data-nodes') || '').split(/\s+/).forEach(function (id) { if (id) { declared[id] = true; } });
    var list = document.querySelector('[data-diagram-links="' + pre.id + '"]');
    var bar = document.querySelector('[data-diagram-bar="' + pre.id + '"]');
    var links = {};
    if (list) {
      Array.prototype.forEach.call(list.querySelectorAll('a[data-node]'), function (a) {
        links[a.getAttribute('data-node')] = { href: a.href, text: a.textContent.trim(), kind: a.getAttribute('data-kind') || 'page' };
      });
    }
    states.push({
      pre: pre, name: name, id: pre.id, source: pre.textContent, declared: declared, list: list, links: links,
      wrap: null, bar: bar, status: bar ? bar.querySelector('.mj-diagram-status') : null, stage: null, nodes: {}, edges: [], hot: null, selected: null,
      rest: '', statusTimer: 0, rendered: false, width: 0, height: 0, scale: 1, fit: 1, x: 0, y: 0
    });
  });

  // ---------------------------------------------------------------- the address

  // `#diagram-<name>` or `#diagram-<name>:<node>`. A diagram's name never holds a colon (the
  // build derives it from a slug), so the first one is the separator.
  function addressed() {
    var hash = window.location.hash.slice(1);
    if (hash.indexOf(PREFIX) !== 0) { return null; }
    var cut = hash.indexOf(':');
    var id = cut < 0 ? hash : hash.slice(0, cut);
    var node = cut < 0 ? null : hash.slice(cut + 1);
    try { id = decodeURIComponent(id); node = node == null ? null : decodeURIComponent(node); } catch (e) { /* as written */ }
    for (var i = 0; i < states.length; i++) {
      if (states[i].id === id) { return { state: states[i], node: node }; }
    }
    return null;
  }

  function open(target) {
    if (!target) { return; }
    var state = target.state;
    states.forEach(function (s) { s.pre.classList.toggle('is-addressed', s === state); });
    queue.then(function () {
      if (!state.rendered) { return; }
      for (var d = state.pre.closest('details'); d; d = d.parentElement && d.parentElement.closest('details')) { d.open = true; }
      (state.wrap || state.pre).scrollIntoView({ block: 'center' });
      if (target.node != null) { select(state, target.node, { centre: true }); }
    });
  }

  // The diagram an address names is drawn first; the rest follow in the page's order.
  var first = addressed();
  if (first) { draw(first.state); }
  states.forEach(function (state) { if (!first || state !== first.state) { draw(state); } });
  open(first);
  window.addEventListener('hashchange', function () { open(addressed()); });

  // ---------------------------------------------------------------- the page changing under it

  // Draw again in the other palette when the theme changes.
  new MutationObserver(function () {
    var theme = isDark() ? 'dark' : 'light';
    states.forEach(function (state) {
      if (state.rendered && state.pre.getAttribute('data-diagram-rendered') !== theme) {
        state.pre.setAttribute('data-diagram-rendered', theme);
        draw(state);
      }
    });
  }).observe(root, { attributes: true, attributeFilter: ['class'] });

  document.addEventListener('fullscreenchange', function () {
    states.forEach(function (state) {
      if (state.rendered) { window.requestAnimationFrame(function () { home(state); }); }
    });
  });

  if ('ResizeObserver' in window) {
    var resized = new ResizeObserver(function (entries) {
      entries.forEach(function (entry) {
        for (var i = 0; i < states.length; i++) {
          var state = states[i];
          if (state.pre !== entry.target || !state.rendered) { continue; }
          // the frame's own first report and a change of height are not a new width
          if (Math.abs((state.lastWidth || 0) - entry.contentRect.width) >= 1) { home(state); }
        }
      });
    });
    states.forEach(function (state) { resized.observe(state.pre); });
  }

  // What a test, a probe or another script on the page may ask: the diagrams by name, and a
  // promise that settles when every one of them has been drawn or has failed.
  window.mjDiagrams = {
    names: function () { return states.map(function (s) { return s.name; }); },
    ready: function () { return queue; },
    state: function (name) {
      for (var i = 0; i < states.length; i++) {
        var s = states[i];
        if (s.name === name) {
          return { name: s.name, rendered: s.rendered, scale: s.scale, fit: s.fit, x: s.x, y: s.y, selected: s.selected,
                   nodes: Object.keys(s.nodes), links: Object.keys(s.links), edges: s.edges.length };
        }
      }
      return null;
    }
  };
})();
