(function () {
  'use strict';
  /* The outlook chapters (sections-outlook.html). Repository figures come from data-outlook,
     which handbook/derive_data.py derived from the tree; model estimates come from
     data-outlook-model, the owner's report written once in outlook-model.json. Colours are
     roles, by class: nothing here chooses one. */
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function readJSON(id) { var e = $(id); try { return e ? JSON.parse(e.textContent) : null; } catch (x) { return null; } }
  function store(key, val) {
    try {
      if (val === undefined) return window.localStorage.getItem(key);
      if (val === null) window.localStorage.removeItem(key); else window.localStorage.setItem(key, val);
    } catch (e) { return null; }
    return null;
  }
  var C = window.mjCharts, D = readJSON('data-outlook'), M = readJSON('data-outlook-model'), R = readJSON('data-repo');
  if (!C || !D || !M) return;
  var svg = C.svg, fmt = C.fmt;
  function num(v, digits) { return fmt((+v).toFixed(digits || 0)).replace('.', ','); }
  function pct(a, b, digits) { return b ? num(100 * a / b, digits === undefined ? 1 : digits) + ' %' : '–'; }
  function czDate(s) { return +s.slice(8, 10) + '. ' + +s.slice(5, 7) + '.'; }
  function d2t(s) { return Date.UTC(+s.slice(0, 4), +s.slice(5, 7) - 1, +s.slice(8, 10)); }
  function table(host, headers, rows) {
    if (!host) return;
    var t = el('table', null, host);
    var tr = el('tr', null, el('thead', null, t));
    headers.forEach(function (h) { el('th', null, tr, h); });
    var tb = el('tbody', null, t);
    rows.forEach(function (r) {
      var row = el('tr', null, tb);
      r.forEach(function (c) {
        var td = el('td', null, row);
        if (c && c.code) el('code', null, td, c.code); else td.textContent = String(c);
      });
    });
  }
  var STATUS = { DONE: 'ok', VERIFY: 's1', ACTIVE: 's1', READY: 's2', BLOCKED: 'bad', CANCELLED: 'mut', PLANNED: 'mut' };

  /* ---------- scenarios: three model curves, each one can be hidden ---------- */
  (function () {
    var host = $('scen-svg'), bar = $('scen-toggles'), out = $('scen-readout');
    if (!host || !bar) return;
    var base = M.baseline, sc = M.scenarios, days = base.days;
    var W = 720, H = 300, L = 46, Rm = 150, T = 18, B = 34;
    host.setAttribute('viewBox', '0 0 ' + W + ' ' + H);
    var top = C.niceMax(Math.max.apply(null, sc.map(function (s) { return s.end; })));
    var low = Math.min(base.index, Math.min.apply(null, sc.map(function (s) { return s.end; })));
    low = Math.floor(low * 0.8 / 20) * 20;
    function X(d) { return L + d / days * (W - L - Rm); }
    function Y(v) { return T + (1 - (v - low) / (top - low)) * (H - T - B); }
    function at(s, d) { return base.index + (s.end - base.index) * Math.pow(d / days, s.curve); }
    for (var k = 0; k <= 4; k++) {
      var v = low + (top - low) * k / 4, gy = Y(v);
      svg('line', { x1: L, x2: W - Rm, y1: gy, y2: gy, 'class': k === 0 ? 'axis' : 'grid' }, host);
      svg('text', { x: L - 8, y: gy + 4, 'text-anchor': 'end', 'class': 'ax' }, host).textContent = Math.round(v);
    }
    for (var d = 0; d <= days; d += days / 6) {
      svg('text', { x: X(d), y: H - 12, 'text-anchor': 'middle', 'class': 'ax' }, host).textContent = Math.round(d) + ' d';
    }
    svg('line', { x1: L, x2: W - Rm, y1: Y(base.index), y2: Y(base.index), 'class': 'ref dash' }, host);
    svg('text', { x: L + 6, y: Y(base.index) - 6, 'class': 'ax' }, host).textContent = 'výchozí stav ' + base.index;
    var built = Math.max(0, Math.min(days, (d2t(D.integration.last) - d2t(base.day)) / 86400000));
    svg('line', { x1: X(built), x2: X(built), y1: T, y2: H - B, 'class': 'ref' }, host);
    svg('text', { x: X(built) + 4, y: T + 10, 'class': 'ref-t' }, host).textContent = 'sestaveno: den ' + Math.round(built);
    var drawn = sc.map(function (s) {
      var pts = [];
      for (var d2 = 0; d2 <= days; d2 += 3) pts.push(X(d2) + ',' + Y(at(s, d2)));
      var g = svg('g', { 'class': 'vs-g' }, host);
      svg('polyline', { points: pts.join(' '), 'class': 'vs-line ' + s.id }, g);
      svg('circle', { cx: X(days), cy: Y(s.end), r: 4.5, 'class': 'vs-dot ' + s.id }, g);
      svg('text', { x: X(days) + 10, y: Y(s.end) + 4, 'class': 'val' }, g).textContent = s.end + ' · ' + s.label;
      var b = el('button', { type: 'button', 'class': 'seg-b vs-b', 'aria-pressed': 'true', 'data-scenario': s.id }, bar);
      el('i', { 'class': 'sw ' + s.id }, b);
      b.appendChild(document.createTextNode(s.label));
      b.addEventListener('click', function () {
        var on = b.getAttribute('aria-pressed') !== 'true';
        b.setAttribute('aria-pressed', on ? 'true' : 'false');
        g.style.display = on ? '' : 'none';
        show(last);
      });
      return { s: s, g: g, b: b };
    });
    var cross = svg('line', { y1: T, y2: H - B, 'class': 'cross' }, host);
    cross.style.display = 'none';
    var hit = svg('rect', { x: L, y: T, width: W - L - Rm, height: H - T - B, fill: 'transparent', tabindex: '0',
      'aria-label': 'Projekce: šipkami vlevo a vpravo se posouvá den' }, host);
    var last = null;
    function show(d) {
      if (d === null) return;
      last = d;
      cross.setAttribute('x1', X(d)); cross.setAttribute('x2', X(d)); cross.style.display = '';
      var vis = drawn.filter(function (x) { return x.b.getAttribute('aria-pressed') === 'true'; });
      out.textContent = 'Den ' + Math.round(d) + ': ' + (vis.length ? vis.map(function (x) {
        return x.s.label + ' ' + Math.round(at(x.s, d));
      }).join(' · ') : 'žádný scénář není zapnutý');
    }
    hit.addEventListener('pointermove', function (e) {
      var r = host.getBoundingClientRect();
      var x = (e.clientX - r.left) * W / r.width;
      show(Math.max(0, Math.min(days, (x - L) / (W - L - Rm) * days)));
    });
    hit.addEventListener('keydown', function (e) {
      if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
      e.preventDefault();
      show(Math.max(0, Math.min(days, (last === null ? 0 : last) + (e.key === 'ArrowRight' ? 3 : -3))));
    });
    window.mjOutlook = { at: function (id, d) { var s = sc.filter(function (x) { return x.id === id; })[0]; return s ? at(s, d) : null; } };
  })();

  /* ---------- model charts: potential, scores, position ---------- */
  if ($('chart-v-potential')) C.hBars($('chart-v-potential'), {
    title: 'Strategický potenciál při úspěšné realizaci', sub: 'odhad reportu, 0–100; nejde o procento implementace',
    data: M.potential.map(function (p) { return [p[0], p[2]]; }), unit: 'bodů ze 100',
    headers: ['Oblast', 'Odhad'], labelW: 230
  });
  if ($('v-scores')) M.scores.forEach(function (s) {
    var row = el('div', { 'class': 'meter-row' }, $('v-scores'));
    el('b', { 'class': 'score-v' }, row, (s[1] === null ? '?' : s[1]) + ' / 10');
    var m = el('span', { 'class': 'meter', 'aria-hidden': 'true' }, row);
    for (var i = 1; i <= 10; i++) el('i', { 'class': s[1] !== null && i <= s[1] ? 'on' : '' }, m);
    el('span', { 'class': 'm-l' }, row, s[0]);
  });
  if ($('chart-v-position')) (function () {
    var host = $('chart-v-position');
    var wrap = C.frame(host, 'Majordomus proti vyspělým platformám', 'skóre 0–10; vyšší znamená silnější předpoklady, ne prokázaný výkon');
    var lg = el('div', { 'class': 'legend' }, host.querySelector('.chart-head'));
    [['s1', 'Majordomus'], ['s2', 'vyspělé platformy (syntetický profil)']].forEach(function (l) {
      var it = el('span', { 'class': 'lg' }, lg); el('i', { 'class': 'sw ' + l[0] }, it); it.appendChild(document.createTextNode(l[1]));
    });
    var W = 720, rowH = 44, labelW = 190, right = 50, H = 8 + M.position.length * rowH;
    var s = svg('svg', { viewBox: '0 0 ' + W + ' ' + H, role: 'img', 'aria-label': 'Majordomus proti vyspělým platformám' }, wrap);
    var plotW = W - labelW - right;
    [0, 0.5, 1].forEach(function (f) { svg('line', { x1: labelW + plotW * f, x2: labelW + plotW * f, y1: 4, y2: H - 4, 'class': 'grid' }, s); });
    M.position.forEach(function (p, i) {
      var y = 8 + i * rowH;
      svg('text', { x: labelW - 10, y: y + 22, 'text-anchor': 'end', 'class': 'ax' }, s).textContent = p[0];
      [[p[1], 's1', 'Majordomus'], [p[2], 's2', 'vyspělé platformy']].forEach(function (b, j) {
        var g = svg('g', { 'class': 'mark' }, s);
        svg('rect', { x: labelW, y: y + 4 + j * 16, width: Math.max(2, plotW * b[0] / 10), height: 12, rx: 2, 'class': b[1] }, g);
        svg('text', { x: labelW + plotW * b[0] / 10 + 6, y: y + 14 + j * 16, 'class': 'val' }, s).textContent = b[0];
        C.bindTip(g, b[0] + ' / 10', p[0] + ' · ' + b[2]);
      });
    });
    C.tableToggle(host, ['Oblast', 'Majordomus', 'Vyspělé platformy'], M.position);
  })();

  /* ---------- the six proofs: a checklist this browser remembers ---------- */
  if ($('v-proofs')) (function () {
    var key = 'mj-outlook-proofs', done = {};
    try { done = JSON.parse(store(key) || '{}') || {}; } catch (e) { done = {}; }
    var boxes = Array.prototype.slice.call($('v-proofs').querySelectorAll('input[type=checkbox]'));
    function update() {
      var n = boxes.filter(function (b) { return b.checked; }).length;
      $('v-proofs-count').textContent = n + ' z ' + boxes.length + ' důkazů odškrtnuto v tomto prohlížeči';
    }
    boxes.forEach(function (b) {
      b.checked = !!done[b.id];
      b.closest('li').classList.toggle('done', b.checked);
      b.addEventListener('change', function () {
        if (b.checked) done[b.id] = true; else delete done[b.id];
        b.closest('li').classList.toggle('done', b.checked);
        store(key, JSON.stringify(done));
        update();
      });
    });
    $('v-proofs-reset').addEventListener('click', function () {
      done = {}; store(key, null);
      boxes.forEach(function (b) { b.checked = false; b.closest('li').classList.remove('done'); });
      update();
    });
    update();
  })();

  /* ---------- integration tax by week: merges stacked on the other commits ---------- */
  if ($('chart-v-weeks')) (function () {
    var host = $('chart-v-weeks'), weeks = D.integration.weeks;
    var wrap = C.frame(host, 'Commity a merge commity po týdnech', 'ISO týdny podle data autora, ' + czDate(weeks[0][0]) + ' až ' + czDate(D.integration.last));
    var lg = el('div', { 'class': 'legend' }, host.querySelector('.chart-head'));
    [['s1', 'ostatní commity'], ['s2', 'merge commity']].forEach(function (l) {
      var it = el('span', { 'class': 'lg' }, lg); el('i', { 'class': 'sw ' + l[0] }, it); it.appendChild(document.createTextNode(l[1]));
    });
    var W = 720, H = 260, left = 50, right = 12, top = 14, bottom = 34;
    var s = svg('svg', { viewBox: '0 0 ' + W + ' ' + H, role: 'img', 'aria-label': 'Commity a merge commity po týdnech' }, wrap);
    var max = C.niceMax(Math.max.apply(null, weeks.map(function (w) { return w[1]; })));
    var plotW = W - left - right, plotH = H - top - bottom, base = top + plotH;
    for (var k = 0; k <= 4; k++) {
      var gy = base - plotH * k / 4;
      svg('line', { x1: left, x2: W - right, y1: gy, y2: gy, 'class': k === 0 ? 'axis' : 'grid' }, s);
      svg('text', { x: left - 6, y: gy + 4, 'text-anchor': 'end', 'class': 'ax' }, s).textContent = fmt(Math.round(max * k / 4));
    }
    var band = plotW / weeks.length, bw = Math.min(56, band - 10);
    weeks.forEach(function (w, i) {
      var x = left + band * i + (band - bw) / 2;
      var hAll = plotH * w[1] / max, hMerge = plotH * w[2] / max;
      var g = svg('g', { 'class': 'mark' }, s);
      svg('rect', { x: left + band * i, y: top, width: band, height: plotH, fill: 'transparent' }, g);
      if (w[1] > w[2]) svg('path', { d: C.roundedBarV(x, base - hAll, bw, hAll - hMerge, base - hMerge), 'class': 's1' }, g);
      if (w[2] > 0) svg('rect', { x: x, y: base - hMerge, width: bw, height: hMerge, 'class': 's2' }, g);
      C.bindTip(g, fmt(w[1]) + ' commitů, z toho ' + fmt(w[2]) + ' merge (' + pct(w[2], w[1]) + ')', 'týden od ' + czDate(w[0]));
      svg('text', { x: x + bw / 2, y: base + 18, 'text-anchor': 'middle', 'class': 'ax' }, s).textContent = czDate(w[0]);
    });
    C.tableToggle(host, ['Týden od', 'Commitů', 'Merge', 'Podíl merge'], weeks.map(function (w) {
      return [czDate(w[0]), w[1], w[2], pct(w[2], w[1])];
    }));
  })();
  if ($('chart-v-types')) C.hBars($('chart-v-types'), {
    title: 'Typy nemerge commitů za ' + D.integration.window + ' dní', sub: 'od ' + czDate(D.integration.since) + ' do ' + czDate(D.integration.last),
    data: D.integration.windowTypes, unit: 'commitů', headers: ['Typ', 'Commitů'], labelW: 110
  });

  /* ---------- plan, intents, rules ---------- */
  if ($('chart-v-plan')) C.hBars($('chart-v-plan'), {
    title: 'Issues podle odvozeného stavu', sub: fmt(D.plan.total) + ' issues; stav se počítá z razítek, závislostí a důkazů',
    data: D.plan.status, unit: 'issues', headers: ['Stav', 'Issues'], labelW: 110,
    cls: function (d) { return STATUS[d[0]] || 's1'; }
  });
  table($('v-intents'), ['Záměr', 'Kritéria s aktuálním důkazem', 'Stavy kritérií', 'Milníky'], D.intents.map(function (i) {
    return [{ code: i.id }, i.m + ' z ' + i.c, i.st.map(function (x) { return x[0].replace('_', ' ') + ' ' + x[1]; }).join(', '),
      i.ms.map(function (x) { return x[0] + ' (' + x[1] + ')'; }).join(', ')];
  }));
  if ($('chart-v-proof') && R) C.hBars($('chart-v-proof'), {
    title: 'Stav důkazu pravidel', sub: 'přepočítaný ze stromu commitu, stejně jako rules report',
    data: R.proof, unit: 'pravidel', headers: ['Stav', 'Pravidel'], labelW: 120
  });
  function telemetry(host) {
    table(host, ['Issue', 'Stav', 'Co přinese'], D.plan.telemetry.issues.map(function (i) { return [{ code: i[0] }, { code: i[1] }, i[2]]; }));
  }
  telemetry($('v-telemetry'));
  telemetry($('v-telemetry-chain'));

  /* ---------- economics: the measured context reduction and the verdict ---------- */
  if ($('v-gauge')) (function () {
    var g = $('v-gauge'), c = D.economics.context, r = 62, len = 2 * Math.PI * r, share = c.ppm / 1e6;
    svg('circle', { cx: 80, cy: 80, r: r, 'class': 'gauge-track' }, g);
    svg('circle', { cx: 80, cy: 80, r: r, 'class': 'gauge-arc', 'stroke-dasharray': (len * share) + ' ' + len,
      transform: 'rotate(-90 80 80)' }, g);
    svg('text', { x: 80, y: 84, 'text-anchor': 'middle', 'class': 'gauge-v' }, g).textContent = num(c.ppm / 1e4, 2) + ' %';
    svg('text', { x: 80, y: 104, 'text-anchor': 'middle', 'class': 'ax' }, g).textContent = 'n = ' + fmt(c.n);
  })();
  if ($('v-statement')) $('v-statement').textContent = D.economics.statement;
  if ($('v-unmet')) D.economics.unmet.forEach(function (u) { el('li', null, $('v-unmet'), u); });
  if ($('chart-v-metrics')) (function () {
    var by = {};
    D.economics.metrics.forEach(function (m) { by[m[1]] = (by[m[1]] || 0) + 1; });
    C.hBars($('chart-v-metrics'), {
      title: 'Metriky ekonomiky podle stavu', sub: D.economics.metrics.length + ' metrik z docs/generated/economics.json',
      data: Object.keys(by).sort(function (a, b) { return by[b] - by[a]; }).map(function (k) { return [k.replace('_', ' '), by[k]]; }),
      unit: 'metrik', headers: ['Stav', 'Metrik'], labelW: 130,
      cls: function (d) { return d[0] === 'measured' ? 'ok' : 'mut'; }
    });
  })();
  table($('v-suites'), ['Sada', 'Běhů', 'Čerstvost'], D.economics.suites.map(function (s) { return [{ code: s[0] }, s[1], { code: s[2] }]; }));
})();
