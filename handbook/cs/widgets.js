(function () {
  'use strict';
  var NS = 'http://www.w3.org/2000/svg';
  function $(id) { return document.getElementById(id); }
  function readJSON(id) {
    var el = $(id);
    if (!el) return null;
    try { return JSON.parse(el.textContent); } catch (e) { return null; }
  }
  function svg(tag, attrs, parent) {
    var el = document.createElementNS(NS, tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { el.setAttribute(k, attrs[k]); });
    if (parent) parent.appendChild(el);
    return el;
  }
  function html(tag, attrs, parent, text) {
    var el = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { el.setAttribute(k, attrs[k]); });
    if (text !== undefined) el.textContent = text;
    if (parent) parent.appendChild(el);
    return el;
  }
  function clear(el) { while (el.firstChild) el.removeChild(el.firstChild); }
  function line(term, text, cls) { return html('div', { 'class': 'ln' + (cls ? ' ' + cls : '') }, term, text); }
  function fmt(n) { return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ' '); }

  /* ---------- tooltip ---------- */
  var tip = html('div', { 'class': 'tip', role: 'status' }, document.body);
  tip.hidden = true;
  function showTip(evt, value, label) {
    clear(tip);
    html('b', null, tip, value);
    if (label) html('span', null, tip, label);
    tip.hidden = false;
    var x, y;
    if (evt.clientX !== undefined && evt.type !== 'focus') { x = evt.clientX; y = evt.clientY; }
    else { var r = evt.target.getBoundingClientRect(); x = r.left + r.width / 2; y = r.top; }
    var w = tip.offsetWidth, h = tip.offsetHeight;
    var left = Math.min(Math.max(8, x - w / 2), window.innerWidth - w - 8);
    var top = y - h - 12;
    if (top < 8) top = y + 16;
    tip.style.left = left + 'px';
    tip.style.top = top + 'px';
  }
  function hideTip() { tip.hidden = true; }
  function bindTip(el, value, label) {
    el.setAttribute('tabindex', '0');
    el.setAttribute('aria-label', label ? label + ': ' + value : value);
    el.addEventListener('pointermove', function (e) { showTip(e, value, label); });
    el.addEventListener('focus', function (e) { showTip(e, value, label); });
    el.addEventListener('pointerleave', hideTip);
    el.addEventListener('blur', hideTip);
  }

  /* ---------- chart helpers ---------- */
  function niceMax(v) {
    var p = Math.pow(10, Math.floor(Math.log10(v)));
    var steps = [1, 2, 2.5, 5, 10];
    for (var i = 0; i < steps.length; i++) if (steps[i] * p >= v) return steps[i] * p;
    return 10 * p;
  }
  function chartFrame(el, title, sub) {
    clear(el);
    var head = html('div', { 'class': 'chart-head' }, el);
    html('p', { 'class': 'chart-title' }, head, title);
    if (sub) html('p', { 'class': 'chart-sub' }, head, sub);
    var wrap = html('div', { 'class': 'graph-wrap' }, el);
    return wrap;
  }
  function tableToggle(el, headers, rows) {
    var btn = html('button', { type: 'button', 'class': 'btn ghost small' }, el, 'Zobrazit tabulku');
    var box = html('div', { 'class': 'table-wrap' }, el);
    box.hidden = true;
    var t = html('table', null, box);
    var tr = html('tr', null, html('thead', null, t));
    headers.forEach(function (h) { html('th', null, tr, h); });
    var tb = html('tbody', null, t);
    rows.forEach(function (r) {
      var row = html('tr', null, tb);
      r.forEach(function (c) { html('td', null, row, String(c)); });
    });
    btn.addEventListener('click', function () {
      box.hidden = !box.hidden;
      btn.textContent = box.hidden ? 'Zobrazit tabulku' : 'Skrýt tabulku';
    });
  }

  function hBars(el, opts) {
    var data = opts.data;
    var wrap = chartFrame(el, opts.title, opts.sub);
    var W = 720, rowH = 30, top = 8, labelW = opts.labelW || 150, right = 70;
    var H = top + data.length * rowH + 8;
    var s = svg('svg', { viewBox: '0 0 ' + W + ' ' + H, role: 'img', 'aria-label': opts.title }, wrap);
    var max = niceMax(Math.max.apply(null, data.map(function (d) { return d[1]; })));
    var plotW = W - labelW - right;
    [0, 0.25, 0.5, 0.75, 1].forEach(function (f) {
      var x = labelW + plotW * f;
      svg('line', { x1: x, x2: x, y1: top - 4, y2: H - 6, 'class': 'grid' }, s);
    });
    data.forEach(function (d, i) {
      var y = top + i * rowH;
      var t = svg('text', { x: labelW - 10, y: y + 19, 'text-anchor': 'end', 'class': 'ax mono' }, s);
      t.textContent = d[0];
      var w = Math.max(2, plotW * d[1] / max);
      var g = svg('g', { 'class': 'mark' }, s);
      svg('rect', { x: labelW, y: y + 4, width: plotW, height: rowH - 8, fill: 'transparent' }, g);
      svg('path', { d: roundedBarH(labelW, y + 6, w, Math.min(18, rowH - 12)), 'class': opts.cls ? opts.cls(d) : 's1' }, g);
      var v = svg('text', { x: labelW + w + 8, y: y + 19, 'class': 'val' }, s);
      v.textContent = fmt(d[1]);
      bindTip(g, fmt(d[1]) + (opts.unit ? ' ' + opts.unit : ''), d[0]);
    });
    tableToggle(el, opts.headers || ['Položka', 'Počet'], data);
  }
  function roundedBarH(x, y, w, h) {
    var r = Math.min(4, w);
    return 'M' + x + ' ' + y + 'H' + (x + w - r) + 'Q' + (x + w) + ' ' + y + ' ' + (x + w) + ' ' + (y + r) +
      'V' + (y + h - r) + 'Q' + (x + w) + ' ' + (y + h) + ' ' + (x + w - r) + ' ' + (y + h) + 'H' + x + 'Z';
  }
  function roundedBarV(x, y, w, h, base) {
    var r = Math.min(4, h, w / 2);
    return 'M' + x + ' ' + base + 'V' + (y + r) + 'Q' + x + ' ' + y + ' ' + (x + r) + ' ' + y +
      'H' + (x + w - r) + 'Q' + (x + w) + ' ' + y + ' ' + (x + w) + ' ' + (y + r) + 'V' + base + 'Z';
  }

  function columns(el, opts) {
    var data = opts.data;
    var wrap = chartFrame(el, opts.title, opts.sub);
    if (opts.legend) {
      var lg = html('div', { 'class': 'legend' }, el.querySelector('.chart-head'));
      opts.legend.forEach(function (l) {
        var it = html('span', { 'class': 'lg' }, lg);
        html('i', { 'class': 'sw ' + l[0] }, it);
        it.appendChild(document.createTextNode(l[1]));
      });
    }
    var W = 720, H = opts.height || 250, left = 44, right = 12, top = opts.markers ? 30 : 14, bottom = 34;
    var s = svg('svg', { viewBox: '0 0 ' + W + ' ' + H, role: 'img', 'aria-label': opts.title }, wrap);
    var max = niceMax(Math.max.apply(null, data.map(function (d) { return d.v; })));
    var plotW = W - left - right, plotH = H - top - bottom, base = top + plotH;
    for (var k = 0; k <= 4; k++) {
      var gy = base - plotH * k / 4;
      svg('line', { x1: left, x2: W - right, y1: gy, y2: gy, 'class': k === 0 ? 'axis' : 'grid' }, s);
      var tl = svg('text', { x: left - 6, y: gy + 4, 'text-anchor': 'end', 'class': 'ax' }, s);
      tl.textContent = fmt(Math.round(max * k / 4));
    }
    var band = plotW / data.length;
    var bw = Math.min(24, Math.max(3, band - 2));
    data.forEach(function (d, i) {
      var x = left + band * i + (band - bw) / 2;
      var h = plotH * d.v / max;
      var g = svg('g', { 'class': 'mark' }, s);
      svg('rect', { x: left + band * i, y: top, width: band, height: plotH, fill: 'transparent' }, g);
      if (d.v > 0) svg('path', { d: roundedBarV(x, base - h, bw, h, base), 'class': d.cls || 's1' }, g);
      bindTip(g, fmt(d.v) + (opts.unit ? ' ' + opts.unit : ''), d.label);
      if (d.tick) {
        var tt = svg('text', { x: x + bw / 2, y: base + 18, 'text-anchor': 'middle', 'class': 'ax' }, s);
        tt.textContent = d.tick;
      }
    });
    if (opts.markers) {
      opts.markers.forEach(function (m) {
        var cx = left + band * m.i + band / 2;
        svg('line', { x1: cx, x2: cx, y1: 14, y2: base, 'class': 'mk-line' }, s);
        var g = svg('g', { 'class': 'mark' }, s);
        svg('circle', { cx: cx, cy: 14, r: 10, fill: 'transparent' }, g);
        svg('circle', { cx: cx, cy: 14, r: 5, 'class': 's2 ring' }, g);
        bindTip(g, m.value, m.label);
      });
    }
    if (opts.refs) {
      opts.refs.forEach(function (r) {
        var rx = left + band * r.i + band / 2;
        svg('line', { x1: rx, x2: rx, y1: top - 6, y2: base, 'class': 'ref' }, s);
        var rt = svg('text', { x: rx + 5, y: top + 6, 'class': 'ref-t' }, s);
        rt.textContent = r.label;
      });
    }
    tableToggle(el, opts.headers, opts.rows || data.map(function (d) { return [d.label, d.v]; }));
  }

  function stepLine(el, opts) {
    var pts = opts.points;
    var wrap = chartFrame(el, opts.title, opts.sub);
    var W = 720, H = 250, left = 44, right = 40, top = 16, bottom = 34;
    var s = svg('svg', { viewBox: '0 0 ' + W + ' ' + H, role: 'img', 'aria-label': opts.title }, wrap);
    var t0 = pts[0].t, t1 = opts.end || pts[pts.length - 1].t;
    var max = niceMax(pts[pts.length - 1].v);
    var plotW = W - left - right, plotH = H - top - bottom, base = top + plotH;
    function X(t) { return left + plotW * (t - t0) / (t1 - t0); }
    function Y(v) { return base - plotH * v / max; }
    for (var k = 0; k <= 4; k++) {
      var gy = base - plotH * k / 4;
      svg('line', { x1: left, x2: W - right, y1: gy, y2: gy, 'class': k === 0 ? 'axis' : 'grid' }, s);
      var tl = svg('text', { x: left - 6, y: gy + 4, 'text-anchor': 'end', 'class': 'ax' }, s);
      tl.textContent = Math.round(max * k / 4);
    }
    (opts.ticks || []).forEach(function (tk) {
      var tx = svg('text', { x: X(tk.t), y: base + 18, 'text-anchor': 'middle', 'class': 'ax' }, s);
      tx.textContent = tk.label;
    });
    var d = 'M' + X(t0) + ' ' + Y(0), area = d;
    pts.forEach(function (p) { d += 'H' + X(p.t) + 'V' + Y(p.v); });
    d += 'H' + X(t1);
    area = d + 'V' + base + 'H' + X(t0) + 'Z';
    svg('path', { d: area, 'class': 'area1' }, s);
    svg('path', { d: d, 'class': 'line1' }, s);
    var last = pts[pts.length - 1];
    svg('circle', { cx: X(t1), cy: Y(last.v), r: 4, 'class': 's1 ring' }, s);
    var lt = svg('text', { x: X(t1) + 8, y: Y(last.v) + 4, 'class': 'val' }, s);
    lt.textContent = last.v;
    var cross = svg('line', { y1: top, y2: base, 'class': 'cross' }, s);
    var dot = svg('circle', { r: 5, 'class': 's1 ring' }, s);
    cross.style.display = dot.style.display = 'none';
    var hit = svg('rect', { x: left, y: top, width: plotW, height: plotH, fill: 'transparent', tabindex: '0', 'aria-label': opts.title }, s);
    function at(t) {
      var cur = pts[0];
      pts.forEach(function (p) { if (p.t <= t) cur = p; });
      return cur;
    }
    function move(e) {
      var r = s.getBoundingClientRect();
      var x = (e.clientX - r.left) * W / r.width;
      var t = t0 + (x - left) / plotW * (t1 - t0);
      var p = at(t);
      var px = X(Math.max(t0, Math.min(t1, t)));
      cross.setAttribute('x1', px); cross.setAttribute('x2', px);
      dot.setAttribute('cx', px); dot.setAttribute('cy', Y(p.v));
      cross.style.display = dot.style.display = '';
      showTip(e, p.v + ' ADR', opts.labelOf(Math.max(t0, Math.min(t1, t))));
    }
    hit.addEventListener('pointermove', move);
    hit.addEventListener('pointerleave', function () { cross.style.display = dot.style.display = 'none'; hideTip(); });
    tableToggle(el, opts.headers, opts.rows);
  }

  /* ---------- data charts ---------- */
  var repo = readJSON('data-repo');
  var DAY = 86400000;
  function d2t(s) { return Date.UTC(+s.slice(0, 4), +s.slice(5, 7) - 1, +s.slice(8, 10)); }
  function czDate(s) { return +s.slice(8, 10) + '. ' + +s.slice(5, 7) + '.'; }
  if (repo) {
    if ($('chart-cadence')) {
      var rel = {};
      repo.releases.forEach(function (r) { (rel[r[0]] = rel[r[0]] || []).push(r[1]); });
      var cad = repo.daily.map(function (d, i) {
        return { v: d[1], label: czDate(d[0]) + ' · commitů', tick: (i % 7 === 0) ? czDate(d[0]) : '' };
      });
      var marks = [];
      repo.daily.forEach(function (d, i) { if (rel[d[0]]) marks.push({ i: i, value: rel[d[0]].join(', '), label: 'vydání ' + czDate(d[0]) }); });
      columns($('chart-cadence'), {
        title: 'Commity za den a vydání', sub: 'bez merge commitů, 3. 9. – 12. 10. 2026',
        data: cad, markers: marks, unit: 'commitů', height: 270,
        legend: [['s1', 'commity za den'], ['s2 dot', 'vydání verze']],
        headers: ['Den', 'Commity', 'Vydání'],
        rows: repo.daily.map(function (d) { return [czDate(d[0]), d[1], (rel[d[0]] || []).join(', ')]; })
      });
    }
    if ($('chart-types')) hBars($('chart-types'), {
      title: 'Typy commitů', sub: fmt(repo.typesTotal) + ' commitů bez merge', data: repo.types,
      unit: 'commitů', headers: ['Typ', 'Commitů'], labelW: 110
    });
    if ($('chart-subject')) {
      var bins = [];
      for (var b = 0; b < 150; b += 5) bins.push({ v: 0, label: b + '–' + (b + 4) + ' znaků', tick: (b % 20 === 0) ? String(b) : '' });
      repo.subject.forEach(function (p) { var i = Math.min(29, Math.floor(p[0] / 5)); bins[i].v += p[1]; });
      columns($('chart-subject'), {
        title: 'Délka subjectu commitu', sub: 'počet commitů podle délky hlavičky ve znacích',
        data: bins, unit: 'commitů',
        refs: [{ i: 14.4 - 0.5, label: 'medián 72' }, { i: 20 - 0.5, label: 'limit 100' }],
        headers: ['Délka', 'Commitů'], rows: bins.map(function (x) { return [x.label, x.v]; })
      });
    }
    if ($('chart-adr')) {
      var acc = 0, apts = [];
      repo.adr.forEach(function (a) { acc += a[1]; apts.push({ t: d2t(a[0]), v: acc, day: a[0], n: a[1] }); });
      var ticks = [];
      ['2026-09-05', '2026-09-15', '2026-09-25', '2026-10-05'].forEach(function (s) { ticks.push({ t: d2t(s), label: czDate(s) }); });
      stepLine($('chart-adr'), {
        title: 'ADR celkem', sub: 'kumulativně podle data prvního commitu souboru',
        points: apts, end: d2t('2026-10-12'), ticks: ticks,
        labelOf: function (t) { var dt = new Date(t); return 'do ' + dt.getUTCDate() + '. ' + (dt.getUTCMonth() + 1) + '.'; },
        headers: ['Den', 'Nových ADR', 'Celkem'],
        rows: apts.map(function (p) { return [czDate(p.day), p.n, p.v]; })
      });
    }
    var proofOpts = {
      title: 'Stav důkazu pravidel', sub: '%%rules%% pravidel, stav důkazu přepočítaný ze stromu commitu %%commit%%',
      data: repo.proof, unit: 'pravidel', headers: ['Stav', 'Pravidel'], labelW: 120
    };
    if ($('chart-proof')) hBars($('chart-proof'), proofOpts);
    if ($('chart-proof-mini')) hBars($('chart-proof-mini'), proofOpts);
    if ($('chart-jobs')) hBars($('chart-jobs'), {
      title: 'CI gates podle jobu', sub: '%%gates%% gates z gates.yaml', data: repo.jobs,
      unit: 'gates', headers: ['Job', 'Gates'], labelW: 120
    });
  }

  /* ---------- chapter graph ---------- */
  var graph = readJSON('data-graph');
  if (graph && $('chapter-graph')) (function () {
    var s = $('chapter-graph');
    var parts = graph.parts, nodes = graph.nodes, edges = graph.edges;
    var maxRows = Math.max.apply(null, parts.map(function (p) { return p.count; }));
    var W = 720, top = 46, rowH = 28, H = top + maxRows * rowH + 10;
    s.setAttribute('viewBox', '0 0 ' + W + ' ' + H);
    var colW = (W - 40) / parts.length;
    var pos = {}, deg = {};
    edges.forEach(function (e) { deg[e[0]] = (deg[e[0]] || 0) + 1; deg[e[1]] = (deg[e[1]] || 0) + 1; });
    parts.forEach(function (p, i) {
      var x = 20 + colW * i + colW / 2;
      var t = svg('text', { x: x, y: 16, 'text-anchor': 'middle', 'class': 'ax strong' }, s);
      t.textContent = String(i + 1);
      var t2 = svg('text', { x: x, y: 32, 'text-anchor': 'middle', 'class': 'ax' }, s);
      t2.textContent = p.short;
    });
    var counters = {};
    nodes.forEach(function (n) {
      var k = counters[n.part] = (counters[n.part] || 0) + 1;
      pos[n.id] = { x: 20 + colW * n.part + colW / 2, y: top + (k - 1) * rowH + 12 };
    });
    var gEdges = svg('g', null, s), gNodes = svg('g', null, s);
    var edgeEls = edges.map(function (e) {
      var a = pos[e[0]], b = pos[e[1]];
      var mx = (a.x + b.x) / 2;
      var d = a.x === b.x
        ? 'M' + a.x + ' ' + a.y + 'C' + (a.x + 26) + ' ' + a.y + ' ' + (b.x + 26) + ' ' + b.y + ' ' + b.x + ' ' + b.y
        : 'M' + a.x + ' ' + a.y + 'C' + mx + ' ' + a.y + ' ' + mx + ' ' + b.y + ' ' + b.x + ' ' + b.y;
      return { e: e, el: svg('path', { d: d, 'class': 'g-edge' }, gEdges) };
    });
    var label = svg('text', { 'class': 'g-label' }, s);
    var readout = $('graph-readout');
    function focus(id) {
      var nb = {};
      nb[id] = true;
      edgeEls.forEach(function (x) {
        var on = x.e[0] === id || x.e[1] === id;
        x.el.setAttribute('class', on ? 'g-edge on' : 'g-edge dim');
        if (on) { nb[x.e[0]] = true; nb[x.e[1]] = true; }
      });
      nodeEls.forEach(function (x) { x.c.setAttribute('class', nb[x.n.id] ? (x.n.id === id ? 'g-node cur' : 'g-node nb') : 'g-node dim'); });
      var p = pos[id], n = byId[id];
      label.textContent = n.label;
      label.setAttribute('x', Math.min(Math.max(p.x, 80), W - 80));
      label.setAttribute('y', p.y - 12);
      label.setAttribute('text-anchor', 'middle');
      var names = Object.keys(nb).filter(function (k) { return k !== id; }).map(function (k) { return byId[k].label; });
      readout.textContent = n.label + ' (' + parts[n.part].name + ') · vazby: ' + (names.length ? names.join(', ') : 'žádné');
    }
    function reset0() {
      edgeEls.forEach(function (x) { x.el.setAttribute('class', 'g-edge'); });
      nodeEls.forEach(function (x) { x.c.setAttribute('class', 'g-node'); });
      label.textContent = '';
    }
    var byId = {};
    nodes.forEach(function (n) { byId[n.id] = n; });
    var nodeEls = nodes.map(function (n) {
      var p = pos[n.id];
      var a = svg('a', { href: '#' + n.id }, gNodes);
      svg('circle', { cx: p.x, cy: p.y, r: 12, fill: 'transparent' }, a);
      var c = svg('circle', { cx: p.x, cy: p.y, r: 3.5 + Math.min(6, (deg[n.id] || 0) * 0.6), 'class': 'g-node' }, a);
      a.setAttribute('aria-label', n.label);
      a.addEventListener('pointerenter', function () { if (!pathIds) { focus(n.id); decorate(n.id); } });
      a.addEventListener('focus', function () { if (!pathIds) { focus(n.id); decorate(n.id); } });
      a.addEventListener('pointerleave', restore);
      a.addEventListener('blur', restore);
      a.addEventListener('click', function (e) {
        e.preventDefault();
        if (pathIds) { clearTemp(); pathIds = null; pinned = n.id; restore(); return; }
        if (pinned === n.id) { go(n.id); return; }
        if (pinned) { var p = bfs(pinned, n.id); if (p) { showPath(p, 'Nejkratší cesta'); pinned = null; return; } }
        pinned = n.id; restore();
      });
      return { n: n, c: c };
    });
    var pinned = null, pathIds = null, temp = [];
    var adj = {};
    edges.forEach(function (e) { (adj[e[0]] = adj[e[0]] || []).push(e[1]); (adj[e[1]] = adj[e[1]] || []).push(e[0]); });
    function go(id) { if (window.mjJump) window.mjJump(id); else { var t = $(id); if (t) t.scrollIntoView(); } }
    function linkTo(id, parent) {
      var a = html('a', { href: '#' + id }, parent, byId[id].label);
      a.addEventListener('click', function (e) { e.preventDefault(); go(id); });
      return a;
    }
    function decorate(id) {
      if (pinned !== id) return;
      readout.appendChild(document.createTextNode(' · '));
      var a = linkTo(id, readout); a.textContent = 'Otevřít kapitolu →';
      readout.appendChild(document.createTextNode(' · klikni na jinou kapitolu a ukážu cestu mezi nimi'));
    }
    function clearTemp() { temp.forEach(function (t) { t.parentNode.removeChild(t); }); temp = []; }
    function restore() {
      if (pathIds) return;
      if (pinned) { focus(pinned); decorate(pinned); }
      else { reset0(); readout.textContent = 'Najeď na kapitolu, klikni pro připnutí. Druhý klik na jinou kapitolu ukáže cestu mezi nimi.'; }
    }
    function bfs(a, b) {
      var prev = {}, q = [a]; prev[a] = null;
      while (q.length) {
        var x = q.shift();
        if (x === b) { var out = []; while (x) { out.unshift(x); x = prev[x]; } return out; }
        (adj[x] || []).forEach(function (y) { if (!(y in prev)) { prev[y] = x; q.push(y); } });
      }
      return null;
    }
    function showPath(ids, title) {
      clearTemp(); pathIds = ids;
      var on = {}; ids.forEach(function (i) { on[i] = true; });
      edgeEls.forEach(function (x) { x.el.setAttribute('class', 'g-edge dim'); });
      nodeEls.forEach(function (x) { x.c.setAttribute('class', on[x.n.id] ? (x.n.id === ids[0] || x.n.id === ids[ids.length - 1] ? 'g-node cur' : 'g-node nb') : 'g-node dim'); });
      label.textContent = '';
      for (var i = 0; i < ids.length - 1; i++) {
        var a = pos[ids[i]], b = pos[ids[i + 1]], mx = (a.x + b.x) / 2;
        var d = a.x === b.x ? 'M' + a.x + ' ' + a.y + 'C' + (a.x + 30) + ' ' + a.y + ' ' + (b.x + 30) + ' ' + b.y + ' ' + b.x + ' ' + b.y
          : 'M' + a.x + ' ' + a.y + 'C' + mx + ' ' + a.y + ' ' + mx + ' ' + b.y + ' ' + b.x + ' ' + b.y;
        temp.push(svg('path', { d: d, 'class': 'g-edge on flow' }, gEdges));
      }
      ids.forEach(function (id, k) {
        var p = pos[id];
        var t = svg('text', { x: p.x + 11, y: p.y - 7, 'class': 'g-step' }, s); t.textContent = k + 1; temp.push(t);
      });
      clear(readout);
      html('b', null, readout, (title || 'Cesta') + ': ');
      ids.forEach(function (id, k) { if (k) readout.appendChild(document.createTextNode(' → ')); linkTo(id, readout); });
      var x = html('button', { type: 'button', 'class': 'btn ghost small', style: 'margin-left:10px' }, readout, 'Zrušit');
      x.addEventListener('click', function () { clearTemp(); pathIds = null; pinned = null; restore(); });
    }
    window.mjGraph = { showPath: showPath, has: function (id) { return !!byId[id]; } };
    restore();
  })();

  /* ---------- session simulator ---------- */
  var STEPS = [
    { t: 'SessionStart', who: 'provider (hook)', lane: 'p', what: 'Otevře epizodu, zmrazí snímek kontextu, vypíše briefing, který provider vloží do kontextu, a zajistí běžící sdílený server.', writes: ['.ai/local/state/sessions-open/<provider session>.yaml', '.ai/local/session-contexts/<stamp>--<session>.md', '.ai/local/state/ledger.jsonl  provider.event.received'], gate: false },
    { t: 'UserPromptSubmit', who: 'provider (hook)', lane: 'p', what: 'Uloží prompt jako dva soubory pod jedním kmenem. Nikdy nevrátí exit 2, jinak by zahodil prompt člověka.', writes: ['.ai/local/prompts/<stamp>-<slug>.json', '.ai/local/prompts/<stamp>-<slug>.md'], gate: false },
    { t: 'majordomus context', who: 'agent', lane: 'a', what: 'Briefing v pořadí autority: git, task a profil, blokery, autorské záznamy, historie. Jen čte.', writes: [], gate: false },
    { t: 'majordomus_announce', who: 'agent přes MCP', lane: 'a', what: 'Záměr a cesty na boardu. Odpověď vyjmenuje peery, jejichž scope se potká. Žije jen v paměti serveru.', writes: ['(paměť sdíleného serveru, nic na disku)'], gate: false },
    { t: 'majordomus start', who: 'agent', lane: 'g', what: 'Založí task se scope, profilem a závazky. Odmítne, když jiný task běží nebo token závazku neexistuje.', writes: ['.ai/local/state/current.yaml', '.ai/local/state/ledger.jsonl  task.started'], gate: true },
    { t: 'práce + PreCompact', who: 'agent, provider (hook)', lane: 'p', what: 'Checkpointy, rozhodnutí s důvodem, otevřené otázky. Když provider komprimuje konverzaci, hook zapíše checkpoint a odvodí kandidáty znalostí.', writes: ['.ai/local/state/checkpoints/', '.ai/local/state/decisions.md', '.ai/local/state/open-questions.md', '.ai/repo/knowledge/candidates/'], gate: false },
    { t: 'majordomus check', who: 'agent', lane: 'g', what: 'Scope, závazky a stav gates. Soubor mimo scope je exit 10.', writes: [], gate: true },
    { t: 'commit a push', who: 'git (hooky)', lane: 'g', what: 'pre-commit: doctor, worktree guard, pages current. commit-msg: commit.validate. pre-push: finish --check.', writes: ['git objekty a refy'], gate: true },
    { t: 'majordomus finish', who: 'agent', lane: 'g', what: 'Projde finish contract řádek po řádku. Při selhání nezapíše nic.', writes: ['.ai/local/state/current.yaml  outcome', '.ai/local/state/ledger.jsonl  task.finished'], gate: true },
    { t: 'SessionEnd', who: 'provider (hook)', lane: 'p', what: 'Checkpoint, kandidáti znalostí, odvozený handover a uzavření epizody do neměnné obálky referencí.', writes: ['.ai/local/state/checkpoints/', '.ai/repo/knowledge/candidates/', '.ai/local/state/handovers/', '.ai/repo/sessions/<epizoda>.md'], gate: false }
  ];
  if ($('lab-session')) (function () {
    var s = $('session-svg'), panel = $('sess-panel'), count = $('sess-count');
    var cur = 0, timer = null;
    var xs = STEPS.map(function (_, i) { return 46 + i * (628 / (STEPS.length - 1)); });
    svg('line', { x1: xs[0], x2: xs[xs.length - 1], y1: 60, y2: 60, 'class': 'axis' }, s);
    var dots = STEPS.map(function (st, i) {
      var a = svg('g', { 'class': 'mark', tabindex: '0', role: 'button', 'aria-label': (i + 1) + '. ' + st.t }, s);
      svg('circle', { cx: xs[i], cy: 60, r: 16, fill: 'transparent' }, a);
      var c = svg('circle', { cx: xs[i], cy: 60, r: 8, 'class': st.gate ? 'st-gate' : (st.lane === 'p' ? 'st-prov' : 'st-agent') }, a);
      var n = svg('text', { x: xs[i], y: 36, 'text-anchor': 'middle', 'class': 'ax strong' }, s);
      n.textContent = i + 1;
      var l = svg('text', { x: xs[i], y: (i % 2 ? 108 : 92), 'text-anchor': 'middle', 'class': 'ax mono small-t' }, s);
      l.textContent = st.t.replace('majordomus ', '');
      a.addEventListener('click', function () { stop(); go(i); });
      a.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); stop(); go(i); } });
      return c;
    });
    var lg = [['st-prov', 'hook providera: jen zapisuje'], ['st-agent', 'agent: čte nebo ohlašuje'], ['st-gate', 'brána: může odmítnout']];
    lg.forEach(function (x, i) {
      svg('circle', { cx: 60 + i * 220, cy: 136, r: 5, 'class': x[0] }, s);
      var t = svg('text', { x: 72 + i * 220, y: 140, 'class': 'ax' }, s);
      t.textContent = x[1];
    });
    function go(i) {
      cur = (i + STEPS.length) % STEPS.length;
      dots.forEach(function (c, k) { c.setAttribute('r', k === cur ? 11 : 7); c.style.opacity = k <= cur ? 1 : 0.45; });
      var st = STEPS[cur];
      clear(panel);
      var h = html('p', { 'class': 'pn-h' }, panel);
      html('b', null, h, (cur + 1) + '. ' + st.t);
      html('span', { 'class': 'pill ' + (st.gate ? 'refuses' : 'reports') }, h, st.gate ? 'může odmítnout' : 'neodmítá');
      html('p', { 'class': 'pn-who' }, panel, 'Spouští: ' + st.who);
      html('p', null, panel, st.what);
      var term = html('div', { 'class': 'term' }, panel);
      if (!st.writes.length) line(term, '(nic nezapisuje, jen čte)', 'mut');
      st.writes.forEach(function (w) { line(term, '+ ' + w, w.indexOf('.ai/repo') === 0 ? 'ok' : ''); });
      if (st.writes.some(function (w) { return w.indexOf('.ai/repo') === 0; })) line(term, 'zeleně: trackovaná polovina .ai/repo, cestuje gitem', 'mut');
      count.textContent = 'krok ' + (cur + 1) + ' z ' + STEPS.length;
    }
    function stop() { if (timer) { clearInterval(timer); timer = null; $('sess-play').textContent = '▶ Přehrát'; } }
    $('sess-prev').addEventListener('click', function () { stop(); go(cur - 1); });
    $('sess-next').addEventListener('click', function () { stop(); go(cur + 1); });
    $('sess-play').addEventListener('click', function () {
      if (timer) { stop(); return; }
      $('sess-play').textContent = '❚❚ Pauza';
      if (cur === STEPS.length - 1) go(0);
      timer = setInterval(function () { if (cur === STEPS.length - 1) { stop(); return; } go(cur + 1); }, 2200);
    });
    go(0);
  })();

  /* ---------- glob helpers ---------- */
  function globRe(p) {
    var dir = /\/$/.test(p);
    var src = '', i = 0;
    while (i < p.length) {
      if (p.substr(i, 3) === '**/') { src += '(?:.*/)?'; i += 3; continue; }
      if (p.substr(i, 2) === '**') { src += '.*'; i += 2; continue; }
      var c = p[i];
      if (c === '*') src += '[^/]*';
      else if (c === '?') src += '[^/]';
      else src += c.replace(/[.+^${}()|[\]\\]/g, '\\$&');
      i++;
    }
    return new RegExp('^' + src + (dir ? '.*' : '') + '$');
  }
  function matches(p, path) { return globRe(p).test(path); }

  /* ---------- repository scope ---------- */
  if ($('lab-scope')) (function () {
    var IN = ['.ai/manifest.yaml', '.ai/README.md', '.ai/repo/**', 'src/**', 'lib/**', 'apps/**/src/**', 'test/**', 'docs/**', 'README.md', 'AGENTS.md', 'Cargo.toml', 'justfile'];
    var OUTP = ['.git/', '.ai/local/', '**/node_modules/', '**/target/', 'vendor/'];
    var CAT_PATHS = { generated: ['docs/generated/', 'site/data/generated/'] };
    var CAT_NAMES = [['secret', ['.env', '.env.*', '*.pem', '*.key', 'id_rsa*']], ['generated', ['*.min.js', '*.map']], ['archive', ['*.zip', '*.tar.gz', '*.jar']], ['image', ['*.png', '*.svg']], ['video', ['*.mp4']], ['pdf', ['*.pdf']], ['database_dump', ['*.dump', '*.sqlite']]];
    function judge(path) {
      path = path.trim().replace(/^\.\//, '');
      if (!path) return null;
      if (/^\/|(^|\/)\.\.(\/|$)/.test(path)) return { out: true, step: 0, why: 'cesta s úvodním lomítkem nebo segmentem .. je odmítnuta' };
      var base = path.split('/').pop();
      for (var i = 0; i < OUTP.length; i++) if (matches(OUTP[i], path)) return { out: true, step: 1, why: 'path', rule: 'out.paths: ' + OUTP[i] };
      for (var k in CAT_PATHS) for (var j = 0; j < CAT_PATHS[k].length; j++) if (matches(CAT_PATHS[k][j], path)) return { out: true, step: 1, why: k, rule: k + '.paths: ' + CAT_PATHS[k][j] };
      for (var c = 0; c < CAT_NAMES.length; c++) for (var n = 0; n < CAT_NAMES[c][1].length; n++) if (matches(CAT_NAMES[c][1][n], base)) return { out: true, step: 2, why: CAT_NAMES[c][0], rule: CAT_NAMES[c][0] + '.names: ' + CAT_NAMES[c][1][n] };
      for (var m = 0; m < IN.length; m++) if (matches(IN[m], path)) return { out: false, step: 3, rule: 'in: ' + IN[m] };
      return { out: true, step: 3, why: 'undeclared', rule: 'žádný pathspec v in' };
    }
    var inp = $('scope-path'), out = $('scope-out');
    function render() {
      clear(out);
      var r = judge(inp.value);
      if (!r) { line(out, 'zadej cestu', 'mut'); return; }
      if (r.out) {
        line(out, 'OUT · ' + (r.why || ''), 'bad');
        if (r.rule) line(out, 'rozhodl krok ' + r.step + ': ' + r.rule);
        line(out, r.why === 'secret' ? 'nikdy se nečte ani neservíruje; trackovaný soubor je varování tracked_secret' : 'neindexuje se a nenajde se v žádné projekci (MCP, HTTP, CLI)', 'mut');
      } else {
        line(out, 'IN', 'ok');
        line(out, 'rozhodl krok 3: ' + r.rule);
        if (/(^|\/)(fixtures|testdata)\//.test(inp.value)) line(out, 'pozor: fixture nad 64 KiB by byl fixture_over_limit (velikost tady nekontroluju)', 'mut');
      }
    }
    var presets = ['.env.production', 'docs/generated/cli.md', 'apps/majordomus-cli/src/main.rs', 'site/assets/logo.svg', 'docs/MESH.md', 'apps/web/node_modules/x/index.js', 'notes/todo.txt', 'test/fixtures/repo.json', '.ai/local/state/current.yaml', 'dist/app.min.js'];
    presets.forEach(function (p) {
      var b = html('button', { type: 'button', 'class': 'chip' }, $('scope-presets'), p);
      b.addEventListener('click', function () { inp.value = p; render(); });
    });
    inp.addEventListener('input', render);
    render();

    var a = $('ov-a'), b2 = $('ov-b'), o = $('ov-out');
    function norm(x) { return x.trim().replace(/^\.\//, '').replace(/\/+$/, ''); }
    function ov() {
      clear(o);
      var x = norm(a.value), y = norm(b2.value);
      if (!x || !y) { line(o, 'zadej oba scope', 'mut'); return; }
      if (x === y) { line(o, 'POTKAJÍ SE · scope jsou stejné', 'bad'); }
      else if (y.indexOf(x + '/') === 0) { line(o, 'POTKAJÍ SE · A obsahuje B', 'bad'); }
      else if (x.indexOf(y + '/') === 0) { line(o, 'POTKAJÍ SE · B obsahuje A', 'bad'); }
      else {
        line(o, 'NEPOTKAJÍ SE', 'ok');
        if (x.indexOf(y) === 0 || y.indexOf(x) === 0) line(o, 'jeden je prefixem druhého jako řetězec, ale ne jako cesta: rozhoduje hranice segmentu', 'mut');
        return;
      }
      line(o, 'board a start to ohlásí; v meshi by exkluzivní claim skončil claim_conflict (exit 10)', 'mut');
    }
    a.addEventListener('input', ov); b2.addEventListener('input', ov);
    ov();
  })();

  /* ---------- profiles ---------- */
  if ($('lab-profiles')) (function () {
    var P = {
      routine: { d: 'malé, dobře pochopené změny: přejmenování, bump, formulace, jednořádkové opravy', cap: 1, eff: 1, verb: 1, pres: 'machine', cp: 30, req: ['ověřovací příkaz, jen když se změnily soubory'], out: ['outcome', 'changes', 'verification'] },
      implementation: { d: 'popsaná funkce nebo změna ve známém návrhu', cap: 2, eff: 2, verb: 2, pres: 'engineering', cp: 15, req: ['ověřovací příkaz'], out: ['outcome', 'changes', 'verification', 'blockers', 'next_action'] },
      debugging: { d: 'reprodukovat, izolovat, opravit a dokázat opravu vady', cap: 3, eff: 3, verb: 2, pres: 'engineering', cp: 15, req: ['ověřovací příkaz', 'regresní test'], out: ['outcome', 'changes', 'verification', 'blockers', 'next_action'] },
      'deep-work': { d: 'návrh, architektura, průřezová změna, problém bez známého tvaru', cap: 4, eff: 3, verb: 3, pres: 'engineering', cp: 30, req: ['ověřovací příkaz', 'záznam rozhodnutí'], out: ['outcome', 'changes', 'verification', 'blockers', 'next_action', 'decisions'] }
    };
    var CAP = ['fast', 'standard', 'strong', 'strongest'], EFF = ['low', 'medium', 'high', 'xhigh', 'max'], VERB = ['terse', 'concise', 'detailed'];
    var seg = $('prof-seg'), body = $('prof-body');
    function meter(parent, label, value, scale) {
      var row = html('div', { 'class': 'meter-row' }, parent);
      html('span', { 'class': 'm-l' }, row, label);
      var m = html('span', { 'class': 'meter', role: 'img', 'aria-label': label + ': ' + scale[value - 1] }, row);
      scale.forEach(function (_, i) { html('i', { 'class': i < value ? 'on' : '' }, m); });
      html('code', null, row, scale[value - 1]);
    }
    function show(name) {
      Array.prototype.forEach.call(seg.children, function (b) { b.setAttribute('aria-selected', b.dataset.k === name ? 'true' : 'false'); });
      var p = P[name];
      clear(body);
      html('p', null, body, p.d);
      meter(body, 'třída modelu', p.cap, CAP);
      meter(body, 'effort', p.eff, EFF);
      meter(body, 'verbosity', p.verb, VERB);
      var row = html('div', { 'class': 'meter-row' }, body);
      html('span', { 'class': 'm-l' }, row, 'checkpoint');
      var m = html('span', { 'class': 'meter' }, row);
      [15, 30].forEach(function (v) { html('i', { 'class': v <= p.cp ? 'on' : '' }, m); });
      html('code', null, row, 'každých ' + p.cp + ' min');
      var t = html('div', { 'class': 'term' }, body);
      line(t, 'presentation: ' + p.pres);
      line(t, 'finish vyžaduje: ' + p.req.join(', '), 'ok');
      line(t, 'output_contract: [' + p.out.join(', ') + ']');
    }
    Object.keys(P).forEach(function (k) {
      var b = html('button', { type: 'button', role: 'tab', 'class': 'seg-b' }, seg, k);
      b.dataset.k = k;
      b.addEventListener('click', function () { show(k); });
    });
    show('implementation');
  })();

  /* ---------- finish contract ---------- */
  if ($('lab-finish')) (function () {
    var LINES = [['scope_respected', 'žádný soubor mimo scope'], ['verification_ran', 'ověřovací příkaz prošel'], ['state_updated', 'záznamy odpovídají práci'], ['no_open_blockers', 'žádná otevřená otázka'], ['note_present', 'závěrečná poznámka existuje'], ['obligations_met', 'závazky z --requires splněny'], ['gates_passed', 'gates prošly (queued ne)'], ['publication_current', 'publikovaný výstup sedí']];
    var fs = $('fin-checks'), out = $('fin-out'), sel = $('fin-outcome');
    var boxes = LINES.map(function (l, i) {
      var lab = html('label', { 'class': 'chk' }, fs);
      var cb = html('input', { type: 'checkbox', id: 'fin-' + l[0] }, lab);
      cb.checked = i !== 6;
      lab.appendChild(document.createTextNode(' ' + l[0]));
      cb.addEventListener('change', render);
      return cb;
    });
    sel.addEventListener('change', render);
    function render() {
      clear(out);
      var oc = sel.value;
      line(out, '$ majordomus finish --outcome ' + oc + (oc === 'completed' ? ' --verify-command "…"' : ''), 'mut');
      if (oc === 'completed') {
        var fail = 0;
        LINES.forEach(function (l, i) {
          var ok = boxes[i].checked;
          if (!ok) fail++;
          line(out, (ok ? 'PASS ' : 'FAIL ') + l[0] + ' · ' + l[1], ok ? 'ok' : 'bad');
        });
        if (fail) { line(out, 'finish odmítnut: ' + fail + ' řádek' + (fail > 1 ? 'y' : '') + ' neprošl' + (fail > 1 ? 'y' : '') + '. Nic se nezapsalo.', 'bad'); line(out, 'exit 10', 'mut'); }
        else { line(out, 'outcome completed zapsán do current.yaml, task.finished do ledgeru', 'ok'); line(out, 'exit 0', 'mut'); }
        return;
      }
      var note = boxes[4].checked;
      if (!boxes[0].checked) line(out, 'WARN soubory mimo scope (u ' + oc + ' jen varování, task se zavře)', 'warn');
      if (!note) {
        line(out, 'FAIL chybí poznámka: # Next Action (co dál) nebo # Reason (proč)', 'bad');
        line(out, 'nic se nezapsalo · exit 10', 'mut');
        return;
      }
      var meaning = { partial: 'část hotová, zbytek je v # Next Action', blocked: 'nejde pokračovat bez něčeho mimo task', no_match: 'práce proběhla, hledaná věc neexistuje', failed: 'práci nešlo udělat', handed_over: 'pokračuje jiná session podle handoveru' };
      line(out, 'outcome ' + oc + ' zapsán: ' + meaning[oc], 'ok');
      line(out, 'ostatní řádky kontraktu se u ' + oc + ' nevyhodnocují: je to poctivé přiznání, že hotovo není', 'mut');
    }
    render();
  })();

  /* ---------- commit-msg ---------- */
  if ($('lab-commit')) (function () {
    var scopes = (repo && repo.scopes) || [];
    var msg = $('cm-msg'), paths = $('cm-paths'), out = $('cm-out');
    function render() {
      clear(out);
      var text = msg.value.replace(/^#.*$/mg, '').replace(/\s+$/, '');
      var lines = text.split('\n');
      var header = lines[0] || '';
      var body = lines.slice(1).join('\n').trim();
      var files = paths.value.split(',').map(function (x) { return x.trim(); }).filter(Boolean);
      var F = [];
      var m = header.match(/^([a-z]+)(\(([^)]+)\))?(!)?: (.*)$/);
      if (!m) F.push(['error', 'commit.not_conventional', 'hlavička není type(scope): subject']);
      else {
        var type = m[1], scope = m[3], bang = !!m[4], subj = m[5];
        if (!subj.trim()) F.push(['error', 'commit.subject_empty', 'za dvojtečkou nic není']);
        if (/\.$/.test(subj)) F.push(['error', 'commit.subject_trailing_period', 'subject je titulek, ne věta']);
        if (scope && scopes.length && scopes.indexOf(scope) === -1) F.push(['warning', 'commit.unknown_scope', 'scope „' + scope + '“ historie nepoužila aspoň třikrát']);
        if (bang && !body && !/BREAKING CHANGE:/.test(text)) F.push(['error', 'commit.breaking_unexplained', '! bez těla a bez BREAKING CHANGE:']);
        if (type === 'fix' && !files.some(function (f) { return /(^|\/)(tests?|spec)\/|_test\.|\.test\.|_spec\.|(^|\/)test\/cases\//.test(f); })) F.push(['warning', 'commit.fix_without_test', 'fix bez testu mezi staged soubory']);
      }
      if ([...header].length > 100) F.push(['error', 'commit.subject_too_long', 'hlavička má ' + [...header].length + ' znaků, limit je 100']);
      line(out, '$ majordomus-cli commit validate .git/COMMIT_EDITMSG --paths ' + (files.join(',') || '—'), 'mut');
      line(out, 'hlavička: ' + [...header].length + ' / 100 znaků', 'mut');
      if (!F.length) { line(out, 'OK · žádný nález, commit projde', 'ok'); return; }
      var err = F.some(function (f) { return f[0] === 'error'; });
      F.forEach(function (f) { line(out, (f[0] === 'error' ? 'ERROR ' : 'WARN  ') + f[1] + ' · ' + f[2], f[0] === 'error' ? 'bad' : 'warn'); });
      line(out, err ? 'exit 10 · commit přerušen' : 'exit 0 · commit projde s varováním', err ? 'bad' : 'mut');
    }
    var presets = [
      ['oprava s testem', 'fix(mesh): a claim of a dead holder expires instead of blocking', 'apps/majordomus-cli/src/mesh/claims.rs, test/cases/412_mesh_claims.sh'],
      ['breaking bez vysvětlení', 'feat(cli)!: rename serve to server', 'apps/majordomus-cli/src/commands/serve.rs'],
      ['ne-konvenční', 'Fixed stuff in the mesh', 'docs/MESH.md'],
      ['neznámý scope', 'docs(marketing): new landing copy', 'docs/README.md'],
      ['dlouhý subject', 'feat(context): the briefing names every section it dropped, the reason it dropped it and the command that would bring it back into view', 'lib/context.sh']
    ];
    presets.forEach(function (p) {
      var b = html('button', { type: 'button', 'class': 'chip' }, $('cm-presets'), p[0]);
      b.addEventListener('click', function () { msg.value = p[1]; paths.value = p[2]; render(); });
    });
    msg.addEventListener('input', render); paths.addEventListener('input', render);
    render();
  })();

  /* ---------- CI planner ---------- */
  var gatesData = readJSON('data-gates');
  if (gatesData && $('lab-ci')) (function () {
    var G = {};
    gatesData.gates.forEach(function (g) { G[g.id] = g; });
    var ta = $('ci-paths'), out = $('ci-out');
    function plan(paths) {
      var sel = {}, why = [], full = null;
      paths.forEach(function (p) {
        var hit = gatesData.classes.filter(function (c) { return c.p.some(function (pat) { return matches(pat, p); }); });
        why.push([p, hit.map(function (c) { return c.id; })]);
        if (!hit.length) full = full || ('cesta „' + p + '“ nesedí na žádnou třídu');
        hit.forEach(function (c) {
          if (c.g === 'full') full = full || ('třída „' + c.id + '“ eskaluje na plný plán');
          else c.g.forEach(function (g) { sel[g] = true; });
        });
      });
      if (full) {
        sel = {};
        gatesData.gates.forEach(function (g) { if (!g.o) sel[g.id] = true; });
      } else {
        gatesData.gates.forEach(function (g) { if (g.a) sel[g.id] = true; });
        var changed = true;
        while (changed) {
          changed = false;
          Object.keys(sel).forEach(function (id) {
            var g = G[id];
            if (!g) return;
            g.i.concat(g.r).forEach(function (x) { if (!sel[x]) { sel[x] = true; changed = true; } });
          });
        }
      }
      return { sel: Object.keys(sel).filter(function (id) { return G[id]; }), why: why, full: full };
    }
    function render() {
      clear(out);
      var paths = ta.value.split('\n').map(function (x) { return x.trim(); }).filter(Boolean);
      if (!paths.length) { html('p', { 'class': 'lab-note' }, out, 'Zadej aspoň jednu cestu.'); return; }
      var r = plan(paths);
      var always = gatesData.gates.filter(function (g) { return g.a; }).length;
      var sum = html('div', { 'class': 'ci-sum' }, out);
      var t1 = html('div', null, sum); html('b', null, t1, String(r.sel.length)); html('span', null, t1, 'gates v plánu');
      var t2 = html('div', null, sum); html('b', null, t2, r.full ? 'plný' : 'cílený'); html('span', null, t2, r.full ? r.full : always + ' always + vybrané třídou');
      var term = html('div', { 'class': 'term' }, out);
      r.why.forEach(function (w) { line(term, w[0] + '  →  ' + (w[1].length ? w[1].join(', ') : 'žádná třída'), w[1].length ? '' : 'bad'); });
      var byJob = {};
      r.sel.forEach(function (id) { var j = G[id].job; (byJob[j] = byJob[j] || []).push(id); });
      var jobs = html('div', { 'class': 'ci-jobs' }, out);
      Object.keys(byJob).sort(function (a, b) { return byJob[b].length - byJob[a].length; }).forEach(function (j) {
        var box = html('div', { 'class': 'ci-job' }, jobs);
        html('p', { 'class': 'ci-jh' }, box, j + ' · ' + byJob[j].length);
        var chips = html('div', { 'class': 'chips-btn' }, box);
        byJob[j].sort().forEach(function (id) {
          var c = html('span', { 'class': 'chip static' + (G[id].a ? ' dim' : '') }, chips, id);
          bindTip(c, id, G[id].s);
        });
      });
      html('p', { 'class': 'lab-note' }, out, 'Šedě gates s always: true, které běží v každém plánu. Najetím na gate uvidíš, co dokazuje.');
    }
    var presets = [
      ['dokumentace meshe', 'docs/MESH.md'],
      ['Rust zdroják', 'apps/majordomus-cli/src/context/mod.rs'],
      ['pravidlo', '.ai/repo/rules/project/scope-is-declared.v1.md'],
      ['commit hook', '.githooks/commit-msg'],
      ['workflow CI', '.github/workflows/validate.yml'],
      ['neznámá cesta', 'notes/roadmap.txt']
    ];
    presets.forEach(function (p) {
      var b = html('button', { type: 'button', 'class': 'chip' }, $('ci-presets'), p[0]);
      b.addEventListener('click', function () { ta.value = p[1]; render(); });
    });
    ta.addEventListener('input', render);
    render();
  })();

  /* ---------- briefing budget ---------- */
  if ($('lab-budget')) (function () {
    var SEC = [
      { n: 'GIT', l: 6, keep: true }, { n: 'PEERS', l: 4, keep: true }, { n: 'TASK', l: 5, keep: true }, { n: 'PROFILE', l: 4, keep: true },
      { n: 'CONTEXT DOCUMENTS', l: 14, keep: true }, { n: 'OPEN QUESTIONS', l: 4, keep: true }, { n: 'KNOWLEDGE', l: 5, keep: true },
      { n: 'DECISIONS', l: 18, drop: 3 }, { n: 'RELEVANT FILES', l: 22, drop: 2 },
      { n: 'LATEST CHECKPOINT', l: 16, ptr: 4 }, { n: 'LATEST COMPATIBLE HANDOVER', l: 24, ptr: 5 },
      { n: 'RECENT HISTORY', l: 30, drop: 1 }, { n: 'EXCLUDED + BUDGET', l: 3, keep: true }
    ];
    var r = $('budget'), v = $('budget-v'), bar = $('budget-bar'), out = $('budget-out');
    function render() {
      var budget = +r.value;
      v.textContent = budget;
      var st = SEC.map(function (s) { return { s: s, lines: s.l, state: 'full' }; });
      function total() { return st.reduce(function (a, x) { return a + x.lines; }, 0); }
      var order = [];
      st.forEach(function (x) { if (x.s.drop || x.s.ptr) order.push(x); });
      order.sort(function (a, b) { return (a.s.drop || a.s.ptr) - (b.s.drop || b.s.ptr); });
      var excluded = [];
      for (var i = 0; i < order.length && total() > budget; i++) {
        var x = order[i];
        if (x.s.drop) { x.lines = 0; x.state = 'drop'; excluded.push(x.s.n + ': vypuštěno pro rozpočet'); }
        else { x.lines = 1; x.state = 'ptr'; excluded.push(x.s.n + ': tělo zkráceno na odkaz'); }
      }
      clear(bar);
      var tot = total(), scale = Math.max(tot, budget);
      st.forEach(function (x) {
        if (!x.lines) return;
        var seg = html('i', { 'class': 'bseg ' + (x.s.keep ? 'keep' : x.state) }, bar);
        seg.style.width = (100 * x.lines / scale) + '%';
        seg.title = x.s.n + ' · ' + x.lines + ' ř.';
      });
      var mark = html('b', { 'class': 'bmark' }, bar);
      mark.style.left = (100 * budget / scale) + '%';
      clear(out);
      st.forEach(function (x) {
        var tag = x.state === 'drop' ? 'vypuštěno' : x.state === 'ptr' ? 'odkaz' : (x.s.keep ? 'vždy' : 'celé');
        line(out, (x.s.n + '                              ').slice(0, 28) + (' ' + x.lines).slice(-3) + ' ř.  ' + tag, x.state === 'drop' ? 'bad' : x.state === 'ptr' ? 'warn' : '');
      });
      line(out, '─────', 'mut');
      line(out, 'celkem ' + tot + ' z ' + budget + ' řádků' + (tot > budget ? ' · nad rozpočtem, ale git, task, profil a blokery nevypadnou nikdy' : ''), tot > budget ? 'warn' : 'ok');
      if (excluded.length) { line(out, 'EXCLUDED', 'mut'); excluded.forEach(function (e) { line(out, '  ' + e, 'mut'); }); }
    }
    r.addEventListener('input', render);
    render();
  })();

  /* ---------- divergence label ---------- */
  if ($('lab-diverge')) (function () {
    var SC = {
      exact: { b: 'HEAD se od zápisu nepohnul', label: 'exact', txt: 'Záznam popisuje přesně tento stav. Dá se mu věřit doslova.' },
      advanced: { b: 'nad zápisem přibyly commity', label: 'advanced', txt: 'Zapsaný HEAD je předek dnešního. Záznam platí, ale něco se od té doby stalo: přečti si commity mezi.' },
      diverged: { b: 'historie se rozešla', label: 'diverged', txt: 'Zapsaný HEAD už není předek dnešního (rebase, reset, jiná větev práce). Záznam je historie, ne přítomnost.' },
      ctx: { b: 'jiný worktree nebo branch', label: 'different_context', txt: 'Záznam vznikl jinde. Resoluce ho nabídne jen v pořadí stejný worktree a branch, pak stejná branch.' },
      none: { b: 'na branchi žádný záznam', label: 'No relevant handover.', txt: 'Nepříbuzný záznam se nikdy nenabídne. Žádný záznam je lepší než nesprávná vzpomínka.' }
    };
    var s = $('div-svg'), out = $('div-out'), seg = $('div-seg');
    function node(x, y, cls, t) { svg('circle', { cx: x, cy: y, r: 9, 'class': cls }, s); if (t) { var tx = svg('text', { x: x, y: y + 30, 'text-anchor': 'middle', 'class': 'ax mono' }, s); tx.textContent = t; } }
    function draw(k) {
      clear(s);
      svg('line', { x1: 60, x2: 300, y1: 60, y2: 60, 'class': 'axis' }, s);
      node(60, 60, 'g-node', ''); node(140, 60, 'g-node', ''); node(220, 60, 'g-node', '');
      if (k === 'exact') { node(300, 60, 'st-gate', 'zapsaný = HEAD'); }
      if (k === 'advanced') { svg('line', { x1: 300, x2: 540, y1: 60, y2: 60, 'class': 'axis' }, s); node(300, 60, 'st-gate', 'zapsaný'); node(420, 60, 'g-node', ''); node(540, 60, 'st-agent', 'HEAD'); }
      if (k === 'diverged') { svg('path', { d: 'M220 60C260 60 260 120 300 120H460', 'class': 'axis', fill: 'none' }, s); node(300, 60, 'st-gate', 'zapsaný'); node(380, 120, 'g-node', ''); node(460, 120, 'st-agent', 'HEAD'); var t = svg('text', { x: 300, y: 26, 'text-anchor': 'middle', 'class': 'ax' }, s); t.textContent = 'už není předek HEAD'; }
      if (k === 'ctx') { node(300, 60, 'st-gate', 'zapsaný (wt-a, branch x)'); svg('line', { x1: 220, x2: 540, y1: 130, y2: 130, 'class': 'axis' }, s); node(540, 130, 'st-agent', 'HEAD (wt-b, branch y)'); }
      if (k === 'none') { node(300, 60, 'st-agent', 'HEAD'); var t2 = svg('text', { x: 520, y: 64, 'text-anchor': 'middle', 'class': 'ax' }, s); t2.textContent = '(žádný záznam pro tuto branch)'; }
      var sc = SC[k];
      clear(out);
      line(out, '$ majordomus handover --resolve', 'mut');
      line(out, k === 'none' ? sc.label : 'handover …md (' + sc.label + ')', k === 'exact' ? 'ok' : (k === 'advanced' ? '' : 'warn'));
      line(out, sc.txt, 'mut');
      Array.prototype.forEach.call(seg.children, function (b) { b.setAttribute('aria-selected', b.dataset.k === k ? 'true' : 'false'); });
    }
    Object.keys(SC).forEach(function (k) {
      var b = html('button', { type: 'button', 'class': 'seg-b' }, seg, SC[k].b);
      b.dataset.k = k;
      b.addEventListener('click', function () { draw(k); });
    });
    draw('advanced');
  })();

  /* ---------- knowledge lifecycle ---------- */
  if ($('lab-know')) (function () {
    var s = $('know-svg'), out = $('know-out'), ev = $('know-ev');
    var pos = { candidate: 130, verified: 380, superseded: 620 };
    var state = 'candidate', log = [];
    function draw() {
      clear(s);
      svg('path', { d: 'M200 60H310', 'class': 'axis' }, s);
      svg('path', { d: 'M450 60H550', 'class': 'axis' }, s);
      svg('path', { d: 'M130 86C130 140 620 140 620 86', 'class': 'axis', fill: 'none' }, s);
      var t1 = svg('text', { x: 255, y: 50, 'text-anchor': 'middle', 'class': 'ax mono' }, s); t1.textContent = 'promote';
      var t2 = svg('text', { x: 500, y: 50, 'text-anchor': 'middle', 'class': 'ax mono' }, s); t2.textContent = 'supersedes';
      var t3 = svg('text', { x: 375, y: 146, 'text-anchor': 'middle', 'class': 'ax mono' }, s); t3.textContent = 'reject';
      Object.keys(pos).forEach(function (k) {
        var on = k === state;
        svg('rect', { x: pos[k] - 70, y: 36, width: 140, height: 48, rx: 8, 'class': on ? 'kn-box on' : 'kn-box' }, s);
        var t = svg('text', { x: pos[k], y: 58, 'text-anchor': 'middle', 'class': 'ax strong' }, s); t.textContent = k;
        var u = svg('text', { x: pos[k], y: 75, 'text-anchor': 'middle', 'class': 'ax' }, s);
        u.textContent = k === 'candidate' ? 'candidates/, deriver' : k === 'verified' ? 'curated/, člověk' : 'zůstává jako záznam';
      });
      clear(out);
      line(out, 'status: ' + state, 'ok');
      log.forEach(function (l) { line(out, l[0], l[1]); });
    }
    $('know-promote').addEventListener('click', function () {
      if (state !== 'candidate') log.push(['promote jde jen z candidate', 'bad']);
      else if (!ev.checked) log.push(['$ knowledge promote <id> < /dev/null  →  odmítnuto: akt bez důkazu je jen tvrzení', 'bad']);
      else { state = 'verified'; log.push(['$ knowledge promote <id> < evidence.md  →  přesunuto do curated/, status: verified', 'ok']); }
      draw();
    });
    $('know-reject').addEventListener('click', function () {
      if (state !== 'candidate') log.push(['reject jde jen z candidate; ověřenou znalost nahrazuje nový záznam', 'bad']);
      else { state = 'superseded'; log.push(['$ knowledge reject <id> --reason "…"  →  superseded, zůstává na místě jako záznam odmítnutí', 'warn']); }
      draw();
    });
    $('know-supersede').addEventListener('click', function () {
      if (state !== 'verified') log.push(['nahradit jde jen ověřenou znalost', 'bad']);
      else { state = 'superseded'; log.push(['nový záznam s relations.supersedes  →  starý superseded; verified se nikdy needituje', 'warn']); }
      draw();
    });
    $('know-reset').addEventListener('click', function () { state = 'candidate'; log = []; ev.checked = false; draw(); });
    draw();
  })();

  /* ---------- liveness ---------- */
  if ($('lab-live')) (function () {
    var s = $('lv-svg'), out = $('lv-out'), hb = $('lv-hb'), ex = $('lv-exp'), tr = $('lv-t'), tv = $('lv-tv');
    var CRASH = 10, timer = null;
    function phases() {
      var h = Math.max(1, +hb.value || 5), e = Math.max(3, +ex.value || 30);
      return { h: h, e: e, conn: CRASH + 2 * h + 1, degr: CRASH + e / 2, exp: CRASH + e };
    }
    function stateAt(t, p) {
      if (t < p.conn) return 'connected';
      if (t < p.degr) return 'degraded';
      if (t < p.exp) return 'unreachable';
      return 'expired';
    }
    function render() {
      var p = phases(), t = +tr.value;
      tv.textContent = t;
      clear(s);
      var L = 90, R = 700, max = 60;
      function X(x) { return L + (R - L) * Math.min(x, max) / max; }
      var labels = [['beat A', 40], ['link na B', 90], ['claim A', 140]];
      labels.forEach(function (l) { var tx = svg('text', { x: 10, y: l[1] + 4, 'class': 'ax' }, s); tx.textContent = l[0]; });
      for (var b = 0; b <= CRASH; b += p.h) svg('circle', { cx: X(b), cy: 40, r: 4, 'class': 's1' }, s);
      svg('line', { x1: X(CRASH), x2: X(CRASH), y1: 24, y2: 160, 'class': 'ref' }, s);
      var ct = svg('text', { x: X(CRASH) + 4, y: 22, 'class': 'ref-t' }, s); ct.textContent = 'kill -9';
      var segs = [['connected', 0, p.conn, 'ok'], ['degraded', p.conn, p.degr, 'warn'], ['unreachable', Math.max(p.conn, p.degr), p.exp, 'bad'], ['expired', p.exp, max, 'mut']];
      segs.forEach(function (g) {
        if (g[2] <= g[1]) return;
        svg('rect', { x: X(g[1]), y: 80, width: Math.max(0, X(g[2]) - X(g[1])), height: 20, rx: 3, 'class': 'band ' + g[3] }, s);
        if (X(g[2]) - X(g[1]) > 60) { var tx = svg('text', { x: (X(g[1]) + X(g[2])) / 2, y: 94, 'text-anchor': 'middle', 'class': 'ax' }, s); tx.textContent = g[0]; }
      });
      svg('rect', { x: X(0), y: 130, width: X(p.exp) - X(0), height: 20, rx: 3, 'class': 'band acc' }, s);
      var ht = svg('text', { x: (X(0) + X(p.exp)) / 2, y: 144, 'text-anchor': 'middle', 'class': 'ax' }, s); ht.textContent = 'exkluzivní claim platí';
      for (var k = 0; k <= max; k += 10) { var tk = svg('text', { x: X(k), y: 186, 'text-anchor': 'middle', 'class': 'ax' }, s); tk.textContent = k + ' s'; }
      svg('line', { x1: X(t), x2: X(t), y1: 28, y2: 166, 'class': 'cross' }, s);
      var st = stateAt(t, p);
      clear(out);
      if (+ex.value < 3 * +hb.value) line(out, 'pozor: expiry má být aspoň tři heartbeaty', 'warn');
      line(out, 't = ' + t + ' s · link: ' + (t < CRASH ? 'connected (A ještě bije)' : st), st === 'connected' || t < CRASH ? 'ok' : st === 'expired' ? 'mut' : 'warn');
      line(out, t < p.exp ? 'claim A na apps/ platí · claim B na apps/majordomus-cli: claim_conflict (exit 10)' : 'claim A: expired, its runtime stopped beating · B může claimnout', t < p.exp ? 'bad' : 'ok');
      line(out, 'connected do ' + p.conn + ' s, degraded do ' + p.degr + ' s, unreachable do ' + p.exp + ' s (monotónní hodiny B, nástěnné hodiny se neporovnávají)', 'mut');
    }
    [hb, ex, tr].forEach(function (x) { x.addEventListener('input', render); });
    $('lv-play').addEventListener('click', function () {
      if (timer) { clearInterval(timer); timer = null; $('lv-play').textContent = '▶ Přehrát pád'; return; }
      $('lv-play').textContent = '❚❚ Pauza';
      if (+tr.value >= 60) tr.value = 0;
      timer = setInterval(function () {
        tr.value = +tr.value + 1; render();
        if (+tr.value >= 60) { clearInterval(timer); timer = null; $('lv-play').textContent = '▶ Přehrát pád'; }
      }, 120);
    });
    render();
  })();

  /* ---------- handshake ---------- */
  if ($('lab-hs')) (function () {
    var CHECKS = ['velikost ≤ 900 KiB a tvar hello', 'podpis Ed25519 v doméně hello', 'čas v rozmezí ±300 s', 'nonce ještě nebyl zodpovězen', 'rozsahy protokolu se potkají', 'peer není tento runtime', 'stejné repo (digest root commitů)', 'klíč je podle politiky důvěryhodný'];
    var CASES = [
      ['vše v pořádku', -1, null],
      ['zpráva má 2 MiB', 0, 'oversized'],
      ['chybí povinné pole', 0, 'malformed'],
      ['podpis nesedí (změněné bajty)', 1, 'signature'],
      ['hodiny A ujely o 7 minut', 2, 'stale'],
      ['přehraný starý hello', 3, 'replay'],
      ['A umí jen protokol 2', 4, 'protocol_unsupported'],
      ['A je ve skutečnosti B', 5, 'self_link'],
      ['A pracuje na jiném repu', 6, 'repository_mismatch'],
      ['klíč A není v trust.allow (deny_unknown)', 7, 'untrusted']
    ];
    var sel = $('hs-sel'), list = $('hs-list'), out = $('hs-out');
    CASES.forEach(function (c, i) { var o = html('option', { value: String(i) }, sel, c[0]); });
    function render() {
      var c = CASES[+sel.value];
      clear(list); clear(out);
      CHECKS.forEach(function (ch, i) {
        var st = c[1] === -1 || i < c[1] ? 'ok' : i === c[1] ? 'bad' : 'skip';
        var li = html('li', { 'class': 'hs-' + st }, list);
        html('span', { 'class': 'hs-mark' }, li, st === 'ok' ? 'PASS' : st === 'bad' ? 'FAIL' : '—');
        li.appendChild(document.createTextNode(ch));
      });
      if (c[1] === -1) {
        line(out, 'B → welcome (karta B, link id, marks, vrácený nonce, podpis)', 'ok');
        line(out, 'A ověří welcome stejně · link connected · sync každý heartbeat', 'ok');
      } else {
        line(out, 'odmítnuto: ' + c[2], 'bad');
        line(out, 'počítá se do refused_in na B, vidět v mesh peers; další kontroly se nevyhodnocují', 'mut');
        if (c[2] === 'untrusted') line(out, 'náprava: přidej veřejný klíč A do trust.allow a commitni deklaraci', 'mut');
      }
    }
    sel.addEventListener('change', render);
    render();
  })();

  /* ---------- fold ---------- */
  if ($('lab-fold')) (function () {
    var EV = [
      { st: 'A', seq: 1, l: 1, k: 'session_opened', s: 'sA' },
      { st: 'A', seq: 2, l: 2, k: 'claim_acquired', s: 'sA', scope: 'apps' },
      { st: 'B', seq: 1, l: 1, k: 'session_opened', s: 'sB' },
      { st: 'B', seq: 2, l: 3, k: 'claim_acquired', s: 'sB', scope: 'apps/majordomus-cli' },
      { st: 'A', seq: 3, l: 4, k: 'handover_published', s: 'sA', id: 'h1' },
      { st: 'B', seq: 3, l: 5, k: 'handover_consumed', s: 'sB', id: 'h1' }
    ];
    function fold(delivered) {
      var seen = {}, evs = [];
      delivered.forEach(function (e) { var key = e.st + ':' + e.seq; if (!seen[key]) { seen[key] = true; evs.push(e); } });
      evs.sort(function (a, b) { return a.l - b.l || (a.st < b.st ? -1 : a.st > b.st ? 1 : 0) || a.seq - b.seq; });
      var sessions = {}, claims = [], handovers = {};
      evs.forEach(function (e) {
        if (e.k === 'session_opened') sessions[e.s] = 'open';
        if (e.k === 'claim_acquired') {
          var clash = claims.some(function (c) { return c.state === 'held' && (c.scope === e.scope || e.scope.indexOf(c.scope + '/') === 0 || c.scope.indexOf(e.scope + '/') === 0); });
          claims.push({ s: e.s, scope: e.scope, state: clash ? 'conflicted' : 'held', key: e.st + '/' + e.seq });
        }
        if (e.k === 'handover_published') handovers[e.id] = 'offered';
        if (e.k === 'handover_consumed') handovers[e.id] = 'consumed by ' + e.s;
      });
      var canon = JSON.stringify({ sessions: sessions, claims: claims, handovers: handovers });
      var h = 2166136261;
      for (var i = 0; i < canon.length; i++) { h ^= canon.charCodeAt(i); h = Math.imul(h, 16777619) >>> 0; }
      return { sessions: sessions, claims: claims, handovers: handovers, digest: ('00000000' + h.toString(16)).slice(-8), unique: evs.length };
    }
    var list = $('fold-events'), out = $('fold-out'), cnt = $('fold-count');
    function run(shuffle) {
      var d = EV.slice();
      if (shuffle) {
        var dups = 1 + Math.floor(Math.random() * 4);
        for (var k = 0; k < dups; k++) d.push(EV[Math.floor(Math.random() * EV.length)]);
        for (var i = d.length - 1; i > 0; i--) { var j = Math.floor(Math.random() * (i + 1)); var t = d[i]; d[i] = d[j]; d[j] = t; }
      }
      var r = fold(d);
      clear(list);
      var seen = {};
      d.forEach(function (e) {
        var key = e.st + ':' + e.seq;
        var dup = seen[key]; seen[key] = true;
        var c = html('span', { 'class': 'ev ' + (e.st === 'A' ? 'ea' : 'eb') + (dup ? ' dup' : '') }, list, e.st + '#' + e.seq + ' ' + e.k + (e.scope ? ' ' + e.scope : ''));
        c.title = 'stream ' + e.st + ', seq ' + e.seq + ', lamport ' + e.l + (dup ? ' (duplikát)' : '');
      });
      cnt.textContent = 'doručeno ' + d.length + ' událostí, z toho ' + (d.length - r.unique) + ' duplikátů';
      clear(out);
      r.claims.forEach(function (c) { line(out, 'claim ' + c.key + ' ' + c.scope + ' (' + c.s + '): ' + c.state, c.state === 'held' ? 'ok' : 'warn'); });
      Object.keys(r.handovers).forEach(function (h) { line(out, 'handover ' + h + ': ' + r.handovers[h]); });
      line(out, 'digest ' + r.digest + ' · vítěz konfliktu: nejnižší (lamport, stream, seq), nikdy „poslední paket“', 'mut');
    }
    $('fold-shuffle').addEventListener('click', function () { run(true); });
    run(false);
  })();

  /* ---------- projections ---------- */
  if ($('lab-proj')) (function () {
    var C = {
      'models.route': { rows: [['příkazová řádka', 'majordomus models route --require … --local-only'], ['MCP nástroj', 'majordomus_models_route'], ['HTTP', 'GET /api/v1/models/route'], ['Cockpit', '/cockpit/models']], note: 'Vybere model podle požadovaných schopností z deklarovaného katalogu. Žádné volání modelu, žádná cena.' },
      'mesh.state': { rows: [['příkazová řádka', 'majordomus mesh state'], ['MCP nástroj', 'majordomus_mesh_state'], ['HTTP', 'GET /api/v1/mesh/state'], ['Cockpit', '/cockpit/mesh']], note: 'Sessions, claims, handovery a reviews spočítané foldem, s digestem.' },
      'mesh.claim': { rows: [['příkazová řádka', 'majordomus mesh claim <scope> [--advisory]'], ['MCP nástroj', 'majordomus_mesh_claim'], ['HTTP', 'POST /api/v1/mesh/claims'], ['odmítnutí', 'claim_conflict: exit 10 / HTTP 422 / isError v MCP']], note: 'Stejné odmítnutí na všech rozhraních, protože všechna volají stejný handler.' },
      'peers.announce': { rows: [['MCP nástroj', 'majordomus_announce (jediná capability druhu command na boardu)'], ['HTTP', 'POST /api/v1/peers/announce → 422, přes čisté HTTP není volající'], ['efekt', 'process_state: mění jen paměť serveru']], note: 'Efekt je deklarovaný a MCP anotace se z něj odvozují: readOnlyHint false, destructiveHint false.' }
    };
    var seg = $('proj-seg'), body = $('proj-body'), note = $('proj-note');
    function show(k) {
      clear(body);
      C[k].rows.forEach(function (r) { var tr = html('tr', null, body); html('td', null, tr, r[0]); var td = html('td', null, tr); html('code', null, td, r[1]); });
      note.textContent = C[k].note;
      Array.prototype.forEach.call(seg.children, function (b) { b.setAttribute('aria-selected', b.dataset.k === k ? 'true' : 'false'); });
    }
    Object.keys(C).forEach(function (k) {
      var b = html('button', { type: 'button', 'class': 'seg-b mono' }, seg, k);
      b.dataset.k = k;
      b.addEventListener('click', function () { show(k); });
    });
    show('mesh.claim');
  })();
})();
