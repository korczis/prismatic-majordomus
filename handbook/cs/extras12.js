(function () {
  'use strict';
  /* The repository's own generated graph: kind map, flows found in the data, ego view of one object. */
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  var sec = $('graf'), raw = $('data-graphdb');
  if (!sec || !raw) return;
  var GH = 'https://github.com/korczis/prismatic-majordomus';
  var ARTIFACT = 'https://claude.ai/artifact/CBgdBRrXSimNAgQZuxt5JU';
  var CDN = 'https://cdnjs.cloudflare.com/ajax/libs/echarts/5.5.0/echarts.min.js';
  var reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  function cssv(n) { return getComputedStyle(document.documentElement).getPropertyValue(n).trim(); }
  function dark() {
    var t = document.documentElement.getAttribute('data-theme');
    return t ? t === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
  }
  function pal() { return dark() ? ['#3987e5', '#d95926', '#199e70', '#c98500', '#d55181', '#9085e9'] : ['#2a78d6', '#eb6834', '#1baf7a', '#eda100', '#e87ba4', '#6250d6']; }

  /* The only hand-written parts: which family a kind is drawn in, a Czech gloss, and the flow patterns. */
  var GROUPS = [
    ['Rozhodnutí a pravidla', ['adr', 'rule', 'policy', 'profile', 'scope', 'doctrine']],
    ['Capabilities a rozhraní', ['capability', 'module', 'projection', 'command']],
    ['Důkazy', ['claim', 'test', 'implementation', 'file']],
    ['Plán', ['issue', 'milestone', 'intent', 'release-record', 'gap', 'critique']],
    ['Znalosti a záznamy', ['document', 'context', 'session', 'knowledge', 'prompt', 'skill', 'taxonomy']],
    ['Produkt a ostatní', []]
  ];
  var GLOSS = {
    adr: 'rozhodnutí', rule: 'pravidlo', capability: 'capability', module: 'modul', projection: 'rozhraní', command: 'příkaz',
    claim: 'garance', test: 'test', implementation: 'implementace', file: 'soubor', issue: 'issue', milestone: 'milník',
    document: 'dokument', context: 'kontext', session: 'session', 'use-case': 'use case', application: 'aplikace', feature: 'feature',
    moment: 'moment selhání', skill: 'skill', prompt: 'prompt', profile: 'profil', policy: 'policy', intent: 'intent'
  };
  var EGLOSS = {
    put_in_force: 'zavádí', exercises: 'procvičuje', evidences: 'dokládá', tested_by: 'dokazuje ji', composes: 'skládá',
    projects: 'promítá se do', defined_in: 'je definována v', implemented_by: 'implementuje ji', belongs_to: 'patří do',
    depends_on: 'závisí na', runs: 'spouští', serves: 'slouží', governed_by: 'řídí se', related_to: 'souvisí s', tracks: 'sleduje',
    supersedes: 'nahrazuje'
  };
  var PATTERNS = [
    { label: 'Od rozhodnutí k testu', start: 'adr', steps: [['put_in_force', 'out', 'rule'], ['exercises', 'in', 'use-case'], ['evidences', 'out', 'claim'], ['tested_by', 'out', 'test']],
      text: 'Rozhodnutí zavede pravidlo, use case to pravidlo procvičuje a dokládá garanci, kterou dokazuje test. Celý řetěz od „proč“ po „funguje to“.', links: [['p-rozhodnuti', 'Decision a ADR'], ['h-vynuceni', 'Pravidlo musí něco volat'], ['x-claims', 'Garance a use cases']] },
    { label: 'Od rozhodnutí ke garanci', start: 'adr', steps: [['put_in_force', 'out', 'claim'], ['tested_by', 'out', 'test']],
      text: 'Kratší cesta: rozhodnutí rovnou zavádí garanci a tu dokazuje test.', links: [['x-claims', 'Garance a use cases'], ['x-evidence', 'Důkazy']] },
    { label: 'Od kódu k důkazu', start: 'implementation', steps: [['implemented_by', 'in', 'claim'], ['tested_by', 'out', 'test']],
      text: 'Soubor s implementací, garance, kterou implementuje, a test, který ji dokazuje. Když změníš ten soubor, tohle je test, který o tom rozhodne.', links: [['x-evidence', 'Důkazy'], ['h-ci', 'CI gates']] },
    { label: 'Od modulu k rozhraní', start: 'module', steps: [['composes', 'out', 'capability'], ['projects', 'out', 'projection']],
      text: 'Modul skládá capability a ta se z jedné deklarace promítá do rozhraní: CLI, MCP, HTTP, Cockpit.', links: [['projekce', 'Jeden fakt, mnoho rozhraní'], ['x-registry', 'Registr zblízka']] },
    { label: 'Od milníku k závislosti', start: 'milestone', steps: [['belongs_to', 'in', 'issue'], ['depends_on', 'out', 'issue']],
      text: 'Milník, issue, které do něj patří, a issue, na kterém závisí. Z toho se počítají vlny plánu.', links: [['x-plan', 'Plán: milníky a vlny']] },
    { label: 'Od aplikace k příkazu', start: 'application', steps: [['serves', 'in', 'use-case'], ['runs', 'out', 'command']],
      text: 'Aplikace, use case, který jí slouží, a příkaz, který ten use case opravdu spouští.', links: [['x-claims', 'Garance a use cases'], ['prikazy', 'Reference příkazů']] }
  ];

  var G = null, K = [], EK = [], N = [], out = [], inc = [], byKind = {}, keyOf = [], byKey = {}, groupOf = {};
  function decode() {
    return new Promise(function (ok, fail) {
      try {
        var bin = atob(raw.textContent.trim()), u8 = new Uint8Array(bin.length);
        for (var i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i);
        if (typeof DecompressionStream !== 'function') { fail(new Error('prohlížeč neumí rozbalit vložená data')); return; }
        new Response(new Blob([u8]).stream().pipeThrough(new DecompressionStream('gzip'))).text().then(function (t) { ok(JSON.parse(t)); }, fail);
      } catch (e) { fail(e); }
    });
  }
  function prepare(g) {
    G = g; K = g.kinds.map(function (k) { return k[0]; }); EK = g.edgeKinds.map(function (k) { return k[0]; });
    N = g.nodes.map(function (n, i) { return { i: i, k: K[n[0]], label: n[1], sum: n[2], src: n[3], st: n[4] }; });
    N.forEach(function (n) {
      out.push([]); inc.push([]);
      (byKind[n.k] = byKind[n.k] || []).push(n);
      var key = n.k + ':' + n.label; keyOf.push(key); byKey[key] = n;
    });
    g.edges.forEach(function (e) { out[e[0]].push([e[1], EK[e[2]]]); inc[e[1]].push([e[0], EK[e[2]]]); });
    K.forEach(function (k) {
      var gi = GROUPS.length - 1;
      GROUPS.forEach(function (gr, i) { if (gr[1].indexOf(k) >= 0) gi = i; });
      groupOf[k] = gi;
    });
  }
  function kindDesc(k) { var r = G.kinds.filter(function (x) { return x[0] === k; })[0]; return r ? r[1] : ''; }
  function edgeDesc(k) { var r = G.edgeKinds.filter(function (x) { return x[0] === k; })[0]; return r ? r[1] : ''; }
  function color(k) { return pal()[groupOf[k]]; }
  function hashOf(n) { return '#g.' + encodeURIComponent(keyOf[n.i]); }
  function inCorpus(p) { return !!(p && window.mjDocs && window.mjDocs.index.docs.some(function (d) { return d.p === p; })); }
  var corpusSet = null;
  function hasDoc(p) {
    if (!corpusSet) { corpusSet = {}; if (window.mjDocs) window.mjDocs.index.docs.forEach(function (d) { corpusSet[d.p] = 1; }); }
    return !!corpusSet[p];
  }

  /* one tile for one object, used by every view */
  function tile(n, parent, opts) {
    opts = opts || {};
    var t = el('div', { 'class': 'g-tile' + (opts.center ? ' center' : '') }, parent);
    t.style.setProperty('--k', color(n.k));
    var head = el('a', { href: hashOf(n), 'class': 'g-tile-h', title: 'Ukázat okolí tohoto objektu' }, t);
    el('span', { 'class': 'g-kind' }, head, GLOSS[n.k] && GLOSS[n.k] !== n.k ? n.k + ' · ' + GLOSS[n.k] : n.k);
    el('b', null, head, n.label);
    if (n.sum) el('span', { 'class': 'g-sum' }, t, n.sum);
    var ln = el('span', { 'class': 'g-links' }, t);
    if (n.src && hasDoc(n.src)) el('a', { href: '#doc.' + n.src, 'class': 'md-in' }, ln, 'čítárna');
    if (n.src && n.src.indexOf('://') < 0) el('a', { href: GH + '/blob/' + G.commit + '/' + n.src, 'class': 'md-gh' }, ln, 'zdroj');
    el('span', { 'class': 'g-deg', title: 'vztahy dovnitř / ven' }, ln, '← ' + inc[n.i].length + ' · → ' + out[n.i].length);
    return t;
  }

  /* ---------- kind map ---------- */
  var chart = null;
  function agg() {
    var m = {};
    G.edges.forEach(function (e) {
      var a = N[e[0]].k, b = N[e[1]].k, k = a + '|' + b;
      (m[k] = m[k] || { a: a, b: b, n: 0, kinds: {} });
      m[k].n++; m[k].kinds[EK[e[2]]] = (m[k].kinds[EK[e[2]]] || 0) + 1;
    });
    return Object.keys(m).map(function (k) { return m[k]; });
  }
  function kindPanel(k) {
    var panel = $('gk-panel'); clear(panel);
    var rows = byKind[k] || [];
    var h = el('p', { 'class': 'gk-h' }, panel);
    var sw = el('span', { 'class': 'g-sw' }, h); sw.style.background = color(k);
    el('b', null, h, k + (GLOSS[k] && GLOSS[k] !== k ? ' · ' + GLOSS[k] : ''));
    h.appendChild(document.createTextNode(' — ' + rows.length + ' objektů'));
    if (kindDesc(k)) el('p', { 'class': 'gk-desc' }, panel, '„' + kindDesc(k) + '“');
    var rel = el('div', { 'class': 'chips-btn' }, panel);
    agg().filter(function (r) { return r.a === k || r.b === k; }).sort(function (x, y) { return y.n - x.n; }).forEach(function (r) {
      Object.keys(r.kinds).forEach(function (ek) {
        var b = el('button', { type: 'button', 'class': 'chip', title: edgeDesc(ek) }, rel, r.a + ' → ' + ek + ' → ' + r.b + ' · ' + r.kinds[ek]);
        b.addEventListener('click', function () { pairPanel(r.a, r.b, ek); });
      });
    });
    var q = el('input', { 'class': 'inp', type: 'search', placeholder: 'filtr v ' + rows.length + ' objektech', 'aria-label': 'Filtr objektů druhu ' + k }, panel);
    var grid = el('div', { 'class': 'g-grid' }, panel), more = el('button', { type: 'button', 'class': 'btn ghost small' }, panel, '');
    var shown = 0, cur = rows;
    function draw(reset) {
      if (reset) { clear(grid); shown = 0; }
      cur.slice(shown, shown + 24).forEach(function (n) { tile(n, grid); });
      shown = Math.min(cur.length, shown + 24);
      more.hidden = shown >= cur.length;
      more.textContent = 'Ukázat dalších ' + Math.min(24, cur.length - shown) + ' z ' + (cur.length - shown);
    }
    q.addEventListener('input', function () {
      var s = q.value.trim().toLowerCase();
      cur = s ? rows.filter(function (n) { return (n.label + ' ' + n.sum).toLowerCase().indexOf(s) >= 0; }) : rows;
      draw(true);
    });
    more.addEventListener('click', function () { draw(false); });
    draw(true);
  }
  function pairPanel(a, b, ek) {
    var panel = $('gk-panel'); clear(panel);
    var pairs = [];
    G.edges.forEach(function (e) { if (N[e[0]].k === a && N[e[1]].k === b && (!ek || EK[e[2]] === ek)) pairs.push(e); });
    var h = el('p', { 'class': 'gk-h' }, panel);
    el('b', null, h, a + ' → ' + (ek || 'vztah') + ' → ' + b);
    h.appendChild(document.createTextNode(' — ' + pairs.length + ' dvojic'));
    if (ek && edgeDesc(ek)) el('p', { 'class': 'gk-desc' }, panel, '„' + edgeDesc(ek) + '“' + (EGLOSS[ek] ? ' · česky: ' + EGLOSS[ek] : ''));
    var back = el('div', { 'class': 'chips-btn' }, panel);
    [a, b].filter(function (x, i, arr) { return arr.indexOf(x) === i; }).forEach(function (k) {
      el('button', { type: 'button', 'class': 'chip' }, back, 'druh ' + k).addEventListener('click', function () { kindPanel(k); });
    });
    var list = el('div', { 'class': 'g-pairs' }, panel), more = el('button', { type: 'button', 'class': 'btn ghost small' }, panel, ''), shown = 0;
    function draw() {
      pairs.slice(shown, shown + 12).forEach(function (e) {
        var row = el('div', { 'class': 'g-pair' }, list);
        tile(N[e[0]], row);
        el('span', { 'class': 'g-arrow', title: edgeDesc(EK[e[2]]) }, row, EK[e[2]] + ' →');
        tile(N[e[1]], row);
      });
      shown = Math.min(pairs.length, shown + 12);
      more.hidden = shown >= pairs.length;
      more.textContent = 'Ukázat dalších ' + Math.min(12, pairs.length - shown) + ' z ' + (pairs.length - shown);
    }
    more.addEventListener('click', draw);
    draw();
  }
  function kindMap() {
    $('gk-total').textContent = N.length + ' objektů, ' + G.edges.length + ' vztahů, ' + K.length + ' druhů · commit ' + G.commit + ' · ' + G.generator;
    var legend = $('gk-legend'); clear(legend);
    GROUPS.forEach(function (gr, i) {
      var s = el('span', { 'class': 'gk-leg' }, legend), sw = el('span', { 'class': 'g-sw' }, s);
      sw.style.background = pal()[i]; s.appendChild(document.createTextNode(gr[0]));
    });
    var t = $('gk-table'); clear(t);
    var tr = el('tr', null, el('thead', null, t));
    ['Druh', 'Objektů', 'Vztahů ven', 'Vztahů dovnitř', 'Popis v repozitáři'].forEach(function (h) { el('th', null, tr, h); });
    var tb = el('tbody', null, t);
    K.slice().sort(function (a, b) { return byKind[b].length - byKind[a].length; }).forEach(function (k) {
      var r = el('tr', null, tb), o = 0, i = 0;
      byKind[k].forEach(function (n) { o += out[n.i].length; i += inc[n.i].length; });
      var c = el('td', null, r);
      el('button', { type: 'button', 'class': 'chip' }, c, k).addEventListener('click', function () { kindPanel(k); $('gk-panel').scrollIntoView({ block: 'nearest' }); });
      [byKind[k].length, o, i, kindDesc(k)].forEach(function (v) { el('td', null, r, String(v)); });
    });
    var box = $('gk-chart');
    function draw() {
      if (chart) chart.dispose();
      chart = window.echarts.init(box);
      var fg = cssv('--fg'), panel = cssv('--panel'), line = cssv('--line'), muted = cssv('--muted');
      var order = [];
      GROUPS.forEach(function (gr, gi) { K.filter(function (k) { return groupOf[k] === gi; }).sort(function (a, b) { return byKind[b].length - byKind[a].length; }).forEach(function (k) { order.push(k); }); });
      var links = agg().filter(function (r) { return r.a !== r.b; }).map(function (r) {
        return { source: r.a, target: r.b, value: r.n, kinds: r.kinds, lineStyle: { width: 1 + Math.log2(r.n) * 0.8 } };
      });
      chart.setOption({
        tooltip: { backgroundColor: panel, borderColor: line, textStyle: { color: fg, fontSize: 13 }, confine: true,
          formatter: function (p) {
            if (p.dataType === 'edge') return p.data.source + ' → ' + p.data.target + '<br>' + Object.keys(p.data.kinds).map(function (k) { return k + ': <b>' + p.data.kinds[k] + '</b>'; }).join('<br>');
            return '<b>' + p.name + '</b>: ' + p.data.count + ' objektů';
          } },
        series: [{
          type: 'graph', layout: 'circular', circular: { rotateLabel: true }, roam: true, animation: !reduce,
          top: 70, bottom: 70, left: 90, right: 90,
          label: { show: true, color: fg, fontSize: 11, formatter: function (p) { return p.name + ' ' + p.data.count; } },
          edgeSymbol: ['none', 'arrow'], edgeSymbolSize: 6,
          lineStyle: { color: muted, opacity: 0.35, curveness: 0.25 },
          emphasis: { focus: 'adjacency', lineStyle: { opacity: 0.95 } },
          itemStyle: { borderColor: panel, borderWidth: 1.5 },
          data: order.map(function (k) { return { name: k, count: byKind[k].length, symbolSize: 9 + Math.sqrt(byKind[k].length) * 2.1, itemStyle: { color: color(k) } }; }),
          links: links
        }]
      });
      chart.on('click', function (p) {
        if (p.dataType === 'node') kindPanel(p.name);
        else if (p.dataType === 'edge') pairPanel(p.data.source, p.data.target, Object.keys(p.data.kinds).length === 1 ? Object.keys(p.data.kinds)[0] : null);
      });
    }
    var p = window.echarts ? Promise.resolve() : new Promise(function (ok, fail) {
      var s = document.createElement('script'); s.src = CDN; s.async = true; s.onload = function () { ok(); };
      s.onerror = function () { fail(new Error('knihovnu grafů se nepodařilo načíst')); }; document.head.appendChild(s);
    });
    p.then(draw, function (e) { clear($('gk-panel')); el('p', { 'class': 'lab-note' }, $('gk-panel'), e.message + '. Stejná data jsou v tabulce níže.'); });
    window.addEventListener('resize', function () { if (chart) chart.resize(); });
    matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function () { if (window.echarts) draw(); });
  }

  /* ---------- flows found in the data ---------- */
  function instances(pat, cap) {
    var res = [], total = 0;
    function walk(path, s) {
      if (s === pat.steps.length) { total++; if (res.length < cap) res.push(path.slice()); return; }
      var st = pat.steps[s], cur = path[path.length - 1];
      (st[1] === 'out' ? out : inc)[cur].forEach(function (x) {
        if (x[1] !== st[0] || N[x[0]].k !== st[2] || path.indexOf(x[0]) >= 0 || total > 20000) return;
        path.push(x[0]); walk(path, s + 1); path.pop();
      });
    }
    (byKind[pat.start] || []).forEach(function (n) { if (total <= 20000) walk([n.i], 0); });
    return { list: res, total: total };
  }
  function chains() {
    var seg = $('gc-seg'), note = $('gc-note'), chain = $('gc-chain'), ctl = $('gc-ctl');
    var found = PATTERNS.map(function (p) { return instances(p, 200); });
    function show(pi, k) {
      var pat = PATTERNS[pi], f = found[pi];
      Array.prototype.forEach.call(seg.children, function (b, i) { b.setAttribute('aria-selected', String(i === pi)); });
      clear(note); clear(chain); clear(ctl);
      el('b', null, note, pat.label + '. ');
      note.appendChild(document.createTextNode(pat.text + ' '));
      pat.links.forEach(function (l, i) { if (i) note.appendChild(document.createTextNode(' · ')); el('a', { href: '#' + l[0] }, note, '§ ' + l[1]); });
      if (!f.total) {
        el('p', { 'class': 'lab-note' }, ctl, 'V grafu commitu ' + G.commit + ' žádný takový řetěz není. Vzor: ' + pat.start + pat.steps.map(function (s) { return (s[1] === 'out' ? ' → ' : ' ← ') + s[0] + (s[1] === 'out' ? ' → ' : ' ← ') + s[2]; }).join(''));
        return;
      }
      k = ((k % f.list.length) + f.list.length) % f.list.length;
      var prev = el('button', { type: 'button', 'class': 'btn ghost small' }, ctl, '◀ předchozí příklad');
      var next = el('button', { type: 'button', 'class': 'btn ghost small' }, ctl, 'další příklad ▶');
      el('span', { 'class': 'flow-pos' }, ctl, 'příklad ' + (k + 1) + ' z ' + f.list.length + (f.total > f.list.length ? ' zobrazených' : '') + ' · v repozitáři je takových řetězů ' + (f.total > 20000 ? 'přes 20 000' : f.total));
      prev.addEventListener('click', function () { show(pi, k - 1); });
      next.addEventListener('click', function () { show(pi, k + 1); });
      f.list[k].forEach(function (ni, i) {
        if (i) {
          var st = pat.steps[i - 1], a = el('li', { 'class': 'gc-rel' }, chain);
          el('span', { 'class': 'gc-arrow' }, a, st[1] === 'out' ? '→' : '←');
          el('b', null, a, st[0]);
          el('span', null, a, (EGLOSS[st[0]] ? EGLOSS[st[0]] + ' · ' : '') + '„' + edgeDesc(st[0]) + '“');
        }
        var li = el('li', { 'class': 'gc-node' }, chain);
        el('span', { 'class': 'flow-step-n' }, li, String(i + 1));
        tile(N[ni], li);
      });
    }
    PATTERNS.forEach(function (p, i) {
      var b = el('button', { type: 'button', 'class': 'seg-b', role: 'tab', 'aria-selected': 'false' }, seg, p.label + ' · ' + (found[i].total > 20000 ? '20 000+' : found[i].total));
      b.addEventListener('click', function () { show(i, 0); });
    });
    show(0, 0);
  }

  /* ---------- one object and what surrounds it ---------- */
  var trail = [];
  function trace(n, quiet) {
    var grid = $('gt-grid'), tr = $('gt-trail');
    if (trail[trail.length - 1] !== n) trail.push(n);
    if (trail.length > 8) trail.shift();
    window.mjGraphHash = hashOf(n).slice(1);
    window.mjDocUntil = Date.now() + 1500;
    try { history.replaceState(null, '', hashOf(n)); } catch (e) { /* sandboxed frame */ }
    clear(tr); el('span', { 'class': 'lbl', style: 'display:inline;margin:0 6px 0 0' }, tr, 'Cesta');
    trail.forEach(function (t, i) {
      if (i) tr.appendChild(document.createTextNode(' › '));
      if (t === n) el('b', null, tr, t.label); else el('a', { href: hashOf(t) }, tr, t.label);
    });
    var cp = el('button', { type: 'button', 'class': 'btn ghost small', style: 'margin:0 0 0 10px' }, tr, 'Kopírovat odkaz');
    cp.addEventListener('click', function () {
      var url = ARTIFACT + hashOf(n), old = cp.textContent;
      function ok() { cp.textContent = 'odkaz zkopírován'; setTimeout(function () { cp.textContent = old; }, 1500); }
      try { navigator.clipboard.writeText(url).then(ok, function () { cp.textContent = url; }); } catch (e) { cp.textContent = url; }
    });
    clear(grid);
    function column(title, list, dir) {
      var col = el('div', { 'class': 'gt-col' }, grid);
      el('p', { 'class': 'rd-box-h' }, col, title + ' · ' + list.length);
      var groups = {};
      list.forEach(function (x) { (groups[x[1]] = groups[x[1]] || []).push(N[x[0]]); });
      Object.keys(groups).sort(function (a, b) { return groups[b].length - groups[a].length; }).forEach(function (ek) {
        var box = el('div', { 'class': 'gt-group' }, col);
        var h = el('p', { 'class': 'gt-rel' }, box);
        el('b', null, h, (dir === 'in' ? ek + ' →' : '→ ' + ek));
        el('span', null, h, ' ' + groups[ek].length + ' · „' + edgeDesc(ek) + '“');
        var items = groups[ek];
        items.slice(0, 6).forEach(function (m) { tile(m, box); });
        if (items.length > 6) {
          var b = el('button', { type: 'button', 'class': 'btn ghost small' }, box, 'Ukázat všech ' + items.length);
          b.addEventListener('click', function () { items.slice(6).forEach(function (m) { box.insertBefore(tile(m, null), b); }); b.remove(); });
        }
      });
      if (!list.length) el('p', { 'class': 'lab-note' }, col, dir === 'in' ? 'Nic na tento objekt neukazuje.' : 'Tento objekt na nic neukazuje.');
    }
    column('Ukazují sem', inc[n.i], 'in');
    var mid = el('div', { 'class': 'gt-col mid' }, grid);
    el('p', { 'class': 'rd-box-h' }, mid, 'Vybraný objekt');
    tile(n, mid, { center: true });
    if (kindDesc(n.k)) el('p', { 'class': 'gk-desc' }, mid, n.k + ': „' + kindDesc(n.k) + '“');
    if (n.st) el('p', { 'class': 'rd-pills' }, mid).appendChild(el('span', { 'class': 'pill reports' }, null, 'status: ' + n.st));
    el('button', { type: 'button', 'class': 'btn ghost small' }, mid, 'Všechny objekty druhu ' + n.k).addEventListener('click', function () { kindPanel(n.k); $('lab-kinds').scrollIntoView({ block: 'start' }); });
    column('Ukazuje na', out[n.i], 'out');
    if (!quiet) {
      var y = $('lab-trace').getBoundingClientRect().top + window.scrollY - 70;
      window.scrollTo({ top: y, behavior: reduce ? 'auto' : 'smooth' });
    }
  }
  function search() {
    var q = $('gt-q'), hits = $('gt-hits');
    q.addEventListener('input', function () {
      var s = q.value.trim().toLowerCase();
      clear(hits); hits.hidden = !s;
      if (!s) return;
      var found = N.filter(function (n) { return n.label.toLowerCase().indexOf(s) >= 0; });
      if (found.length < 30) found = found.concat(N.filter(function (n) { return found.indexOf(n) < 0 && n.sum.toLowerCase().indexOf(s) >= 0; }));
      found.slice(0, 30).forEach(function (n) {
        var li = el('li', { 'class': 'rd-item' }, hits);
        el('a', { href: hashOf(n), 'class': 'rd-t' }, li, n.label);
        var k = el('span', { 'class': 'rd-k' }, li, n.k); k.style.setProperty('--k', color(n.k));
        el('span', { 'class': 'rd-p' }, li, n.sum);
      });
      if (!found.length) el('li', { 'class': 'rd-item' }, hits, 'Nic nenalezeno.');
    });
  }
  function open(hash, quiet) {
    var n = byKey[decodeURIComponent(hash.replace(/^#?g\./, ''))];
    if (!n) return false;
    trace(n, quiet);
    return true;
  }

  var ready = null;
  function boot() {
    if (ready) return ready;
    ready = decode().then(function (g) {
      prepare(g); kindMap(); chains(); search();
      var first = (byKind.claim || byKind.rule || N).filter(function (n) { return inc[n.i].length > 1 && out[n.i].length > 2; })[0] || N[0];
      trace(first, true);
      window.mjGraphHash = null;
      return true;
    });
    ready.catch(function (e) { el('p', { 'class': 'lab-note' }, $('gk-panel'), 'Graf se nepodařilo načíst: ' + e.message); });
    return ready;
  }
  window.mjGraph = function (hash) { return boot().then(function () { return open(hash); }); };
  window.mjGraphUI = { ready: function () { return boot(); }, tile: function (i, parent, opts) { return tile(N[i], parent, opts); }, kind: function (i) { return N[i].k; }, color: color, kindPanel: function (k) { kindPanel(k); } };
  window.mjGraphData = function () { return boot().then(function () { return { nodes: N.length, edges: G.edges.length, kinds: K.length, keys: keyOf }; }); };
  document.addEventListener('click', function (e) {
    var a = e.target.closest ? e.target.closest('a') : null;
    if (!a) return;
    var h = a.getAttribute('href') || '';
    if (h.indexOf('#g.') !== 0) return;
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey || e.button) return;
    e.preventDefault(); e.stopPropagation();
    window.mjGraph(h);
  }, true);
  window.addEventListener('hashchange', function () { if (location.hash.indexOf('#g.') === 0) window.mjGraph(location.hash); });
  if (location.hash.indexOf('#g.') === 0) setTimeout(function () { window.mjGraph(location.hash); }, 150);
  else if ('IntersectionObserver' in window) {
    var io = new IntersectionObserver(function (es) { es.forEach(function (x) { if (x.isIntersecting) { boot(); io.disconnect(); } }); }, { rootMargin: '800px' });
    io.observe(sec);
  } else boot();
})();
