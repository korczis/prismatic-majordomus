(function () {
  'use strict';
  /* Flows computed from repository data: rule -> proof, changed path -> gates. */
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  if (!$('toky') || !window.mjDocs) return;
  var CDN = 'https://cdnjs.cloudflare.com/ajax/libs/echarts/5.5.0/echarts.min.js';
  var reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
  function cssv(n) { return getComputedStyle(document.documentElement).getPropertyValue(n).trim(); }
  function dark() {
    var t = document.documentElement.getAttribute('data-theme');
    return t ? t === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
  }
  var libP = null;
  function lib() {
    if (window.echarts) return Promise.resolve();
    if (libP) return libP;
    libP = new Promise(function (ok, fail) {
      var s = document.createElement('script'); s.src = CDN; s.async = true;
      s.onload = function () { ok(); }; s.onerror = function () { libP = null; fail(new Error('knihovnu grafů se nepodařilo načíst')); };
      document.head.appendChild(s);
    });
    return libP;
  }
  function table(host, headRow, rows) {
    clear(host);
    var tr = el('tr', null, el('thead', null, host));
    headRow.forEach(function (h) { el('th', null, tr, h); });
    var tb = el('tbody', null, host);
    rows.forEach(function (r) { var t = el('tr', null, tb); r.forEach(function (c) { el('td', null, t, String(c)); }); });
  }
  function sankey(box, nodes, links, onClick) {
    var c = window.echarts.init(box), fg = cssv('--fg'), panel = cssv('--panel'), line = cssv('--line');
    c.setOption({
      tooltip: { trigger: 'item', backgroundColor: panel, borderColor: line, textStyle: { color: fg, fontSize: 13 }, confine: true,
                 formatter: function (p) { return p.dataType === 'edge' ? p.data.source + ' → ' + p.data.target + ': <b>' + p.data.value + '</b>' : '<b>' + p.name + '</b>: ' + p.value; } },
      series: [{
        type: 'sankey', left: 8, right: 150, top: 10, bottom: 10, nodeWidth: 14, nodeGap: 10, draggable: false, animation: !reduce,
        emphasis: { focus: 'adjacency' }, label: { color: fg, fontSize: 12 },
        lineStyle: { color: 'gradient', opacity: dark() ? 0.35 : 0.3, curveness: 0.5 },
        itemStyle: { borderColor: panel, borderWidth: 1 },
        data: nodes, links: links
      }]
    });
    c.on('click', onClick);
    window.addEventListener('resize', function () { c.resize(); });
    return c;
  }
  var PAL = function () { return dark() ? ['#3987e5', '#d95926', '#199e70', '#c98500', '#d55181', '#9085e9'] : ['#2a78d6', '#eb6834', '#1baf7a', '#eda100', '#e87ba4', '#6250d6']; };

  /* ---------- rule -> proof ---------- */
  var DOCS = window.mjDocs.index.docs;
  var rules = DOCS.filter(function (d) { return d.k === 'rule' && d.rid; });
  var CLS = { blocking: 'blokující', advisory: 'doporučující' };
  function proofOf(d) {
    var pf = d.pf || [];
    if (pf.indexOf('tests') >= 0 && pf.indexOf('validator') >= 0) return 'test a validátor';
    if (pf.indexOf('tests') >= 0) return 'test';
    if (pf.indexOf('validator') >= 0 || pf.indexOf('enforced_by') >= 0) return 'validátor';
    if (pf.indexOf('reviewed_because') >= 0) return 'posuzuje člověk';
    return 'bez jmenovaného důkazu';
  }
  function nsOf(d) { return 'prostor ' + d.rid.split('.')[0]; }
  function clsOf(d) { return CLS[d.cls] || d.cls || 'neuvedeno'; }
  function ruleFlow() {
    var total = $('fr-total'), note = $('fr-note'), list = $('fr-list');
    total.textContent = rules.length + ' pravidel z commitu ' + window.mjDocs.index.commit;
    var count = {}, a = {}, b = {};
    rules.forEach(function (d) {
      var c = clsOf(d), n = nsOf(d), p = proofOf(d);
      count[c] = (count[c] || 0) + 1; count[n] = (count[n] || 0) + 1; count[p] = (count[p] || 0) + 1;
      a[c + '|' + n] = (a[c + '|' + n] || 0) + 1; b[n + '|' + p] = (b[n + '|' + p] || 0) + 1;
    });
    var pal = PAL(), colors = { 'blokující': pal[1], 'doporučující': pal[3], 'test a validátor': pal[2], 'test': pal[2], 'validátor': pal[2], 'posuzuje člověk': pal[5], 'bez jmenovaného důkazu': cssv('--bad') };
    var nodes = Object.keys(count).map(function (k) { return { name: k, itemStyle: { color: colors[k] || pal[0] } }; });
    var links = [];
    [a, b].forEach(function (m) { Object.keys(m).forEach(function (k) { var p = k.split('|'); links.push({ source: p[0], target: p[1], value: m[k] }); }); });
    function show(title, pick) {
      var rows = rules.filter(pick);
      clear(note); el('b', null, note, title + ': ' + rows.length + ' pravidel. ');
      note.appendChild(document.createTextNode('Každé otevřeš v čítárně.'));
      clear(list); list.hidden = false;
      rows.forEach(function (d) {
        var li = el('li', { 'class': 'rd-item' }, list);
        el('a', { href: '#doc.' + d.p, 'class': 'rd-t md-in k-rule' }, li, d.t);
        el('span', { 'class': 'rd-k k-rule' }, li, clsOf(d) + ' · ' + proofOf(d));
        el('code', { 'class': 'rd-p' }, li, d.rid);
      });
    }
    function match(name) { return function (d) { return clsOf(d) === name || nsOf(d) === name || proofOf(d) === name; }; }
    lib().then(function () {
      sankey($('fr-chart'), nodes, links, function (p) {
        if (p.dataType === 'node') show(p.name, match(p.name));
        else if (p.dataType === 'edge') show(p.data.source + ' → ' + p.data.target, function (d) { return match(p.data.source)(d) && match(p.data.target)(d); });
      });
    }, function (e) { note.textContent = e.message + '. Data jsou v tabulce níže.'; });
    table($('fr-table'), ['Třída', 'Prostor', 'Důkaz', 'Pravidel'], (function () {
      var m = {};
      rules.forEach(function (d) { var k = clsOf(d) + '|' + nsOf(d) + '|' + proofOf(d); m[k] = (m[k] || 0) + 1; });
      return Object.keys(m).sort().map(function (k) { return k.split('|').concat([m[k]]); });
    })());
  }

  /* ---------- changed path -> gates ---------- */
  function gateFlow() {
    var G;
    try { G = JSON.parse($('data-gates').textContent); } catch (e) { return; }
    var byId = {}; G.gates.forEach(function (g) { byId[g.id] = g; });
    var note = $('fg-note'), chips = $('fg-chips');
    var always = G.gates.filter(function (g) { return g.a; });
    $('fg-total').textContent = G.classes.length + ' tříd cest, ' + G.gates.length + ' gates, z toho ' + always.length + ' běží vždy';
    var m = {}, jobs = {}, rows = [];
    G.classes.forEach(function (c) {
      var ids = c.g === 'full' ? G.gates.filter(function (g) { return !g.o; }).map(function (g) { return g.id; }) : c.g;
      ids.forEach(function (id) {
        var g = byId[id]; if (!g) return;
        var j = 'job ' + g.job; jobs[j] = 1;
        m[c.id + '|' + j] = (m[c.id + '|' + j] || 0) + 1;
      });
      rows.push([c.id, c.g === 'full' ? 'plný plán' : ids.length, c.p.join(', ')]);
    });
    var pal = PAL();
    var nodes = G.classes.map(function (c) { return { name: c.id, itemStyle: { color: c.g === 'full' ? pal[1] : pal[0] } }; })
      .concat(Object.keys(jobs).map(function (j) { return { name: j, itemStyle: { color: pal[2] } }; }));
    var links = Object.keys(m).map(function (k) { var p = k.split('|'); return { source: p[0], target: p[1], value: m[k] }; });
    function chip(id) {
      var b = el('button', { type: 'button', 'class': 'chip' }, chips, id);
      b.title = byId[id] ? byId[id].s : '';
      b.addEventListener('click', function () { if (window.mjExplore) window.mjExplore('gate', id); });
    }
    function showClass(c) {
      clear(note); clear(chips);
      el('b', null, note, 'Třída ' + c.id + '. ');
      note.appendChild(document.createTextNode('Cesty: ' + c.p.join(', ') + '. ' + (c.g === 'full' ? 'Eskaluje na plný plán: běží všechny gates kromě těch na vyžádání.' : 'Vybere ' + c.g.length + ' gates; klik na gate ho otevře v průzkumníku.')));
      (c.g === 'full' ? [] : c.g).forEach(chip);
    }
    function showJob(j) {
      var name = j.replace(/^job /, ''), ids = G.gates.filter(function (g) { return g.job === name; }).map(function (g) { return g.id; });
      clear(note); clear(chips);
      el('b', null, note, 'Job ' + name + '. ');
      note.appendChild(document.createTextNode(ids.length + ' gates; klik na gate ho otevře v průzkumníku.'));
      ids.forEach(chip);
    }
    lib().then(function () {
      $('fg-chart').style.height = '640px';
      var c = sankey($('fg-chart'), nodes, links, function (p) {
        var name = p.dataType === 'node' ? p.name : p.data.source;
        var cls = G.classes.filter(function (x) { return x.id === name; })[0];
        if (cls) showClass(cls); else showJob(name);
      });
      c.resize();
    }, function (e) { note.textContent = e.message + '. Data jsou v tabulce níže.'; });
    table($('fg-table'), ['Třída cest', 'Vybraných gates', 'Cesty'], rows);
  }

  /* charts are built when the chapter comes near the viewport */
  var done = false;
  function build() { if (done) return; done = true; ruleFlow(); gateFlow(); }
  if ('IntersectionObserver' in window) {
    var io = new IntersectionObserver(function (es) { es.forEach(function (e) { if (e.isIntersecting) { build(); io.disconnect(); } }); }, { rootMargin: '600px' });
    io.observe($('toky'));
  } else build();
  window.mjFlows = build;
})();
