(function () {
  'use strict';
  var GH = 'https://github.com/korczis/prismatic-majordomus';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  function readJSON(id) { var e = $(id); try { return e ? JSON.parse(e.textContent) : null; } catch (x) { return null; } }
  function norm(s) { return String(s || '').toLowerCase(); }
  function jump(id) { if (window.mjJump) window.mjJump(id); }
  function chapterTitle(id) { var s = $(id), h = s && s.querySelector('h2'); return h && h.firstChild ? h.firstChild.textContent.trim() : id; }
  function chapterTldr(id) { var s = $(id), p = s && s.querySelectorAll('.tldr p'); return p && p.length ? [p[0].textContent.replace(/^TL;DR/, '').trim(), p[1] ? p[1].textContent.replace(/^Dopad/, '').trim() : ''] : ['', '']; }

  /* ---------- architecture drill-down ---------- */
  var arch = readJSON('data-arch'), ex = readJSON('data-explore') || {}, gates = readJSON('data-gates') || { gates: [] };
  var tracked = {};
  (readJSON('data-tracked') || []).forEach(function (p) { tracked[p] = true; });
  if (arch && $('lab-arch')) (function () {
    var colL = $('arch-l'), colI = $('arch-i'), det = $('arch-d'), crumb = $('arch-crumb');
    var li = 0, ii = -1;
    function hits(list, kw, text) {
      return (list || []).filter(function (x) { var t = norm(text(x)); return kw.some(function (k) { return t.indexOf(k) !== -1; }); });
    }
    function related(item) {
      var kw = item.kw || [];
      return {
        caps: (ex.caps || []).filter(function (c) { return (item.mod || []).indexOf(c.m) !== -1; }),
        rules: hits(ex.rules, kw, function (r) { return r.id; }),
        gates: hits(gates.gates, kw, function (g) { return g.id + ' ' + g.job; }),
        adrs: hits(ex.adrs, kw, function (a) { return a.t; }),
        claims: hits(ex.claims, kw, function (c) { return c.id; })
      };
    }
    function group(parent, title, n, more) {
      var g = el('div', { 'class': 'arch-g' }, parent);
      var h = el('p', { 'class': 'arch-gh' }, g);
      el('b', null, h, title);
      if (n !== undefined) el('span', { 'class': 'arch-n' }, h, String(n));
      if (more) { var b = el('button', { type: 'button', 'class': 'arch-more' }, h, 'všechny v průzkumníku →'); b.addEventListener('click', more); }
      return el('div', { 'class': 'arch-gb' }, g);
    }
    function srcLink(parent, path) {
      var p = path.replace(/\/$/, '');
      if (tracked[path] || tracked[p]) {
        var a = el('a', { href: GH + (/\/$/.test(path) ? '/tree/master/' : '/blob/master/') + p, 'class': 'src' }, parent);
        el('code', null, a, path);
      } else el('code', null, parent, path);
    }
    function renderDetail() {
      clear(det); clear(crumb);
      var layer = arch[li], item = ii >= 0 ? layer.items[ii] : null;
      var c0 = el('button', { type: 'button', 'class': 'arch-c' }, crumb, 'Architektura');
      c0.addEventListener('click', function () { ii = -1; render(); });
      crumb.appendChild(document.createTextNode(' › '));
      var c1 = el('button', { type: 'button', 'class': 'arch-c' }, crumb, layer.n);
      c1.addEventListener('click', function () { ii = -1; render(); });
      if (item) { crumb.appendChild(document.createTextNode(' › ')); el('span', { 'class': 'arch-c cur' }, crumb, item.n); }
      if (!item) {
        el('h3', { 'class': 'h4' }, det, layer.n);
        el('p', null, det, layer.d);
        var open = el('button', { type: 'button', 'class': 'btn small' }, det, 'Otevřít kapitolu „' + chapterTitle(layer.ch) + '“ →');
        open.addEventListener('click', function () { jump(layer.ch); });
        var cards = el('div', { 'class': 'arch-cards' }, det);
        layer.items.forEach(function (it, k) {
          var r = related(it);
          var b = el('button', { type: 'button', 'class': 'arch-card' }, cards);
          el('b', null, b, it.n);
          el('span', null, b, chapterTldr(it.ch)[0]);
          el('small', null, b, r.caps.length + ' capabilities · ' + r.rules.length + ' pravidel · ' + r.gates.length + ' gates · ' + r.adrs.length + ' ADR');
          b.addEventListener('click', function () { ii = k; render(); });
        });
        return;
      }
      var tl = chapterTldr(item.ch), r = related(item);
      el('h3', { 'class': 'h4' }, det, item.n);
      if (tl[0]) el('p', null, det, tl[0]);
      if (tl[1]) { var im = el('p', { 'class': 'arch-imp' }, det); el('span', { 'class': 'k i' }, im, 'Dopad'); im.appendChild(document.createTextNode(' ' + tl[1])); }
      var open2 = el('button', { type: 'button', 'class': 'btn primary small' }, det, 'Otevřít kapitolu „' + chapterTitle(item.ch) + '“ →');
      open2.addEventListener('click', function () { jump(item.ch); });
      var stat = el('div', { 'class': 'arch-stat' }, det);
      [['capabilities', r.caps.length], ['pravidel', r.rules.length], ['gates', r.gates.length], ['ADR', r.adrs.length], ['garancí', r.claims.length]].forEach(function (s) {
        var d = el('div', null, stat); el('b', null, d, String(s[1])); el('span', null, d, s[0]);
      });
      if (item.paths && item.paths.length) {
        var gp = group(det, 'Kde to je v repu');
        item.paths.forEach(function (p) { srcLink(gp, p); gp.appendChild(document.createTextNode(' ')); });
      }
      if (item.cmds && item.cmds.length) {
        var gc = group(det, 'Příkazy');
        item.cmds.forEach(function (c) {
          var b = el('button', { type: 'button', 'class': 'chip' }, gc, c);
          b.title = 'Hledat v příručce';
          b.addEventListener('click', function () { if (window.mjSearch) window.mjSearch(c.replace(/^majordomus(-cli)? /, '').replace(/[<>"…]/g, ' ').split(' --')[0].trim()); });
        });
      }
      if (r.caps.length) {
        var gk = group(det, 'Capabilities', r.caps.length, function () { window.mjExplore('caps', '', item.mod[0]); });
        r.caps.slice(0, 8).forEach(function (c) {
          var b = el('button', { type: 'button', 'class': 'chip' }, gk, c.id);
          b.title = c.t;
          b.addEventListener('click', function () { window.mjExplore('caps', c.id); });
        });
        if (item.mod.length > 1) { var mm = el('p', { 'class': 'arch-mods' }, gk, 'moduly: '); item.mod.forEach(function (m, k) { if (k) mm.appendChild(document.createTextNode(', ')); var b = el('button', { type: 'button', 'class': 'arch-more' }, mm, m); b.addEventListener('click', function () { window.mjExplore('caps', '', m); }); }); }
      }
      function listGroup(title, items, tab, label, href) {
        if (!items.length) return;
        var g = group(det, title, items.length, function () { window.mjExplore(tab, item.kw[0]); });
        var ul = el('ul', { 'class': 'arch-ul' }, g);
        items.slice(0, 5).forEach(function (x) {
          var liEl = el('li', null, ul), h = href(x);
          if (h) el('a', { href: h, 'class': 'src' }, liEl, label(x)); else el('span', null, liEl, label(x));
        });
      }
      listGroup('Pravidla', r.rules, 'rule', function (x) { return x.id; }, function (x) { return GH + '/blob/master/' + x.p; });
      listGroup('CI gates', r.gates, 'gate', function (x) { return x.id + ' · ' + x.job; }, function () { return null; });
      listGroup('Rozhodnutí', r.adrs, 'adr', function (x) { return 'ADR ' + x.n + ' · ' + x.t; }, function (x) { return GH + '/blob/master/' + x.p; });
      listGroup('Garance', r.claims, 'claims', function (x) { return x.id; }, function (x) { return 'https://majordomus.dev/guarantees/' + x.id + '/'; });
    }
    function render() {
      clear(colL); clear(colI);
      arch.forEach(function (L, k) {
        var b = el('button', { type: 'button', role: 'option', 'class': 'arch-b', 'aria-selected': k === li ? 'true' : 'false' }, colL);
        el('b', null, b, L.n);
        el('small', null, b, L.items.length + ' komponent');
        b.addEventListener('click', function () { li = k; ii = -1; render(); });
      });
      arch[li].items.forEach(function (it, k) {
        var b = el('button', { type: 'button', role: 'option', 'class': 'arch-b', 'aria-selected': k === ii ? 'true' : 'false' }, colI);
        el('b', null, b, it.n);
        el('small', null, b, chapterTitle(it.ch));
        b.addEventListener('click', function () { ii = k; render(); });
      });
      renderDetail();
    }
    window.mjArch = function (l, i) {
      li = l; ii = i === undefined ? -1 : i; render();
      $('lab-arch').scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block: 'start' });
    };
    render();
    var MAPD = [[0], [0, 1], [1, 1], [1, 2], [1, 3], [1, 0], [1, 4], [2], [3], [2, 0], [2, 1], [2, 2], [2, 3], [2, 4], [2, 5], [2, 6], [2, 7], [3, 0], [3, 1], [3, 3], [3, 4], [4, 0], [2, 6], [5]];
    var sec = $('stavba'), dia = sec && sec.querySelector('.fig svg');
    if (dia) Array.prototype.forEach.call(dia.querySelectorAll('a'), function (a, k) {
      if (!MAPD[k]) return;
      a.addEventListener('click', function (e) { e.preventDefault(); window.mjArch(MAPD[k][0], MAPD[k][1]); });
    });
  })();

  /* ---------- sticky context bar ---------- */
  (function () {
    var main = document.querySelector('main');
    if (!main) return;
    var secs = Array.prototype.slice.call(main.querySelectorAll('section[id]'));
    var parts = Array.prototype.slice.call(main.querySelectorAll('.part'));
    var bar = el('div', { 'class': 'ctx-bar' });
    bar.hidden = true;
    main.insertBefore(bar, main.firstChild);
    var prev = el('button', { type: 'button', 'class': 'ctx-b', 'aria-label': 'Předchozí kapitola' }, bar, '←');
    var where = el('div', { 'class': 'ctx-w' }, bar);
    var partA = el('a', { 'class': 'ctx-p' }, where);
    el('span', { 'class': 'ctx-sep' }, where, '›');
    var chapA = el('a', { 'class': 'ctx-c' }, where);
    var pos = el('span', { 'class': 'ctx-n' }, bar);
    var find = el('button', { type: 'button', 'class': 'ctx-b', 'aria-label': 'Hledat' }, bar, 'Hledat');
    var next = el('button', { type: 'button', 'class': 'ctx-b', 'aria-label': 'Další kapitola' }, bar, '→');
    var cur = 0;
    function partOf(sec) { var p = null; parts.forEach(function (x) { if (x.compareDocumentPosition(sec) & Node.DOCUMENT_POSITION_FOLLOWING) p = x; }); return p; }
    function update() {
      var y = 90, best = -1;
      secs.forEach(function (s, i) { if (s.getBoundingClientRect().top <= y) best = i; });
      bar.hidden = best < 0;
      if (best < 0) return;
      cur = best;
      var s = secs[cur], p = partOf(s);
      var h = s.querySelector('h2');
      chapA.textContent = h && h.firstChild ? h.firstChild.textContent.trim() : s.id;
      chapA.href = '#' + s.id;
      if (p) { partA.textContent = p.textContent.replace(/^Část /, ''); partA.href = '#' + p.id; }
      pos.textContent = (cur + 1) + ' / ' + secs.length;
      prev.disabled = cur === 0; next.disabled = cur === secs.length - 1;
    }
    prev.addEventListener('click', function () { if (secs[cur - 1]) jump(secs[cur - 1].id); });
    next.addEventListener('click', function () { if (secs[cur + 1]) jump(secs[cur + 1].id); });
    find.addEventListener('click', function () { if (window.mjSearch) window.mjSearch(''); });
    chapA.addEventListener('click', function (e) { e.preventDefault(); jump(secs[cur].id); });
    var ticking = false;
    window.addEventListener('scroll', function () { if (!ticking) { ticking = true; requestAnimationFrame(function () { ticking = false; update(); }); } }, { passive: true });
    update();
  })();
})();
