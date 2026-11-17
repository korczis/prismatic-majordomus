(function () {
  'use strict';
  function $(id) { return document.getElementById(id); }
  function el(tag, attrs, parent, text) {
    var e = document.createElement(tag);
    if (attrs) Object.keys(attrs).forEach(function (k) { e.setAttribute(k, attrs[k]); });
    if (text !== undefined) e.textContent = text;
    if (parent) parent.appendChild(e);
    return e;
  }
  function clear(e) { while (e.firstChild) e.removeChild(e.firstChild); }
  function store(key, val) {
    try {
      if (val === undefined) return window.localStorage.getItem(key);
      if (val === null) window.localStorage.removeItem(key); else window.localStorage.setItem(key, val);
    } catch (e) { return null; }
    return null;
  }
  function jump(id) { if (window.mjJump) window.mjJump(id); else { var t = $(id); if (t) t.scrollIntoView(); } }
  var sections = Array.prototype.slice.call(document.querySelectorAll('main section[id]'));
  var titleOf = {}, tldrOf = {}, impactOf = {};
  sections.forEach(function (s) {
    var h = s.querySelector('h2');
    titleOf[s.id] = h && h.firstChild ? h.firstChild.textContent.trim() : s.id;
    var ps = s.querySelectorAll('.tldr p');
    if (ps[0]) tldrOf[s.id] = ps[0].textContent.replace(/^TL;DR/, '').trim();
    if (ps[1]) impactOf[s.id] = ps[1].textContent.replace(/^Dopad/, '').trim();
  });
  function sectionOf(node) { var s = node.closest('main section[id]'); return s ? s.id : null; }
  function targetSection(id) { var t = $(id); if (!t) return null; var s = t.closest('main section[id]'); return s ? s.id : null; }

  /* ---------- backlinks ---------- */
  var back = {};
  document.querySelectorAll('main a[href^="#"]').forEach(function (a) {
    if (a.closest('.pn, .see, #gloss, nav, .pal, .backlinks, svg, #lab-graph, .card ol.path')) return;
    var from = sectionOf(a), to = targetSection(a.getAttribute('href').slice(1));
    if (!from || !to || from === to) return;
    (back[to] = back[to] || {})[from] = true;
  });
  document.querySelectorAll('.see a[href^="#"]').forEach(function (a) {
    var from = sectionOf(a), to = targetSection(a.getAttribute('href').slice(1));
    if (!from || !to || from === to) return;
    (back[to] = back[to] || {})[from] = true;
  });
  sections.forEach(function (s) {
    var from = Object.keys(back[s.id] || {});
    if (!from.length) return;
    var d = el('details', { 'class': 'backlinks' });
    el('summary', null, d, 'Odkazuje sem ' + from.length + (from.length === 1 ? ' kapitola' : from.length < 5 ? ' kapitoly' : ' kapitol'));
    var p = el('p', null, d);
    from.sort(function (a, b) { return sections.findIndex(function (x) { return x.id === a; }) - sections.findIndex(function (x) { return x.id === b; }); })
      .forEach(function (id, i) {
        if (i) p.appendChild(document.createTextNode(' · '));
        var a = el('a', { href: '#' + id }, p, titleOf[id]);
        a.addEventListener('click', function (e) { e.preventDefault(); jump(id); });
      });
    var pn = s.querySelector('.pn');
    s.insertBefore(d, pn || null);
  });

  /* ---------- link previews ---------- */
  var pv = el('div', { 'class': 'term-pop link-pop', role: 'tooltip' }, document.body);
  pv.hidden = true;
  var pvTimer = null;
  function showPreview(a, id) {
    clearTimeout(pvTimer);
    if (!tldrOf[id]) return;
    clear(pv);
    el('b', null, pv, titleOf[id]);
    el('p', null, pv, tldrOf[id]);
    if (impactOf[id]) { var q = el('p', { 'class': 'lp-i' }, pv); el('span', { 'class': 'k i' }, q, 'Dopad'); q.appendChild(document.createTextNode(' ' + impactOf[id])); }
    pv.hidden = false;
    var r = a.getBoundingClientRect(), w = pv.offsetWidth, h = pv.offsetHeight;
    pv.style.left = Math.min(Math.max(8, r.left + r.width / 2 - w / 2), window.innerWidth - w - 8) + 'px';
    pv.style.top = (r.top - h - 10 < 8 ? r.bottom + 10 : r.top - h - 10) + 'px';
  }
  function hidePreview() { pvTimer = setTimeout(function () { pv.hidden = true; }, 120); }
  window.addEventListener('scroll', function () { pv.hidden = true; }, { passive: true });
  function bindPreview(a) {
    var id = targetSection(a.getAttribute('href').slice(1));
    if (!id || id === sectionOf(a)) return;
    a.addEventListener('pointerenter', function (e) { if (e.pointerType !== 'touch') showPreview(a, id); });
    a.addEventListener('pointerleave', hidePreview);
    a.addEventListener('focus', function () { showPreview(a, id); });
    a.addEventListener('blur', hidePreview);
  }
  document.querySelectorAll('main a[href^="#"], nav.rail ol a, header.hero .links a').forEach(function (a) {
    if (a.classList.contains('h-anchor') || a.classList.contains('pg') || a.closest('svg, .ctx-bar')) return;
    bindPreview(a);
  });

  /* ---------- inline code is clickable ---------- */
  var caps = window.mjCaps || {};
  var codes = 0;
  document.querySelectorAll('main section p code, main section li code, main section td code, main section figcaption code').forEach(function (c) {
    if (c.closest('a, pre, .lab, .term, button, #gloss, .tldr')) return;
    var text = c.textContent.trim();
    if (text.length < 3 || text.length > 60) return;
    var isCap = caps[text] || caps[text.replace(/^majordomus:\/\//, '')];
    c.classList.add('ck');
    c.setAttribute('tabindex', '0');
    c.setAttribute('role', 'button');
    c.title = isCap ? 'Ukázat capability v průzkumníku' : 'Hledat v příručce';
    function act() {
      if (isCap && window.mjExplore) window.mjExplore('caps', text);
      else if (window.mjSearch) window.mjSearch(text.replace(/^majordomus(-cli)? /, '').replace(/[<>\[\]"|]/g, ' ').trim());
    }
    c.addEventListener('click', act);
    c.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); act(); } });
    codes++;
  });

  /* ---------- diagram labels and table first cells jump to chapters ---------- */
  var MAP = {
    'SessionStart': 'h-hooky', 'UserPromptSubmit': 'h-hooky', 'PreCompact': 'h-hooky', 'SessionEnd': 'h-hooky',
    'announce': 'h-server', 'majordomus_announce': 'h-server', 'start': 'h-finish', 'check': 'h-zavazky', 'finish': 'h-finish',
    'pre-commit': 'h-git', 'commit-msg': 'h-git', 'pre-push': 'h-git', 'gates': 'h-ci', 'CI gates': 'h-ci',
    'adrs/': 'p-rozhodnuti', 'rules/': 'p-pravidla', 'knowledge/curated/': 'p-znalosti', 'knowledge/candidates/': 'p-znalosti',
    'sessions/': 'p-session', 'policy + profiles/': 'h-profily', 'skills/, prompts/': 'p-skills', 'project/': 'x-plan',
    '**/README.md': 'p-briefing', 'mesh/': 'm-stroje', 'state/current.yaml': 'h-finish', 'state/checkpoints/': 'p-zaznamy',
    'state/handovers/': 'p-zaznamy', 'state/decisions.md': 'p-rozhodnuti', 'state/ledger.jsonl': 'p-zaznamy', 'session-contexts/': 'p-session',
    'git remote': 'p-stroje', 'Dialer · runtime A': 'm-identita', 'Answerer · runtime B': 'm-identita', 'welcome': 'm-handshake',
    'LAN segment': 'm-stroje', 'Jiný segment / tailnet': 'm-stroje', 'Sdílený server checkoutu': 'h-server',
    'Claude Code': 'h-bootstrap', 'Codex': 'h-bootstrap', 'Gemini CLI': 'h-bootstrap', 'Registr': 'x-registry', 'Deklarace': 'x-registry',
    'Cockpit': 'x-cockpit', 'Swagger UI': 'x-registry', 'příkazová řádka': 'prikazy', 'docs/generated → web': 'x-release',
    'queued': 'x-exec', 'running': 'x-exec', 'cancelling': 'x-exec',
    'task': 'h-finish', 'checkpoint': 'p-zaznamy', 'handover': 'p-zaznamy', 'decision': 'p-rozhodnuti', 'open question': 'h-finish',
    'history event': 'p-zaznamy', 'session-context': 'p-session', 'session record': 'p-session', 'ADR': 'p-rozhodnuti',
    'knowledge': 'p-znalosti', 'rule': 'p-pravidla', 'context document': 'p-briefing',
    'scope_respected': 'h-scope', 'verification_ran': 'h-profily', 'no_open_blockers': 'p-zaznamy', 'obligations_met': 'h-zavazky',
    'gates_passed': 'h-ci', 'publication_current': 'x-release', 'note_present': 'p-zaznamy', 'state_updated': 'p-zaznamy',
    'tests': 'x-evidence', 'docs': 'x-claims', 'generated': 'x-registry', 'rules': 'p-pravidla', 'pages': 'x-release', 'commit': 'h-git', 'push': 'h-git',
    'Task scope': 'h-scope', 'Překryv worktrees': 'x-worktree', 'Peer board': 'h-server', 'Mesh': 'm-claims',
    'dispatched': 'h-git', 'gated': 'h-ci', 'reviewed': 'p-pravidla', 'proven': 'x-evidence', 'stale': 'x-evidence', 'inputs_unchanged': 'x-evidence',
    'routine': 'h-profily', 'implementation': 'h-profily', 'debugging': 'h-profily', 'deep-work': 'h-profily',
    'GIT': 'p-briefing', 'PEERS': 'h-server', 'KNOWLEDGE': 'p-znalosti', 'OPEN QUESTIONS': 'p-zaznamy', 'CONTEXT DOCUMENTS': 'p-briefing', 'RECENT HISTORY': 'p-zaznamy',
    'node': 'm-identita', 'runtime': 'm-identita', 'stream': 'm-journal', 'session': 'p-session', 'repository': 'm-identita'
  };
  var svgLinks = 0;
  document.querySelectorAll('main section svg text').forEach(function (t) {
    if (t.closest('a, .lab, .chart')) return;
    var id = MAP[t.textContent.trim()];
    if (!id || id === sectionOf(t)) return;
    t.classList.add('svg-link');
    t.setAttribute('tabindex', '0');
    t.setAttribute('role', 'link');
    t.addEventListener('click', function () { jump(id); });
    t.addEventListener('keydown', function (e) { if (e.key === 'Enter') jump(id); });
    t.addEventListener('pointerenter', function () { showPreview(t, id); });
    t.addEventListener('pointerleave', hidePreview);
    svgLinks++;
  });
  var cellLinks = 0;
  document.querySelectorAll('main section table tbody tr').forEach(function (tr) {
    if (tr.closest('.lab, #prikazy, #p-prikazy, #m-prikazy, table.hub')) return;
    var first = tr.querySelector('td');
    if (!first || first.querySelector('a')) return;
    var key = first.textContent.trim().split(/\s*[,\/]\s*/)[0];
    var id = MAP[key];
    if (!id || id === sectionOf(tr)) return;
    var a = el('a', { href: '#' + id, 'class': 'cell-go', 'aria-label': 'Přejít na kapitolu ' + titleOf[id] }, first, '→');
    a.addEventListener('click', function (e) { e.preventDefault(); e.stopPropagation(); jump(id); });
    bindPreview(a);
    cellLinks++;
  });

  /* ---------- sortable and filterable tables ---------- */
  var tables = 0;
  document.querySelectorAll('main section .table-wrap > table').forEach(function (table) {
    if (table.closest('.lab, .chart-lab')) return;
    var tbody = table.querySelector('tbody'), head = table.querySelectorAll('thead th');
    if (!tbody || !head.length) return;
    var rows = Array.prototype.slice.call(tbody.rows);
    if (rows.length < 4 || table.querySelector('[rowspan]')) return;
    tables++;
    var original = rows.slice();
    head.forEach(function (th, col) {
      th.classList.add('sortable');
      th.setAttribute('tabindex', '0');
      th.setAttribute('role', 'button');
      th.setAttribute('aria-sort', 'none');
      function sort() {
        var dir = th.getAttribute('aria-sort') === 'ascending' ? 'descending' : th.getAttribute('aria-sort') === 'descending' ? 'none' : 'ascending';
        head.forEach(function (o) { o.setAttribute('aria-sort', 'none'); });
        th.setAttribute('aria-sort', dir);
        var list = original.slice();
        if (dir !== 'none') list.sort(function (a, b) {
          var x = (a.cells[col] || {}).textContent || '', y = (b.cells[col] || {}).textContent || '';
          var nx = parseFloat(x.replace(/\s/g, '').replace(',', '.')), ny = parseFloat(y.replace(/\s/g, '').replace(',', '.'));
          var c = (!isNaN(nx) && !isNaN(ny)) ? nx - ny : x.trim().localeCompare(y.trim(), 'cs');
          return dir === 'ascending' ? c : -c;
        });
        list.forEach(function (r) { tbody.appendChild(r); });
      }
      th.addEventListener('click', sort);
      th.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); sort(); } });
    });
    if (rows.length >= 9) {
      var wrap = table.parentElement;
      var inp = el('input', { type: 'search', 'class': 'inp tbl-filter', placeholder: 'Filtrovat ' + rows.length + ' řádků…', 'aria-label': 'Filtrovat tabulku' });
      wrap.parentElement.insertBefore(inp, wrap);
      inp.addEventListener('input', function () {
        var q = inp.value.trim().toLowerCase();
        original.forEach(function (r) { r.hidden = !!q && r.textContent.toLowerCase().indexOf(q) === -1; });
      });
    }
  });

  /* ---------- read tracking ---------- */
  var read = {};
  try { read = JSON.parse(store('mj-read') || '{}') || {}; } catch (e) { read = {}; }
  var navLinks = {};
  document.querySelectorAll('nav.rail ol a').forEach(function (a) { navLinks[a.getAttribute('href').slice(1)] = a; });
  var counter = el('p', { 'class': 'read-count' });
  var resetBtn = el('button', { type: 'button', 'class': 'read-reset' }, null, 'vynulovat');
  var toc = $('toc');
  if (toc) { toc.parentElement.insertBefore(counter, toc); }
  function paint() {
    var n = 0;
    sections.forEach(function (s) {
      var on = !!read[s.id];
      if (on) n++;
      if (navLinks[s.id]) navLinks[s.id].classList.toggle('is-read', on);
    });
    clear(counter);
    var bar = el('span', { 'class': 'read-bar' }, counter);
    var fill = el('i', null, bar);
    fill.style.width = (100 * n / sections.length) + '%';
    counter.appendChild(document.createTextNode('Přečteno ' + n + ' z ' + sections.length + ' '));
    if (n) counter.appendChild(resetBtn);
  }
  resetBtn.addEventListener('click', function () { read = {}; store('mj-read', null); paint(); });
  if ('IntersectionObserver' in window) {
    var seen = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (!en.isIntersecting) return;
        var id = sectionOf(en.target);
        if (id && !read[id] && !document.body.classList.contains('brief')) { read[id] = 1; store('mj-read', JSON.stringify(read)); paint(); }
      });
    }, { rootMargin: '0px 0px -30% 0px' });
    document.querySelectorAll('main section > .pn').forEach(function (p) { seen.observe(p); });
  }
  paint();
  document.documentElement.setAttribute('data-links', [codes, svgLinks, cellLinks, tables, Object.keys(back).length].join(','));
})();
