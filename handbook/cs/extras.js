(function () {
  'use strict';
  var ARTIFACT = 'https://claude.ai/artifact/XgCzwV1MFNDL1ssJHstPQ1';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function store(key, val) {
    try {
      if (val === undefined) return window.localStorage.getItem(key);
      if (val === null) window.localStorage.removeItem(key); else window.localStorage.setItem(key, val);
    } catch (e) { return null; }
    return null;
  }
  function copy(text, done, fail) {
    try { navigator.clipboard.writeText(text).then(done, fail); } catch (e) { fail(); }
  }
  function norm(s) { return s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, ''); }
  function jump(id) {
    var t = $(id);
    if (!t) return;
    t.scrollIntoView({ behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth', block: 'start' });
    try { history.replaceState(null, '', '#' + id); } catch (e) { /* sandboxed */ }
    var h = t.matches('section') ? t.querySelector('h2') : t;
    if (h) { h.setAttribute('tabindex', '-1'); h.focus({ preventScroll: true }); }
  }

  /* ---------- heading anchors + copy link ---------- */
  document.querySelectorAll('main section[id] > h2').forEach(function (h) {
    var id = h.parentElement.id;
    var a = el('a', { 'class': 'h-anchor', href: '#' + id, 'aria-label': 'Odkaz na kapitolu' }, h, '#');
    var b = el('button', { type: 'button', 'class': 'h-copy', 'aria-label': 'Kopírovat odkaz na kapitolu' }, h, 'kopírovat odkaz');
    b.addEventListener('click', function () {
      copy(ARTIFACT + '#' + id, function () { b.textContent = 'zkopírováno'; setTimeout(function () { b.textContent = 'kopírovat odkaz'; }, 1500); },
        function () { b.textContent = ARTIFACT + '#' + id; });
    });
  });

  /* ---------- reading progress + back to top ---------- */
  var bar = el('div', { 'class': 'progress', 'aria-hidden': 'true' }, document.body);
  var top = el('button', { type: 'button', 'class': 'to-top', 'aria-label': 'Nahoru' }, document.body, '↑');
  top.hidden = true;
  top.addEventListener('click', function () { window.scrollTo({ top: 0, behavior: 'smooth' }); });
  function onScroll() {
    var d = document.documentElement;
    var max = d.scrollHeight - d.clientHeight;
    bar.style.transform = 'scaleX(' + (max > 0 ? Math.min(1, d.scrollTop / max) : 0) + ')';
    top.hidden = d.scrollTop < 900;
  }
  window.addEventListener('scroll', onScroll, { passive: true });
  onScroll();

  /* ---------- brief (TL;DR only) mode ---------- */
  var briefBtns = document.querySelectorAll('[data-brief]');
  function setBrief(on) {
    document.body.classList.toggle('brief', on);
    briefBtns.forEach(function (b) { b.setAttribute('aria-pressed', on ? 'true' : 'false'); b.textContent = on ? 'Zobrazit celé kapitoly' : 'Jen TL;DR'; });
    store('mj-brief', on ? '1' : null);
  }
  briefBtns.forEach(function (b) { b.addEventListener('click', function () { setBrief(!document.body.classList.contains('brief')); }); });
  setBrief(store('mj-brief') === '1');

  /* ---------- command palette ---------- */
  var index = [];
  document.querySelectorAll('main section[id]').forEach(function (s) {
    var h2 = s.querySelector('h2');
    if (!h2) return;
    var title = h2.firstChild ? h2.firstChild.textContent.trim() : h2.textContent;
    var tl = s.querySelector('.tldr p');
    index.push({ k: 'kapitola', t: title, c: tl ? tl.textContent.replace(/^TL;DR/, '').trim() : '', id: s.id });
    var n = 0;
    s.querySelectorAll('h3:not(.h4)').forEach(function (h3) {
      if (!h3.id) h3.id = s.id + '--' + (++n);
      index.push({ k: 'oddíl', t: h3.textContent.trim(), c: title, id: h3.id });
    });
    var lab = s.querySelector('.lab-h');
    if (lab) {
      s.querySelectorAll('.lab-h').forEach(function (lh, i) {
        var host = lh.closest('.lab');
        if (!host.id) host.id = s.id + '--lab' + i;
        index.push({ k: 'interaktivní', t: lh.textContent.replace(/^Vyzkoušej si · /, ''), c: title, id: host.id });
      });
    }
  });
  document.querySelectorAll('#gloss > div').forEach(function (row) {
    var a = row.querySelector('dt a');
    if (a) index.push({ k: 'pojem', t: a.textContent, c: row.querySelector('dd').textContent, id: a.getAttribute('href').slice(1) });
  });
  ['prikazy', 'p-prikazy', 'm-prikazy'].forEach(function (sid) {
    var s = $(sid);
    if (!s) return;
    s.querySelectorAll('tbody tr').forEach(function (tr) {
      var code = tr.querySelector('td code');
      var tds = tr.querySelectorAll('td');
      if (code) index.push({ k: 'příkaz', t: code.textContent, c: tds[tds.length - 1].textContent, id: sid });
    });
  });

  var bodies = [];
  document.querySelectorAll('main section[id]').forEach(function (s) {
    var h2 = s.querySelector('h2');
    var parts = [];
    s.querySelectorAll(':scope > p, :scope > ul, :scope > ol, :scope > .table-wrap, :scope > .two, :scope > .note, :scope > figure figcaption, :scope > .code, :scope > .ladder, :scope > h3').forEach(function (n) { parts.push(n.textContent); });
    var text = parts.join(' ').replace(/\s+/g, ' ');
    bodies.push({ id: s.id, title: h2 && h2.firstChild ? h2.firstChild.textContent.trim() : s.id, text: text, n: norm(text) });
  });
  var overlay = el('div', { 'class': 'pal-overlay', id: 'palette' }, document.body);
  overlay.hidden = true;
  var dlg = el('div', { 'class': 'pal', role: 'dialog', 'aria-modal': 'true', 'aria-label': 'Hledat v příručce' }, overlay);
  var input = el('input', { 'class': 'pal-in', id: 'pal-in', type: 'search', placeholder: 'Hledat kapitolu, pojem, příkaz, simulátor…', autocomplete: 'off', 'aria-controls': 'pal-list' }, dlg);
  var list = el('ul', { 'class': 'pal-list', id: 'pal-list', role: 'listbox' }, dlg);
  el('p', { 'class': 'pal-foot' }, dlg, '↑ ↓ výběr · Enter skok · Esc zavřít · ⌘K nebo / otevře · j a k: další a předchozí kapitola');
  var results = [], sel = 0;
  function search(q) {
    var toks = norm(q).split(/\s+/).filter(Boolean);
    if (!toks.length) return index.filter(function (x) { return x.k === 'kapitola'; }).slice(0, 12);
    return index.map(function (x) {
      var t = norm(x.t), c = norm(x.c);
      var ok = toks.every(function (tk) { return t.indexOf(tk) !== -1 || c.indexOf(tk) !== -1; });
      if (!ok) return null;
      var score = toks.reduce(function (a, tk) { return a + (t.indexOf(tk) === 0 ? 6 : t.indexOf(tk) !== -1 ? 4 : 1); }, 0) + (x.k === 'kapitola' ? 2 : 0);
      return { x: x, s: score };
    }).filter(Boolean).sort(function (a, b) { return b.s - a.s; }).slice(0, 14).map(function (r) { return r.x; });
  }
  function searchAll(q) {
    var res = search(q);
    var toks = norm(q).split(/\s+/).filter(Boolean);
    if (!toks.length || res.length >= 14) return res;
    var have = {};
    res.forEach(function (r) { if (r.k === 'kapitola') have[r.id] = true; });
    bodies.forEach(function (b) {
      if (res.length >= 14 || have[b.id]) return;
      if (!toks.every(function (tk) { return b.n.indexOf(tk) !== -1; })) return;
      var at = b.n.indexOf(toks[0]);
      var from = Math.max(0, at - 50);
      res.push({ k: 'v textu', t: b.title, c: (from ? '…' : '') + b.text.slice(from, at + 90) + '…', id: b.id });
    });
    return res;
  }
  function render() {
    results = searchAll(input.value);
    sel = Math.min(sel, Math.max(0, results.length - 1));
    while (list.firstChild) list.removeChild(list.firstChild);
    if (!results.length) { el('li', { 'class': 'pal-empty' }, list, 'Nic nenalezeno.'); return; }
    results.forEach(function (r, i) {
      var li = el('li', { role: 'option', id: 'pal-o' + i, 'aria-selected': i === sel ? 'true' : 'false' }, list);
      el('span', { 'class': 'pal-k' }, li, r.k);
      var b = el('span', { 'class': 'pal-t' }, li);
      el('b', null, b, r.t);
      if (r.c) el('small', null, b, r.c.length > 110 ? r.c.slice(0, 110) + '…' : r.c);
      li.addEventListener('pointermove', function () { if (sel !== i) { sel = i; mark(); } });
      li.addEventListener('click', function () { go(i); });
    });
    input.setAttribute('aria-activedescendant', 'pal-o' + sel);
  }
  function mark() {
    Array.prototype.forEach.call(list.children, function (li, i) { li.setAttribute('aria-selected', i === sel ? 'true' : 'false'); });
    var cur = list.children[sel];
    if (cur && cur.scrollIntoView) cur.scrollIntoView({ block: 'nearest' });
    input.setAttribute('aria-activedescendant', 'pal-o' + sel);
  }
  function open() { overlay.hidden = false; input.value = ''; sel = 0; render(); input.focus(); }
  function close() { overlay.hidden = true; }
  window.mjSearch = function (q) { open(); input.value = q; sel = 0; render(); };
  window.mjJump = jump;
  function go(i) {
    var r = results[i];
    if (!r) return;
    close();
    if (document.body.classList.contains('brief') && r.k !== 'kapitola') setBrief(false);
    jump(r.id);
  }
  input.addEventListener('input', function () { sel = 0; render(); });
  input.addEventListener('keydown', function (e) {
    if (e.key === 'ArrowDown') { e.preventDefault(); sel = Math.min(results.length - 1, sel + 1); mark(); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); sel = Math.max(0, sel - 1); mark(); }
    else if (e.key === 'Enter') { e.preventDefault(); go(sel); }
    else if (e.key === 'Escape') { e.preventDefault(); close(); }
  });
  overlay.addEventListener('click', function (e) { if (e.target === overlay) close(); });
  document.querySelectorAll('[data-palette]').forEach(function (b) { b.addEventListener('click', open); });

  var chapters = Array.prototype.slice.call(document.querySelectorAll('main section[id]'));
  function currentIndex() {
    var y = 120, best = 0;
    chapters.forEach(function (s, i) { if (s.getBoundingClientRect().top <= y) best = i; });
    return best;
  }
  document.addEventListener('keydown', function (e) {
    var tag = (e.target.tagName || '').toLowerCase();
    var typing = tag === 'input' || tag === 'textarea' || tag === 'select' || e.target.isContentEditable;
    if ((e.key === 'k' || e.key === 'K') && (e.metaKey || e.ctrlKey)) { e.preventDefault(); overlay.hidden ? open() : close(); return; }
    if (typing || e.metaKey || e.ctrlKey || e.altKey) return;
    if (e.key === '/') { e.preventDefault(); open(); }
    else if (e.key === 'j') { var i = currentIndex(); if (chapters[i + 1]) jump(chapters[i + 1].id); }
    else if (e.key === 'k') { var k = currentIndex(); if (chapters[k - 1]) jump(chapters[k - 1].id); }
    else if (e.key === 'Escape' && !overlay.hidden) close();
  });

  /* ---------- team self-assessment ---------- */
  var Q = [
    { id: 'ctx', q: 'Přežije stav práce konec session a compaction?', step: 'Hook na konci session, který zapíše cíl, stav a další krok do souboru v repu.', ch: [['p-session', 'Session'], ['h-hooky', 'Hooky']] },
    { id: 'done', q: 'Je „hotovo“ kontrola, ne věta agenta?', step: 'Checklist hotového v CI: scope, testy, závazky, gates. Selhání jednoho řádku znamená, že hotovo není.', ch: [['h-finish', 'Finish contract'], ['h-zavazky', 'Závazky']] },
    { id: 'rules', q: 'Jmenuje každé pravidlo kontrolu, která ho vynucuje?', step: 'Ke každému pravidlu validátor, test nebo důvod lidské kontroly. Počet pravidel bez důkazu se smí jen snižovat.', ch: [['h-vynuceni', 'Vynucení'], ['p-pravidla', 'Pravidla']] },
    { id: 'par', q: 'Vidí paralelní agenti a lidé, kdo na čem pracuje?', step: 'Před začátkem práce ohlásit záměr a cesty (board, issue, kanál). Kolize hledat před merge.', ch: [['h-server', 'Board'], ['koordinace', 'Kruhy koordinace']] },
    { id: 'instr', q: 'Generují se instrukce pro všechny AI nástroje z jednoho zdroje?', step: 'Jedna šablona pro CLAUDE.md, AGENTS.md a GEMINI.md, CI hlídá ruční úpravy výstupu.', ch: [['h-bootstrap', 'Bootstrap']] },
    { id: 'scope', q: 'Je deklarované, co agent nesmí číst (tajemství, generované soubory)?', step: 'Jeden soubor se seznamem cest a kategorií mimo dosah, vynucený nástrojem, ne instrukcí.', ch: [['h-scope', 'Scope']] },
    { id: 'ci', q: 'Čte vaše CI chybějící verdikt jako neúspěch?', step: 'Gate bez odpovědi je queued, přeskočený test není pass a chyba ve výstupu rozhoduje exit.', ch: [['h-ci', 'CI gates']] },
    { id: 'dec', q: 'Zapisujete rozhodnutí s důvodem a zamítnutými variantami?', step: 'ADR šablona v repu vedle kódu a zvyk, že architektonická změna nese ADR.', ch: [['p-rozhodnuti', 'Decision a ADR'], ['h-reasoning', 'Nejistota']] },
    { id: 'know', q: 'Ukládá se know-how z práce agentů mimo jejich vlastní paměť?', step: 'Adresář se znalostmi jako tvrzeními s důkazem; ověřenými je dělá člověk.', ch: [['p-znalosti', 'Knowledge base'], ['p-pamet', 'Paměť']] },
    { id: 'cross', q: 'Dá se rozdělaná práce bezpečně převzít na jiném stroji nebo jiným člověkem?', step: 'Handover jako soubor se sekcemi Objective, Current State, Next Action, ne zpráva v chatu.', ch: [['p-stroje', 'Kontinuita'], ['m-handovery', 'Handovery']] },
    { id: 'proj', q: 'Vzniká dokumentace a API z jedné deklarace?', step: 'Operaci deklarovat jednou, rozhraní generovat a rozdíl hlídat v CI.', ch: [['projekce', 'Projekce']] },
    { id: 'measure', q: 'Měříte přínos AI nástrojů na dokončených úkolech?', step: 'Párové běhy stejného úkolu s a bez nástroje; tokeny a čas na úspěšně dokončený úkol.', ch: [['h-mereni', 'Měření']] }
  ];
  var LV = ['nemáme', 'částečně', 'máme'];
  if ($('lab-assess')) (function () {
    var rows = $('assess-rows'), sum = $('assess-sum'), msg = $('assess-msg');
    var saved = {};
    try { saved = JSON.parse(store('mj-assess') || '{}') || {}; } catch (e) { saved = {}; }
    Q.forEach(function (q, qi) {
      var row = el('fieldset', { 'class': 'as-row' }, rows);
      el('legend', null, row, (qi + 1) + '. ' + q.q);
      var opts = el('div', { 'class': 'as-opts' }, row);
      LV.forEach(function (l, v) {
        var lab = el('label', { 'class': 'as-opt' }, opts);
        var r = el('input', { type: 'radio', name: 'as-' + q.id, value: String(v), id: 'as-' + q.id + '-' + v }, lab);
        if (saved[q.id] === v) r.checked = true;
        lab.appendChild(document.createTextNode(l));
        r.addEventListener('change', function () { saved[q.id] = v; store('mj-assess', JSON.stringify(saved)); renderSum(); });
      });
    });
    function summaryText() {
      var answered = Q.filter(function (q) { return saved[q.id] !== undefined; });
      var pts = answered.reduce(function (a, q) { return a + saved[q.id]; }, 0);
      var lines = ['Sebehodnocení práce s AI agenty (' + answered.length + '/12 odpovědí, ' + pts + '/' + answered.length * 2 + ' bodů)', ''];
      var gaps = Q.filter(function (q) { return saved[q.id] !== undefined && saved[q.id] < 2; });
      if (gaps.length) {
        lines.push('Priority podle dopadu:');
        gaps.forEach(function (q, i) { lines.push((i + 1) + '. ' + q.q + ' (' + LV[saved[q.id]] + ')'); lines.push('   První krok: ' + q.step); lines.push('   Více: ' + ARTIFACT + '#' + q.ch[0][0]); });
      } else lines.push('Bez mezer v zodpovězených oblastech.');
      return lines.join('\n');
    }
    function renderSum() {
      while (sum.firstChild) sum.removeChild(sum.firstChild);
      var answered = Q.filter(function (q) { return saved[q.id] !== undefined; });
      if (!answered.length) { el('p', { 'class': 'lab-note' }, sum, 'Odpověz aspoň na jednu otázku a uvidíš priority.'); return; }
      var pts = answered.reduce(function (a, q) { return a + saved[q.id]; }, 0);
      var pct = Math.round(100 * pts / (answered.length * 2));
      var head = el('div', { 'class': 'as-head' }, sum);
      var big = el('div', { 'class': 'as-big' }, head);
      el('b', null, big, pct + ' %');
      el('span', null, big, 'připravenost ze ' + answered.length + ' odpovědí');
      var strip = el('div', { 'class': 'as-strip', role: 'img', 'aria-label': 'Odpovědi po otázkách' }, head);
      Q.forEach(function (q, i) {
        var v = saved[q.id];
        var c = el('i', { 'class': 'as-c ' + (v === undefined ? 'na' : 'v' + v), title: (i + 1) + '. ' + q.q + ': ' + (v === undefined ? 'bez odpovědi' : LV[v]) }, strip);
        el('span', null, c, String(i + 1));
      });
      var gaps = Q.filter(function (q) { return saved[q.id] !== undefined && saved[q.id] < 2; });
      if (!gaps.length) { el('p', null, sum, 'V zodpovězených oblastech nemáte mezery. Další krok je kapitola Pro velkou organizaci.'); return; }
      el('p', { 'class': 'lab-h', style: 'margin-top:14px' }, sum, 'Priority podle dopadu');
      var ol = el('ol', { 'class': 'as-gaps' }, sum);
      gaps.slice(0, 5).forEach(function (q) {
        var li = el('li', null, ol);
        var h = el('p', null, li);
        el('b', null, h, q.q);
        el('span', { 'class': 'pill ' + (saved[q.id] === 0 ? 'refuses' : 'reports') }, h, LV[saved[q.id]]);
        el('p', null, li, 'První krok: ' + q.step);
        var links = el('p', { 'class': 'as-links' }, li);
        links.appendChild(document.createTextNode('Kapitoly: '));
        q.ch.forEach(function (c, i) {
          if (i) links.appendChild(document.createTextNode(' · '));
          var a = el('a', { href: '#' + c[0] }, links, c[1]);
          a.addEventListener('click', function (e) { e.preventDefault(); jump(c[0]); });
        });
      });
      if (gaps.length > 5) el('p', { 'class': 'lab-note' }, sum, 'A dalších ' + (gaps.length - 5) + ' v kopírovaném shrnutí.');
    }
    $('assess-copy').addEventListener('click', function () {
      var t = summaryText();
      copy(t, function () { msg.textContent = 'Shrnutí zkopírováno.'; }, function () { msg.textContent = 'Kopírování prohlížeč nepovolil; shrnutí je níže.'; var pre = el('pre', { 'class': 'term' }, sum); pre.textContent = t; });
    });
    $('assess-reset').addEventListener('click', function () {
      saved = {}; store('mj-assess', null);
      rows.querySelectorAll('input').forEach(function (i) { i.checked = false; });
      msg.textContent = ''; renderSum();
    });
    renderSum();
  })();
})();
