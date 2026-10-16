(function () {
  'use strict';
  /* Every chapter: what it rests on in the repository, found in the repository's own graph. */
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  var raw = $('data-evidence');
  if (!raw || !window.mjGraphUI) return;
  var EV = JSON.parse(raw.textContent);
  var NAME = { document: ['dokument', 'dokumenty', 'dokumentů'], feature: ['feature', 'features', 'features'], module: ['modul', 'moduly', 'modulů'],
    command: ['příkaz', 'příkazy', 'příkazů'], adr: ['rozhodnutí', 'rozhodnutí', 'rozhodnutí'], rule: ['pravidlo', 'pravidla', 'pravidel'],
    claim: ['garance', 'garance', 'garancí'], test: ['test', 'testy', 'testů'], capability: ['capability', 'capabilities', 'capabilities'],
    implementation: ['implementace', 'implementace', 'implementací'], file: ['soubor', 'soubory', 'souborů'], 'use-case': ['use case', 'use cases', 'use cases'],
    skill: ['skill', 'skills', 'skills'] };
  var ORDER = ['document', 'feature', 'adr', 'rule', 'claim', 'test', 'module', 'capability', 'command', 'use-case', 'implementation', 'file', 'skill'];
  function plural(k, n) { var f = NAME[k] || [k, k, k]; return n + ' ' + (n === 1 ? f[0] : n < 5 ? f[1] : f[2]); }
  function group(host, title, ids, declared) {
    var by = {};
    ids.forEach(function (i) { var k = window.mjGraphUI.kind(i); (by[k] = by[k] || []).push(i); });
    var kinds = Object.keys(by).sort(function (a, b) { return (ORDER.indexOf(a) + 99) % 99 - (ORDER.indexOf(b) + 99) % 99; });
    if (title) el('p', { 'class': 'rd-box-h' }, host, title);
    kinds.forEach(function (k) {
      var box = el('div', { 'class': 'ev-kind' }, host);
      var h = el('p', { 'class': 'ev-kind-h' }, box);
      var b = el('button', { type: 'button', 'class': 'chip', title: 'Všechny objekty tohoto druhu v mapě druhů' }, h, plural(k, by[k].length));
      b.addEventListener('click', function () { window.mjGraphUI.kindPanel(k); var t = $('lab-kinds'); if (t) t.scrollIntoView({ block: 'start' }); });
      var grid = el('div', { 'class': 'g-grid' }, box);
      by[k].forEach(function (i) {
        var t = window.mjGraphUI.tile(i, grid);
        if (declared && declared[i]) el('span', { 'class': 'ev-decl', title: 'Kotva kapitoly: sestavení selže, když v repozitáři přestane existovat' }, t.querySelector('.g-links'), 'kotva');
      });
    });
  }
  Object.keys(EV).forEach(function (sid) {
    var sec = $(sid), ev = EV[sid];
    if (!sec) return;
    var det = el('details', { 'class': 'ev-panel', id: 'ev-' + sid });
    var sum = el('summary', null, det);
    el('span', { 'class': 'k' }, sum, 'Na čem kapitola stojí');
    var parts = ORDER.filter(function (k) { return ev.k[k]; }).map(function (k) { return plural(k, ev.k[k]); });
    Object.keys(ev.k).forEach(function (k) { if (ORDER.indexOf(k) < 0) parts.push(plural(k, ev.k[k])); });
    sum.appendChild(document.createTextNode(' ' + parts.join(' · ') + (ev.nt ? ' · v sousedství dalších ' + ev.nt : '')));
    var body = el('div', { 'class': 'ev-body' }, det);
    var see = sec.querySelector(':scope > aside.see');
    if (see) sec.insertBefore(det, see); else sec.appendChild(det);
    var done = false;
    det.addEventListener('toggle', function () {
      if (!det.open || done) return;
      done = true;
      el('p', { 'class': 'lab-note' }, body, 'Načítám graf repozitáře …');
      window.mjGraphUI.ready().then(function () {
        body.textContent = '';
        el('p', { 'class': 'lab-note' }, body, 'Objekty repozitáře, o kterých kapitola je. „Kotva“ je deklarovaná ručně a sestavení ji ověřuje proti grafu; ostatní jsou nalezené v textu kapitoly. Klik na dlaždici ukáže okolí objektu, „čítárna“ otevře jeho text.');
        var decl = {}; (ev.d || []).forEach(function (i) { decl[i] = 1; });
        group(body, null, ev.s, decl);
        if (ev.n.length) {
          var more = el('details', { 'class': 'ev-more' }, body);
          el('summary', null, more, 'Sousedství přes vztahy tested_by, put_in_force, implemented_by, exercises, evidences, composes, runs · ' + (ev.nt > ev.n.length ? ev.n.length + ' z ' + ev.nt : ev.nt));
          var mb = el('div', null, more), filled = false;
          more.addEventListener('toggle', function () { if (more.open && !filled) { filled = true; group(mb, null, ev.n); } });
        }
      }, function (e) { body.textContent = 'Graf se nepodařilo načíst: ' + e.message; });
    });
  });
  window.mjEvidence = EV;
})();
